//! The names XAML and the lifted compositor import, minus the six conversions
//! that `lib.rs` implements.
//!
//! A stub takes no parameters on purpose: on x86-64 the caller's arguments stay
//! in registers and are ignored, so one body answers every prototype. That works
//! for the loader, which binds by name, and for a caller that checks the
//! `HRESULT`.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn AudioGraphSettingsExtension_SetUseSpatialAudioApi() -> i32 {
    E_NOTIMPL
}

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

#[unsafe(no_mangle)]
pub extern "system" fn FinalUnhandledErrorDetected_RegisterHandler() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn FinalUnhandledErrorDetected_UnregisterHandler() -> i32 {
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
pub extern "system" fn MediaPlaybackDataSourceExtension_SendMediaPlaybackCommand() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackDataSourceExtension_SendPlaybackPositionChangeRequest() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackDataSourceExtension_SendPlaybackRateChangeRequest() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackDataSourceExtension_SendRepeatModeChangeRequest() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackListExtension_AddAutoRepeatChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackListExtension_RemoveAutoRepeatChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_AddCastingRenderLocationChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_AddIsLoopingEnabledChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_GetCurrentMediaPlaybackItem() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_GetMediaPlayerCastingRenderLocation() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_GetVideoSwapchainHandle() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_RemoveCastingRenderLocationChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_RemoveIsLoopingEnabledChanged() -> i32 {
    E_NOTIMPL
}

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
pub extern "system" fn Theming_TryEnableHostBackdropBrush() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Theming_TrySetDarkTheme() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TimedMetadataTrackExtension_AddCueEntered() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TimedMetadataTrackExtension_RemoveCueEntered() -> i32 {
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

#[unsafe(no_mangle)]
pub extern "system" fn WindowPositionChanged_RegisterListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn WindowPositionChanged_UnregisterListener() -> i32 {
    E_NOTIMPL
}
