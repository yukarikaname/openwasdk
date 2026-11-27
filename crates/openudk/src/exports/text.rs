//! Stubs whose contract is not implemented yet: text.
//!
//! A stub takes no parameters on purpose: on x86-64 the caller's arguments stay in
//! registers and are ignored, so one body answers every prototype. That works for the
//! loader, which binds by name, and for a caller that checks the `HRESULT`.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn Mui_GetFontFallbackLanguageList() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Mui_PropagateApplicationLanguages() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextAnalysis_CreateDWritePrivateTextAnalysisSourceProxy() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextAnalysis_GetContentReadingDirection() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextAnalysis_SetLocaleNameList() -> i32 {
    E_NOTIMPL
}
