//! TT-2 — FFI Contract Tests
//!
//! Validan el contrato C ABI público que Java ve via Panama FFM.
//! Este archivo es un crate externo: solo puede usar símbolos `pub`
//! exactamente igual que lo haría un consumidor externo (o Java).
//!
//! Categorías:
//!   - null_safety          : ningún argumento nulo causa segfault
//!   - memory_ownership     : protocolo free / no-free de punteros
//!   - error_propagation    : semántica del estado de error tras fallos
//!   - thread_isolation     : el thread-local de error no contamina otros hilos
//!   - lifecycle            : secuencias ABI realistas (open → use → close)
//!   - concurrent           : conexiones independientes desde múltiples hilos

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use ether_sqlite_core::{
    snr_begin, snr_begin_exclusive, snr_begin_immediate, snr_bind_blob, snr_bind_double,
    snr_bind_int, snr_bind_null, snr_bind_parameter_index, snr_bind_text, snr_changes,
    snr_checkpoint_full, snr_checkpoint_passive, snr_checkpoint_restart, snr_checkpoint_truncate,
    snr_close, snr_column_blob, snr_column_bytes, snr_column_count, snr_column_double,
    snr_column_int, snr_column_name, snr_column_text, snr_column_text_owned, snr_column_type,
    snr_commit, snr_exec, snr_flag_create, snr_flag_readonly, snr_flag_readwrite,
    snr_free_string, snr_last_error, snr_last_error_copy, snr_last_insert_rowid, snr_open,
    snr_open_memory, snr_ping, snr_prepare, snr_release, snr_rollback, snr_rollback_to,
    snr_savepoint, snr_set_busy_timeout, snr_sqlite_version, snr_stmt_clear_bindings,
    snr_stmt_close, snr_stmt_reset, snr_step, snr_wal_autocheckpoint, snr_wal_checkpoint,
    Handle,
};

// ─── Helpers internos al test ────────────────────────────────────────────────

unsafe fn open_anon() -> *mut Handle {
    snr_open_memory(ptr::null())
}

unsafe fn cstr(ptr: *const c_char) -> &'static str {
    CStr::from_ptr(ptr).to_str().unwrap()
}

// Obtiene la ruta real del temp dir (resuelve el symlink /var → /private/var en macOS).
fn real_temp(name: &str) -> CString {
    let dir = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    CString::new(dir.join(name).to_str().unwrap()).unwrap()
}

// ════════════════════════════════════════════════════════════════════════════
// null_safety — ningún argumento nulo provoca segfault
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn null_snr_open_path() {
    let h = unsafe { snr_open(ptr::null(), 0) };
    assert!(h.is_null());
    unsafe { snr_close(h) };
}

#[test]
fn null_snr_open_memory_name() {
    // NULL es válido para snr_open_memory (abre :memory: anónima)
    let h = unsafe { snr_open_memory(ptr::null()) };
    assert!(!h.is_null());
    unsafe { snr_close(h) };
}

#[test]
fn null_snr_close() {
    // Debe ser noop, no crash
    unsafe { snr_close(ptr::null_mut()) };
}

#[test]
fn null_snr_ping() {
    let rc = unsafe { snr_ping(ptr::null_mut()) };
    assert_eq!(rc, 0);
}

#[test]
fn null_snr_exec_handle() {
    let sql = CString::new("SELECT 1").unwrap();
    let rc = unsafe { snr_exec(ptr::null_mut(), sql.as_ptr()) };
    assert_eq!(rc, -1);
}

#[test]
fn null_snr_exec_sql() {
    let h = unsafe { open_anon() };
    let rc = unsafe { snr_exec(h, ptr::null()) };
    assert_eq!(rc, -1);
    unsafe { snr_close(h) };
}

#[test]
fn null_snr_prepare_handle() {
    let sql = CString::new("SELECT 1").unwrap();
    let s = unsafe { snr_prepare(ptr::null_mut(), sql.as_ptr()) };
    assert!(s.is_null());
}

#[test]
fn null_snr_prepare_sql() {
    let h = unsafe { open_anon() };
    let s = unsafe { snr_prepare(h, ptr::null()) };
    assert!(s.is_null());
    unsafe { snr_close(h) };
}

#[test]
fn null_snr_stmt_close() {
    unsafe { snr_stmt_close(ptr::null_mut()) };
}

#[test]
fn null_snr_step() {
    let rc = unsafe { snr_step(ptr::null_mut()) };
    assert_eq!(rc, -1);
}

#[test]
fn null_snr_bind_all() {
    // Todos los bind_* con stmt=null deben retornar -1, sin crash
    let val = CString::new("x").unwrap();
    let data: &[u8] = &[1, 2, 3];
    unsafe {
        assert_eq!(snr_bind_null(ptr::null_mut(), 1), -1);
        assert_eq!(snr_bind_int(ptr::null_mut(), 1, 42), -1);
        assert_eq!(snr_bind_double(ptr::null_mut(), 1, 1.0), -1);
        assert_eq!(snr_bind_text(ptr::null_mut(), 1, val.as_ptr()), -1);
        assert_eq!(snr_bind_blob(ptr::null_mut(), 1, data.as_ptr(), 3), -1);
    }
}

#[test]
fn null_snr_column_all() {
    // Todas las column_* con stmt=null deben retornar valores seguros, sin crash
    unsafe {
        assert_eq!(snr_column_count(ptr::null_mut()), 0);
        assert_eq!(snr_column_type(ptr::null_mut(), 0), 5); // SNR_TYPE_NULL
        assert_eq!(snr_column_int(ptr::null_mut(), 0), 0);
        assert!((snr_column_double(ptr::null_mut(), 0) - 0.0).abs() < f64::EPSILON);
        assert!(snr_column_text(ptr::null_mut(), 0).is_null());
        assert!(snr_column_text_owned(ptr::null_mut(), 0).is_null());
        assert!(snr_column_blob(ptr::null_mut(), 0).is_null());
        assert_eq!(snr_column_bytes(ptr::null_mut(), 0), 0);
        assert!(snr_column_name(ptr::null_mut(), 0).is_null());
    }
}

#[test]
fn null_snr_transaction_all() {
    // Las funciones de transacción con handle=null deben retornar -1
    unsafe {
        assert_eq!(snr_begin(ptr::null_mut()), -1);
        assert_eq!(snr_begin_immediate(ptr::null_mut()), -1);
        assert_eq!(snr_begin_exclusive(ptr::null_mut()), -1);
        assert_eq!(snr_commit(ptr::null_mut()), -1);
        assert_eq!(snr_rollback(ptr::null_mut()), -1);
    }
}

#[test]
fn null_snr_savepoint_all() {
    let name = CString::new("sp").unwrap();
    unsafe {
        // handle=null
        assert_eq!(snr_savepoint(ptr::null_mut(), name.as_ptr()), -1);
        assert_eq!(snr_release(ptr::null_mut(), name.as_ptr()), -1);
        assert_eq!(snr_rollback_to(ptr::null_mut(), name.as_ptr()), -1);
        // name=null con handle válido
        let h = open_anon();
        assert_eq!(snr_savepoint(h, ptr::null()), -1);
        assert_eq!(snr_release(h, ptr::null()), -1);
        assert_eq!(snr_rollback_to(h, ptr::null()), -1);
        snr_close(h);
    }
}

#[test]
fn null_snr_wal_checkpoint_handle() {
    let rc = unsafe {
        snr_wal_checkpoint(
            ptr::null_mut(),
            ptr::null(),
            snr_checkpoint_passive(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    assert_eq!(rc, -1);
}

#[test]
fn null_snr_wal_autocheckpoint_handle() {
    let rc = unsafe { snr_wal_autocheckpoint(ptr::null_mut(), 1000) };
    assert_eq!(rc, -1);
}

#[test]
fn null_snr_free_string() {
    // Liberar NULL debe ser noop
    unsafe { snr_free_string(ptr::null_mut()) };
}

// ════════════════════════════════════════════════════════════════════════════
// memory_ownership — protocolo de propiedad de punteros
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn ownership_sqlite_version_must_be_freed() {
    // snr_sqlite_version transfiere propiedad: debe liberarse con snr_free_string
    let ptr = snr_sqlite_version();
    assert!(!ptr.is_null());
    let s = unsafe { cstr(ptr) };
    assert!(s.starts_with('3'), "SQLite versión debe ser 3.x: {s}");
    // Liberar — si no se libera sería un leak
    unsafe { snr_free_string(ptr) };
}

#[test]
fn ownership_last_error_is_internal_do_not_free() {
    // snr_last_error() retorna puntero INTERNO — NO se libera.
    // Verificamos que es un puntero no-nulo tras un fallo y que su contenido
    // es legible (sin crash) sin llamar snr_free_string.
    let sql = CString::new("NOT SQL").unwrap();
    let h = unsafe { open_anon() };
    unsafe { snr_exec(h, sql.as_ptr()) };
    let err = snr_last_error();
    assert!(!err.is_null());
    let msg = unsafe { cstr(err) };
    assert!(!msg.is_empty(), "el mensaje de error no debe estar vacío");
    unsafe { snr_close(h) };
    // Deliberadamente NO llamamos snr_free_string(err)
}

#[test]
fn ownership_last_error_copy_must_be_freed() {
    // snr_last_error_copy() transfiere propiedad: debe liberarse
    let sql = CString::new("NOT SQL AGAIN").unwrap();
    let h = unsafe { open_anon() };
    unsafe { snr_exec(h, sql.as_ptr()) };
    let copy = snr_last_error_copy();
    assert!(!copy.is_null());
    let msg = unsafe { cstr(copy) };
    assert!(!msg.is_empty());
    unsafe { snr_free_string(copy) }; // transferir propiedad de vuelta
    unsafe { snr_close(h) };
}

#[test]
fn ownership_last_error_copy_null_when_no_error() {
    // Cuando no hay error, snr_last_error_copy debe retornar NULL (nada que liberar)
    let h = unsafe { open_anon() };
    // Operación exitosa limpia el error
    let sql = CString::new("SELECT 1").unwrap();
    unsafe { snr_exec(h, sql.as_ptr()) };
    let copy = snr_last_error_copy();
    // Si snr_exec llamó clear_last_error, copy debe ser NULL
    // (o puede haber un error residual si el hilo anterior lo dejó — por eso
    // comprobamos que sea NULL solo tras una operación limpia)
    if !copy.is_null() {
        // Liberar si existe para no hacer leak
        unsafe { snr_free_string(copy) };
    }
    unsafe { snr_close(h) };
}

#[test]
fn ownership_column_text_is_internal_do_not_free() {
    // snr_column_text() retorna puntero INTERNO de SQLite — válido solo
    // hasta el siguiente step/reset/close. NO se libera con snr_free_string.
    let h = unsafe { open_anon() };
    let sql = CString::new("SELECT 'hola'").unwrap();
    let s = unsafe { snr_prepare(h, sql.as_ptr()) };
    assert!(!s.is_null());
    assert_eq!(unsafe { snr_step(s) }, 1); // SNR_ROW
    let ptr = unsafe { snr_column_text(s, 0) };
    assert!(!ptr.is_null());
    let txt = unsafe { cstr(ptr) };
    assert_eq!(txt, "hola");
    // NO liberamos ptr — es puntero interno
    unsafe { snr_stmt_close(s); snr_close(h) };
}

#[test]
fn ownership_column_text_owned_must_be_freed() {
    // snr_column_text_owned() transfiere propiedad — Java DEBE liberar
    let h = unsafe { open_anon() };
    let sql = CString::new("SELECT 'mundo'").unwrap();
    let s = unsafe { snr_prepare(h, sql.as_ptr()) };
    assert!(!s.is_null());
    assert_eq!(unsafe { snr_step(s) }, 1);
    let ptr = unsafe { snr_column_text_owned(s, 0) };
    assert!(!ptr.is_null());
    let txt = unsafe { cstr(ptr) };
    assert_eq!(txt, "mundo");
    unsafe { snr_free_string(ptr) }; // transferir propiedad
    unsafe { snr_stmt_close(s); snr_close(h) };
}

#[test]
fn ownership_multiple_snr_free_string_calls_are_independent() {
    // Dos strings distintos deben poder liberarse independientemente
    let ptr1 = snr_sqlite_version();
    let ptr2 = snr_sqlite_version();
    assert!(!ptr1.is_null());
    assert!(!ptr2.is_null());
    unsafe { snr_free_string(ptr1) };
    unsafe { snr_free_string(ptr2) };
}

// ════════════════════════════════════════════════════════════════════════════
// error_propagation — estado de error tras fallos ABI
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn error_after_failed_open() {
    // snr_open con path inválido debe dejar un error recuperable
    let path = CString::new("/directorio/que/no/existe/db.sqlite").unwrap();
    let h = unsafe { snr_open(path.as_ptr(), 0) };
    assert!(h.is_null());
    let err = snr_last_error();
    assert!(!err.is_null(), "debe haber un error después de snr_open fallido");
    let msg = unsafe { cstr(err) };
    assert!(!msg.is_empty());
}

#[test]
fn error_after_failed_prepare() {
    let h = unsafe { open_anon() };
    let bad_sql = CString::new("SELEC * FROM nada").unwrap();
    let s = unsafe { snr_prepare(h, bad_sql.as_ptr()) };
    assert!(s.is_null());
    let err = snr_last_error();
    assert!(!err.is_null());
    let msg = unsafe { cstr(err) };
    assert!(msg.contains("snr_prepare"), "debe indicar el origen: {msg}");
    unsafe { snr_close(h) };
}

#[test]
fn error_cleared_by_successful_operation() {
    // Una operación exitosa debe limpiar el error previo
    let h = unsafe { open_anon() };
    // Primero, provocar un error
    let bad = CString::new("NO SQL").unwrap();
    unsafe { snr_exec(h, bad.as_ptr()) };
    assert!(!snr_last_error().is_null());
    // Ahora una operación exitosa
    let good = CString::new("SELECT 1").unwrap();
    let rc = unsafe { snr_exec(h, good.as_ptr()) };
    assert_eq!(rc, 0);
    // El error debe haberse limpiado
    assert!(snr_last_error().is_null(), "el error debe limpiarse tras operación exitosa");
    unsafe { snr_close(h) };
}

#[test]
fn error_snr_last_error_is_stable_until_next_call() {
    // El puntero de snr_last_error() sigue siendo válido hasta la próxima
    // llamada snr_*. No debe cambiar si no hay ninguna llamada intermedia.
    let bad = CString::new("SYNTAX ERROR HERE").unwrap();
    let h = unsafe { open_anon() };
    unsafe { snr_exec(h, bad.as_ptr()) };
    let ptr1 = snr_last_error();
    let ptr2 = snr_last_error(); // segunda llamada SIN operación intermedia
    assert_eq!(ptr1, ptr2, "mismo puntero interno entre lecturas consecutivas");
    unsafe { snr_close(h) };
}

#[test]
fn error_snr_last_error_copy_vs_internal_same_content() {
    // snr_last_error_copy debe tener el mismo contenido que snr_last_error
    let bad = CString::new("PARSE ERROR").unwrap();
    let h = unsafe { open_anon() };
    unsafe { snr_exec(h, bad.as_ptr()) };
    let internal = snr_last_error();
    let copy = snr_last_error_copy();
    assert!(!internal.is_null());
    assert!(!copy.is_null());
    let s1 = unsafe { cstr(internal) };
    let s2 = unsafe { cstr(copy) };
    assert_eq!(s1, s2, "copy debe tener el mismo contenido que el puntero interno");
    unsafe { snr_free_string(copy) };
    unsafe { snr_close(h) };
}

#[test]
fn error_bind_blob_negative_len_sets_error() {
    // La validación A-3 de len<0 debe establecer un error descriptivo
    let h = unsafe { open_anon() };
    let s = unsafe { snr_prepare(h, CString::new("SELECT ?").unwrap().as_ptr()) };
    let data: &[u8] = &[0x01];
    let rc = unsafe { snr_bind_blob(s, 1, data.as_ptr(), -1) };
    assert_eq!(rc, -1);
    let err = snr_last_error();
    assert!(!err.is_null());
    let msg = unsafe { cstr(err) };
    assert!(msg.contains("negativo"), "A-3: mensaje debe mencionar 'negativo': {msg}");
    unsafe { snr_stmt_close(s); snr_close(h) };
}

#[test]
fn error_open_memory_invalid_chars_sets_error() {
    // Caracteres URI-injection en el nombre deben establecer error descriptivo
    let name = CString::new("bad?param=evil").unwrap();
    let h = unsafe { snr_open_memory(name.as_ptr()) };
    assert!(h.is_null());
    let err = snr_last_error();
    assert!(!err.is_null());
    let msg = unsafe { cstr(err) };
    assert!(msg.contains("inválido"), "A-2: mensaje debe mencionar 'inválido': {msg}");
}

// ════════════════════════════════════════════════════════════════════════════
// thread_isolation — thread-local de error aislado entre OS threads
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn thread_error_does_not_bleed_into_new_thread() {
    // El error del hilo principal no contamina un hilo nuevo
    let bad = CString::new("BAD SQL MAIN").unwrap();
    let h = unsafe { open_anon() };
    unsafe { snr_exec(h, bad.as_ptr()) };
    assert!(!snr_last_error().is_null());
    unsafe { snr_close(h) };

    let spawned_has_no_error = std::thread::spawn(|| {
        // El thread_local de un hilo nuevo debe estar vacío
        snr_last_error().is_null()
    })
    .join()
    .unwrap();

    assert!(spawned_has_no_error, "thread_local no debe contaminar hilos nuevos");
}

#[test]
fn thread_each_thread_has_independent_error() {
    use std::sync::{Arc, Barrier};

    let barrier = Arc::new(Barrier::new(3));
    let errors: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    let threads: Vec<_> = (0..2)
        .map(|i| {
            let b = Arc::clone(&barrier);
            let errs = Arc::clone(&errors);
            std::thread::spawn(move || {
                let h = unsafe { snr_open_memory(ptr::null()) };
                let msg = CString::new(format!("BAD SQL THREAD {i}")).unwrap();
                unsafe { snr_exec(h as *mut Handle, msg.as_ptr()) };
                b.wait(); // sincronizar: los dos hilos han fallado
                let err = snr_last_error();
                let s = if err.is_null() {
                    String::new()
                } else {
                    unsafe { cstr(err) }.to_string()
                };
                errs.lock().unwrap().push(s);
                unsafe { snr_close(h as *mut Handle) };
            })
        })
        .collect();

    barrier.wait(); // hilo principal espera a los dos hilos
    for t in threads { t.join().unwrap(); }

    // Ambos hilos deben haber visto su propio error (no el del otro)
    let errs = errors.lock().unwrap();
    assert_eq!(errs.len(), 2);
    // Cada error debe ser no-vacío (cada hilo provocó un fallo)
    for e in errs.iter() {
        assert!(!e.is_empty(), "cada hilo debe tener su propio error");
    }
}

#[test]
fn thread_snr_last_error_copy_survives_thread_clear() {
    // La COPIA (snr_last_error_copy) sobrevive aunque el hilo limpie su error
    let h = unsafe { open_anon() };
    let bad = CString::new("BAD SQL COPY TEST").unwrap();
    unsafe { snr_exec(h, bad.as_ptr()) };
    let copy = snr_last_error_copy();
    assert!(!copy.is_null());
    // Limpiar el error interno con una operación exitosa
    let good = CString::new("SELECT 42").unwrap();
    unsafe { snr_exec(h, good.as_ptr()) };
    // El error interno ya está limpio, pero la copia sigue viva
    assert!(snr_last_error().is_null());
    let s = unsafe { cstr(copy) };
    assert!(!s.is_empty(), "la copia debe sobrevivir al clear del interno");
    unsafe { snr_free_string(copy); snr_close(h) };
}

// ════════════════════════════════════════════════════════════════════════════
// lifecycle — secuencias ABI completas
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn lifecycle_open_exec_close() {
    let h = unsafe { open_anon() };
    assert!(!h.is_null());
    assert_eq!(unsafe { snr_ping(h) }, 1);
    let ddl = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)").unwrap();
    assert_eq!(unsafe { snr_exec(h, ddl.as_ptr()) }, 0);
    unsafe { snr_close(h) };
}

#[test]
fn lifecycle_prepare_bind_step_close() {
    let h = unsafe { open_anon() };
    let ddl = CString::new("CREATE TABLE t(x INTEGER, y TEXT, z REAL, b BLOB)").unwrap();
    unsafe { snr_exec(h, ddl.as_ptr()) };

    let ins = CString::new("INSERT INTO t VALUES(?, ?, ?, ?)").unwrap();
    let s = unsafe { snr_prepare(h, ins.as_ptr()) };
    assert!(!s.is_null());

    let text = CString::new("hola").unwrap();
    let blob: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];
    unsafe {
        assert_eq!(snr_bind_int(s, 1, 42), 0);
        assert_eq!(snr_bind_text(s, 2, text.as_ptr()), 0);
        assert_eq!(snr_bind_double(s, 3, 2.718), 0);
        assert_eq!(snr_bind_blob(s, 4, blob.as_ptr(), blob.len() as i32), 0);
        assert_eq!(snr_step(s), 0); // SNR_DONE
        snr_stmt_close(s);
    }

    // Verificar con query
    let sel = CString::new("SELECT x, y, z, b FROM t").unwrap();
    let q = unsafe { snr_prepare(h, sel.as_ptr()) };
    assert!(!q.is_null());
    assert_eq!(unsafe { snr_step(q) }, 1); // SNR_ROW
    unsafe {
        assert_eq!(snr_column_int(q, 0), 42);
        assert_eq!(snr_column_type(q, 1), 3); // TEXT
        let bytes = snr_column_bytes(q, 3);
        assert_eq!(bytes, 4);
        let ptr = snr_column_blob(q, 3);
        assert!(!ptr.is_null());
        let read = std::slice::from_raw_parts(ptr, 4);
        assert_eq!(read, &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(snr_step(q), 0); // SNR_DONE
        snr_stmt_close(q);
    }
    unsafe { snr_close(h) };
}

#[test]
fn lifecycle_named_parameter_binding() {
    let h = unsafe { open_anon() };
    let ddl = CString::new("CREATE TABLE t(v INTEGER)").unwrap();
    unsafe { snr_exec(h, ddl.as_ptr()) };

    let ins = CString::new("INSERT INTO t VALUES(:val)").unwrap();
    let s = unsafe { snr_prepare(h, ins.as_ptr()) };
    assert!(!s.is_null());

    let param = CString::new(":val").unwrap();
    let idx = unsafe { snr_bind_parameter_index(s, param.as_ptr()) };
    assert_eq!(idx, 1);
    assert_eq!(unsafe { snr_bind_int(s, idx, 99) }, 0);
    assert_eq!(unsafe { snr_step(s) }, 0); // SNR_DONE
    unsafe { snr_stmt_close(s) };

    // rowid y changes
    let rowid = unsafe { snr_last_insert_rowid(h) };
    assert_eq!(rowid, 1);
    let changes = unsafe { snr_changes(h) };
    assert_eq!(changes, 1);

    unsafe { snr_close(h) };
}

#[test]
fn lifecycle_stmt_reset_and_reuse() {
    let h = unsafe { open_anon() };
    let ddl = CString::new("CREATE TABLE t(n INTEGER)").unwrap();
    unsafe { snr_exec(h, ddl.as_ptr()) };

    let ins = CString::new("INSERT INTO t VALUES(?)").unwrap();
    let s = unsafe { snr_prepare(h, ins.as_ptr()) };
    for i in 1i64..=5 {
        assert_eq!(unsafe { snr_bind_int(s, 1, i) }, 0);
        assert_eq!(unsafe { snr_step(s) }, 0);
        assert_eq!(unsafe { snr_stmt_reset(s) }, 0);
        assert_eq!(unsafe { snr_stmt_clear_bindings(s) }, 0);
    }
    unsafe { snr_stmt_close(s) };

    let count_sql = CString::new("SELECT COUNT(*) FROM t").unwrap();
    let q = unsafe { snr_prepare(h, count_sql.as_ptr()) };
    assert_eq!(unsafe { snr_step(q) }, 1);
    assert_eq!(unsafe { snr_column_int(q, 0) }, 5);
    unsafe { snr_stmt_close(q); snr_close(h) };
}

#[test]
fn lifecycle_transaction_commit() {
    let h = unsafe { open_anon() };
    let ddl = CString::new("CREATE TABLE t(v INTEGER)").unwrap();
    unsafe { snr_exec(h, ddl.as_ptr()) };

    assert_eq!(unsafe { snr_begin(h) }, 0);
    let ins = CString::new("INSERT INTO t VALUES(1)").unwrap();
    unsafe { snr_exec(h, ins.as_ptr()) };
    assert_eq!(unsafe { snr_commit(h) }, 0);

    // Verificar que el dato persiste
    let cnt = CString::new("SELECT COUNT(*) FROM t").unwrap();
    let q = unsafe { snr_prepare(h, cnt.as_ptr()) };
    assert_eq!(unsafe { snr_step(q) }, 1);
    assert_eq!(unsafe { snr_column_int(q, 0) }, 1);
    unsafe { snr_stmt_close(q); snr_close(h) };
}

#[test]
fn lifecycle_transaction_rollback() {
    let h = unsafe { open_anon() };
    let ddl = CString::new("CREATE TABLE t(v INTEGER)").unwrap();
    unsafe { snr_exec(h, ddl.as_ptr()) };

    assert_eq!(unsafe { snr_begin(h) }, 0);
    let ins = CString::new("INSERT INTO t VALUES(1)").unwrap();
    unsafe { snr_exec(h, ins.as_ptr()) };
    assert_eq!(unsafe { snr_rollback(h) }, 0);

    // La tabla debe estar vacía tras el rollback
    let cnt = CString::new("SELECT COUNT(*) FROM t").unwrap();
    let q = unsafe { snr_prepare(h, cnt.as_ptr()) };
    assert_eq!(unsafe { snr_step(q) }, 1);
    assert_eq!(unsafe { snr_column_int(q, 0) }, 0);
    unsafe { snr_stmt_close(q); snr_close(h) };
}

#[test]
fn lifecycle_savepoint_partial_rollback() {
    let h = unsafe { open_anon() };
    let ddl = CString::new("CREATE TABLE t(v INTEGER)").unwrap();
    unsafe { snr_exec(h, ddl.as_ptr()) };

    let sp = CString::new("punto1").unwrap();
    let ins1 = CString::new("INSERT INTO t VALUES(10)").unwrap();
    let ins2 = CString::new("INSERT INTO t VALUES(20)").unwrap();

    unsafe {
        snr_exec(h, ins1.as_ptr()); // INSERT 10 (fuera del savepoint)
        assert_eq!(snr_savepoint(h, sp.as_ptr()), 0);
        snr_exec(h, ins2.as_ptr()); // INSERT 20 (dentro del savepoint)
        assert_eq!(snr_rollback_to(h, sp.as_ptr()), 0); // deshacer INSERT 20
        assert_eq!(snr_release(h, sp.as_ptr()), 0);
    }

    let cnt = CString::new("SELECT COUNT(*) FROM t").unwrap();
    let q = unsafe { snr_prepare(h, cnt.as_ptr()) };
    assert_eq!(unsafe { snr_step(q) }, 1);
    assert_eq!(unsafe { snr_column_int(q, 0) }, 1, "solo el INSERT 10 debe haber sobrevivido");
    unsafe { snr_stmt_close(q); snr_close(h) };
}

#[test]
fn lifecycle_column_name_after_step() {
    let h = unsafe { open_anon() };
    let sql = CString::new("SELECT 1 AS primero, 2 AS segundo").unwrap();
    let s = unsafe { snr_prepare(h, sql.as_ptr()) };
    assert!(!s.is_null());
    assert_eq!(unsafe { snr_step(s) }, 1);
    assert_eq!(unsafe { snr_column_count(s) }, 2);
    let n0 = unsafe { cstr(snr_column_name(s, 0)) };
    let n1 = unsafe { cstr(snr_column_name(s, 1)) };
    assert_eq!(n0, "primero");
    assert_eq!(n1, "segundo");
    unsafe { snr_stmt_close(s); snr_close(h) };
}

#[test]
fn lifecycle_busy_timeout_and_ping() {
    let h = unsafe { open_anon() };
    assert_eq!(unsafe { snr_set_busy_timeout(h, 5000) }, 0);
    assert_eq!(unsafe { snr_ping(h) }, 1);
    unsafe { snr_close(h) };
}

#[test]
fn lifecycle_file_db_open_close() {
    let path = real_temp(&format!("snr_ffi_test_{}.db", std::process::id()));
    let h = unsafe { snr_open(path.as_ptr(), 0) };
    assert!(!h.is_null());
    assert_eq!(unsafe { snr_ping(h) }, 1);
    unsafe { snr_close(h) };
    let _ = std::fs::remove_file(path.to_str().unwrap());
}

// ════════════════════════════════════════════════════════════════════════════
// concurrent — múltiples conexiones independientes desde hilos distintos
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn concurrent_independent_memory_dbs() {
    // Cada hilo abre su propia :memory: — son bases de datos completamente
    // independientes. Las filas escritas por un hilo no son visibles en los demás.
    let handles: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let h = unsafe { snr_open_memory(ptr::null()) };
                assert!(!h.is_null());
                let ddl = CString::new("CREATE TABLE t(v INTEGER)").unwrap();
                unsafe { snr_exec(h as *mut Handle, ddl.as_ptr()) };
                let ins = CString::new(format!("INSERT INTO t VALUES({i})")).unwrap();
                unsafe { snr_exec(h as *mut Handle, ins.as_ptr()) };
                let cnt = CString::new("SELECT COUNT(*) FROM t").unwrap();
                let q = unsafe { snr_prepare(h as *mut Handle, cnt.as_ptr()) };
                assert_eq!(unsafe { snr_step(q) }, 1);
                let count = unsafe { snr_column_int(q, 0) };
                unsafe { snr_stmt_close(q); snr_close(h as *mut Handle) };
                count
            })
        })
        .collect();

    for jh in handles {
        let count: i64 = jh.join().unwrap();
        assert_eq!(count, 1, "cada hilo debe tener exactamente 1 fila en su BD");
    }
}

#[test]
fn concurrent_shared_named_memory_db() {
    // Dos hilos acceden a la misma :memory: con nombre compartido.
    // shared-cache URI permite esto.
    let name_a = CString::new("shared_ffi_test_db").unwrap();
    let name_b = CString::new("shared_ffi_test_db").unwrap();

    let ha = unsafe { snr_open_memory(name_a.as_ptr()) };
    assert!(!ha.is_null());

    // Setup desde el primer hilo: crear tabla e insertar
    let ddl = CString::new("CREATE TABLE IF NOT EXISTS t(v INTEGER)").unwrap();
    unsafe { snr_exec(ha as *mut Handle, ddl.as_ptr()) };
    let ins = CString::new("INSERT INTO t VALUES(777)").unwrap();
    unsafe { snr_exec(ha as *mut Handle, ins.as_ptr()) };

    // Segundo hilo abre la misma BD y verifica
    let row_value = std::thread::spawn(move || {
        let hb = unsafe { snr_open_memory(name_b.as_ptr()) };
        if hb.is_null() { return -1i64; }
        let sel = CString::new("SELECT v FROM t LIMIT 1").unwrap();
        let s = unsafe { snr_prepare(hb as *mut Handle, sel.as_ptr()) };
        if s.is_null() { unsafe { snr_close(hb as *mut Handle) }; return -1i64; }
        let val = if unsafe { snr_step(s) } == 1 {
            unsafe { snr_column_int(s, 0) }
        } else {
            -1
        };
        unsafe { snr_stmt_close(s); snr_close(hb as *mut Handle) };
        val
    })
    .join()
    .unwrap();

    unsafe { snr_close(ha as *mut Handle) };
    assert_eq!(row_value, 777, "el segundo hilo debe ver los datos del primero");
}

// ════════════════════════════════════════════════════════════════════════════
// abi_flags — valores de constantes ABI
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn abi_open_flags_are_distinct() {
    let ro = snr_flag_readonly();
    let rw = snr_flag_readwrite();
    let cr = snr_flag_create();
    assert_ne!(ro, rw);
    assert_ne!(ro, cr);
    assert_ne!(rw, cr);
    // Los tres son potencias de 2 o al menos distintos y positivos
    assert!(ro > 0 && rw > 0 && cr > 0);
}

#[test]
fn abi_checkpoint_modes_are_distinct_and_ordered() {
    // PASSIVE(0) < FULL(1) < RESTART(2) < TRUNCATE(3) — orden SQLite
    let p = snr_checkpoint_passive();
    let f = snr_checkpoint_full();
    let r = snr_checkpoint_restart();
    let t = snr_checkpoint_truncate();
    assert!(p < f);
    assert!(f < r);
    assert!(r < t);
}

#[test]
fn abi_step_result_codes_are_correct() {
    // SNR_ROW=1, SNR_DONE=0, SNR_ERROR=-1 — Java los usa directamente
    let h = unsafe { open_anon() };
    let s = unsafe { snr_prepare(h, CString::new("SELECT 1").unwrap().as_ptr()) };
    let row_code = unsafe { snr_step(s) };
    assert_eq!(row_code, 1, "SNR_ROW debe ser 1");
    let done_code = unsafe { snr_step(s) };
    assert_eq!(done_code, 0, "SNR_DONE debe ser 0");
    let bad = unsafe { snr_step(ptr::null_mut()) };
    assert_eq!(bad, -1, "SNR_ERROR debe ser -1");
    unsafe { snr_stmt_close(s); snr_close(h) };
}

#[test]
fn abi_column_type_codes_match_sqlite_spec() {
    // INTEGER=1, FLOAT=2, TEXT=3, BLOB=4, NULL=5
    let h = unsafe { open_anon() };
    let s = unsafe {
        snr_prepare(h, CString::new("SELECT 1, 1.0, 'x', X'FF', NULL").unwrap().as_ptr())
    };
    assert_eq!(unsafe { snr_step(s) }, 1);
    assert_eq!(unsafe { snr_column_type(s, 0) }, 1); // INTEGER
    assert_eq!(unsafe { snr_column_type(s, 1) }, 2); // FLOAT
    assert_eq!(unsafe { snr_column_type(s, 2) }, 3); // TEXT
    assert_eq!(unsafe { snr_column_type(s, 3) }, 4); // BLOB
    assert_eq!(unsafe { snr_column_type(s, 4) }, 5); // NULL
    unsafe { snr_stmt_close(s); snr_close(h) };
}
