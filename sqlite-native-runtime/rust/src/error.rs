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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn clear() {
        clear_last_error();
    }

    #[test]
    fn last_error_initially_null() {
        clear();
        let ptr = snr_last_error();
        assert!(ptr.is_null(), "sin error el puntero debe ser nulo");
    }

    #[test]
    fn set_and_read_error() {
        clear();
        set_last_error("error de prueba");
        let ptr = snr_last_error();
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "error de prueba");
    }

    #[test]
    fn clear_resets_error() {
        set_last_error("algo");
        clear_last_error();
        let ptr = snr_last_error();
        assert!(ptr.is_null());
    }

    #[test]
    fn last_error_copy_null_when_no_error() {
        clear();
        let ptr = snr_last_error_copy();
        assert!(ptr.is_null());
    }

    #[test]
    fn last_error_copy_returns_owned_string() {
        clear();
        set_last_error("copia heap");
        let ptr = snr_last_error_copy();
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "copia heap");
        // Liberar la copia
        unsafe { snr_free_string(ptr) };
    }

    #[test]
    fn last_error_copy_is_independent() {
        // La copia debe seguir siendo válida después de clear
        clear();
        set_last_error("copia independiente");
        let ptr = snr_last_error_copy();
        clear_last_error();
        // El error interno está limpio pero la copia sigue viva
        assert!(snr_last_error().is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "copia independiente");
        unsafe { snr_free_string(ptr) };
    }

    #[test]
    fn free_null_is_noop() {
        // No debe panicar
        unsafe { snr_free_string(std::ptr::null_mut()) };
    }

    #[test]
    fn set_error_with_interior_nul_truncates() {
        clear();
        // El mensaje tiene un byte nulo interior: solo "antes" debe quedar
        set_last_error(b"antes\0despues".to_vec());
        let ptr = snr_last_error();
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "antes");
    }

    #[test]
    fn set_error_empty_string() {
        clear();
        set_last_error("");
        let ptr = snr_last_error();
        // Un string vacío sigue siendo Some(""), no None
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "");
    }

    #[test]
    fn thread_local_isolation() {
        // El error en el hilo principal no contamina un hilo nuevo
        set_last_error("error hilo principal");
        let handle = std::thread::spawn(|| {
            // En el nuevo hilo no debe haber error (thread_local fresco)
            snr_last_error().is_null()
        });
        assert!(handle.join().unwrap(), "thread_local debe estar vacío en nuevo hilo");
    }

    #[test]
    fn set_error_overrides_previous() {
        clear();
        set_last_error("primero");
        set_last_error("segundo");
        let ptr = snr_last_error();
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "segundo");
    }
}
