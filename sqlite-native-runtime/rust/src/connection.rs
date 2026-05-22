use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use libsqlite3_sys as ffi;

use crate::error::{clear_last_error, set_last_error};
use crate::handle::{Handle, handle_ref};
use crate::util::cstr_to_str;

// ─── Flags de apertura (mirrors de SQLITE_OPEN_*) ────────────────────────────
// Definidos aquí para que Java no necesite conocer los valores C internos.
pub const SNR_OPEN_READONLY:  i32 = ffi::SQLITE_OPEN_READONLY  as i32;
pub const SNR_OPEN_READWRITE: i32 = ffi::SQLITE_OPEN_READWRITE as i32;
pub const SNR_OPEN_CREATE:    i32 = ffi::SQLITE_OPEN_CREATE    as i32;

// ─── snr_open ────────────────────────────────────────────────────────────────

/// Abre (o crea) la base de datos en `path`.
///
/// `flags` combina SNR_OPEN_READONLY / SNR_OPEN_READWRITE / SNR_OPEN_CREATE.
/// Usa 0 para el comportamiento por defecto (read-write + create).
///
/// Devuelve un puntero opaco `*mut Handle` o NULL en error.
/// Java debe cerrar con `snr_close` cuando termine.
///
/// # Safety
/// `path` debe ser un puntero C válido, no-nulo, nul-terminado, UTF-8.
#[no_mangle]
pub unsafe extern "C" fn snr_open(path: *const c_char, flags: i32) -> *mut Handle {
    clear_last_error();

    let path_str = match cstr_to_str(path) {
        Some(s) => s,
        None => {
            set_last_error("snr_open: path es nulo o no es UTF-8 válido");
            return std::ptr::null_mut();
        }
    };

    // El directorio padre debe existir antes de llamar snr_open.
    // Crear directorios arbitrarios desde una librería FFI es un vector de
    // directory-traversal — se elimina deliberadamente.
    let c_path = match CString::new(path_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("snr_open: path contiene bytes nulos: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Enmascarar a solo los flags públicamente permitidos (I-2).
    // Flags internos de SQLite (DELETEONCLOSE, TEMP_DB, etc.) no son accesibles.
    const ALLOWED_FLAGS: i32 = ffi::SQLITE_OPEN_READONLY as i32
        | ffi::SQLITE_OPEN_READWRITE as i32
        | ffi::SQLITE_OPEN_CREATE as i32
        | ffi::SQLITE_OPEN_URI as i32
        | ffi::SQLITE_OPEN_NOFOLLOW as i32; // rechaza symlinks

    let open_flags = if flags == 0 {
        // SQLITE_OPEN_NOFOLLOW incluido por defecto: rechaza symlinks para evitar
        // que un atacante redirija la apertura a un archivo arbitrario.
        ffi::SQLITE_OPEN_READWRITE
            | ffi::SQLITE_OPEN_CREATE
            | ffi::SQLITE_OPEN_FULLMUTEX
            | ffi::SQLITE_OPEN_NOFOLLOW
    } else {
        (flags & ALLOWED_FLAGS) | ffi::SQLITE_OPEN_FULLMUTEX
    };

    let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();
    let rc = ffi::sqlite3_open_v2(c_path.as_ptr(), &mut db, open_flags as i32, std::ptr::null());

    if rc != ffi::SQLITE_OK {
        let msg = if db.is_null() {
            format!("snr_open: sqlite3_open_v2 falló (rc={rc})")
        } else {
            let err = CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy().to_string();
            ffi::sqlite3_close(db);
            format!("snr_open: {err}")
        };
        set_last_error(msg);
        return std::ptr::null_mut();
    }

    Box::into_raw(Box::new(Handle::new(db)))
}

// ─── snr_open_memory ─────────────────────────────────────────────────────────

/// Abre una base de datos en memoria.
/// `name` puede ser NULL para `:memory:` anónima, o un nombre para
/// bases de datos en memoria compartidas (shared-cache URI).
///
/// # Safety
/// Si `name` no es NULL debe ser un puntero C válido, nul-terminado, UTF-8.
#[no_mangle]
pub unsafe extern "C" fn snr_open_memory(name: *const c_char) -> *mut Handle {
    let path = if name.is_null() {
        CString::new(":memory:").unwrap()
    } else {
        match cstr_to_str(name) {
            Some(n) => {
                // Sanear el nombre: solo [A-Za-z0-9_-] para evitar URI injection (A-2).
                // Caracteres como '?', '&', '/' romperían la URI y podrían abrir archivos reales.
                if !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    set_last_error(
                        "snr_open_memory: name inválido — solo [A-Za-z0-9_-] permitido"
                    );
                    return std::ptr::null_mut();
                }
                CString::new(format!("file:{n}?mode=memory&cache=shared")).unwrap()
            }
            None => {
                set_last_error("snr_open_memory: name no es UTF-8 válido");
                return std::ptr::null_mut();
            }
        }
    };

    let flags = ffi::SQLITE_OPEN_READWRITE
        | ffi::SQLITE_OPEN_CREATE
        | ffi::SQLITE_OPEN_FULLMUTEX
        | ffi::SQLITE_OPEN_URI;

    let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();
    let rc = ffi::sqlite3_open_v2(path.as_ptr(), &mut db, flags as i32, std::ptr::null());

    if rc != ffi::SQLITE_OK {
        let msg = if db.is_null() {
            format!("snr_open_memory: sqlite3_open_v2 falló (rc={rc})")
        } else {
            let err = CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy().to_string();
            ffi::sqlite3_close(db);
            format!("snr_open_memory: {err}")
        };
        set_last_error(msg);
        return std::ptr::null_mut();
    }

    Box::into_raw(Box::new(Handle::new(db)))
}

// ─── snr_close ───────────────────────────────────────────────────────────────

/// Cierra la conexión y libera el Handle. No usar `handle` después de esta llamada.
///
/// La conexión SQLite se cierra físicamente cuando TODOS los StmtHandle derivados
/// de esta conexión también hayan sido cerrados con `snr_stmt_close`. Si hay
/// statements abiertos al llamar `snr_close`, SQLite permanecerá abierto hasta
/// que el último statement se finalice — comportamiento correcto y sin use-after-free.
///
/// # Safety
/// `handle` debe ser un puntero válido obtenido de `snr_open` o `snr_open_memory`,
/// y NO debe usarse después de esta llamada. Llamar exactamente una vez por handle.
#[no_mangle]
pub unsafe extern "C" fn snr_close(handle: *mut Handle) {
    if !handle.is_null() {
        // Soltar el Box libera el Arc<Mutex<RawConn>> del Handle.
        // Si ningún StmtHandle activo sostiene otro Arc, el refcount llega a 0
        // y RawConn::drop llama sqlite3_close automáticamente.
        // Si hay statements abiertos, el Arc sigue vivo en ellos y sqlite3_close
        // se llamará cuando el último snr_stmt_close libere su Arc. (C-1)
        drop(Box::from_raw(handle));
    }
}

// ─── snr_ping ────────────────────────────────────────────────────────────────

/// Verifica que el handle es válido y la conexión responde.
/// Devuelve 1 si OK, 0 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido obtenido de `snr_open`.
#[no_mangle]
pub unsafe extern "C" fn snr_ping(handle: *mut Handle) -> i64 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_ping: handle nulo"); return 0; }
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_ping: mutex envenenado"); return 0; }
    };

    let mut stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
    let sql = b"SELECT 1\0";
    let rc = ffi::sqlite3_prepare_v2(guard.0, sql.as_ptr() as *const _, -1, &mut stmt, std::ptr::null_mut());
    if rc != ffi::SQLITE_OK {
        set_last_error("snr_ping: prepare falló");
        return 0;
    }
    let step_rc = ffi::sqlite3_step(stmt);
    ffi::sqlite3_finalize(stmt);
    if step_rc == ffi::SQLITE_ROW { 1 } else { 0 }
}

// ─── snr_sqlite_version ──────────────────────────────────────────────────────

/// Devuelve la versión de SQLite como *mut c_char. Java debe liberar con snr_free_string.
#[no_mangle]
pub extern "C" fn snr_sqlite_version() -> *mut c_char {
    let ver = unsafe { CStr::from_ptr(ffi::sqlite3_libversion()) }
        .to_string_lossy()
        .into_owned();
    match CString::new(ver) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ─── snr_exec ────────────────────────────────────────────────────────────────

/// Ejecuta una o más sentencias SQL sin resultado.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` y `sql` deben ser punteros válidos y no-nulos.
#[no_mangle]
pub unsafe extern "C" fn snr_exec(handle: *mut Handle, sql: *const c_char) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_exec: handle nulo"); return -1; }
    };
    if sql.is_null() {
        set_last_error("snr_exec: sql es nulo");
        return -1;
    }
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_exec: mutex envenenado"); return -1; }
    };

    let mut errmsg: *mut c_char = std::ptr::null_mut();
    let rc = ffi::sqlite3_exec(guard.0, sql, None, std::ptr::null_mut(), &mut errmsg);

    if rc != ffi::SQLITE_OK {
        if !errmsg.is_null() {
            let msg = CStr::from_ptr(errmsg).to_string_lossy().to_string();
            ffi::sqlite3_free(errmsg as *mut _);
            set_last_error(msg);
        } else {
            set_last_error(format!("snr_exec: error rc={rc}"));
        }
        return -1;
    }
    0
}

// ─── snr_last_insert_rowid ───────────────────────────────────────────────────

/// Devuelve el rowid de la última inserción exitosa.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_last_insert_rowid(handle: *mut Handle) -> i64 {
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => return 0,
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    ffi::sqlite3_last_insert_rowid(guard.0)
}

// ─── snr_changes ─────────────────────────────────────────────────────────────

/// Devuelve el número de filas modificadas por la última operación DML.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_changes(handle: *mut Handle) -> i64 {
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => return 0,
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    ffi::sqlite3_changes64(guard.0)
}

// ─── snr_set_busy_timeout ────────────────────────────────────────────────────

/// Configura el tiempo máximo (ms) que SQLite esperará un lock antes de retornar SQLITE_BUSY.
/// Devuelve 0 en éxito, -1 en error.
///
/// # Safety
/// `handle` debe ser un puntero válido.
#[no_mangle]
pub unsafe extern "C" fn snr_set_busy_timeout(handle: *mut Handle, ms: i32) -> i32 {
    clear_last_error();
    let h = match handle_ref(handle) {
        Some(h) => h,
        None => { set_last_error("snr_set_busy_timeout: handle nulo"); return -1; }
    };
    let guard = match h.inner.lock() {
        Ok(g) => g,
        Err(_) => { set_last_error("snr_set_busy_timeout: mutex envenenado"); return -1; }
    };
    let rc = ffi::sqlite3_busy_timeout(guard.0, ms);
    if rc == ffi::SQLITE_OK { 0 } else { -1 }
}

// ─── snr_open_flags_* (constantes exportadas para Java) ─────────────────────

#[no_mangle] pub extern "C" fn snr_flag_readonly()  -> i32 { SNR_OPEN_READONLY }
#[no_mangle] pub extern "C" fn snr_flag_readwrite() -> i32 { SNR_OPEN_READWRITE }
#[no_mangle] pub extern "C" fn snr_flag_create()    -> i32 { SNR_OPEN_CREATE }
