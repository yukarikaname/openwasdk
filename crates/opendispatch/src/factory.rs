//! The activation factory for a `Microsoft.UI.Dispatching` runtime class.
//!
//! This is a vtable and a refcount, written by hand for the same reason
//! `opencomp` writes its factory by hand: `#[implement]` describes the object
//! behind the factory, not the `IActivationFactory` object itself. The statics
//! answer through [`query_statics`], one interface pointer per class.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::{GUID, HSTRING};

use crate::Class;

const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);
const IID_IINSPECTABLE: GUID = GUID::from_u128(0xaf86_e2e0_b12d_4c6a_9c5a_d7aa_6510_1e90);
const IID_IACTIVATION_FACTORY: GUID = GUID::from_u128(0x0000_0035_0000_0000_c000_0000_0000_0046);

const S_OK: i32 = 0;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

#[repr(C)]
pub struct Factory {
    vtbl: *const FactoryVtbl,
    refs: AtomicU32,
    class: Class,
}

/// The `IActivationFactory` layout: three `IUnknown` slots, three
/// `IInspectable` slots, then `ActivateInstance`.
#[repr(C)]
struct FactoryVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_iids: unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut GUID) -> i32,
    get_runtime_class_name: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_trust_level: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
    activate_instance: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

static FACTORY_VTBL: FactoryVtbl = FactoryVtbl {
    query_interface,
    add_ref,
    release,
    get_iids,
    get_runtime_class_name,
    get_trust_level,
    activate_instance,
};

impl Factory {
    /// A factory with one reference, owned by the caller.
    pub fn create(class: Class) -> *mut c_void {
        Box::into_raw(Box::new(Factory {
            vtbl: &FACTORY_VTBL,
            refs: AtomicU32::new(1),
            class,
        }))
        .cast()
    }
}

/// Answer a static interface on the factory. Handing back the interface that was
/// asked for — not a cast of the outer object — is what keeps the vtable slots
/// lined up with the caller's vtable type.
#[cfg(windows)]
fn query_statics(class: Class, iid: GUID, out: *mut *mut c_void) -> Option<i32> {
    use windows_core::Interface;

    use crate::bindings::Microsoft::UI::Dispatching::{
        IDispatcherQueueControllerStatics, IDispatcherQueueStatics,
    };

    let statics: Option<Result<*mut c_void, windows_core::Error>> = match class {
        Class::Controller if iid == IDispatcherQueueControllerStatics::IID => {
            Some(crate::controller::statics().map(|statics| statics.into_raw()))
        }
        Class::Queue if iid == IDispatcherQueueStatics::IID => {
            Some(crate::queue_object::statics().map(|statics| statics.into_raw()))
        }
        _ => None,
    };
    let statics = statics?;
    Some(match statics {
        Ok(pointer) => {
            unsafe { *out = pointer };
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

/// An empty IID list is allowed; it keeps the factory off the COM allocator.
unsafe extern "system" fn get_iids(
    _this: *mut c_void,
    count: *mut u32,
    values: *mut *mut GUID,
) -> i32 {
    if count.is_null() || values.is_null() {
        return E_POINTER;
    }
    unsafe {
        *count = 0;
        *values = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "system" fn get_runtime_class_name(
    this: *mut c_void,
    value: *mut *mut c_void,
) -> i32 {
    if value.is_null() {
        return E_POINTER;
    }
    let factory = unsafe { &*(this as *const Factory) };
    // The handle moves into the out parameter; nothing here drops it.
    unsafe {
        *value = std::mem::transmute::<HSTRING, *mut c_void>(HSTRING::from(factory.class.name()));
    }
    S_OK
}

unsafe extern "system" fn get_trust_level(_this: *mut c_void, value: *mut i32) -> i32 {
    if value.is_null() {
        return E_POINTER;
    }
    unsafe { *value = 0 };
    S_OK
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
        use windows_core::Interface;

        let factory = unsafe { &*(this as *const Factory) };
        match factory.class {
            // The only class with a default constructor. It is created so a
            // caller that activates it gets an object, even though `Complete`
            // is `E_NOTIMPL`.
            Class::ExitDeferral => match crate::deferral::create() {
                Ok(deferral) => {
                    let unknown: windows_core::IUnknown = match deferral.cast() {
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
            // Static classes and the queue/timer/args classes are reached
            // through their statics or the queue, not through a default constructor.
            Class::Controller | Class::Queue | Class::Timer | Class::ShutdownArgs => {
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

    fn vtbl_of(factory: *mut c_void) -> &'static FactoryVtbl {
        unsafe { &**(factory as *const *const FactoryVtbl) }
    }

    /// The table is `IUnknown` + `IInspectable` + `ActivateInstance`; a shorter
    /// table is a call through whatever follows it in `.rdata`.
    #[test]
    fn the_vtable_has_the_inspectable_layout() {
        assert_eq!(
            std::mem::size_of::<FactoryVtbl>(),
            7 * std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn the_factory_answers_the_three_interface_ids() {
        let factory = Factory::create(Class::Controller);
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
        let factory = Factory::create(Class::Queue);
        let vtbl = vtbl_of(factory);
        let unknown = GUID::from_u128(0xdead_beef_0000_0000_0000_0000_0000_0000);
        let mut out: *mut c_void = std::ptr::null_mut();
        let result = unsafe { (vtbl.query_interface)(factory, &unknown, &mut out) };
        assert_eq!(result, E_NOINTERFACE);
        assert!(out.is_null());
        unsafe { (vtbl.release)(factory) };
    }

    #[test]
    fn a_class_without_a_constructor_says_so() {
        let factory = Factory::create(Class::Queue);
        let vtbl = vtbl_of(factory);
        let mut out: *mut c_void = std::ptr::null_mut();
        let result = unsafe { (vtbl.activate_instance)(factory, &mut out) };
        assert_eq!(result, E_NOTIMPL);
        assert!(out.is_null());
        unsafe { (vtbl.release)(factory) };
    }
}
