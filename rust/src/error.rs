use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::c_char;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

pub(crate) fn set_last_error(msg: impl Into<Vec<u8>>) {
    let bytes: Vec<u8> = msg.into();
    // Truncar en el primer byte nulo: CString no puede contener bytes nulos internos.
    // Esta función nunca puede entrar en pánico — un panic a través del boundary FFI es UB.
    let nul_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    // SAFETY: hemos eliminado todos los bytes nulos internos.
    let cs = unsafe { CString::from_vec_unchecked(bytes[..nul_pos].to_vec()) };
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(cs));
}

pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
}

/// Devuelve puntero interno del hilo al último error. Java NO debe liberarlo.
/// Válido hasta la siguiente llamada a cualquier función snr_*.
///
/// ADVERTENCIA con Project Loom: si dos virtual threads comparten el mismo carrier
/// thread OS, el error puede sobreescribirse entre la llamada que lo generó y la
/// lectura de este puntero. Usar `snr_last_error_copy()` en entornos con virtual threads.
#[no_mangle]
pub extern "C" fn snr_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |cs| cs.as_ptr())
    })
}

/// Devuelve una COPIA en heap del último error del hilo.
/// Java DEBE liberar el resultado con `snr_free_string` cuando termine.
/// Devuelve NULL si no hay error pendiente.
///
/// Seguro con Project Loom: la copia se toma en el instante de la llamada,
/// evitando carreras con otras virtual threads en el mismo carrier OS.
#[no_mangle]
pub extern "C" fn snr_last_error_copy() -> *mut c_char {
    LAST_ERROR.with(|cell| {
        match cell.borrow().as_ref() {
            None => std::ptr::null_mut(),
            Some(cs) => {
                // Clonar bytes y construir nueva CString — nunca puede fallar porque
                // la CString de origen ya fue validada al crearse en set_last_error.
                // SAFETY: cs.as_bytes() no contiene bytes nulos internos por construcción.
                let copy = unsafe { CString::from_vec_unchecked(cs.as_bytes().to_vec()) };
                copy.into_raw()
            }
        }
    })
}

/// Libera un *mut c_char transferido por Rust. Llamar exactamente una vez por puntero.
///
/// # Safety
/// `ptr` debe haber sido obtenido de una función snr_* que transfiere propiedad.
#[no_mangle]
pub unsafe extern "C" fn snr_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}
