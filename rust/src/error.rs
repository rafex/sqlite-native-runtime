use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::c_char;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

pub(crate) fn set_last_error(msg: impl Into<Vec<u8>>) {
    let cs = CString::new(msg).unwrap_or_else(|_| {
        CString::new("error interno con bytes nulos").expect("static string")
    });
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(cs));
}

pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
}

/// Devuelve puntero interno al último error del hilo. Java NO debe liberarlo.
/// Válido hasta la siguiente llamada a cualquier función snr_*.
#[no_mangle]
pub extern "C" fn snr_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |cs| cs.as_ptr())
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
