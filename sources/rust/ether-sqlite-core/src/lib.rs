//! `ether-sqlite-core` — lógica SQLite reutilizable para FFM y JNI.
//!
//! # Diseño
//!
//! Esta crate rlib contiene TODA la lógica SQLite con ABI Rust pura.
//! No expone símbolos C (`extern "C"` / `#[no_mangle]`); esos los añaden
//! las crates consumidoras según su mecanismo de binding:
//!
//! - `ether-sqlite-ffm` (cdylib): añade `#[no_mangle] extern "C"` y exporta
//!   los símbolos `snr_*` para Panama FFM.
//! - `ether-sqlite-jni` (cdylib): añade `#[no_mangle] extern "system"` y exporta
//!   los símbolos `Java_*` para JNI.
//!
//! ## Prefijo `snr_`
//!
//! Todas las funciones públicas usan el prefijo `snr_` (SQLite Native Runtime)
//! para evitar colisiones con los símbolos de la propia `libsqlite3`.
//!
//! ## Gestión de memoria
//!
//! - Funciones que devuelven `*mut c_char` transfieren propiedad al llamador.
//!   El llamador DEBE llamar `snr_free_string(ptr)` cuando termine.
//! - `snr_last_error()` devuelve un puntero interno del hilo — NO liberar.
//! - `snr_column_text()` devuelve puntero interno de SQLite — NO liberar,
//!   válido solo hasta el siguiente `snr_step`, `snr_stmt_reset` o `snr_stmt_close`.
//! - `snr_column_text_owned()` devuelve copia en heap — el llamador DEBE liberar.
//! - `snr_close(handle)` y `snr_stmt_close(stmt)` liberan sus respectivos recursos.
//!
//! ## Seguridad de hilos
//!
//! El `Handle` y `StmtHandle` serializan internamente vía `Mutex`. Es seguro
//! llamar funciones desde múltiples hilos virtuales (Project Loom) con el
//! mismo handle. Los statements de un mismo handle también están serializados.

#![allow(clippy::missing_safety_doc)]

pub mod error;
mod handle;
mod stmt;
mod util;

pub mod connection;
pub mod statement;
pub mod transaction;
pub mod wal;

// Re-exportar tipos opacos de handle: aparecen en las firmas de las funciones
// pub (snr_open → *mut Handle, snr_prepare → *mut StmtHandle) y los crates
// externos necesitan poder nombrarlos para integración y tests FFI.
pub use handle::Handle;
pub use stmt::StmtHandle;

// Re-exportar toda la API pública para que las crates consumidoras
// (ether-sqlite-ffm, ether-sqlite-jni) puedan acceder a las funciones snr_*.
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
