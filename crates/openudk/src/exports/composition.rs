//! Stubs whose contract is not implemented yet: composition.
//!
//! A stub takes no parameters on purpose: on x86-64 the caller's arguments stay in
//! registers and are ignored, so one body answers every prototype. That works for the
//! loader, which binds by name, and for a caller that checks the `HRESULT`.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_CreateSharedVisual() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_CreateSharedVisualHandle() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_OpenAndAttachSharedTarget() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_OpenSharedResourceHandle() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DxPrivates_PartnerDeviceFromDevice() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DxPrivates_SwapChainPartnerFromDxgiSwapChain() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DxPrivates_WarpPalCreateExtensionFactory() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DxPrivates_WarpPalGetDeviceCaps() -> i32 {
    E_NOTIMPL
}
