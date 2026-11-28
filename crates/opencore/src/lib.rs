//! `Windows.ApplicationModel.Core.CoreApplication`, with the interface Wine lacks.
//!
//! `Microsoft.ui.xaml.dll` reaches this class inside `Application.Start` and asks the
//! factory for `{17b0e613-942a-422d-904c-f90dc71a7dae}`. Wine's `twinapi.appcore.dll`
//! serves the class but not that interface, so activation there ends in
//! `E_NOINTERFACE` and nothing starts. The activation manifest points the class at this
//! DLL instead; every other interface on the factory is handed back to Wine.
//!
//! The interface is undocumented: it is not in the Windows SDK's contracts and has no
//! page on learn.microsoft.com, so [`private`] carries what is known about its slots
//! and says so.

mod backtrace;
mod factory;
mod private;

use std::ffi::c_void;

use windows_core::HSTRING;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// The one class this DLL answers for.
pub const CLASS: &str = "Windows.ApplicationModel.Core.CoreApplication";

/// Say what was asked when `OPENCOMP_TRACE` is set.
pub(crate) fn traced() -> bool {
    std::env::var_os("OPENCOMP_TRACE").is_some()
}

/// WinRT entry point: return the activation factory for `class`.
///
/// # Safety
/// `class` is an `HSTRING` owned by the caller and `factory` is writable for one
/// pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetActivationFactory(
    class: *mut c_void,
    factory: *mut *mut c_void,
) -> i32 {
    if class.is_null() || factory.is_null() {
        return E_POINTER;
    }
    backtrace::install();
    // The parameter is the HSTRING handle itself; bit-copy it and keep it from
    // dropping, because the caller owns the reference.
    let name = std::mem::ManuallyDrop::new(unsafe {
        std::mem::transmute_copy::<*mut c_void, HSTRING>(&class)
    });
    if name.to_string_lossy() != CLASS {
        unsafe { *factory = std::ptr::null_mut() };
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    unsafe { *factory = factory::Factory::create() };
    S_OK
}

/// There is one object per process, but a caller may still hold it.
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
    0x8004_0154u32 as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(name: &HSTRING) -> *mut c_void {
        unsafe { std::mem::transmute_copy(name) }
    }

    #[test]
    fn the_class_this_dll_was_built_for_gets_a_factory() {
        let name = HSTRING::from(CLASS);
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, S_OK);
        assert!(!factory.is_null());
        unsafe { factory::release_for_test(factory) };
    }

    #[test]
    fn another_class_is_reported_as_missing() {
        let name = HSTRING::from("Windows.ApplicationModel.Core.NotAClass");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, CLASS_E_CLASSNOTAVAILABLE);
        assert!(factory.is_null());
    }
}
