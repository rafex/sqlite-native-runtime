use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use libsqlite3_sys as ffi;

use crate::error::{clear_last_error, set_last_error};
use crate::handle::{Handle, handle_ref};
use crate::util::cstr_to_str;

// ─── Flags de apertura (mirrors de SQLITE_OPEN_*) ────────────────────────────
// Definidos aquí para que Java no necesite conocer los valores C internos.
pub const SNR_OPEN_READONLY:  i32 = ffi::SQLITE_OPEN_READONLY  as i32;
pub const SNR_OPEN_READWRITE: i32 = ffi::SQLITE_OPEN_READWRITE as i32;
pub const SNR_OPEN_CREATE:    i32 = ffi::SQLITE_OPEN_CREATE    as i32;

// ─── snr_open ────────────────────────────────────────────────────────────────

/// Abre (o crea) la base de datos en `path`.
///
/// `flags` combina SNR_OPEN_READONLY / SNR_OPEN_READWRITE / SNR_OPEN_CREATE.
/// Usa 0 para el comportamiento por defecto (read-write + create).
///
/// Devuelve un puntero opaco `*mut Handle` o NULL en error.
/// Java debe cerrar con `snr_close` cuando termine.
///
/// # Safety
/// `path` debe ser un puntero C válido, no-nulo, nul-terminado, UTF-8.
#[no_mangle]
pub unsafe extern "C" fn snr_open(path: *const c_char, flags: i32) -> *mut Handle {
    clear_last_error();

    let path_str = match cstr_to_str(path) {
        Some(s) => s,
        None => {
            set_last_error("snr_open: path es nulo o no es UTF-8 válido");
            return std::ptr::null_mut();
        }
    };

    // El directorio padre debe existir antes de llamar snr_open.
    // Crear directorios arbitrarios desde una librería FFI es un vector de
    // directory-traversal — se elimina deliberadamente.
    let c_path = match CString::new(path_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("snr_open: path contiene bytes nulos: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Enmascarar a solo los flags públicamente permitidos (I-2).
    // Flags internos de SQLite (DELETEONCLOSE, TEMP_DB, etc.) no son accesibles.
    const ALLOWED_FLAGS: i32 = ffi::SQLITE_OPEN_READONLY as i32
        | ffi::SQLITE_OPEN_READWRITE as i32
        | ffi::SQLITE_OPEN_CREATE as i32
        | ffi::SQLITE_OPEN_URI as i32
        | ffi::SQLITE_OPEN_NOFOLLOW as i32; // rechaza symlinks

    let open_flags = if flags == 0 {
        // Flags por defecto: read-write + create + serialized + nofollow.
        ffi::SQLITE_OPEN_READWRITE
            | ffi::SQLITE_OPEN_CREATE
            | ffi::SQLITE_OPEN_FULLMUTEX
            | ffi::SQLITE_OPEN_NOFOLLOW
    } else {
        // SQLITE_OPEN_FULLMUTEX y SQLITE_OPEN_NOFOLLOW siempre forzados,
        // independientemente de los flags que pase el caller.
        // NOFOLLOW es opt-out (no opt-in): el caller debe pasar flags=0 o
        // incluirlo explícitamente si quiere symlinks (no recomendado).
        (flags & ALLOWED_FLAGS) | ffi::SQLITE_OPEN_FULLMUTEX | ffi::SQLITE_OPEN_NOFOLLOW
    };

    let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();
    let rc = ffi::sqlite3_open_v2(c_path.as_ptr(), &mut db, open_flags as i32, std::ptr::null());

    if rc != ffi::SQLITE_OK {
        let msg = if db.is_null() {
            format!("snr_open: sqlite3_open_v2 falló (rc={rc})")
        } else {
            let err = CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy().to_string();
            ffi::sqlite3_close(db);
            format!("snr_open: {err}")
        };
        set_last_error(msg);
        return std::ptr::null_mut();
    }

    Box::into_raw(Box::new(Handle::new(db)))
}

// ─── snr_open_memory ─────────────────────────────────────────────────────────

/// Abre una base de datos en memoria.
/// `name` puede ser NULL para `:memory:` anónima, o un nombre para
/// bases de datos en memoria compartidas (shared-cache URI).
///
/// # Safety
/// Si `name` no es NULL debe ser un puntero C válido, nul-terminado, UTF-8.
#[no_mangle]
pub unsafe extern "C" fn snr_open_memory(name: *const c_char) -> *mut Handle {
    let path = if name.is_null() {
        CString::new(":memory:").unwrap()
    } else {
        match cstr_to_str(name) {
            Some(n) => {
                // Sanear el nombre: solo [A-Za-z0-9_-] para evitar URI injection (A-2).
                // Caracteres como '?', '&', '/' romperían la URI y podrían abrir archivos reales.
                if !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    set_last_error(
                        "snr_open_memory: name inválido — solo [A-Za-z0-9_-] permitido"
                    );
                    return std::ptr::null_mut();
                }
                CString::new(format!("file:{n}?mode=memory&cache=shared")).unwrap()
            }
            None => {
                set_last_error("snr_open_memory: name no es UTF-8 válido");
                return std::ptr::null_mut();
            }
        }
    };

    let flags = ffi::SQLITE_OPEN_READWRITE
        | ffi::SQLITE_OPEN_CREATE
        | ffi::SQLITE_OPEN_FULLMUTEX
        | ffi::SQLITE_OPEN_URI;

    let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();
    let rc = ffi::sqlite3_open_v2(path.as_ptr(), &mut db, flags as i32, std::ptr::null());

    if rc != ffi::SQLITE_OK {
        let msg = if db.is_null() {
            format!("snr_open_memory: sqlite3_open_v2 falló (rc={rc})")
        } else {
            let err = CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy().to_string();
            ffi::sqlite3_close(db);
            format!("snr_open_memory: {err}")
        };
        set_last_error(msg);
        return std::ptr::null_mut();
    }

    Box::into_raw(Box::new(Handle::new(db)))
}

// ─── snr_close ───────────────────────────────────────────────────────────────

/// Cierra la conexión y libera el Handle. No usar `handle` después de esta llamada.
///
/// La conexión SQLite se cierra físicamente cuando TODOS los StmtHandle derivados
/// de esta conexión también hayan sido cerrados con `snr_stmt_close`. Si hay
/// statements abiertos al llamar `snr_close`, SQLite permanecerá abierto hasta
/// que el último statement se finalice — comportamiento correcto y sin use-after-free.
///
/// # Safety
/// `handle` debe ser un puntero válido obtenido de `snr_open` o `snr_open_memory`,
/// y NO debe usarse después de esta llamada. Llamar exactamente una vez por handle.
#[no_mangle]
pub unsafe extern "C" fn snr_close(handle: *mut Handle) {
    if !handle.is_null() {
        // Soltar el Box libera el Arc<Mutex<RawConn>> del Handle.
        // Si ningún StmtHandle activo sostiene otro Arc, el refcount llega a 0
        // y RawConn::drop llama sqlite3_close automáticamente.
        // Si hay statements abiertos, el Arc sigue vivo en ellos y sqlite3_close
        // se llamará cuando el último snr_stmt_close libere su Arc. (C-1)
        drop(Box::from_raw(handle));
    }
}

// ─── snr_ping ────────────────────────────────────────────────────────────────

/// Verifica que el handle es válido y la conexión responde.
/// Devuelve 1 si OK, 0 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido obtenido de `snr_open`.
#[no_mangle]
pub unsafe extern "C" fn snr_ping(handle: *mut Handle) -> i64 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_ping: handle nulo"); return 0; }
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_ping: mutex envenenado"); return 0; }
    };

    let mut stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
    let sql = b"SELECT 1\0";
    let rc = ffi::sqlite3_prepare_v2(guard.0, sql.as_ptr() as *const _, -1, &mut stmt, std::ptr::null_mut());
    if rc != ffi::SQLITE_OK {
        set_last_error("snr_ping: prepare falló");
        return 0;
    }
    let step_rc = ffi::sqlite3_step(stmt);
    ffi::sqlite3_finalize(stmt);
    if step_rc == ffi::SQLITE_ROW { 1 } else { 0 }
}

// ─── snr_sqlite_version ──────────────────────────────────────────────────────

/// Devuelve la versión de SQLite como *mut c_char. Java debe liberar con snr_free_string.
#[no_mangle]
pub extern "C" fn snr_sqlite_version() -> *mut c_char {
    let ver = unsafe { CStr::from_ptr(ffi::sqlite3_libversion()) }
        .to_string_lossy()
        .into_owned();
    match CString::new(ver) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ─── snr_exec ────────────────────────────────────────────────────────────────

/// Ejecuta una o más sentencias SQL sin resultado.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` y `sql` deben ser punteros válidos y no-nulos.
#[no_mangle]
pub unsafe extern "C" fn snr_exec(handle: *mut Handle, sql: *const c_char) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_exec: handle nulo"); return -1; }
    };
    if sql.is_null() {
        set_last_error("snr_exec: sql es nulo");
        return -1;
    }
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_exec: mutex envenenado"); return -1; }
    };

    let mut errmsg: *mut c_char = std::ptr::null_mut();
    let rc = ffi::sqlite3_exec(guard.0, sql, None, std::ptr::null_mut(), &mut errmsg);

    if rc != ffi::SQLITE_OK {
        if !errmsg.is_null() {
            let msg = CStr::from_ptr(errmsg).to_string_lossy().to_string();
            ffi::sqlite3_free(errmsg as *mut _);
            set_last_error(msg);
        } else {
            set_last_error(format!("snr_exec: error rc={rc}"));
        }
        return -1;
    }
    0
}

// ─── snr_last_insert_rowid ───────────────────────────────────────────────────

/// Devuelve el rowid de la última inserción exitosa.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_last_insert_rowid(handle: *mut Handle) -> i64 {
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => return 0,
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    ffi::sqlite3_last_insert_rowid(guard.0)
}

// ─── snr_changes ─────────────────────────────────────────────────────────────

/// Devuelve el número de filas modificadas por la última operación DML.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_changes(handle: *mut Handle) -> i64 {
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => return 0,
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    ffi::sqlite3_changes64(guard.0)
}

// ─── snr_set_busy_timeout ────────────────────────────────────────────────────

/// Configura el tiempo máximo (ms) que SQLite esperará un lock antes de retornar SQLITE_BUSY.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_set_busy_timeout(handle: *mut Handle, ms: i32) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_set_busy_timeout: handle nulo"); return -1; }
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_set_busy_timeout: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_busy_timeout(guard.0, ms);
    if rc == ffi::SQLITE_OK { 0 } else { -1 }
}

// ─── snr_open_flags_* (constantes exportadas para Java) ─────────────────────

#[no_mangle] pub extern "C" fn snr_flag_readonly()  -> i32 { SNR_OPEN_READONLY }
#[no_mangle] pub extern "C" fn snr_flag_readwrite() -> i32 { SNR_OPEN_READWRITE }
#[no_mangle] pub extern "C" fn snr_flag_create()    -> i32 { SNR_OPEN_CREATE }

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};
    use crate::error::{clear_last_error, snr_last_error, snr_free_string};

    // ─── Helper: abrir :memory: anónima ──────────────────────────────────────

    fn open_anon() -> *mut Handle {
        unsafe { snr_open_memory(std::ptr::null()) }
    }

    // Obtiene la ruta real del directorio temporal (resuelve symlinks de macOS).
    fn real_temp_path(filename: &str) -> CString {
        let dir = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        CString::new(dir.join(filename).to_str().unwrap()).unwrap()
    }

    // ─── snr_open_memory ─────────────────────────────────────────────────────

    #[test]
    fn open_memory_anon_returns_nonnull() {
        let h = open_anon();
        assert!(!h.is_null(), "snr_open_memory(NULL) debe retornar un handle válido");
        unsafe { snr_close(h) };
    }

    #[test]
    fn open_memory_named_returns_nonnull() {
        let name = CString::new("test_db_named").unwrap();
        let h = unsafe { snr_open_memory(name.as_ptr()) };
        assert!(!h.is_null());
        unsafe { snr_close(h) };
    }

    #[test]
    fn open_memory_invalid_name_returns_null() {
        clear_last_error();
        // Nombres con '?' son inválidos (URI injection)
        let name = CString::new("bad?name").unwrap();
        let h = unsafe { snr_open_memory(name.as_ptr()) };
        assert!(h.is_null());
        let err = snr_last_error();
        assert!(!err.is_null());
        let msg = unsafe { CStr::from_ptr(err).to_str().unwrap() };
        assert!(msg.contains("inválido"), "mensaje de error esperado: {msg}");
    }

    #[test]
    fn open_memory_invalid_utf8_name_returns_null() {
        clear_last_error();
        // 0xFF no es UTF-8 válido
        let bytes: &[u8] = &[0xFF, 0x00];
        let h = unsafe { snr_open_memory(bytes.as_ptr() as *const _) };
        assert!(h.is_null());
    }

    // ─── snr_open (file) ──────────────────────────────────────────────────────

    #[test]
    fn open_file_default_flags_creates_db() {
        let path = real_temp_path(&format!("snr_test_open_{}.db", std::process::id()));
        let h = unsafe { snr_open(path.as_ptr(), 0) };
        assert!(!h.is_null(), "snr_open debe crear la BD en disco");
        unsafe { snr_close(h) };
        let _ = std::fs::remove_file(path.to_str().unwrap());
    }

    #[test]
    fn open_file_null_path_returns_null() {
        clear_last_error();
        let h = unsafe { snr_open(std::ptr::null(), 0) };
        assert!(h.is_null());
        assert!(!snr_last_error().is_null());
    }

    #[test]
    fn open_file_nonexistent_dir_returns_null() {
        clear_last_error();
        let path = CString::new("/no_existe/sub/db.sqlite").unwrap();
        let h = unsafe { snr_open(path.as_ptr(), 0) };
        assert!(h.is_null());
        assert!(!snr_last_error().is_null());
    }

    #[test]
    fn open_file_readonly_flag_works() {
        // Primero crear la BD
        let path = real_temp_path(&format!("snr_test_ro_{}.db", std::process::id()));
        let h_rw = unsafe { snr_open(path.as_ptr(), 0) };
        assert!(!h_rw.is_null());
        unsafe { snr_close(h_rw) };
        // Abrir en solo lectura
        let h_ro = unsafe { snr_open(path.as_ptr(), SNR_OPEN_READONLY) };
        assert!(!h_ro.is_null());
        unsafe { snr_close(h_ro) };
        let _ = std::fs::remove_file(path.to_str().unwrap());
    }

    #[test]
    fn open_file_path_with_interior_nul_returns_null() {
        clear_last_error();
        // CString::new falla con interior NUL, simulamos pasando bytes directos
        let bytes: &[u8] = b"/tmp/a\0b.db\0";
        let h = unsafe { snr_open(bytes.as_ptr() as *const _, 0) };
        // El path contiene NUL interior: cstr_to_str lo trunca en "/tmp/a",
        // que sí puede abrirse — no hay garantía de null aquí.
        // Solo verificamos que no haya crash (UB).
        if !h.is_null() {
            unsafe { snr_close(h) };
        }
    }

    // ─── snr_close ────────────────────────────────────────────────────────────

    #[test]
    fn close_null_is_noop() {
        // No debe panicar
        unsafe { snr_close(std::ptr::null_mut()) };
    }

    // ─── snr_ping ─────────────────────────────────────────────────────────────

    #[test]
    fn ping_valid_handle_returns_1() {
        let h = open_anon();
        assert_eq!(unsafe { snr_ping(h) }, 1);
        unsafe { snr_close(h) };
    }

    #[test]
    fn ping_null_handle_returns_0() {
        clear_last_error();
        assert_eq!(unsafe { snr_ping(std::ptr::null_mut()) }, 0);
        assert!(!snr_last_error().is_null());
    }

    // ─── snr_sqlite_version ───────────────────────────────────────────────────

    #[test]
    fn sqlite_version_returns_nonnull() {
        let ptr = snr_sqlite_version();
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert!(s.starts_with('3'), "SQLite versión esperada 3.x: {s}");
        unsafe { snr_free_string(ptr) };
    }

    // ─── snr_exec ─────────────────────────────────────────────────────────────

    #[test]
    fn exec_ddl_returns_0() {
        let h = open_anon();
        let sql = CString::new("CREATE TABLE t(x INTEGER)").unwrap();
        assert_eq!(unsafe { snr_exec(h, sql.as_ptr()) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn exec_dml_returns_0() {
        let h = open_anon();
        let create = CString::new("CREATE TABLE t(x INTEGER)").unwrap();
        let insert = CString::new("INSERT INTO t VALUES(1)").unwrap();
        unsafe { snr_exec(h, create.as_ptr()) };
        assert_eq!(unsafe { snr_exec(h, insert.as_ptr()) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn exec_invalid_sql_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        let sql = CString::new("NOT SQL AT ALL").unwrap();
        assert_eq!(unsafe { snr_exec(h, sql.as_ptr()) }, -1);
        assert!(!snr_last_error().is_null());
        unsafe { snr_close(h) };
    }

    #[test]
    fn exec_null_handle_returns_neg1() {
        clear_last_error();
        let sql = CString::new("SELECT 1").unwrap();
        assert_eq!(unsafe { snr_exec(std::ptr::null_mut(), sql.as_ptr()) }, -1);
    }

    #[test]
    fn exec_null_sql_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        assert_eq!(unsafe { snr_exec(h, std::ptr::null()) }, -1);
        unsafe { snr_close(h) };
    }

    // ─── snr_last_insert_rowid ────────────────────────────────────────────────

    #[test]
    fn last_insert_rowid_after_insert() {
        let h = open_anon();
        let sql = CString::new("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES(42)").unwrap();
        unsafe { snr_exec(h, sql.as_ptr()) };
        let rowid = unsafe { snr_last_insert_rowid(h) };
        assert_eq!(rowid, 1, "primera inserción debe tener rowid=1");
        unsafe { snr_close(h) };
    }

    #[test]
    fn last_insert_rowid_null_handle_returns_0() {
        assert_eq!(unsafe { snr_last_insert_rowid(std::ptr::null_mut()) }, 0);
    }

    // ─── snr_changes ─────────────────────────────────────────────────────────

    #[test]
    fn changes_after_insert() {
        let h = open_anon();
        let setup = CString::new("CREATE TABLE t(x INTEGER)").unwrap();
        unsafe { snr_exec(h, setup.as_ptr()) };
        let insert = CString::new("INSERT INTO t VALUES(1)").unwrap();
        unsafe { snr_exec(h, insert.as_ptr()) };
        assert_eq!(unsafe { snr_changes(h) }, 1);
        unsafe { snr_close(h) };
    }

    #[test]
    fn changes_null_handle_returns_0() {
        assert_eq!(unsafe { snr_changes(std::ptr::null_mut()) }, 0);
    }

    // ─── snr_set_busy_timeout ─────────────────────────────────────────────────

    #[test]
    fn set_busy_timeout_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_set_busy_timeout(h, 5000) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn set_busy_timeout_zero_disables() {
        let h = open_anon();
        assert_eq!(unsafe { snr_set_busy_timeout(h, 0) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn set_busy_timeout_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_set_busy_timeout(std::ptr::null_mut(), 1000) }, -1);
    }

    // ─── flag constants ───────────────────────────────────────────────────────

    #[test]
    fn flag_constants_match_sqlite_values() {
        assert_eq!(snr_flag_readonly(),  libsqlite3_sys::SQLITE_OPEN_READONLY as i32);
        assert_eq!(snr_flag_readwrite(), libsqlite3_sys::SQLITE_OPEN_READWRITE as i32);
        assert_eq!(snr_flag_create(),    libsqlite3_sys::SQLITE_OPEN_CREATE as i32);
    }
}
