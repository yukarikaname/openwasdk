//! The activation factory for a WinRT runtime class.
//!
//! WinRT asks a DLL for a factory by class name, and the factory creates instances of the class.
//! This is written by hand instead of through `#[implement]`: the factory is three methods and a
//! refcount, and the real work — the class itself — will use the generated `*_Impl` traits.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::GUID;
#[cfg(windows)]
use windows_core::Interface;

use crate::Class;

const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);
const IID_IINSPECTABLE: GUID = GUID::from_u128(0xaf86_e2e0_b12d_4c6a_9c5a_d7aa_6510_1e90);
const IID_IACTIVATION_FACTORY: GUID = GUID::from_u128(0x0000_0035_0000_0000_c000_0000_0000_0046);

/// `E_NOTIMPL`: the factory exists, but nothing can be created yet.
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const S_OK: i32 = 0;

#[repr(C)]
pub struct Factory {
    vtbl: *const FactoryVtbl,
    refs: AtomicU32,
    class: Class,
}

/// The vtable of `IActivationFactory`: `IUnknown` followed by `ActivateInstance`.
#[repr(C)]
struct FactoryVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    activate_instance: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

static FACTORY_VTBL: FactoryVtbl = FactoryVtbl {
    query_interface,
    add_ref,
    release,
    activate_instance,
};

impl Factory {
    /// A factory with one reference, owned by the caller.
    pub fn create(class: Class) -> *mut c_void {
        let factory = Box::new(Factory {
            vtbl: &FACTORY_VTBL,
            refs: AtomicU32::new(1),
            class,
        });
        Box::into_raw(factory).cast()
    }
}

/// Answer a static interface on the factory. A class with only static constructors has no
/// `ActivateInstance`, so this is the only way a client reaches its methods.
///
/// The statics objects are stateless; a fresh one per query keeps this out of the factory's
/// reference count.
#[cfg(windows)]
fn query_statics(class: Class, iid: GUID, out: *mut *mut c_void) -> Option<i32> {
    use windows_core::Interface;

    use crate::bindings::Microsoft::UI::Content::{
        IContentIslandStatics, IDesktopChildSiteBridgeStatics,
    };

    let statics: Result<windows_core::IUnknown, windows_core::Error> = match class {
        Class::ContentIsland if iid == IContentIslandStatics::IID => {
            crate::content::island_statics().and_then(|statics| statics.cast())
        }
        Class::DesktopChildSiteBridge if iid == IDesktopChildSiteBridgeStatics::IID => {
            crate::content::bridge_statics().and_then(|statics| statics.cast())
        }
        _ => return None,
    };
    Some(match statics {
        Ok(statics) => {
            unsafe { *out = statics.into_raw() };
            S_OK
        }
        Err(error) => {
            unsafe { *out = std::ptr::null_mut() };
            error.code().0
        }
    })
}

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_NOINTERFACE;
    }
    let iid = unsafe { *iid };
    if iid == IID_IUNKNOWN || iid == IID_IINSPECTABLE || iid == IID_IACTIVATION_FACTORY {
        unsafe { add_ref(this) };
        unsafe { *out = this };
        return S_OK;
    }
    #[cfg(windows)]
    {
        let factory = unsafe { &*(this as *const Factory) };
        if let Some(result) = query_statics(factory.class, iid, out) {
            return result;
        }
    }
    unsafe { *out = std::ptr::null_mut() };
    E_NOINTERFACE
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let factory = unsafe { &*(this as *const Factory) };
    factory.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let factory = unsafe { &*(this as *const Factory) };
    let left = factory.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        unsafe { drop(Box::from_raw(this as *mut Factory)) };
    }
    left
}

unsafe extern "system" fn activate_instance(this: *mut c_void, out: *mut *mut c_void) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    #[cfg(windows)]
    {
        let factory = unsafe { &*(this as *const Factory) };
        match factory.class {
            Class::Compositor => match crate::compositor::instance() {
                Ok(instance) => {
                    let unknown: windows_core::IUnknown = match instance.cast() {
                        Ok(unknown) => unknown,
                        Err(error) => return error.code().0,
                    };
                    unsafe { *out = unknown.into_raw() };
                    S_OK
                }
                Err(error) => {
                    unsafe { *out = std::ptr::null_mut() };
                    error.code().0
                }
            },
            // A static class has no default constructor; its statics create the object.
            Class::ContentIsland | Class::DesktopChildSiteBridge => {
                unsafe { *out = std::ptr::null_mut() };
                E_NOTIMPL
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = this;
        unsafe { *out = std::ptr::null_mut() };
        E_NOTIMPL
    }
}

#[cfg(test)]
pub(crate) unsafe fn release_for_test(factory: *mut c_void) {
    unsafe { release(factory) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_core::GUID;

    fn vtbl_of(factory: *mut c_void) -> &'static FactoryVtbl {
        unsafe { &**(factory as *const *const FactoryVtbl) }
    }

    #[test]
    fn the_factory_answers_the_three_interface_ids() {
        let factory = Factory::create(Class::Compositor);
        let vtbl = vtbl_of(factory);
        for iid in [IID_IUNKNOWN, IID_IINSPECTABLE, IID_IACTIVATION_FACTORY] {
            let mut out: *mut c_void = std::ptr::null_mut();
            let result = unsafe { (vtbl.query_interface)(factory, &iid, &mut out) };
            assert_eq!(result, S_OK);
            assert_eq!(out, factory);
        }
        for _ in 0..4 {
            unsafe { (vtbl.release)(factory) };
        }
    }

    #[test]
    fn an_unknown_interface_is_refused() {
        let factory = Factory::create(Class::Compositor);
        let vtbl = vtbl_of(factory);
        let unknown = GUID::from_u128(0xdead_beef_0000_0000_0000_0000_0000_0000);
        let mut out: *mut c_void = std::ptr::null_mut();
        let result = unsafe { (vtbl.query_interface)(factory, &unknown, &mut out) };
        assert_eq!(result, E_NOINTERFACE);
        assert!(out.is_null());
        unsafe { (vtbl.release)(factory) };
    }

    #[test]
    #[cfg(windows)]
    fn a_static_class_answers_its_statics_and_has_no_instance() {
        use crate::bindings::Microsoft::UI::Content::IDesktopChildSiteBridgeStatics;

        let factory = Factory::create(Class::DesktopChildSiteBridge);
        let vtbl = vtbl_of(factory);
        let mut out: *mut c_void = std::ptr::null_mut();
        let result = unsafe {
            (vtbl.query_interface)(factory, &IDesktopChildSiteBridgeStatics::IID, &mut out)
        };
        assert_eq!(result, S_OK);
        assert!(!out.is_null());
        unsafe {
            drop(IDesktopChildSiteBridgeStatics::from_raw(out));
            let mut instance: *mut c_void = std::ptr::null_mut();
            let result = (vtbl.activate_instance)(factory, &mut instance);
            assert_eq!(result, E_NOTIMPL);
            assert!(instance.is_null());
            (vtbl.release)(factory);
        }
    }

    #[test]
    fn activation_creates_the_compositor() {
        let factory = Factory::create(Class::Compositor);
        let vtbl = vtbl_of(factory);
        let mut out: *mut c_void = std::ptr::null_mut();
        let result = unsafe { (vtbl.activate_instance)(factory, &mut out) };
        if cfg!(windows) {
            assert_eq!(result, S_OK);
            assert!(!out.is_null());
        } else {
            // The class is behind the Windows-only bindings, so off Windows there is nothing to
            // create and the factory says so.
            assert_eq!(result, E_NOTIMPL);
            assert!(out.is_null());
        }
        unsafe { (vtbl.release)(factory) };
    }
}
