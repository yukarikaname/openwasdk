//! Stubs whose contract is not implemented yet: window.
//!
//! A stub takes no parameters on purpose: on x86-64 the caller's arguments stay in
//! registers and are ignored, so one body answers every prototype. That works for the
//! loader, which binds by name, and for a caller that checks the `HRESULT`.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn FinalUnhandledErrorDetected_RegisterHandler() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn FinalUnhandledErrorDetected_UnregisterHandler() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Theming_TryEnableHostBackdropBrush() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Theming_TrySetDarkTheme() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn VisualDiagnosticsPort_Register() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn VisualDiagnosticsPort_Unregister() -> i32 {
    E_NOTIMPL
}
