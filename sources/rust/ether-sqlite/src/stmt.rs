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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{snr_open_memory, snr_close};
    use crate::statement::{snr_prepare, snr_stmt_close};

    #[test]
    fn null_stmt_ref_returns_none() {
        let result = unsafe { stmt_ref(std::ptr::null_mut()) };
        assert!(result.is_none());
    }

    #[test]
    fn valid_stmt_ref_returns_some() {
        let h = unsafe { snr_open_memory(std::ptr::null()) };
        assert!(!h.is_null());
        let sql = b"SELECT 1\0";
        let s = unsafe { snr_prepare(h, sql.as_ptr() as *const _) };
        assert!(!s.is_null());
        let r = unsafe { stmt_ref(s) };
        assert!(r.is_some());
        unsafe { snr_stmt_close(s) };
        unsafe { snr_close(h) };
    }

    #[test]
    fn stmt_handle_holds_arc_to_conn() {
        let h = unsafe { snr_open_memory(std::ptr::null()) };
        assert!(!h.is_null());
        let sql = b"SELECT 42\0";
        let s = unsafe { snr_prepare(h, sql.as_ptr() as *const _) };
        assert!(!s.is_null());
        // El StmtHandle tiene un Arc al Mutex de la conexión
        let sh = unsafe { stmt_ref(s) }.unwrap();
        // Arc::strong_count >= 2 (Handle + StmtHandle)
        assert!(Arc::strong_count(&sh.conn) >= 2);
        unsafe { snr_stmt_close(s) };
        unsafe { snr_close(h) };
    }
}
