use std::sync::{Arc, Mutex};

use crate::handle::RawConn;

/// Handle opaco para un prepared statement.
/// Mantiene un Arc al Mutex de la conexión para serializar operaciones
/// y evitar que la conexión se cierre mientras el statement está vivo.
pub struct StmtHandle {
    pub(crate) stmt: *mut libsqlite3_sys::sqlite3_stmt,
    pub(crate) conn: Arc<Mutex<RawConn>>,
}

// SAFETY: el stmt está protegido por el mismo Mutex que la conexión.
unsafe impl Send for StmtHandle {}

/// Convierte el puntero raw de Java a una referencia al StmtHandle.
///
/// # Safety
/// `ptr` debe ser no-nulo y apuntar a un StmtHandle vivo obtenido de `snr_prepare`.
pub(crate) unsafe fn stmt_ref<'a>(ptr: *mut StmtHandle) -> Option<&'a StmtHandle> {
    if ptr.is_null() { None } else { Some(&*ptr) }
}
