use std::ffi::CStr;
use std::os::raw::c_char;

use libsqlite3_sys as ffi;

use crate::error::{clear_last_error, set_last_error};
use crate::handle::{Handle, handle_ref};
use crate::stmt::{StmtHandle, stmt_ref};

// Tipos de columna SQLite (mirrors de SQLITE_INTEGER, etc.)
pub const SNR_TYPE_INTEGER: i32 = 1;
pub const SNR_TYPE_FLOAT:   i32 = 2;
pub const SNR_TYPE_TEXT:    i32 = 3;
pub const SNR_TYPE_BLOB:    i32 = 4;
pub const SNR_TYPE_NULL:    i32 = 5;

// Resultados de snr_step
pub const SNR_ROW:   i32 =  1;
pub const SNR_DONE:  i32 =  0;
pub const SNR_ERROR: i32 = -1;


// ─── snr_prepare ─────────────────────────────────────────────────────────────

/// Compila `sql` en un prepared statement.
/// Devuelve `*mut StmtHandle` o NULL en error.
/// Cerrar con `snr_stmt_close` cuando ya no se use.
///
/// # Safety
/// `handle` y `sql` deben ser punteros válidos no-nulos.
#[no_mangle]
pub unsafe extern "C" fn snr_prepare(handle: *mut Handle, sql: *const c_char) -> *mut StmtHandle {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_prepare: handle nulo"); return std::ptr::null_mut(); }
    };
    if sql.is_null() {
        set_last_error("snr_prepare: sql es nulo");
        return std::ptr::null_mut();
    }

    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_prepare: mutex envenenado"); return std::ptr::null_mut(); }
    };

    let mut raw_stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
    let rc = ffi::sqlite3_prepare_v2(guard.0, sql, -1, &mut raw_stmt, std::ptr::null_mut());

    if rc != ffi::SQLITE_OK {
        let msg = CStr::from_ptr(ffi::sqlite3_errmsg(guard.0))
            .to_string_lossy()
            .to_string();
        set_last_error(format!("snr_prepare: {msg}"));
        return std::ptr::null_mut();
    }

    let sh = StmtHandle {
        stmt: raw_stmt,
        conn: std::sync::Arc::clone(&h.inner),
    };
    Box::into_raw(Box::new(sh))
}

// ─── snr_stmt_close ──────────────────────────────────────────────────────────

/// Finaliza el statement y libera la memoria. No usar `stmt` después.
///
/// # Safety
/// `stmt` debe ser un puntero válido obtenido de `snr_prepare`.
#[no_mangle]
pub unsafe extern "C" fn snr_stmt_close(stmt: *mut StmtHandle) {
    if stmt.is_null() {
        return;
    }
    let sh = Box::from_raw(stmt);
    let raw_stmt = sh.stmt;
    {
        // Recuperar del poison: siempre hay que finalizar el statement (A-4).
        let _guard = sh.conn.lock().unwrap_or_else(|e| e.into_inner());
        ffi::sqlite3_finalize(raw_stmt);
    }
    // sh cae aquí → Arc decrementado
}

// ─── snr_stmt_reset ──────────────────────────────────────────────────────────

/// Resetea el statement para re-ejecutarlo. No borra los bindings.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `stmt` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_stmt_reset(stmt: *mut StmtHandle) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_stmt_reset: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_stmt_reset: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_reset(sh.stmt);
    if rc == ffi::SQLITE_OK { 0 } else {
        set_last_error(format!("snr_stmt_reset: rc={rc}"));
        -1
    }
}

// ─── snr_stmt_clear_bindings ─────────────────────────────────────────────────

/// Limpia todos los parámetros enlazados (los pone a NULL).
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `stmt` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_stmt_clear_bindings(stmt: *mut StmtHandle) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_stmt_clear_bindings: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_stmt_clear_bindings: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_clear_bindings(sh.stmt);
    if rc == ffi::SQLITE_OK { 0 } else { -1 }
}

// ─── Bind (índice 1-based, igual que SQLite) ─────────────────────────────────

/// Enlaza NULL al parámetro en posición `idx` (1-based).
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `stmt` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_bind_null(stmt: *mut StmtHandle, idx: i32) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_bind_null: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_bind_null: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_bind_null(sh.stmt, idx);
    if rc == ffi::SQLITE_OK { 0 } else { set_error_from_stmt(sh); -1 }
}

/// Enlaza un entero (i64) al parámetro `idx` (1-based).
///
/// # Safety
/// `stmt` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_bind_int(stmt: *mut StmtHandle, idx: i32, val: i64) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_bind_int: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_bind_int: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_bind_int64(sh.stmt, idx, val);
    if rc == ffi::SQLITE_OK { 0 } else { set_error_from_stmt(sh); -1 }
}

/// Enlaza un double (f64) al parámetro `idx` (1-based).
///
/// # Safety
/// `stmt` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_bind_double(stmt: *mut StmtHandle, idx: i32, val: f64) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_bind_double: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_bind_double: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_bind_double(sh.stmt, idx, val);
    if rc == ffi::SQLITE_OK { 0 } else { set_error_from_stmt(sh); -1 }
}

/// Enlaza una cadena UTF-8 al parámetro `idx` (1-based).
/// SQLite copia el texto internamente; `val` puede liberarse después.
/// Si `val` es NULL, enlaza NULL.
///
/// # Safety
/// `stmt` y `val` deben ser punteros válidos.
#[no_mangle]
pub unsafe extern "C" fn snr_bind_text(stmt: *mut StmtHandle, idx: i32, val: *const c_char) -> i32 {
    clear_last_error();
    if val.is_null() {
        return snr_bind_null(stmt, idx);
    }
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_bind_text: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_bind_text: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_bind_text(sh.stmt, idx, val, -1, ffi::SQLITE_TRANSIENT());
    if rc == ffi::SQLITE_OK { 0 } else { set_error_from_stmt(sh); -1 }
}

/// Enlaza un blob al parámetro `idx` (1-based).
/// SQLite copia los bytes internamente.
/// Si `data` es NULL, enlaza NULL.
///
/// # Safety
/// `stmt` y `data` deben ser punteros válidos; `len` debe ser el tamaño real de `data`.
#[no_mangle]
pub unsafe extern "C" fn snr_bind_blob(
    stmt: *mut StmtHandle,
    idx: i32,
    data: *const u8,
    len: i32,
) -> i32 {
    clear_last_error();
    if data.is_null() {
        return snr_bind_null(stmt, idx);
    }
    // Rechazar longitud negativa: sqlite3_bind_blob interpreta len<0 como nul-terminated
    // sobre un *const u8 que puede no tener nul-terminador — buffer over-read (A-3).
    if len < 0 {
        set_last_error("snr_bind_blob: len negativo no permitido");
        return -1;
    }
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_bind_blob: stmt nulo"); return -1; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_bind_blob: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_bind_blob(
        sh.stmt, idx, data as *const _, len, ffi::SQLITE_TRANSIENT(),
    );
    if rc == ffi::SQLITE_OK { 0 } else { set_error_from_stmt(sh); -1 }
}

/// Devuelve el índice (1-based) del parámetro con nombre `name`.
/// Devuelve 0 si no existe. Devuelve 0 y establece error si `name` es NULL.
///
/// # Safety
/// `stmt` y `name` deben ser punteros válidos.
#[no_mangle]
pub unsafe extern "C" fn snr_bind_parameter_index(stmt: *mut StmtHandle, name: *const c_char) -> i32 {
    clear_last_error();
    if name.is_null() {
        set_last_error("snr_bind_parameter_index: name nulo");
        return 0;
    }
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_bind_parameter_index: stmt nulo"); return 0; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_bind_parameter_index: mutex envenenado"); return 0; }
    };
    ffi::sqlite3_bind_parameter_index(sh.stmt, name)
}

// ─── snr_step ────────────────────────────────────────────────────────────────

/// Avanza el statement un paso.
/// Devuelve SNR_ROW (1) si hay fila disponible, SNR_DONE (0) si terminó, SNR_ERROR (-1) en error.
///
/// # Safety
/// `stmt` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_step(stmt: *mut StmtHandle) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_step: stmt nulo"); return SNR_ERROR; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_step: mutex envenenado"); return SNR_ERROR; }
    };
    let rc = ffi::sqlite3_step(sh.stmt);
    match rc {
        ffi::SQLITE_ROW  => SNR_ROW,
        ffi::SQLITE_DONE => SNR_DONE,
        _ => {
            set_error_from_stmt(sh);
            SNR_ERROR
        }
    }
}

// ─── Column (índice 0-based, igual que SQLite) ───────────────────────────────

/// Número de columnas en el resultado.
///
/// # Safety
/// `stmt` debe ser un puntero válido y haber retornado SNR_ROW en el último step.
#[no_mangle]
pub unsafe extern "C" fn snr_column_count(stmt: *mut StmtHandle) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_count: stmt nulo"); return 0; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_count: mutex envenenado"); return 0; }
    };
    ffi::sqlite3_column_count(sh.stmt)
}

/// Tipo de la columna `col` (0-based): SNR_TYPE_INTEGER=1, FLOAT=2, TEXT=3, BLOB=4, NULL=5.
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_type(stmt: *mut StmtHandle, col: i32) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_type: stmt nulo"); return SNR_TYPE_NULL; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_type: mutex envenenado"); return SNR_TYPE_NULL; }
    };
    ffi::sqlite3_column_type(sh.stmt, col)
}

/// Lee la columna `col` (0-based) como i64.
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_int(stmt: *mut StmtHandle, col: i32) -> i64 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_int: stmt nulo"); return 0; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_int: mutex envenenado"); return 0; }
    };
    ffi::sqlite3_column_int64(sh.stmt, col)
}

/// Lee la columna `col` (0-based) como f64.
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_double(stmt: *mut StmtHandle, col: i32) -> f64 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_double: stmt nulo"); return 0.0; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_double: mutex envenenado"); return 0.0; }
    };
    ffi::sqlite3_column_double(sh.stmt, col)
}

/// Lee la columna `col` como texto UTF-8. Devuelve puntero INTERNO de SQLite.
///
/// IMPORTANTE: El puntero es válido SOLO hasta el siguiente snr_step, snr_stmt_reset
/// o snr_stmt_close. Java debe leer y copiar el string inmediatamente.
/// NO llamar snr_free_string sobre este puntero.
///
/// Devuelve NULL si la columna es NULL.
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_text(stmt: *mut StmtHandle, col: i32) -> *const c_char {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_text: stmt nulo"); return std::ptr::null(); }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_text: mutex envenenado"); return std::ptr::null(); }
    };
    let ptr = ffi::sqlite3_column_text(sh.stmt, col);
    ptr as *const c_char
}

/// Lee la columna `col` como texto y devuelve una copia en heap que Java DEBE liberar
/// con snr_free_string. Más seguro que snr_column_text para código multi-step.
/// Devuelve NULL si la columna es NULL.
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_text_owned(stmt: *mut StmtHandle, col: i32) -> *mut c_char {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_text_owned: stmt nulo"); return std::ptr::null_mut(); }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_text_owned: mutex envenenado"); return std::ptr::null_mut(); }
    };
    let ptr = ffi::sqlite3_column_text(sh.stmt, col);
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // Clonar directamente desde CStr: una sola allocación, sin String intermedio.
    // SAFETY: ptr apunta a texto UTF-8 nul-terminado gestionado por SQLite.
    let owned = CStr::from_ptr(ptr as *const c_char).to_owned();
    owned.into_raw()
}

/// Lee la columna `col` como blob. Devuelve puntero INTERNO de SQLite.
/// Válido solo hasta el siguiente step/reset/close.
/// Usar snr_column_bytes para obtener la longitud.
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_blob(stmt: *mut StmtHandle, col: i32) -> *const u8 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_blob: stmt nulo"); return std::ptr::null(); }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_blob: mutex envenenado"); return std::ptr::null(); }
    };
    ffi::sqlite3_column_blob(sh.stmt, col) as *const u8
}

/// Número de bytes del valor blob o text de la columna `col` (sin el nul-terminador para text).
///
/// # Safety
/// `stmt` debe ser válido y haber retornado SNR_ROW.
#[no_mangle]
pub unsafe extern "C" fn snr_column_bytes(stmt: *mut StmtHandle, col: i32) -> i32 {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_bytes: stmt nulo"); return 0; }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_bytes: mutex envenenado"); return 0; }
    };
    ffi::sqlite3_column_bytes(sh.stmt, col)
}

/// Nombre de la columna `col` (0-based). Puntero INTERNO, NO liberar.
/// Válido mientras el statement esté abierto.
///
/// # Safety
/// `stmt` debe ser válido.
#[no_mangle]
pub unsafe extern "C" fn snr_column_name(stmt: *mut StmtHandle, col: i32) -> *const c_char {
    clear_last_error();
    let sh = match stmt_ref(stmt) {
        Some(s) => s,
        None => { set_last_error("snr_column_name: stmt nulo"); return std::ptr::null(); }
    };
    let _guard = match sh.conn.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_column_name: mutex envenenado"); return std::ptr::null(); }
    };
    ffi::sqlite3_column_name(sh.stmt, col)
}

// ─── Helpers internos ────────────────────────────────────────────────────────

unsafe fn set_error_from_stmt(sh: &StmtHandle) {
    let db = ffi::sqlite3_db_handle(sh.stmt);
    if !db.is_null() {
        let msg = CStr::from_ptr(ffi::sqlite3_errmsg(db))
            .to_string_lossy()
            .to_string();
        set_last_error(msg);
    }
}
