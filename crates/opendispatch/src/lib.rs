//! opendispatch takes the place of the `Microsoft.UI.Dispatching` server that
//! the Windows App SDK ships in `CoreMessagingXP.dll`.
//!
//! That DLL allocates a reserve object at load time, which Wine does not
//! implement, so the class cannot be activated there at all. This DLL answers
//! the same class names from its own activation table and gives the caller a
//! real dispatcher queue: a Win32 message loop on the creating thread, work
//! posted as a private window message, timers on `SetTimer`.
//!
//! The ABI is generated from `Microsoft.UI.winmd` into [`bindings`]; everything
//! else here is the server behind those interfaces.
//!
//! The crate-level allows are for the ABI: method names such as `TryEnqueue` and
//! `CreateOnCurrentThread` are fixed by the metadata, and the generated module
//! has its own spelling conventions.

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

#[cfg(windows)]
mod args;
#[cfg(windows)]
// Generated from metadata by `comptrace generate`; its lints are the generator's to fix.
#[allow(clippy::all)]
pub mod bindings;
#[cfg(windows)]
mod controller;
#[cfg(windows)]
mod deferral;
#[cfg(windows)]
mod event;
#[cfg(windows)]
mod foundation;
#[cfg(windows)]
mod platform;
#[cfg(any(windows, test))]
mod queue;
#[cfg(windows)]
mod queue_object;
#[cfg(any(windows, test))]
mod thread;
#[cfg(windows)]
mod timer;

mod factory;

use std::ffi::c_void;

use windows_core::HSTRING;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// The runtime classes this DLL answers for, in the order a manifest lists them.
///
/// The SDK's activation table points every one of these at `CoreMessagingXP.dll`.
/// This array is the only place that says which DLL owns them now.
pub const CLASSES: &[&str] = &[
    "Microsoft.UI.Dispatching.DispatcherQueueController",
    "Microsoft.UI.Dispatching.DispatcherQueue",
    "Microsoft.UI.Dispatching.DispatcherQueueTimer",
    "Microsoft.UI.Dispatching.DispatcherQueueShutdownStartingEventArgs",
    "Microsoft.UI.Dispatching.DispatcherExitDeferral",
];

/// What the factory should create for a class name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Class {
    Controller,
    Queue,
    Timer,
    ShutdownArgs,
    ExitDeferral,
}

impl Class {
    /// The name combase, a manifest or a host uses for this class.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Class::Controller => "Microsoft.UI.Dispatching.DispatcherQueueController",
            Class::Queue => "Microsoft.UI.Dispatching.DispatcherQueue",
            Class::Timer => "Microsoft.UI.Dispatching.DispatcherQueueTimer",
            Class::ShutdownArgs => {
                "Microsoft.UI.Dispatching.DispatcherQueueShutdownStartingEventArgs"
            }
            Class::ExitDeferral => "Microsoft.UI.Dispatching.DispatcherExitDeferral",
        }
    }
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
    // The parameter is the HSTRING handle itself; bit-copy it and keep it from
    // dropping, because the caller owns the reference.
    let name = std::mem::ManuallyDrop::new(unsafe {
        std::mem::transmute_copy::<*mut c_void, HSTRING>(&class)
    });
    let name = name.to_string_lossy();
    let Some(class) = class_of(&name) else {
        unsafe { *factory = std::ptr::null_mut() };
        return CLASS_E_CLASSNOTAVAILABLE;
    };
    unsafe { *factory = factory::Factory::create(class) };
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

#[cfg(test)]
fn known(class: &str) -> bool {
    class_of(class).is_some()
}

/// The class a runtime class name names, if this DLL serves it. The export and
/// everything below it go through this one mapping.
fn class_of(name: &str) -> Option<Class> {
    match name {
        "Microsoft.UI.Dispatching.DispatcherQueueController" => Some(Class::Controller),
        "Microsoft.UI.Dispatching.DispatcherQueue" => Some(Class::Queue),
        "Microsoft.UI.Dispatching.DispatcherQueueTimer" => Some(Class::Timer),
        "Microsoft.UI.Dispatching.DispatcherQueueShutdownStartingEventArgs" => {
            Some(Class::ShutdownArgs)
        }
        "Microsoft.UI.Dispatching.DispatcherExitDeferral" => Some(Class::ExitDeferral),
        _ => None,
    }
}

#[cfg(windows)]
pub(crate) fn not_implemented<T>() -> windows_core::Result<T> {
    Err(windows_core::Error::from_hresult(windows_core::HRESULT(
        E_NOTIMPL,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_router_knows_the_dispatcher_classes_and_rejects_the_rest() {
        for class in CLASSES {
            assert!(known(class), "{class}");
        }
        assert!(!known("Microsoft.UI.Dispatching.DispatcherQueuePriority"));
        assert!(!known("Microsoft.UI.Xaml.Application"));
        assert!(!known(""));
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    /// WinRT passes the HSTRING handle by value, which is the pointer inside the
    /// HSTRING.
    fn handle(name: &HSTRING) -> *mut c_void {
        unsafe { std::mem::transmute_copy(name) }
    }

    #[test]
    fn a_known_class_gets_a_factory() {
        let name = HSTRING::from("Microsoft.UI.Dispatching.DispatcherQueueController");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, S_OK);
        assert!(!factory.is_null());
        unsafe { crate::factory::release_for_test(factory) };
    }

    #[test]
    fn a_class_this_dll_does_not_serve_is_reported_as_missing() {
        let name = HSTRING::from("Microsoft.UI.Xaml.Application");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, CLASS_E_CLASSNOTAVAILABLE);
        assert!(factory.is_null());
    }
}
