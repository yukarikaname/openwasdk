//! A stand-in for `Microsoft.Internal.FrameworkUdk.dll`.
//!
//! `Microsoft.ui.xaml.dll` imports 43 names from that DLL and `dcompi.dll` ten,
//! statically, so neither can be loaded without it. This crate provides them:
//! the six `Windowing_*` conversions are real, the rest answer `E_NOTIMPL`.
//!
//! It is a loader shim, not an implementation. Most of the names are called with
//! arguments this file does not declare; on x86-64 the callee ignores them, which
//! is why a stub can be written with no parameters and still be reachable. A
//! caller that checks the `HRESULT` sees a failed feature, which is the same
//! thing it sees from the real DLL when the feature is not present.
//!
//! Build it and rename `openudk.dll` to `Microsoft.Internal.FrameworkUdk.dll`
//! next to the application; packaging is where that happens.

mod exports;

use std::ffi::c_void;

const S_OK: i32 = 0;
pub(crate) const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057u32 as i32;

/// The App SDK treats a `WindowId` as the `HWND` reinterpreted as a 64-bit
/// value, and the display and icon ids the same way, so every one of these six
/// conversions is a cast.
/// # Safety
/// `id` must be writable for one `u64`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Windowing_GetWindowIdFromWindow(
    window: *mut c_void,
    id: *mut u64,
) -> i32 {
    if window.is_null() {
        return E_INVALIDARG;
    }
    if id.is_null() {
        return E_POINTER;
    }
    unsafe { *id = window as u64 };
    S_OK
}

/// # Safety
/// `window` must be writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Windowing_GetWindowFromWindowId(
    id: u64,
    window: *mut *mut c_void,
) -> i32 {
    if id == 0 {
        return E_INVALIDARG;
    }
    if window.is_null() {
        return E_POINTER;
    }
    unsafe { *window = id as *mut c_void };
    S_OK
}

/// # Safety
/// `id` must be writable for one `u64`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Windowing_GetDisplayIdFromMonitor(
    monitor: *mut c_void,
    id: *mut u64,
) -> i32 {
    if id.is_null() {
        return E_POINTER;
    }
    unsafe { *id = monitor as u64 };
    S_OK
}

/// # Safety
/// `monitor` must be writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Windowing_GetMonitorFromDisplayId(
    id: u64,
    monitor: *mut *mut c_void,
) -> i32 {
    if monitor.is_null() {
        return E_POINTER;
    }
    unsafe { *monitor = id as *mut c_void };
    S_OK
}

/// # Safety
/// `id` must be writable for one `u64`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Windowing_GetIconIdFromIcon(icon: *mut c_void, id: *mut u64) -> i32 {
    if id.is_null() {
        return E_POINTER;
    }
    unsafe { *id = icon as u64 };
    S_OK
}

/// # Safety
/// `icon` must be writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Windowing_GetIconFromIconId(id: u64, icon: *mut *mut c_void) -> i32 {
    if icon.is_null() {
        return E_POINTER;
    }
    unsafe { *icon = id as *mut c_void };
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(value: usize) -> *mut c_void {
        value as *mut c_void
    }

    #[test]
    fn a_window_id_is_the_window_handle() {
        let window = address(0x1234_5000);
        let mut id = 0u64;
        unsafe {
            assert_eq!(Windowing_GetWindowIdFromWindow(window, &mut id), S_OK);
            assert_eq!(id, 0x1234_5000);

            let mut back: *mut c_void = std::ptr::null_mut();
            assert_eq!(Windowing_GetWindowFromWindowId(id, &mut back), S_OK);
            assert_eq!(back, window);
        }
    }

    #[test]
    fn a_missing_out_pointer_is_reported() {
        let window = address(16);
        unsafe {
            assert_eq!(
                Windowing_GetWindowIdFromWindow(window, std::ptr::null_mut()),
                E_POINTER
            );
            assert_eq!(
                Windowing_GetWindowFromWindowId(16, std::ptr::null_mut()),
                E_POINTER
            );
        }
    }

    #[test]
    fn a_null_window_is_not_a_window() {
        let mut id = 0u64;
        let mut back = address(1);
        unsafe {
            assert_eq!(
                Windowing_GetWindowIdFromWindow(std::ptr::null_mut(), &mut id),
                E_INVALIDARG
            );
            assert_eq!(Windowing_GetWindowFromWindowId(0, &mut back), E_INVALIDARG);
        }
    }

    #[test]
    fn displays_and_icons_round_trip_the_same_way() {
        let monitor = address(0x9000);
        let mut display = 0u64;
        let mut back: *mut c_void = std::ptr::null_mut();
        unsafe {
            assert_eq!(
                Windowing_GetDisplayIdFromMonitor(monitor, &mut display),
                S_OK
            );
            assert_eq!(Windowing_GetMonitorFromDisplayId(display, &mut back), S_OK);
            assert_eq!(back, monitor);
        }

        let icon = address(0x7000);
        let mut icon_id = 0u64;
        unsafe {
            assert_eq!(Windowing_GetIconIdFromIcon(icon, &mut icon_id), S_OK);
            assert_eq!(Windowing_GetIconFromIconId(icon_id, &mut back), S_OK);
            assert_eq!(back, icon);
        }
    }

    #[test]
    fn a_stub_reports_that_it_is_not_implemented() {
        assert_eq!(exports::Theming_TrySetDarkTheme(), E_NOTIMPL);
    }
}
