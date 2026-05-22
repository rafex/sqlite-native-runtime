use std::sync::{Arc, Mutex};

/// Newtype sobre el puntero SQLite para poder implementar Send.
/// `sqlite3` con SQLITE_THREADSAFE=1 (default bundled) es thread-safe
/// a nivel C, pero Rust requiere que declaremos Send explícitamente.
pub(crate) struct RawConn(pub *mut libsqlite3_sys::sqlite3);

// SAFETY: bundled SQLite compila con SQLITE_THREADSAFE=1 (serialized mode).
// La serialización adicional del Mutex en Handle es una capa extra de seguridad.
unsafe impl Send for RawConn {}

/// Handle opaco que Java recibe de `snr_open`.
/// Envuelve la conexión SQLite con un Mutex para serializar llamadas concurrentes.
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

impl Drop for Handle {
    fn drop(&mut self) {
        // Si el Arc ya no tiene otras referencias (statements abiertos),
        // la conexión se cierra aquí a través de snr_close.
        // La cerramos explícitamente en snr_close para control total.
    }
}

/// Convierte el puntero raw de Java a una referencia al Handle.
///
/// # Safety
/// `ptr` debe ser no-nulo, alineado y apuntar a un Handle vivo obtenido de `snr_open`.
pub(crate) unsafe fn handle_ref<'a>(ptr: *mut Handle) -> Option<&'a Handle> {
    if ptr.is_null() { None } else { Some(&*ptr) }
}
