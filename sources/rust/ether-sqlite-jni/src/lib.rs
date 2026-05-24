//! `ether-sqlite-jni` — JNI binding para Java.
//!
//! Expone cada función `snr_*` de `ether-sqlite-core` como método JNI
//! con la firma `Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_*`.
//!
//! Diseño de tipos:
//! - `*mut Handle` / `*mut StmtHandle` ←→ `jlong` (puntero opaco como entero 64-bit)
//! - `*const c_char` (input) ←→ `JString` (Java String → CString en Rust)
//! - `*const c_char` / `*mut c_char` (output) ←→ `jstring` (copiado a String Java)
//! - `*const u8` (blob) ←→ `jbyteArray`
//! - `i32` ←→ `jint`, `i64` ←→ `jlong`, `f64` ←→ `jdouble`
//! - WAL checkpoint: devuelve `jlong` empaquetado = `(walFrames << 32) | checkpointed`
//!   o `-1` en error.
//!
//! Produce: `libether_sqlite_jni_runtime.so` / `.dylib`

#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jbyteArray, jdouble, jint, jlong, jstring};
use jni::JNIEnv;

use ether_sqlite_core::{Handle, StmtHandle};
use ether_sqlite_core::{connection, error, statement, transaction, wal};

// ── Helpers internos ──────────────────────────────────────────────────────────

/// Convierte un JString a CString. Devuelve None si es null o hay error.
fn jstring_to_cstring(env: &mut JNIEnv, s: JString) -> Option<CString> {
    if s.is_null() {
        return None;
    }
    let jstr = env.get_string(&s).ok()?;
    let rust_str: String = jstr.into();
    CString::new(rust_str).ok()
}

/// Convierte *const c_char a jstring (Java String). NULL → null jobject.
unsafe fn cchar_to_jstring(env: &mut JNIEnv, ptr: *const c_char) -> jstring {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    let cstr = std::ffi::CStr::from_ptr(ptr);
    match cstr.to_str() {
        Ok(s) => env.new_string(s).map(|js| js.into_raw()).unwrap_or(ptr::null_mut()),
        Err(_) => ptr::null_mut(),
    }
}

/// Convierte *mut c_char (owned, Rust-allocated) a jstring y libera el puntero.
unsafe fn owned_cchar_to_jstring(env: &mut JNIEnv, ptr: *mut c_char) -> jstring {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    let js = cchar_to_jstring(env, ptr as *const c_char);
    // Liberar la memoria Rust-allocated
    error::snr_free_string(ptr);
    js
}

// Prefijo JNI base
// Java package: mx.rafex.ether.sqlite.jni
// Java class:   SqliteLibraryJni
// JNI prefix:   Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_

// ─── Error ────────────────────────────────────────────────────────────────────

/// `String snrLastError()` — error del hilo actual, o null si no hay error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrLastError(
    mut env: JNIEnv,
    _: JClass,
) -> jstring {
    let ptr = error::snr_last_error();
    cchar_to_jstring(&mut env, ptr)
}

// ─── Connection ───────────────────────────────────────────────────────────────

/// `long snrOpen(String path, int flags)` — abre BD en disco. Devuelve handle o 0 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrOpen(
    mut env: JNIEnv,
    _: JClass,
    path: JString,
    flags: jint,
) -> jlong {
    let Some(c_path) = jstring_to_cstring(&mut env, path) else {
        error::snr_free_string(ptr::null_mut()); // noop, but clear error state
        return 0;
    };
    let h = connection::snr_open(c_path.as_ptr(), flags);
    h as jlong
}

/// `long snrOpenMemory(String name)` — abre BD en memoria. `name` puede ser null.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrOpenMemory(
    mut env: JNIEnv,
    _: JClass,
    name: JString,
) -> jlong {
    let h = if name.is_null() {
        connection::snr_open_memory(ptr::null())
    } else {
        match jstring_to_cstring(&mut env, name) {
            Some(c_name) => connection::snr_open_memory(c_name.as_ptr()),
            None => connection::snr_open_memory(ptr::null()),
        }
    };
    h as jlong
}

/// `void snrClose(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrClose(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) {
    connection::snr_close(handle as *mut Handle)
}

/// `long snrPing(long handle)` — 1 si OK, 0 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrPing(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jlong {
    connection::snr_ping(handle as *mut Handle)
}

/// `String snrSqliteVersion()`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrSqliteVersion(
    mut env: JNIEnv,
    _: JClass,
) -> jstring {
    let ptr = connection::snr_sqlite_version();
    owned_cchar_to_jstring(&mut env, ptr)
}

/// `int snrExec(long handle, String sql)` — 0 en éxito, -1 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrExec(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    sql: JString,
) -> jint {
    let Some(c_sql) = jstring_to_cstring(&mut env, sql) else { return -1; };
    connection::snr_exec(handle as *mut Handle, c_sql.as_ptr())
}

/// `long snrLastInsertRowid(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrLastInsertRowid(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jlong {
    connection::snr_last_insert_rowid(handle as *mut Handle)
}

/// `long snrChanges(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrChanges(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jlong {
    connection::snr_changes(handle as *mut Handle)
}

/// `int snrSetBusyTimeout(long handle, int ms)` — 0 en éxito, -1 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrSetBusyTimeout(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
    ms: jint,
) -> jint {
    connection::snr_set_busy_timeout(handle as *mut Handle, ms)
}

/// `int snrFlagReadonly()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrFlagReadonly(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    connection::snr_flag_readonly()
}

/// `int snrFlagReadwrite()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrFlagReadwrite(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    connection::snr_flag_readwrite()
}

/// `int snrFlagCreate()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrFlagCreate(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    connection::snr_flag_create()
}

// ─── Statement ────────────────────────────────────────────────────────────────

/// `long snrPrepare(long handle, String sql)` — StmtHandle opaco o 0 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrPrepare(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    sql: JString,
) -> jlong {
    let Some(c_sql) = jstring_to_cstring(&mut env, sql) else { return 0; };
    let s = statement::snr_prepare(handle as *mut Handle, c_sql.as_ptr());
    s as jlong
}

/// `void snrStmtClose(long stmt)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrStmtClose(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
) {
    statement::snr_stmt_close(stmt as *mut StmtHandle)
}

/// `int snrStmtReset(long stmt)` — 0 en éxito, -1 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrStmtReset(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
) -> jint {
    statement::snr_stmt_reset(stmt as *mut StmtHandle)
}

/// `int snrStmtClearBindings(long stmt)` — 0 en éxito, -1 en error.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrStmtClearBindings(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
) -> jint {
    statement::snr_stmt_clear_bindings(stmt as *mut StmtHandle)
}

/// `int snrBindNull(long stmt, int idx)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBindNull(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    idx: jint,
) -> jint {
    statement::snr_bind_null(stmt as *mut StmtHandle, idx)
}

/// `int snrBindInt(long stmt, int idx, long val)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBindInt(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    idx: jint,
    val: jlong,
) -> jint {
    statement::snr_bind_int(stmt as *mut StmtHandle, idx, val)
}

/// `int snrBindDouble(long stmt, int idx, double val)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBindDouble(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    idx: jint,
    val: jdouble,
) -> jint {
    statement::snr_bind_double(stmt as *mut StmtHandle, idx, val)
}

/// `int snrBindText(long stmt, int idx, String val)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBindText(
    mut env: JNIEnv,
    _: JClass,
    stmt: jlong,
    idx: jint,
    val: JString,
) -> jint {
    if val.is_null() {
        return statement::snr_bind_null(stmt as *mut StmtHandle, idx);
    }
    let Some(c_val) = jstring_to_cstring(&mut env, val) else { return -1; };
    statement::snr_bind_text(stmt as *mut StmtHandle, idx, c_val.as_ptr())
}

/// `int snrBindBlob(long stmt, int idx, byte[] data)` — null data → bind NULL.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBindBlob(
    env: JNIEnv,
    _: JClass,
    stmt: jlong,
    idx: jint,
    data: JByteArray,
) -> jint {
    if data.is_null() {
        return statement::snr_bind_null(stmt as *mut StmtHandle, idx);
    }
    let bytes: Vec<i8> = match env.convert_byte_array(&data) {
        Ok(b) => b.into_iter().map(|b| b as i8).collect(),
        Err(_) => return -1,
    };
    let len = bytes.len() as i32;
    statement::snr_bind_blob(stmt as *mut StmtHandle, idx, bytes.as_ptr() as *const u8, len)
}

/// `int snrBindParameterIndex(long stmt, String name)` — índice 1-based o 0 si no existe.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBindParameterIndex(
    mut env: JNIEnv,
    _: JClass,
    stmt: jlong,
    name: JString,
) -> jint {
    let Some(c_name) = jstring_to_cstring(&mut env, name) else { return 0; };
    statement::snr_bind_parameter_index(stmt as *mut StmtHandle, c_name.as_ptr())
}

/// `int snrStep(long stmt)` — 1=ROW, 0=DONE, -1=ERROR.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrStep(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
) -> jint {
    statement::snr_step(stmt as *mut StmtHandle)
}

/// `int snrColumnCount(long stmt)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnCount(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
) -> jint {
    statement::snr_column_count(stmt as *mut StmtHandle)
}

/// `int snrColumnType(long stmt, int col)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnType(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jint {
    statement::snr_column_type(stmt as *mut StmtHandle, col)
}

/// `long snrColumnInt(long stmt, int col)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnInt(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jlong {
    statement::snr_column_int(stmt as *mut StmtHandle, col)
}

/// `double snrColumnDouble(long stmt, int col)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnDouble(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jdouble {
    statement::snr_column_double(stmt as *mut StmtHandle, col)
}

/// `String snrColumnText(long stmt, int col)` — null si la columna es NULL.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnText(
    mut env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jstring {
    let ptr = statement::snr_column_text(stmt as *mut StmtHandle, col);
    cchar_to_jstring(&mut env, ptr)
}

/// `byte[] snrColumnBlob(long stmt, int col)` — null si la columna es NULL.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnBlob(
    env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jbyteArray {
    let ptr = statement::snr_column_blob(stmt as *mut StmtHandle, col);
    if ptr.is_null() {
        return ptr::null_mut();
    }
    let len = statement::snr_column_bytes(stmt as *mut StmtHandle, col) as usize;
    let bytes: &[u8] = std::slice::from_raw_parts(ptr, len);
    let signed: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    // Create Java byte[] and copy
    let arr = match env.new_byte_array(len as i32) {
        Ok(a) => a,
        Err(_) => return ptr::null_mut(),
    };
    let _ = env.set_byte_array_region(&arr, 0, &signed);
    arr.into_raw()
}

/// `int snrColumnBytes(long stmt, int col)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnBytes(
    _env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jint {
    statement::snr_column_bytes(stmt as *mut StmtHandle, col)
}

/// `String snrColumnName(long stmt, int col)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrColumnName(
    mut env: JNIEnv,
    _: JClass,
    stmt: jlong,
    col: jint,
) -> jstring {
    let ptr = statement::snr_column_name(stmt as *mut StmtHandle, col);
    cchar_to_jstring(&mut env, ptr)
}

// ─── Transaction ──────────────────────────────────────────────────────────────

/// `int snrBegin(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBegin(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    transaction::snr_begin(handle as *mut Handle)
}

/// `int snrBeginImmediate(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBeginImmediate(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    transaction::snr_begin_immediate(handle as *mut Handle)
}

/// `int snrBeginExclusive(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrBeginExclusive(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    transaction::snr_begin_exclusive(handle as *mut Handle)
}

/// `int snrCommit(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrCommit(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    transaction::snr_commit(handle as *mut Handle)
}

/// `int snrRollback(long handle)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrRollback(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    transaction::snr_rollback(handle as *mut Handle)
}

/// `int snrSavepoint(long handle, String name)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrSavepoint(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    name: JString,
) -> jint {
    let Some(c_name) = jstring_to_cstring(&mut env, name) else { return -1; };
    transaction::snr_savepoint(handle as *mut Handle, c_name.as_ptr())
}

/// `int snrRelease(long handle, String name)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrRelease(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    name: JString,
) -> jint {
    let Some(c_name) = jstring_to_cstring(&mut env, name) else { return -1; };
    transaction::snr_release(handle as *mut Handle, c_name.as_ptr())
}

/// `int snrRollbackTo(long handle, String name)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrRollbackTo(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    name: JString,
) -> jint {
    let Some(c_name) = jstring_to_cstring(&mut env, name) else { return -1; };
    transaction::snr_rollback_to(handle as *mut Handle, c_name.as_ptr())
}

// ─── WAL ──────────────────────────────────────────────────────────────────────

/// `long snrWalCheckpoint(long handle, String dbName, int mode)`
///
/// Devuelve long empaquetado: `(walFrames << 32) | (checkpointed & 0xFFFFFFFFL)`
/// o `-1L` en error.
/// Java desempaqueta: `int walFrames = (int)(result >> 32)`,
///                    `int checkpointed = (int)(result & 0xFFFFFFFFL)`.
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrWalCheckpoint(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    db_name: JString,
    mode: jint,
) -> jlong {
    let name_cs: Option<CString> = if db_name.is_null() {
        None
    } else {
        jstring_to_cstring(&mut env, db_name)
    };
    let name_ptr: *const c_char = name_cs.as_ref().map_or(ptr::null(), |cs| cs.as_ptr());

    let mut n_log: i32 = 0;
    let mut n_ckpt: i32 = 0;
    let rc = wal::snr_wal_checkpoint(handle as *mut Handle, name_ptr, mode, &mut n_log, &mut n_ckpt);
    if rc != 0 {
        return -1;
    }
    ((n_log as jlong) << 32) | (n_ckpt as jlong & 0xFFFF_FFFF)
}

/// `int snrWalAutocheckpoint(long handle, int n)`
#[no_mangle]
pub unsafe extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrWalAutocheckpoint(
    _env: JNIEnv,
    _: JClass,
    handle: jlong,
    n: jint,
) -> jint {
    wal::snr_wal_autocheckpoint(handle as *mut Handle, n)
}

/// `int snrCheckpointPassive()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrCheckpointPassive(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    wal::snr_checkpoint_passive()
}

/// `int snrCheckpointFull()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrCheckpointFull(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    wal::snr_checkpoint_full()
}

/// `int snrCheckpointRestart()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrCheckpointRestart(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    wal::snr_checkpoint_restart()
}

/// `int snrCheckpointTruncate()`
#[no_mangle]
pub extern "system" fn Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrCheckpointTruncate(
    _env: JNIEnv,
    _: JClass,
) -> jint {
    wal::snr_checkpoint_truncate()
}
