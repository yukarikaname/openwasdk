//! The activation factory for a WinRT runtime class.
//!
//! WinRT asks a DLL for a factory by class name, and the factory creates instances of the class.
//! This is written by hand instead of through `#[implement]`: the factory is a vtable and a
//! refcount, and the real work — the class itself — uses the generated `*_Impl` traits.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(windows)]
use windows_core::Interface;
use windows_core::{GUID, HSTRING};

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

/// The vtable of `IActivationFactory`.
///
/// The order is the ABI: three `IUnknown` slots, three `IInspectable` slots, then
/// `ActivateInstance`. The middle three are not optional even though this factory has no
/// meaningful answer for them — a caller reads the seventh slot by offset, so a four-entry
/// table hands it whatever happens to follow in `.rdata`, which is a call through garbage.
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

    // Hand out the interface that was asked for, not a cast of it. `#[implement]` gives
    // every interface its own vtable reference inside the outer object, so `IUnknown` and
    // `IContentIslandStatics` are eight bytes apart: answering a request for the second
    // with the first hands the caller a vtable that does not have `Create` in it.
    let statics: Result<*mut c_void, windows_core::Error> = match class {
        Class::ContentIsland if iid == IContentIslandStatics::IID => {
            crate::content::island_statics().map(|statics| statics.into_raw())
        }
        Class::DesktopChildSiteBridge if iid == IDesktopChildSiteBridgeStatics::IID => {
            crate::content::bridge_statics().map(|statics| statics.into_raw())
        }
        _ => return None,
    };
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

/// No interface IDs are reported, which the ABI allows and `windows-core`'s own
/// `IInspectable` vtable does: the list is for diagnostics, and answering `S_OK` with an
/// empty array keeps the factory off the COM allocator on every host.
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

/// The runtime class this factory makes, as an `HSTRING` the caller owns.
unsafe extern "system" fn get_runtime_class_name(
    this: *mut c_void,
    value: *mut *mut c_void,
) -> i32 {
    if value.is_null() {
        return E_POINTER;
    }
    let factory = unsafe { &*(this as *const Factory) };
    // The handle moves into the out parameter; `transmute` consumes the owned string, so
    // nothing here drops it. This is the same shape `windows-core` uses for its own
    // `IInspectable` vtables.
    unsafe {
        *value = std::mem::transmute::<HSTRING, *mut c_void>(HSTRING::from(factory.class.name()));
    }
    S_OK
}

/// `BaseTrust`, the only level a WinRT component of ours can claim.
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
    #[cfg(windows)]
    use windows_core::Interface;

    fn vtbl_of(factory: *mut c_void) -> &'static FactoryVtbl {
        unsafe { &**(factory as *const *const FactoryVtbl) }
    }

    /// The table is `IUnknown` + `IInspectable` + `ActivateInstance`, and a caller reads
    /// `ActivateInstance` at index six. A shorter table is not a compile error; it is a
    /// call through whatever follows in `.rdata`, which is how the gate first crashed.
    #[test]
    fn the_vtable_has_the_inspectable_layout() {
        assert_eq!(
            std::mem::size_of::<FactoryVtbl>(),
            7 * std::mem::size_of::<usize>(),
            "QueryInterface, AddRef, Release, GetIids, GetRuntimeClassName, GetTrustLevel, \
             ActivateInstance"
        );
    }

    /// The same call the runtime makes: cast, then read the seventh slot.
    #[test]
    #[cfg(windows)]
    fn activation_is_reachable_through_the_generic_factory() {
        let factory = Factory::create(Class::Compositor);
        let unknown: windows_core::IUnknown = unsafe { windows_core::IUnknown::from_raw(factory) };
        let activation: windows_core::imp::IGenericFactory =
            unknown.cast().expect("IActivationFactory");
        let instance = activation.ActivateInstance::<windows_core::IInspectable>();
        assert!(
            instance.is_ok(),
            "a compositor comes out of the generic factory"
        );
    }

    /// `#[implement]` puts one vtable reference per interface inside the object, so a
    /// `QueryInterface` that answers with a *cast* of the object answers with the wrong
    /// interface: `IUnknown` and `IContentIslandStatics` sit eight bytes apart, and the
    /// caller then reads `Create` past the end of the smaller table. Comparing the vtable
    /// pointers catches that; calling a slot is what the gate did, and it faulted.
    #[test]
    #[cfg(windows)]
    fn a_statics_query_answers_with_the_interface_that_was_asked_for() {
        use crate::bindings::Microsoft::UI::Content::IContentIslandStatics;

        let table = |statics: &IContentIslandStatics| unsafe {
            (Interface::as_raw(statics) as *const usize).read_unaligned()
        };
        let factory = Factory::create(Class::ContentIsland);
        let unknown: windows_core::IUnknown = unsafe { windows_core::IUnknown::from_raw(factory) };
        let queried: IContentIslandStatics = unknown.cast().expect("statics");
        let direct: IContentIslandStatics = crate::content::island_statics().expect("direct");
        assert_eq!(
            table(&queried),
            table(&direct),
            "the query shifted the interface"
        );
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
