//! A `dcompi.dll` that loads.
//!
//! The SDK's file is the in-process compositor and the server for every
//! `Microsoft.UI.Composition` class. Two of its callers matter here:
//!
//! `Microsoft.DirectManipulation.dll` imports exactly one function,
//! `WinUIDCompositionCreateDevice3`, and the shipped file cannot load under Wine at all: it
//! needs `api-ms-win-appmodel-unlock-l1-1-0.dll!IsDeveloperModeEnabled`, an API set Wine
//! does not carry. The manipulation stack is the input plumbing behind touch and pen, which
//! a probe never feeds, so that export exists and refuses instead of pretending to hand
//! back a device.
//!
//! Everything else arrives through the activation manifest, which names `dcompi.dll` for
//! the composition classes. Those classes are implemented in `opencomp.dll`, so
//! `DllGetActivationFactory` forwards there the same way `coremessaging` forwards the
//! dispatcher classes to `opendispatch`. Without the forward, combase finds the library,
//! finds no entry point, and reports `E_FAIL` -- which is what the engine saw when it asked
//! for `CompositionEasingFunction`.

use std::ffi::c_void;

const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// `WinUIDCompositionCreateDevice3`: the lifted compositor's device factory.
#[unsafe(no_mangle)]
pub extern "system" fn WinUIDCompositionCreateDevice3() -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("dcompi: WinUIDCompositionCreateDevice3 refused");
    }
    E_NOTIMPL
}

/// The activation entry point combase calls for the classes this DLL is named for.
///
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
        eprintln!("dcompi: no factory for the class asked for");
    }
    CLASS_E_CLASSNOTAVAILABLE
}

/// `opencomp.dll`'s own entry point, or nothing when it is not beside us.
fn server() -> Option<unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32> {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let module = unsafe { LoadLibraryA(c"opencomp.dll".as_ptr().cast()) };
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
