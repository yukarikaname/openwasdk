//! Stubs whose contract is not implemented yet: input.
//!
//! A stub takes no parameters on purpose: on x86-64 the caller's arguments stay in
//! registers and are ignored, so one body answers every prototype. That works for the
//! loader, which binds by name, and for a caller that checks the `HRESULT`.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn BackButtonIntegration_InjectBackButtonPress() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn BackButtonIntegration_RegisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn BackButtonIntegration_UnregisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn FrameworkInputView_RegisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn FrameworkInputView_UnregisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPaneFramework_Advise() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPaneFramework_RegisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPaneFramework_UnregisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextInputProxy_putInputSettings() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextInput_SetKeyboardSkin() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextInput_SetTextPrediction() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn WindowPositionChanged_RegisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn WindowPositionChanged_UnregisterListener() -> i32 {
    E_NOTIMPL
}
