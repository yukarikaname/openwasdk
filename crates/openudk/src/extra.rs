//! The rest of the export table.
//!
//! No DLL in the package imports these, but another App SDK binary might, and a
//! missing export fails the load rather than the call.

use crate::E_NOTIMPL;

#[unsafe(no_mangle)]
pub extern "system" fn AppModelApi_GetProgIdAndACIDForInputs() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn AppxPackageVolumeReset_CheckDeviceForAppReset() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn AppxPackageVolumeReset_RepairDeviceWithAppReset() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Containment_SetConfiguration() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn CoreDragOperationFunctions_StartAsync() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_CreateColorContent() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_CreateCompositionBatchQuery() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_MarkBoundary() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_OpenSharedDCompositionObject() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_ResizeDCompositionSurface() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_SetCommitCompletionEvent() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_SetInputSinkHandle() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_SetTransparentForInput() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_SetVisualInput() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_SetVisualRelativeSize() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DCompPrivates_SetVisualSize() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DisplayPrivates_CreateDisplayEvents() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DisplayPrivates_DisplayConfigGetAdvancedColorInfo() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DisplayPrivates_GetCurrentColorProfilePath() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DisplayPrivates_RegisterStereoChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DisplayPrivates_UnregisterStereoChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn DxPrivates_WarpPrivateAPIFromDevice() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn FrameworkUdk_Diagnostic() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InitializeXamlDiagnosticsEx() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPrivates_EnableMouseInPointerForWindow() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPrivates_GetActivationConfiguration() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPrivates_InputSite_GetForInputSink() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPrivates_IsShellManagedHwnd() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPrivates_IsShellManagedZBand() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MFMediaEngineExtension_DisableVisibilityHandling() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MFMediaEngineExtension_GetCurrentMediaPlaybackItem() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MFMediaEngineExtension_SetSourceFromMediaSource() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaCaptureExtension_GetPreviewSource() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackListExtension_AddCurrentItemChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlaybackListExtension_RemoveCurrentItemChanged() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_DisableVisibilityHandling() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn MediaPlayerExtension_GetCurrentMediaPlaybackItem_Deprecated() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PackageManagement_RemovePackageByUriAsync() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PackageManagement_RemovePackageByUriAsync2() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PackageManagement_RepairPackageAsync() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PackageManagement_RepairPackageAsync2() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PackageManagement_ResetPackageAsync() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PackageManagement_ResetPackageAsync2() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_GetCompositeBatteryStatus() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_GetDischargeTime() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_GetDisplayStatus() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_GetEnergySaverStatus() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_GetPowerCondition() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_GetUserPresenceStatus() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterCompositeBatteryStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterDischargeTimeChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterDisplayStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterEnergySaverStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterPowerConditionChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterSystemIdleStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_RegisterUserPresenceStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterCompositeBatteryStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterDischargeTimeChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterDisplayStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterEnergySaverStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterPowerConditionChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterSystemIdleStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PowerNotifications_UnregisterUserPresenceStatusChangedListener() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_CloseChannel() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_CreateChannelWithRemoteIdentifier() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_GetFullTrustApplicationsWithChannels() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_RegisterFullTrustApplication() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_RegisterNotificationSinkForFullTrustApplication() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_UnregisterFullTrustApplication() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn PushNotifications_UnregisterNotificationSinkForFullTrustApplication() -> i32
{
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ResourceManager_GetDefaultResourcePathForPackageFullName() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn SyncLayout_CreateLayoutSyncForHwnd() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn SyncLayout_EnableSynchronizedLayout() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_GetHistory() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_PostToast() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_QuerySettings() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_RemoveAllToastsForApp() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_RemoveToast() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_RemoveToastsWithTagAndGroup() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_RetrieveAssets() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn ToastNotifications_UpdateNotificationData() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Uup_SRAddOrUpdateUupProduct() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Uup_SRFindPackageFullNamesByUupProductId() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Uup_SRGetAllPackageFullNamesInUupProducts() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Uup_SRGetAllUupProductsForPackageFullName() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn WnpNotifications_IsCallingPreviewSupported() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn WnpNotifications_RemoveAllNotificationsForAppOfType() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Workloads_FindIdsForUserOrDefaultAccount() -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn Workloads_GetWorkloadForUserOrDefaultAccount() -> i32 {
    E_NOTIMPL
}
