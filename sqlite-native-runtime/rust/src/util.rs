use std::ffi::CStr;
use std::os::raw::c_char;

/// Convierte `*const c_char` a `&str`. Devuelve `None` si es nulo o no es UTF-8.
pub(crate) unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn null_ptr_returns_none() {
        let result = unsafe { cstr_to_str(std::ptr::null()) };
        assert!(result.is_none());
    }

    #[test]
    fn valid_utf8_returns_some() {
        let cs = CString::new("hola mundo").unwrap();
        let result = unsafe { cstr_to_str(cs.as_ptr()) };
        assert_eq!(result, Some("hola mundo"));
    }

    #[test]
    fn empty_string_returns_some_empty() {
        let cs = CString::new("").unwrap();
        let result = unsafe { cstr_to_str(cs.as_ptr()) };
        assert_eq!(result, Some(""));
    }

    #[test]
    fn invalid_utf8_returns_none() {
        // 0xFF no es UTF-8 válido
        let bytes: &[u8] = &[0xFF, 0x00];
        let result = unsafe { cstr_to_str(bytes.as_ptr() as *const c_char) };
        assert!(result.is_none());
    }
}

