use std::ffi::CString;
use std::os::raw::c_char;

use libsqlite3_sys as ffi;

use crate::error::{clear_last_error, set_last_error};
use crate::handle::{Handle, handle_ref};
use crate::util::cstr_to_str;

// Modos de checkpoint
pub const SNR_CHECKPOINT_PASSIVE:  i32 = ffi::SQLITE_CHECKPOINT_PASSIVE  as i32;
pub const SNR_CHECKPOINT_FULL:     i32 = ffi::SQLITE_CHECKPOINT_FULL     as i32;
pub const SNR_CHECKPOINT_RESTART:  i32 = ffi::SQLITE_CHECKPOINT_RESTART  as i32;
pub const SNR_CHECKPOINT_TRUNCATE: i32 = ffi::SQLITE_CHECKPOINT_TRUNCATE as i32;

// ─── snr_wal_checkpoint ──────────────────────────────────────────────────────

/// Ejecuta un WAL checkpoint.
///
/// `db_name` puede ser NULL o "" para la base de datos principal ("main").
/// `mode` debe ser uno de SNR_CHECKPOINT_PASSIVE/FULL/RESTART/TRUNCATE.
///
/// Devuelve 0 en éxito, -1 en error.
/// Después del checkpoint, snr_wal_checkpoint_result permite leer frames/wal_frames.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_wal_checkpoint(
    handle: *mut Handle,
    db_name: *const c_char,
    mode: i32,
) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_wal_checkpoint: handle nulo"); return -1; }
    };

    let name_cs: Option<CString> = if db_name.is_null() {
        None
    } else {
        match cstr_to_str(db_name) {
            Some(s) if !s.is_empty() => CString::new(s).ok(),
            _ => None,
        }
    };
    let name_ptr = name_cs.as_ref().map_or(std::ptr::null(), |cs| cs.as_ptr());

    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_wal_checkpoint: mutex envenenado"); return -1; }
    };

    let mut n_log: i32 = 0;
    let mut n_ckpt: i32 = 0;
    let rc = ffi::sqlite3_wal_checkpoint_v2(guard.0, name_ptr, mode, &mut n_log, &mut n_ckpt);

    if rc != ffi::SQLITE_OK {
        set_last_error(format!("snr_wal_checkpoint: rc={rc} n_log={n_log} n_ckpt={n_ckpt}"));
        return -1;
    }
    0
}

// ─── snr_wal_autocheckpoint ──────────────────────────────────────────────────

/// Configura el auto-checkpoint de WAL.
/// `n` es el número de frames de WAL tras los cuales se hace checkpoint automáticamente.
/// Usa 0 para desactivar el auto-checkpoint.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_wal_autocheckpoint(handle: *mut Handle, n: i32) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_wal_autocheckpoint: handle nulo"); return -1; }
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_wal_autocheckpoint: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_wal_autocheckpoint(guard.0, n);
    if rc == ffi::SQLITE_OK { 0 } else {
        set_last_error(format!("snr_wal_autocheckpoint: rc={rc}"));
        -1
    }
}

// ─── snr_checkpoint_mode_* (constantes) ─────────────────────────────────────

#[no_mangle] pub extern "C" fn snr_checkpoint_passive()  -> i32 { SNR_CHECKPOINT_PASSIVE }
#[no_mangle] pub extern "C" fn snr_checkpoint_full()     -> i32 { SNR_CHECKPOINT_FULL }
#[no_mangle] pub extern "C" fn snr_checkpoint_restart()  -> i32 { SNR_CHECKPOINT_RESTART }
#[no_mangle] pub extern "C" fn snr_checkpoint_truncate() -> i32 { SNR_CHECKPOINT_TRUNCATE }
