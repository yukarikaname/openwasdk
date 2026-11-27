//! Stubs whose contract is not implemented yet: media.
//!
//! A stub takes no parameters on purpose: on x86-64 the caller's arguments stay in
//! registers and are ignored, so one body answers every prototype. That works for the
//! loader, which binds by name, and for a caller that checks the `HRESULT`.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn AudioGraphSettingsExtension_SetUseSpatialAudioApi() -> i32 {
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
pub extern "system" fn TimedMetadataTrackExtension_AddCueEntered() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TimedMetadataTrackExtension_RemoveCueEntered() -> i32 {
    E_NOTIMPL
}
