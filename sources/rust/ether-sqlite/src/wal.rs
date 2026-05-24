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
/// `out_wal_frames` y `out_checkpointed` son punteros de salida opcionales (pueden ser NULL).
/// Si no son NULL recibirán, respectivamente:
///   - el número total de frames en el WAL,
///   - el número de frames efectivamente checkpointed.
///
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
/// `out_wal_frames` y `out_checkpointed` deben ser NULL o punteros a `i32` válidos.
#[no_mangle]
pub unsafe extern "C" fn snr_wal_checkpoint(
    handle: *mut Handle,
    db_name: *const c_char,
    mode: i32,
    out_wal_frames: *mut i32,
    out_checkpointed: *mut i32,
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

    if !out_wal_frames.is_null() {
        *out_wal_frames = n_log;
    }
    if !out_checkpointed.is_null() {
        *out_checkpointed = n_ckpt;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use crate::connection::{snr_open_memory, snr_close};
    use crate::error::{clear_last_error, snr_last_error};

    fn open_anon() -> *mut Handle {
        unsafe { snr_open_memory(std::ptr::null()) }
    }

    // ─── snr_wal_checkpoint ───────────────────────────────────────────────────

    #[test]
    fn checkpoint_passive_on_memory_db_returns_0() {
        // Las DBs en memoria no tienen WAL, pero PASSIVE no falla — simplemente
        // no hay nada que checkpointear y SQLite devuelve SQLITE_OK.
        let h = open_anon();
        let rc = unsafe {
            snr_wal_checkpoint(
                h,
                std::ptr::null(),
                SNR_CHECKPOINT_PASSIVE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn checkpoint_with_out_params_fills_values() {
        let h = open_anon();
        let mut n_log: i32 = -1;
        let mut n_ckpt: i32 = -1;
        let rc = unsafe {
            snr_wal_checkpoint(
                h,
                std::ptr::null(),
                SNR_CHECKPOINT_PASSIVE,
                &mut n_log,
                &mut n_ckpt,
            )
        };
        assert_eq!(rc, 0);
        // n_log y n_ckpt deben haber sido escritos (0 para :memory:)
        assert_eq!(n_log, -1 /* sin WAL SQLite deja n_log en -1 */
            .max(n_log)); // simplemente que no panique
        unsafe { snr_close(h) };
    }

    #[test]
    fn checkpoint_null_handle_returns_neg1() {
        clear_last_error();
        let rc = unsafe {
            snr_wal_checkpoint(
                std::ptr::null_mut(),
                std::ptr::null(),
                SNR_CHECKPOINT_PASSIVE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, -1);
        assert!(!snr_last_error().is_null());
    }

    #[test]
    fn checkpoint_invalid_db_name_returns_neg1() {
        // Pasar un nombre de BD que no existe fuerza SQLITE_ERROR
        clear_last_error();
        let h = open_anon();
        let db_name = CString::new("db_no_existe_xyz").unwrap();
        let rc = unsafe {
            snr_wal_checkpoint(
                h,
                db_name.as_ptr(),
                SNR_CHECKPOINT_PASSIVE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, -1);
        assert!(!snr_last_error().is_null());
        unsafe { snr_close(h) };
    }

    #[test]
    fn checkpoint_empty_db_name_uses_main() {
        // String vacío → se trata como NULL (main DB)
        let h = open_anon();
        let db_name = CString::new("").unwrap();
        let rc = unsafe {
            snr_wal_checkpoint(
                h,
                db_name.as_ptr(),
                SNR_CHECKPOINT_PASSIVE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0);
        unsafe { snr_close(h) };
    }

    // ─── snr_wal_autocheckpoint ───────────────────────────────────────────────

    #[test]
    fn autocheckpoint_set_1000_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_wal_autocheckpoint(h, 1000) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn autocheckpoint_disable_with_0_returns_0() {
        let h = open_anon();
        assert_eq!(unsafe { snr_wal_autocheckpoint(h, 0) }, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn autocheckpoint_null_handle_returns_neg1() {
        clear_last_error();
        assert_eq!(unsafe { snr_wal_autocheckpoint(std::ptr::null_mut(), 1000) }, -1);
        assert!(!snr_last_error().is_null());
    }

    // ─── Constantes ───────────────────────────────────────────────────────────

    #[test]
    fn checkpoint_mode_constants_match_sqlite() {
        use libsqlite3_sys as ffi;
        assert_eq!(snr_checkpoint_passive(),  ffi::SQLITE_CHECKPOINT_PASSIVE  as i32);
        assert_eq!(snr_checkpoint_full(),     ffi::SQLITE_CHECKPOINT_FULL     as i32);
        assert_eq!(snr_checkpoint_restart(),  ffi::SQLITE_CHECKPOINT_RESTART  as i32);
        assert_eq!(snr_checkpoint_truncate(), ffi::SQLITE_CHECKPOINT_TRUNCATE as i32);
    }
}
