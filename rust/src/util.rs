use std::ffi::CStr;
use std::os::raw::c_char;

/// Convierte `*const c_char` a `&str`. Devuelve `None` si es nulo o no es UTF-8.
pub(crate) unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// Convierte `*const c_char` a `String`. Devuelve `None` si es nulo o no es UTF-8.
pub(crate) unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    cstr_to_str(ptr).map(|s| s.to_owned())
}
