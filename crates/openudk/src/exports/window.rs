//! Window and process registrations, and the theme attempts this shim cannot honour.
//!
//! The two register/unregister pairs are real: each records what the caller passed
//! and forgets it when the matching unregister arrives. The records are bookkeeping;
//! there is no message loop or error pipeline here that would deliver an event to
//! them. The theme calls stay `E_NOTIMPL` because there is no window manager or host
//! compositor either; a caller that believed the theme had changed would be worse off
//! than one told it did not.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::E_NOTIMPL;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_FAIL: i32 = 0x8000_4005u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057u32 as i32;

/// One handler and the token that removes it. The handler is the caller's object
/// kept as an address; this shim never dereferences it.
struct ErrorHandler {
    token: i64,
    handler: usize,
}

struct ErrorHandlers {
    next_token: i64,
    handler: Option<ErrorHandler>,
}

impl ErrorHandlers {
    const fn new() -> Self {
        Self {
            next_token: 1,
            handler: None,
        }
    }

    /// Remove the handler for `token`, returning the address that was stored.
    fn take(&mut self, token: i64) -> Option<usize> {
        let entry = self.handler.as_ref()?;
        if entry.token != token {
            return None;
        }
        self.handler.take().map(|entry| entry.handler)
    }
}

static ERROR_HANDLERS: Mutex<ErrorHandlers> = Mutex::new(ErrorHandlers::new());

/// The contract is an event registration: the handler goes in and a token comes out,
/// and the token alone is enough to remove it. The real export forwards both to the
/// WinRT event behind `FinalUnhandledErrorDetected`.
///
/// # Safety
/// `token` must be writable for one `i64`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn FinalUnhandledErrorDetected_RegisterHandler(
    handler: *mut c_void,
    token: *mut i64,
) -> i32 {
    if handler.is_null() {
        return E_INVALIDARG;
    }
    if token.is_null() {
        return E_POINTER;
    }
    let Ok(mut state) = ERROR_HANDLERS.lock() else {
        return E_FAIL;
    };
    let value = state.next_token;
    state.next_token += 1;
    state.handler = Some(ErrorHandler {
        token: value,
        handler: handler as usize,
    });
    unsafe { *token = value };
    S_OK
}

#[unsafe(no_mangle)]
pub extern "system" fn FinalUnhandledErrorDetected_UnregisterHandler(token: i64) -> i32 {
    let Ok(mut state) = ERROR_HANDLERS.lock() else {
        return E_FAIL;
    };
    match state.take(token) {
        Some(_) => S_OK,
        None => E_INVALIDARG,
    }
}

/// The callback a visual diagnostics port invokes: the context it was registered
/// with, the message, and the message length in bytes.
type VisualDiagnosticsCallback =
    Option<unsafe extern "system" fn(*mut c_void, *const c_void, u32) -> i32>;

struct DiagnosticPort {
    name: Vec<u16>,
    callback: VisualDiagnosticsCallback,
    context: usize,
}

static DIAGNOSTIC_PORTS: Mutex<Vec<DiagnosticPort>> = Mutex::new(Vec::new());

/// Function pointer addresses are not unique enough to compare with `==`.
fn same_callback(left: VisualDiagnosticsCallback, right: VisualDiagnosticsCallback) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

/// Read a NUL-terminated UTF-16 string, returning `None` for a null or unterminated
/// pointer.
///
/// # Safety
/// `text` must be null or point to a terminated UTF-16 string.
unsafe fn wide_string(text: *const u16) -> Option<Vec<u16>> {
    if text.is_null() {
        return None;
    }
    let mut len = 0usize;
    while unsafe { *text.add(len) } != 0 {
        len += 1;
        if len > 0x8000 {
            return None;
        }
    }
    Some(unsafe { std::slice::from_raw_parts(text, len) }.to_vec())
}

/// A port is identified by its endpoint name, not by a numeric token.
///
/// # Safety
/// `name` must point to a NUL-terminated UTF-16 string. `context` is handed back to
/// `callback` unchanged and is never dereferenced here.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn VisualDiagnosticsPort_Register(
    name: *const u16,
    callback: VisualDiagnosticsCallback,
    context: *mut c_void,
) -> i32 {
    if callback.is_none() {
        return E_INVALIDARG;
    }
    let Some(name) = (unsafe { wide_string(name) }) else {
        return E_INVALIDARG;
    };
    if name.is_empty() {
        return E_INVALIDARG;
    }
    let Ok(mut ports) = DIAGNOSTIC_PORTS.lock() else {
        return E_FAIL;
    };
    // The same name with the same callback and context is already published.
    if let Some(existing) = ports.iter().find(|port| port.name == name)
        && same_callback(existing.callback, callback)
        && existing.context == context as usize
    {
        return S_OK;
    }
    ports.retain(|port| port.name != name);
    ports.push(DiagnosticPort {
        name,
        callback,
        context: context as usize,
    });
    S_OK
}

/// # Safety
/// `name` must point to a NUL-terminated UTF-16 string.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn VisualDiagnosticsPort_Unregister(name: *const u16) -> i32 {
    let Some(name) = (unsafe { wide_string(name) }) else {
        return E_INVALIDARG;
    };
    let Ok(mut ports) = DIAGNOSTIC_PORTS.lock() else {
        return E_FAIL;
    };
    let before = ports.len();
    ports.retain(|port| port.name != name);
    if ports.len() == before {
        // Nothing was published under that name; that is not an error.
        return S_FALSE;
    }
    S_OK
}

// The real export takes a window and a dark-theme flag, and applies the theme with
// the window manager. There is no window manager here, so both are absent and the
// call reports that it did nothing.
#[unsafe(no_mangle)]
pub extern "system" fn Theming_TrySetDarkTheme() -> i32 {
    E_NOTIMPL
}

// The real export takes a window and asks the host compositor for a backdrop brush.
// This shim has neither the host nor a compositor, so nothing can be enabled.
#[unsafe(no_mangle)]
pub extern "system" fn Theming_TryEnableHostBackdropBrush() -> i32 {
    E_NOTIMPL
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "system" fn port_callback(
        _context: *mut c_void,
        _message: *const c_void,
        _size: u32,
    ) -> i32 {
        S_OK
    }

    fn port_count() -> usize {
        DIAGNOSTIC_PORTS
            .lock()
            .map(|ports| ports.len())
            .unwrap_or(0)
    }

    #[test]
    fn the_registration_pairs_remember_and_forget() {
        let handler = 0x1234usize as *mut c_void;
        let mut first = 0i64;
        let mut second = 0i64;
        unsafe {
            assert_eq!(
                FinalUnhandledErrorDetected_RegisterHandler(handler, &mut first),
                S_OK
            );
            assert_ne!(first, 0);
            // Registering again replaces the handler and invalidates the old token.
            assert_eq!(
                FinalUnhandledErrorDetected_RegisterHandler(handler, &mut second),
                S_OK
            );
            assert_ne!(second, first);
            assert_eq!(
                FinalUnhandledErrorDetected_UnregisterHandler(first),
                E_INVALIDARG
            );
            assert_eq!(FinalUnhandledErrorDetected_UnregisterHandler(second), S_OK);
            assert_eq!(
                FinalUnhandledErrorDetected_UnregisterHandler(second),
                E_INVALIDARG
            );
        }
        assert!(ERROR_HANDLERS.lock().unwrap().handler.is_none());
    }

    #[test]
    fn a_visual_diagnostics_port_is_remembered_by_name() {
        let name: Vec<u16> = "WinUIVisualDiagConnection1\0".encode_utf16().collect();
        let context = 0x5678usize as *mut c_void;
        unsafe {
            assert_eq!(
                VisualDiagnosticsPort_Register(name.as_ptr(), Some(port_callback), context),
                S_OK
            );
        }
        {
            let ports = DIAGNOSTIC_PORTS.lock().unwrap();
            assert_eq!(ports.len(), 1);
            assert_eq!(ports[0].name.as_slice(), &name[..name.len() - 1]);
            assert_eq!(ports[0].context, context as usize);
        }
        unsafe {
            assert_eq!(VisualDiagnosticsPort_Unregister(name.as_ptr()), S_OK);
            assert_eq!(VisualDiagnosticsPort_Unregister(name.as_ptr()), S_FALSE);
        }
        assert_eq!(port_count(), 0);
    }

    #[test]
    fn bad_registration_arguments_leave_state_alone() {
        let mut token = 0x5a5ai64;
        let handler = 0x1234usize as *mut c_void;
        let name: Vec<u16> = "port\0".encode_utf16().collect();
        unsafe {
            assert_eq!(
                FinalUnhandledErrorDetected_RegisterHandler(std::ptr::null_mut(), &mut token),
                E_INVALIDARG
            );
            assert_eq!(token, 0x5a5a);
            assert_eq!(
                FinalUnhandledErrorDetected_RegisterHandler(handler, std::ptr::null_mut()),
                E_POINTER
            );
            assert_eq!(
                VisualDiagnosticsPort_Register(std::ptr::null(), Some(port_callback), handler),
                E_INVALIDARG
            );
            assert_eq!(
                VisualDiagnosticsPort_Register(name.as_ptr(), None, handler),
                E_INVALIDARG
            );
            assert_eq!(
                VisualDiagnosticsPort_Unregister(std::ptr::null()),
                E_INVALIDARG
            );
        }
    }

    #[test]
    fn the_theme_attempts_report_failure() {
        assert_eq!(Theming_TrySetDarkTheme(), E_NOTIMPL);
        assert_eq!(Theming_TryEnableHostBackdropBrush(), E_NOTIMPL);
    }
}
