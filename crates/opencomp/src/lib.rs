//! opencomp replaces `Microsoft.UI.Composition.dll` with an ABI-compatible WinRT server.
//!
//! The goal is to run the official `Microsoft.UI.Xaml` binaries without DWM or DirectComposition:
//! the XAML layer asks this DLL for visuals, surfaces and animations, and this DLL draws them.
//!
//! The generated metadata lives in `bindings`: interface definitions with their IIDs and the
//! `*_Impl` traits a class implements. It is produced by `comptrace generate` and only compiles
//! for a Windows target (the class helpers call into `windows-core`'s Windows-only factory cache).

#![allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    dead_code
)]

#[cfg(windows)]
pub mod bindings;
#[cfg(windows)]
mod compositor;

mod factory;

use std::ffi::c_void;

use windows_core::HSTRING;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// The runtime classes this DLL answers for.
const CLASSES: &[&str] = &["Microsoft.UI.Composition.Compositor"];

/// WinRT entry point: return the activation factory for `class`.
///
/// # Safety
/// `class` is an `HSTRING` owned by the caller and `factory` is writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetActivationFactory(
    class: *mut c_void,
    factory: *mut *mut c_void,
) -> i32 {
    if class.is_null() || factory.is_null() {
        return E_POINTER;
    }
    // The parameter is the HSTRING handle itself, which is a pointer to the string header. Bit
    // copy it into an HSTRING and keep it from dropping: the caller owns the reference.
    let name = std::mem::ManuallyDrop::new(unsafe {
        std::mem::transmute_copy::<*mut c_void, HSTRING>(&class)
    });
    if !known(&name.to_string_lossy()) {
        unsafe { *factory = std::ptr::null_mut() };
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    unsafe { *factory = factory::Factory::create() };
    S_OK
}

/// A DLL with no global objects can always be unloaded.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> i32 {
    S_FALSE
}

/// Classic COM activation is not part of WinRT; only the factory entry point is.
#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    _class: *const c_void,
    _iid: *const c_void,
    _out: *mut *mut c_void,
) -> i32 {
    E_NOTIMPL
}

fn known(class: &str) -> bool {
    CLASSES.contains(&class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_router_knows_compositor_and_rejects_the_rest() {
        assert!(known("Microsoft.UI.Composition.Compositor"));
        assert!(!known("Microsoft.UI.Composition.Visual"));
        assert!(!known(""));
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    /// WinRT passes the HSTRING handle by value, which is the pointer inside the HSTRING.
    fn handle(name: &HSTRING) -> *mut c_void {
        unsafe { std::mem::transmute_copy(name) }
    }

    #[test]
    fn a_known_class_gets_a_factory() {
        let name = HSTRING::from("Microsoft.UI.Composition.Compositor");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, S_OK);
        assert!(!factory.is_null());
        unsafe { crate::factory::release_for_test(factory) };
    }

    #[test]
    fn a_class_this_dll_does_not_serve_is_reported_as_missing() {
        let name = HSTRING::from("Microsoft.UI.Composition.Visual");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, CLASS_E_CLASSNOTAVAILABLE);
        assert!(factory.is_null());
    }
}
