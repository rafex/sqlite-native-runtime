use std::os::raw::c_char;

use libsqlite3_sys as ffi;

use crate::error::{clear_last_error, set_last_error};
use crate::handle::{Handle, handle_ref};
use crate::util::cstr_to_str;

// ─── Transacciones ───────────────────────────────────────────────────────────

/// Inicia una transacción DEFERRED (lectura hasta primer write).
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_begin(handle: *mut Handle) -> i32 {
    exec_simple(handle, b"BEGIN\0", "snr_begin")
}

/// Inicia una transacción IMMEDIATE (reserva write lock desde el inicio).
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_begin_immediate(handle: *mut Handle) -> i32 {
    exec_simple(handle, b"BEGIN IMMEDIATE\0", "snr_begin_immediate")
}

/// Inicia una transacción EXCLUSIVE (bloquea toda la base de datos).
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_begin_exclusive(handle: *mut Handle) -> i32 {
    exec_simple(handle, b"BEGIN EXCLUSIVE\0", "snr_begin_exclusive")
}

/// Confirma la transacción activa.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_commit(handle: *mut Handle) -> i32 {
    exec_simple(handle, b"COMMIT\0", "snr_commit")
}

/// Revierte la transacción activa.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_rollback(handle: *mut Handle) -> i32 {
    exec_simple(handle, b"ROLLBACK\0", "snr_rollback")
}

// ─── Savepoints ──────────────────────────────────────────────────────────────

/// Crea un savepoint con el nombre `name`.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` y `name` deben ser punteros válidos.
#[no_mangle]
pub unsafe extern "C" fn snr_savepoint(handle: *mut Handle, name: *const c_char) -> i32 {
    exec_with_name(handle, name, "SAVEPOINT", "snr_savepoint")
}

/// Libera (confirma) el savepoint con nombre `name`.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` y `name` deben ser punteros válidos.
#[no_mangle]
pub unsafe extern "C" fn snr_release(handle: *mut Handle, name: *const c_char) -> i32 {
    exec_with_name(handle, name, "RELEASE", "snr_release")
}

/// Revierte hasta el savepoint con nombre `name` (sin eliminarlo).
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` y `name` deben ser punteros válidos.
#[no_mangle]
pub unsafe extern "C" fn snr_rollback_to(handle: *mut Handle, name: *const c_char) -> i32 {
    exec_with_name(handle, name, "ROLLBACK TO", "snr_rollback_to")
}

// ─── Helpers internos ────────────────────────────────────────────────────────

unsafe fn exec_simple(handle: *mut Handle, sql: &[u8], caller: &str) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error(format!("{caller}: handle nulo")); return -1; }
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error(format!("{caller}: mutex envenenado")); return -1; }
    };

    let mut errmsg: *mut c_char = std::ptr::null_mut();
    let rc = ffi::sqlite3_exec(
        guard.0,
        sql.as_ptr() as *const c_char,
        None,
        std::ptr::null_mut(),
        &mut errmsg,
    );

    if rc != ffi::SQLITE_OK {
        if !errmsg.is_null() {
            let msg = std::ffi::CStr::from_ptr(errmsg).to_string_lossy().to_string();
            ffi::sqlite3_free(errmsg as *mut _);
            set_last_error(format!("{caller}: {msg}"));
        } else {
            set_last_error(format!("{caller}: error rc={rc}"));
        }
        return -1;
    }
    0
}

unsafe fn exec_with_name(
    handle: *mut Handle,
    name: *const c_char,
    command: &str,
    caller: &str,
) -> i32 {
    clear_last_error();
    let name_str = match cstr_to_str(name) {
        Some(s) => s,
        None => { set_last_error(format!("{caller}: name nulo o no es UTF-8")); return -1; }
    };
    // Escapar " como "" (SQL identifier escaping) para prevenir inyección SQL (C-2).
    let safe_name = name_str.replace('"', "\"\"");
    let sql = format!("{command} \"{safe_name}\"\0");
    exec_simple(handle, sql.as_bytes(), caller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use crate::connection::{snr_open_memory, snr_close};
    use crate::error::{clear_last_error, snr_last_error};

    fn open_anon() -> *mut Handle {
        unsafe { snr_open_memory(std::ptr::null()) }
    }

    // ─── snr_begin / snr_commit ───────────────────────────────────────────────

    #[test]
    fn begin_and_commit_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_begin(h) }, 0);
        assert_eq!(unsafe { snr_commit(h) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn begin_immediate_and_commit_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_begin_immediate(h) }, 0);
        assert_eq!(unsafe { snr_commit(h) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn begin_exclusive_and_commit_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_begin_exclusive(h) }, 0);
        assert_eq!(unsafe { snr_commit(h) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn begin_and_rollback_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_begin(h) }, 0);
        assert_eq!(unsafe { snr_rollback(h) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn begin_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_begin(std::ptr::null_mut()) }, -1);
        assert!(!snr_last_error().is_null());
    }

    #[test]
    fn begin_immediate_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_begin_immediate(std::ptr::null_mut()) }, -1);
    }

    #[test]
    fn begin_exclusive_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_begin_exclusive(std::ptr::null_mut()) }, -1);
    }

    #[test]
    fn commit_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_commit(std::ptr::null_mut()) }, -1);
    }

    #[test]
    fn rollback_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_rollback(std::ptr::null_mut()) }, -1);
    }

    #[test]
    fn double_begin_returns_neg1() {
        // SQLite rechaza BEGIN dentro de una transacción activa
        clear_last_error();
        let h = open_anon();
        assert_eq!(unsafe { snr_begin(h) }, 0);
        assert_eq!(unsafe { snr_begin(h) }, -1);
        let err = snr_last_error();
        assert!(!err.is_null());
        unsafe { snr_rollback(h) };
        unsafe { snr_close(h) };
    }

    #[test]
    fn commit_without_transaction_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        // No hay transacción activa
        let rc = unsafe { snr_commit(h) };
        assert_eq!(rc, -1);
        assert!(!snr_last_error().is_null());
        unsafe { snr_close(h) };
    }

    #[test]
    fn rollback_without_transaction_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        let rc = unsafe { snr_rollback(h) };
        assert_eq!(rc, -1);
        assert!(!snr_last_error().is_null());
        unsafe { snr_close(h) };
    }

    // ─── snr_savepoint / snr_release / snr_rollback_to ───────────────────────

    #[test]
    fn savepoint_and_release_returns_0() {
        let h = open_anon();
        let name = CString::new("sp1").unwrap();
        assert_eq!(unsafe { snr_savepoint(h, name.as_ptr()) }, 0);
        assert_eq!(unsafe { snr_release(h, name.as_ptr()) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn savepoint_and_rollback_to_returns_0() {
        let h = open_anon();
        let name = CString::new("sp2").unwrap();
        assert_eq!(unsafe { snr_savepoint(h, name.as_ptr()) }, 0);
        assert_eq!(unsafe { snr_rollback_to(h, name.as_ptr()) }, 0);
        // Después de ROLLBACK TO el savepoint sigue activo, hay que liberarlo
        assert_eq!(unsafe { snr_release(h, name.as_ptr()) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn savepoint_name_with_double_quotes_escapes_correctly() {
        // El nombre contiene " — debe escaparse como "" (SQL identifier)
        let h = open_anon();
        let name = CString::new("sp\"injection").unwrap();
        // La función debe manejar el nombre sin error (escaping correcto)
        let rc = unsafe { snr_savepoint(h, name.as_ptr()) };
        assert_eq!(rc, 0);
        // Release también debe funcionar con el mismo nombre escapado
        assert_eq!(unsafe { snr_release(h, name.as_ptr()) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn savepoint_null_name_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        assert_eq!(unsafe { snr_savepoint(h, std::ptr::null()) }, -1);
        assert!(!snr_last_error().is_null());
        unsafe { snr_close(h) };
    }

    #[test]
    fn release_null_name_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        assert_eq!(unsafe { snr_release(h, std::ptr::null()) }, -1);
        unsafe { snr_close(h) };
    }

    #[test]
    fn rollback_to_null_name_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        assert_eq!(unsafe { snr_rollback_to(h, std::ptr::null()) }, -1);
        unsafe { snr_close(h) };
    }

    #[test]
    fn savepoint_null_handle_returns_neg1() {
        clear_last_error();
        let name = CString::new("sp").unwrap();
        assert_eq!(unsafe { snr_savepoint(std::ptr::null_mut(), name.as_ptr()) }, -1);
    }

    #[test]
    fn release_nonexistent_savepoint_returns_neg1() {
        clear_last_error();
        let h = open_anon();
        let name = CString::new("no_existe_sp").unwrap();
        let rc = unsafe { snr_release(h, name.as_ptr()) };
        assert_eq!(rc, -1);
        assert!(!snr_last_error().is_null());
        unsafe { snr_close(h) };
    }

    #[test]
    fn nested_savepoints_work() {
        let h = open_anon();
        let sp1 = CString::new("outer").unwrap();
        let sp2 = CString::new("inner").unwrap();
        assert_eq!(unsafe { snr_savepoint(h, sp1.as_ptr()) }, 0);
        assert_eq!(unsafe { snr_savepoint(h, sp2.as_ptr()) }, 0);
        assert_eq!(unsafe { snr_release(h, sp2.as_ptr()) }, 0);
        assert_eq!(unsafe { snr_release(h, sp1.as_ptr()) }, 0);
        unsafe { snr_close(h) };
    }
}
