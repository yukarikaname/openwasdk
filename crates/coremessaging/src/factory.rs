//! The one class this DLL answers, by handing the request to the dispatcher server.
//!
//! `Microsoft.UI.Windowing.Core.dll`, `Microsoft.UI.Input.dll` and the UDK ask this DLL
//! for `Microsoft.UI.Dispatching.DispatcherQueueController` **directly** -- they import its
//! `DllGetActivationFactory`, so the activation manifest does not decide the answer. On
//! Windows the class is here; Wine cannot load the shipped file at all, so this stand-in
//! answers it and lets `opendispatch.dll` create the object, which is where the queue
//! lives.
//!
//! Every class is asked of the server, not just the controller: `DispatcherQueue` itself
//! arrives the same way once a manifest names it, and a name-by-name list here would only
//! be a second copy of the server's table. What the server declines is what this DLL tells
//! combase it does not serve.
//!
//! That answer is also what keeps the engine off the system CoreUI session: with a
//! dispatcher in hand it takes the same path it takes on Windows, and the private
//! conversation with `coremessaging.dll` never starts.

use std::ffi::c_void;

const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;

/// # Safety
/// `class` is the caller's HSTRING and `factory` is writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetActivationFactory(
    class: *mut c_void,
    factory: *mut *mut c_void,
) -> i32 {
    if class.is_null() || factory.is_null() {
        return E_POINTER;
    }
    unsafe { *factory = std::ptr::null_mut() };
    if let Some(get) = server() {
        let hr = unsafe { get(class, factory) };
        if hr >= 0 {
            return hr;
        }
    }
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        // The parameter is the HSTRING handle itself; the caller owns the reference.
        let name = std::mem::ManuallyDrop::new(unsafe {
            std::mem::transmute_copy::<*mut c_void, windows_core::HSTRING>(&class)
        });
        eprintln!("coremessaging: no factory for {}", name.to_string_lossy());
    }
    CLASS_E_CLASSNOTAVAILABLE
}

/// `opendispatch.dll`'s own entry point, or nothing when it is not beside us.
#[cfg(windows)]
fn server() -> Option<unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32> {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let module = unsafe { LoadLibraryA(c"opendispatch.dll".as_ptr().cast()) };
    if module.is_null() {
        return None;
    }
    let entry = unsafe { GetProcAddress(module, c"DllGetActivationFactory".as_ptr().cast()) };
    if entry.is_null() {
        return None;
    }
    type GetFactory = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    Some(unsafe { std::mem::transmute::<*mut c_void, GetFactory>(entry) })
}

#[cfg(not(windows))]
fn server() -> Option<unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32> {
    None
}
