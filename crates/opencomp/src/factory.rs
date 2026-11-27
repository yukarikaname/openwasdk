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
    pub fn create() -> *mut c_void {
        let factory = Box::new(Factory {
            vtbl: &FACTORY_VTBL,
            refs: AtomicU32::new(1),
        });
        Box::into_raw(factory).cast()
    }
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
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
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

unsafe extern "system" fn activate_instance(_this: *mut c_void, out: *mut *mut c_void) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    #[cfg(windows)]
    {
        match crate::compositor::instance() {
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
        }
    }
    #[cfg(not(windows))]
    {
        unsafe { *out = std::ptr::null_mut() };
        E_NOTIMPL
    }
}
