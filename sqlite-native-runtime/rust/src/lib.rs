//! `sqlite-native-runtime` — C ABI completo de SQLite para Java/GraalVM via Panama FFI.
//!
//! # Diseño
//!
//! Esta librería expone SQLite con una ABI estable tipo C que Java consume
//! directamente sin JNI. El binding Java usa `java.lang.foreign.*` (Panama FFI,
//! estable desde Java 21).
//!
//! ## Prefijo `snr_`
//!
//! Todas las funciones exportadas usan el prefijo `snr_` (SQLite Native Runtime)
//! para evitar colisiones con los símbolos de la propia `libsqlite3`.
//!
//! ## Gestión de memoria
//!
//! - Funciones que devuelven `*mut c_char` transfieren propiedad a Java.
//!   Java DEBE llamar `snr_free_string(ptr)` cuando termine.
//! - `snr_last_error()` devuelve un puntero interno del hilo — NO liberar.
//! - `snr_column_text()` devuelve puntero interno de SQLite — NO liberar,
//!   válido solo hasta el siguiente `snr_step`, `snr_stmt_reset` o `snr_stmt_close`.
//! - `snr_column_text_owned()` devuelve copia en heap — Java DEBE liberar.
//! - `snr_close(handle)` y `snr_stmt_close(stmt)` liberan sus respectivos recursos.
//!
//! ## Seguridad de hilos
//!
//! El `Handle` y `StmtHandle` serializan internamente vía `Mutex`. Es seguro
//! llamar funciones desde múltiples hilos virtuales (Project Loom) con el
//! mismo handle. Los statements de un mismo handle también están serializados.
//!
//! ## Uso básico desde Java
//!
//! ```java
//! // 1. Abrir
//! MemorySegment db = SqliteLibrary.snr_open("/data/app.db", 0);
//!
//! // 2. PRAGMA WAL
//! SqliteLibrary.snr_exec(db, "PRAGMA journal_mode=WAL");
//!
//! // 3. Prepared statement con binding
//! MemorySegment stmt = SqliteLibrary.snr_prepare(db, "INSERT INTO t(x) VALUES(?)");
//! SqliteLibrary.snr_bind_text(stmt, 1, "hola");
//! SqliteLibrary.snr_step(stmt);
//! SqliteLibrary.snr_stmt_close(stmt);
//!
//! // 4. Query con streaming
//! MemorySegment q = SqliteLibrary.snr_prepare(db, "SELECT id, val FROM t");
//! while (SqliteLibrary.snr_step(q) == 1) {
//!     long id  = SqliteLibrary.snr_column_int(q, 0);
//!     String v = SqliteConnection.readColumnText(q, 1);
//! }
//! SqliteLibrary.snr_stmt_close(q);
//!
//! // 5. Cerrar
//! SqliteLibrary.snr_close(db);
//! ```

#![allow(clippy::missing_safety_doc)]

mod error;
mod handle;
mod stmt;
mod util;

mod connection;
mod statement;
mod transaction;
mod wal;

// Re-exportar tipos opacos de handle: aparecen en las firmas de las funciones
// pub (snr_open → *mut Handle, snr_prepare → *mut StmtHandle) y los crates
// externos necesitan poder nombrarlos para integración y tests FFI.
pub use handle::Handle;
pub use stmt::StmtHandle;

// Re-exportar símbolos #[no_mangle] — son visibles en la ABI C sin re-export
// pero los listamos aquí para que el doc sea claro.
#[allow(unused_imports)]
pub use connection::*;
#[allow(unused_imports)]
pub use error::*;
#[allow(unused_imports)]
pub use statement::*;
#[allow(unused_imports)]
pub use transaction::*;
#[allow(unused_imports)]
pub use wal::*;
