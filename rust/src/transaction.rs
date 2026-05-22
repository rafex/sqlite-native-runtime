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
    // Construir SQL: SAVEPOINT "nombre" (comillas para escapar nombres con espacios)
    let sql = format!("{command} \"{name_str}\"\0");
    exec_simple(handle, sql.as_bytes(), caller)
}
