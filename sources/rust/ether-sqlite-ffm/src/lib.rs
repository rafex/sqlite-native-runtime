//! `ether-sqlite-ffm` — ABI C completa para Java Panama FFM.
//!
//! Este cdylib re-exporta todas las funciones de `ether-sqlite-core`
//! añadiendo `#[no_mangle]` + `extern "C"` para generar los símbolos
//! `snr_*` que Java consume vía `java.lang.foreign.*`.
//!
//! Produce: `libether_sqlite_ffm_runtime.so` / `.dylib`

#![allow(clippy::missing_safety_doc)]

use std::os::raw::c_char;

use ether_sqlite_core::{Handle, StmtHandle};
use ether_sqlite_core::{
    connection, error, statement, transaction, wal,
};

// ─── Error ────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn snr_last_error() -> *const c_char {
    error::snr_last_error()
}

#[no_mangle]
pub extern "C" fn snr_last_error_copy() -> *mut c_char {
    error::snr_last_error_copy()
}

#[no_mangle]
pub unsafe extern "C" fn snr_free_string(ptr: *mut c_char) {
    error::snr_free_string(ptr)
}

// ─── Connection ───────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn snr_open(path: *const c_char, flags: i32) -> *mut Handle {
    connection::snr_open(path, flags)
}

#[no_mangle]
pub unsafe extern "C" fn snr_open_memory(name: *const c_char) -> *mut Handle {
    connection::snr_open_memory(name)
}

#[no_mangle]
pub unsafe extern "C" fn snr_close(handle: *mut Handle) {
    connection::snr_close(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_ping(handle: *mut Handle) -> i64 {
    connection::snr_ping(handle)
}

#[no_mangle]
pub extern "C" fn snr_sqlite_version() -> *mut c_char {
    connection::snr_sqlite_version()
}

#[no_mangle]
pub unsafe extern "C" fn snr_exec(handle: *mut Handle, sql: *const c_char) -> i32 {
    connection::snr_exec(handle, sql)
}

#[no_mangle]
pub unsafe extern "C" fn snr_last_insert_rowid(handle: *mut Handle) -> i64 {
    connection::snr_last_insert_rowid(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_changes(handle: *mut Handle) -> i64 {
    connection::snr_changes(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_set_busy_timeout(handle: *mut Handle, ms: i32) -> i32 {
    connection::snr_set_busy_timeout(handle, ms)
}

#[no_mangle]
pub extern "C" fn snr_flag_readonly() -> i32 {
    connection::snr_flag_readonly()
}

#[no_mangle]
pub extern "C" fn snr_flag_readwrite() -> i32 {
    connection::snr_flag_readwrite()
}

#[no_mangle]
pub extern "C" fn snr_flag_create() -> i32 {
    connection::snr_flag_create()
}

// ─── Statement ────────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn snr_prepare(handle: *mut Handle, sql: *const c_char) -> *mut StmtHandle {
    statement::snr_prepare(handle, sql)
}

#[no_mangle]
pub unsafe extern "C" fn snr_stmt_close(stmt: *mut StmtHandle) {
    statement::snr_stmt_close(stmt)
}

#[no_mangle]
pub unsafe extern "C" fn snr_stmt_reset(stmt: *mut StmtHandle) -> i32 {
    statement::snr_stmt_reset(stmt)
}

#[no_mangle]
pub unsafe extern "C" fn snr_stmt_clear_bindings(stmt: *mut StmtHandle) -> i32 {
    statement::snr_stmt_clear_bindings(stmt)
}

#[no_mangle]
pub unsafe extern "C" fn snr_bind_null(stmt: *mut StmtHandle, idx: i32) -> i32 {
    statement::snr_bind_null(stmt, idx)
}

#[no_mangle]
pub unsafe extern "C" fn snr_bind_int(stmt: *mut StmtHandle, idx: i32, val: i64) -> i32 {
    statement::snr_bind_int(stmt, idx, val)
}

#[no_mangle]
pub unsafe extern "C" fn snr_bind_double(stmt: *mut StmtHandle, idx: i32, val: f64) -> i32 {
    statement::snr_bind_double(stmt, idx, val)
}

#[no_mangle]
pub unsafe extern "C" fn snr_bind_text(
    stmt: *mut StmtHandle,
    idx: i32,
    val: *const c_char,
) -> i32 {
    statement::snr_bind_text(stmt, idx, val)
}

#[no_mangle]
pub unsafe extern "C" fn snr_bind_blob(
    stmt: *mut StmtHandle,
    idx: i32,
    data: *const u8,
    len: i32,
) -> i32 {
    statement::snr_bind_blob(stmt, idx, data, len)
}

#[no_mangle]
pub unsafe extern "C" fn snr_bind_parameter_index(
    stmt: *mut StmtHandle,
    name: *const c_char,
) -> i32 {
    statement::snr_bind_parameter_index(stmt, name)
}

#[no_mangle]
pub unsafe extern "C" fn snr_step(stmt: *mut StmtHandle) -> i32 {
    statement::snr_step(stmt)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_count(stmt: *mut StmtHandle) -> i32 {
    statement::snr_column_count(stmt)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_type(stmt: *mut StmtHandle, col: i32) -> i32 {
    statement::snr_column_type(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_int(stmt: *mut StmtHandle, col: i32) -> i64 {
    statement::snr_column_int(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_double(stmt: *mut StmtHandle, col: i32) -> f64 {
    statement::snr_column_double(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_text(stmt: *mut StmtHandle, col: i32) -> *const c_char {
    statement::snr_column_text(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_text_owned(stmt: *mut StmtHandle, col: i32) -> *mut c_char {
    statement::snr_column_text_owned(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_blob(stmt: *mut StmtHandle, col: i32) -> *const u8 {
    statement::snr_column_blob(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_bytes(stmt: *mut StmtHandle, col: i32) -> i32 {
    statement::snr_column_bytes(stmt, col)
}

#[no_mangle]
pub unsafe extern "C" fn snr_column_name(stmt: *mut StmtHandle, col: i32) -> *const c_char {
    statement::snr_column_name(stmt, col)
}

// ─── Transaction ──────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn snr_begin(handle: *mut Handle) -> i32 {
    transaction::snr_begin(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_begin_immediate(handle: *mut Handle) -> i32 {
    transaction::snr_begin_immediate(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_begin_exclusive(handle: *mut Handle) -> i32 {
    transaction::snr_begin_exclusive(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_commit(handle: *mut Handle) -> i32 {
    transaction::snr_commit(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_rollback(handle: *mut Handle) -> i32 {
    transaction::snr_rollback(handle)
}

#[no_mangle]
pub unsafe extern "C" fn snr_savepoint(handle: *mut Handle, name: *const c_char) -> i32 {
    transaction::snr_savepoint(handle, name)
}

#[no_mangle]
pub unsafe extern "C" fn snr_release(handle: *mut Handle, name: *const c_char) -> i32 {
    transaction::snr_release(handle, name)
}

#[no_mangle]
pub unsafe extern "C" fn snr_rollback_to(handle: *mut Handle, name: *const c_char) -> i32 {
    transaction::snr_rollback_to(handle, name)
}

// ─── WAL ──────────────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn snr_wal_checkpoint(
    handle: *mut Handle,
    db_name: *const c_char,
    mode: i32,
    out_wal_frames: *mut i32,
    out_checkpointed: *mut i32,
) -> i32 {
    wal::snr_wal_checkpoint(handle, db_name, mode, out_wal_frames, out_checkpointed)
}

#[no_mangle]
pub unsafe extern "C" fn snr_wal_autocheckpoint(handle: *mut Handle, n: i32) -> i32 {
    wal::snr_wal_autocheckpoint(handle, n)
}

#[no_mangle]
pub extern "C" fn snr_checkpoint_passive() -> i32 {
    wal::snr_checkpoint_passive()
}

#[no_mangle]
pub extern "C" fn snr_checkpoint_full() -> i32 {
    wal::snr_checkpoint_full()
}

#[no_mangle]
pub extern "C" fn snr_checkpoint_restart() -> i32 {
    wal::snr_checkpoint_restart()
}

#[no_mangle]
pub extern "C" fn snr_checkpoint_truncate() -> i32 {
    wal::snr_checkpoint_truncate()
}
