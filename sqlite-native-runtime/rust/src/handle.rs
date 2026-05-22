use std::sync::{Arc, Mutex};

/// Newtype sobre el puntero SQLite para poder implementar Send.
/// `sqlite3` con SQLITE_THREADSAFE=1 (default bundled) es thread-safe
/// a nivel C, pero Rust requiere declarar Send explícitamente.
pub(crate) struct RawConn(pub *mut libsqlite3_sys::sqlite3);

// SAFETY: bundled SQLite compila con SQLITE_THREADSAFE=1 (serialized mode).
// El Mutex en Handle es una capa adicional de serialización desde Rust.
unsafe impl Send for RawConn {}

impl Drop for RawConn {
    fn drop(&mut self) {
        // SAFETY: cuando Drop se ejecuta el Arc<Mutex<RawConn>> tiene refcount=0,
        // lo que garantiza que ningún otro hilo puede acceder a self.0 en este momento.
        // Todos los StmtHandle (que también sostienen Arcs) ya han sido dropeados y
        // sus sqlite3_stmt* finalizados ANTES de llegar aquí, por lo que
        // sqlite3_close no retornará SQLITE_BUSY en uso normal.
        if !self.0.is_null() {
            let rc = unsafe { libsqlite3_sys::sqlite3_close(self.0) };
            if rc != libsqlite3_sys::SQLITE_OK {
                // No se puede propagar error desde Drop. Si ocurre SQLITE_BUSY significa
                // que hay statements sin finalizar — fallo del llamador, no de la librería.
                eprintln!(
                    "sqlite-native-runtime: sqlite3_close devolvió rc={} — \
                     posibles statements sin cerrar antes de snr_close",
                    rc
                );
            }
            self.0 = std::ptr::null_mut();
        }
    }
}

/// Handle opaco que Java recibe de `snr_open`.
/// La conexión SQLite se cierra automáticamente cuando todos los Arcs
/// (Handle + StmtHandles activos) son liberados.
///
/// # Sobre el Mutex
///
/// `SQLITE_OPEN_FULLMUTEX` (forzado en `snr_open`) hace que SQLite serialice
/// internamente todas las operaciones. El `Mutex<RawConn>` aquí no duplica esa
/// serialización — su único rol es:
///   1. Proporcionar `Send` seguro al Arc (Rust requiere `Sync` para compartir entre hilos).
///   2. Garantizar que `sqlite3_close` en `RawConn::Drop` se ejecuta con exclusión
///      mutua respecto a cualquier operación activa en el momento del cierre.
///
/// NO garantiza atomicidad de secuencias multi-llamada (p.ej. `column_text` +
/// `column_bytes`): el lock se libera entre llamadas. Para uso multi-hilo real,
/// el caller debe sincronizar externamente las secuencias que necesiten ser atómicas.
pub struct Handle {
    pub(crate) inner: Arc<Mutex<RawConn>>,
}

impl Handle {
    pub(crate) fn new(conn: *mut libsqlite3_sys::sqlite3) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RawConn(conn))),
        }
    }
}

/// Convierte el puntero raw de Java a una referencia al Handle.
///
/// # Safety
/// `ptr` debe ser no-nulo, alineado y apuntar a un Handle vivo obtenido de `snr_open`.
pub(crate) unsafe fn handle_ref<'a>(ptr: *mut Handle) -> Option<&'a Handle> {
    if ptr.is_null() { None } else { Some(&*ptr) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::snr_open_memory;
    use crate::connection::snr_close;

    #[test]
    fn null_handle_ref_returns_none() {
        let result = unsafe { handle_ref(std::ptr::null_mut()) };
        assert!(result.is_none());
    }

    #[test]
    fn valid_handle_ref_returns_some() {
        let h = unsafe { snr_open_memory(std::ptr::null()) };
        assert!(!h.is_null());
        let r = unsafe { handle_ref(h) };
        assert!(r.is_some());
        unsafe { snr_close(h) };
    }

    #[test]
    fn raw_conn_drop_closes_sqlite() {
        // Crear y cerrar un handle: Drop de RawConn llama sqlite3_close sin panicar
        let h = unsafe { snr_open_memory(std::ptr::null()) };
        assert!(!h.is_null());
        unsafe { snr_close(h) };
        // Si llegamos aquí sin panicar, Drop funcionó correctamente
    }

    #[test]
    fn handle_inner_arc_is_shared() {
        let h = unsafe { snr_open_memory(std::ptr::null()) };
        assert!(!h.is_null());
        let handle_ref_opt = unsafe { handle_ref(h) };
        let handle = handle_ref_opt.unwrap();
        // Arc::strong_count == 1 (solo el Handle, sin statements)
        assert_eq!(Arc::strong_count(&handle.inner), 1);
        unsafe { snr_close(h) };
    }
}
