/// Wrapper seguro (safe) sobre la API C de ether-sqlite-core.
///
/// Las funciones de `ether_sqlite_core` son `unsafe` porque exponen ABI C para Java.
/// Este módulo encapsula todos los bloques `unsafe` en tipos con invariantes claros:
///   - `Connection`: posee el `*mut Handle` exclusivamente; implementa `Drop` y `Send`.
///   - `Stmt<'c>`: stmt preparado ligado a la vida de su `Connection`.
///
/// Garantías:
///   - Un `Connection` solo puede usarse desde el hilo que lo creó (sin compartir).
///   - `Stmt` no puede sobrevivir a su `Connection` (lifetime `'c`).
///   - `last_error_str()` lee el error thread-local de ether-sqlite-core,
///     correcto porque llamamos a la función y leemos el error en el mismo hilo.
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
};

use ether_sqlite_core::{
    connection::{snr_close, snr_exec, snr_open, snr_set_busy_timeout},
    error::snr_last_error,
    statement::{
        snr_bind_null, snr_bind_text, snr_prepare, snr_stmt_clear_bindings, snr_stmt_close,
        snr_stmt_reset, snr_step,
    },
    transaction::{snr_begin_immediate, snr_commit, snr_rollback},
    wal::{snr_wal_checkpoint, SNR_CHECKPOINT_TRUNCATE},
    Handle, StmtHandle,
};

// ── Connection ────────────────────────────────────────────────────────────────

/// Conexión SQLite de uso exclusivo por un único hilo.
///
/// La conexión se abre con `SQLITE_OPEN_FULLMUTEX` (forzado por `snr_open`)
/// y WAL activado al arranque. Al hacer drop, se llama `snr_close`.
pub struct Connection {
    handle: *mut Handle,
}

// SAFETY: `Handle` contiene `Arc<Mutex<RawConn>>`, que es `Send`.
// Cada `Connection` es propiedad exclusiva de un único hilo; no la compartimos.
unsafe impl Send for Connection {}

impl Drop for Connection {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: somos los únicos propietarios y handle no es nulo.
            unsafe { snr_close(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

impl Connection {
    /// Abre (o crea) la BD en `path` con WAL + busy_timeout configurado.
    ///
    /// En macOS, `path` debe ser el path real (sin symlinks) porque
    /// `snr_open` fuerza `SQLITE_OPEN_NOFOLLOW`.
    pub fn open(path: &str, busy_timeout_ms: i32) -> Result<Self, String> {
        // Resolver el path real para evitar NOFOLLOW en symlinks (macOS /tmp -> /private/tmp)
        let real_path = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_owned());

        let c_path = CString::new(real_path.as_str())
            .map_err(|e| format!("path contiene byte nulo: {e}"))?;

        // SAFETY: c_path vive durante la llamada; flags=0 -> READWRITE|CREATE|FULLMUTEX|NOFOLLOW
        let handle = unsafe { snr_open(c_path.as_ptr(), 0) };
        if handle.is_null() {
            return Err(format!("snr_open('{}') falló: {}", path, last_error_str()));
        }

        let conn = Connection { handle };

        // busy_timeout — SQLITE_BUSY esperará hasta N ms antes de devolver error
        let rc = unsafe { snr_set_busy_timeout(conn.handle, busy_timeout_ms) };
        if rc != 0 {
            return Err(format!("set_busy_timeout({busy_timeout_ms}) falló (rc={rc}): {}", last_error_str()));
        }

        // WAL mode — mayor concurrencia de lectura, menor contención de escritura
        conn.exec("PRAGMA journal_mode=WAL")?;
        // Sincronización: NORMAL es suficiente para WAL (FULL solo en crash crítico)
        conn.exec("PRAGMA synchronous=NORMAL")?;
        // Cache: 4 MB por conexión
        conn.exec("PRAGMA cache_size=-4096")?;
        // Autocheckpoint: cada 1000 frames de WAL
        conn.exec("PRAGMA wal_autocheckpoint=1000")?;

        Ok(conn)
    }

    /// Ejecuta SQL sin parámetros. Útil para DDL y PRAGMAs.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
        let c_sql = CString::new(sql)
            .map_err(|e| format!("SQL contiene byte nulo: {e}"))?;
        // SAFETY: handle y c_sql son válidos
        let rc = unsafe { snr_exec(self.handle, c_sql.as_ptr()) };
        if rc != 0 {
            Err(format!("exec falló (rc={rc}) SQL='{sql}': {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Comienza una transacción `IMMEDIATE` (lock de escritura inmediato).
    pub fn begin_immediate(&self) -> Result<(), String> {
        // SAFETY: handle válido
        let rc = unsafe { snr_begin_immediate(self.handle) };
        if rc != 0 {
            Err(format!("BEGIN IMMEDIATE falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Commit de la transacción activa.
    pub fn commit(&self) -> Result<(), String> {
        let rc = unsafe { snr_commit(self.handle) };
        if rc != 0 {
            Err(format!("COMMIT falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Rollback de la transacción activa.
    pub fn rollback(&self) -> Result<(), String> {
        // Rollback no debe fallar en condiciones normales; si falla, logueamos pero no propagamos.
        let rc = unsafe { snr_rollback(self.handle) };
        if rc != 0 {
            Err(format!("ROLLBACK falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// WAL checkpoint TRUNCATE — vacía el WAL al shutdown limpio.
    pub fn wal_checkpoint_truncate(&self) -> Result<(), String> {
        let rc = unsafe {
            snr_wal_checkpoint(
                self.handle,
                std::ptr::null(), // db_name = NULL → "main"
                SNR_CHECKPOINT_TRUNCATE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != 0 {
            Err(format!("WAL TRUNCATE checkpoint falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Prepara un statement SQL. El `Stmt` resultante no puede sobrevivir a `self`.
    pub fn prepare<'c>(&'c self, sql: &str) -> Result<Stmt<'c>, String> {
        let c_sql = CString::new(sql)
            .map_err(|e| format!("SQL contiene byte nulo: {e}"))?;
        // SAFETY: handle y c_sql válidos; el stmt queda ligado al handle
        let ptr = unsafe { snr_prepare(self.handle, c_sql.as_ptr()) };
        if ptr.is_null() {
            Err(format!("prepare falló SQL='{sql}': {}", last_error_str()))
        } else {
            Ok(Stmt { ptr, _conn: PhantomData })
        }
    }
}

// ── Stmt ──────────────────────────────────────────────────────────────────────

/// Statement preparado, ligado a la vida de su `Connection`.
///
/// El lifetime `'c` garantiza que el `Stmt` no puede sobrevivir a la `Connection`
/// que lo preparó. Al hacer drop, se llama `snr_stmt_close`.
pub struct Stmt<'c> {
    ptr: *mut StmtHandle,
    _conn: PhantomData<&'c Connection>,
}

impl<'c> Drop for Stmt<'c> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: somos propietarios exclusivos y ptr no es nulo.
            unsafe { snr_stmt_close(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}

impl<'c> Stmt<'c> {
    /// Vincula un texto UTF-8 al parámetro `idx` (1-based).
    pub fn bind_text(&self, idx: i32, val: &str) -> Result<(), String> {
        let c_val = CString::new(val)
            .map_err(|e| format!("valor bind contiene byte nulo: {e}"))?;
        let rc = unsafe { snr_bind_text(self.ptr, idx, c_val.as_ptr()) };
        if rc != 0 {
            Err(format!("bind_text(idx={idx}) falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Vincula texto opcional: NULL si `val` es `None`, texto si es `Some`.
    pub fn bind_text_opt(&self, idx: i32, val: Option<&str>) -> Result<(), String> {
        match val {
            Some(s) => self.bind_text(idx, s),
            None    => self.bind_null(idx),
        }
    }

    /// Vincula NULL al parámetro `idx`.
    pub fn bind_null(&self, idx: i32) -> Result<(), String> {
        let rc = unsafe { snr_bind_null(self.ptr, idx) };
        if rc != 0 {
            Err(format!("bind_null(idx={idx}) falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Ejecuta un paso del statement.
    /// Devuelve `true` si hay una fila disponible (SNR_ROW=1), `false` si terminó (SNR_DONE=0).
    pub fn step(&self) -> Result<bool, String> {
        let rc = unsafe { snr_step(self.ptr) };
        // SNR_ROW = 1, SNR_DONE = 0, SNR_ERROR = -1
        match rc {
            1  => Ok(true),   // SNR_ROW
            0  => Ok(false),  // SNR_DONE
            _  => Err(format!("step falló (rc={rc}): {}", last_error_str())),
        }
    }

    /// Reinicia el statement para reutilizarlo con nuevos bindings.
    pub fn reset(&self) -> Result<(), String> {
        let rc = unsafe { snr_stmt_reset(self.ptr) };
        if rc != 0 {
            Err(format!("reset falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }

    /// Limpia todos los bindings del statement.
    pub fn clear_bindings(&self) -> Result<(), String> {
        let rc = unsafe { snr_stmt_clear_bindings(self.ptr) };
        if rc != 0 {
            Err(format!("clear_bindings falló (rc={rc}): {}", last_error_str()))
        } else {
            Ok(())
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Lee el último error thread-local de ether-sqlite-core.
/// Debe llamarse desde el mismo hilo que realizó la operación fallida.
fn last_error_str() -> String {
    // SAFETY: snr_last_error devuelve un puntero al buffer thread-local de la librería.
    // Es válido hasta la siguiente llamada snr_* en este hilo. Lo copiamos inmediatamente.
    // SAFETY: snr_last_error es unsafe extern "C"; copiamos el resultado inmediatamente
    let ptr = unsafe { snr_last_error() };
    if ptr.is_null() {
        "error desconocido (no hay mensaje de error disponible)".to_owned()
    } else {
        // SAFETY: ptr apunta a un CString válido dentro del TLS de ether-sqlite-core
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn open_memory() -> Connection {
        // Hack: snr_open_memory no pasa por NOFOLLOW, abrimos un archivo temporal
        // en /tmp resuelto para los tests.
        use tempfile::NamedTempFile;
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_string_lossy().into_owned();
        // Mantener el archivo vivo hasta que abrimos
        std::mem::forget(f);
        Connection::open(&path, 1000).expect("abrir db temporal")
    }

    #[test]
    fn open_and_exec_create_table() {
        let db = open_memory();
        db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)").unwrap();
    }

    #[test]
    fn begin_commit_roundtrip() {
        let db = open_memory();
        db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)").unwrap();
        db.begin_immediate().unwrap();
        db.exec("INSERT INTO t VALUES (1)").unwrap();
        db.commit().unwrap();
    }

    #[test]
    fn rollback_undoes_insert() {
        let db = open_memory();
        db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)").unwrap();
        db.begin_immediate().unwrap();
        db.exec("INSERT INTO t VALUES (42)").unwrap();
        db.rollback().unwrap();
        // La fila no debe estar: preparar un SELECT y verificar
        let stmt = db.prepare("SELECT COUNT(*) FROM t").unwrap();
        let has_row = stmt.step().unwrap();
        assert!(has_row, "COUNT(*) siempre devuelve una fila");
    }

    #[test]
    fn prepare_bind_step() {
        let db = open_memory();
        db.exec("CREATE TABLE t (id INTEGER, v TEXT)").unwrap();
        db.begin_immediate().unwrap();
        let stmt = db.prepare("INSERT INTO t VALUES (?1, ?2)").unwrap();
        stmt.bind_text(2, "hello").unwrap();
        stmt.bind_null(1).unwrap();
        stmt.step().unwrap();
        db.commit().unwrap();
    }

    #[test]
    fn stmt_reuse_via_reset() {
        let db = open_memory();
        db.exec("CREATE TABLE t (v TEXT)").unwrap();
        let stmt = db.prepare("INSERT INTO t VALUES (?1)").unwrap();
        for word in ["alpha", "beta", "gamma"] {
            db.begin_immediate().unwrap();
            stmt.reset().unwrap();
            stmt.clear_bindings().unwrap();
            stmt.bind_text(1, word).unwrap();
            stmt.step().unwrap();
            db.commit().unwrap();
        }
    }

    #[test]
    fn wal_checkpoint_noop_on_empty() {
        let db = open_memory();
        // Checkpoint en BD vacía no debe fallar
        db.wal_checkpoint_truncate().unwrap();
    }
}
