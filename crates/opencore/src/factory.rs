//! The activation factory for `CoreApplication`.
//!
//! It answers `IActivationFactory`, the class name, and one interface of its own; every
//! other question is forwarded to Wine's `twinapi.appcore.dll`, which keeps whatever
//! behaviour Wine already implements. A vtable and a refcount are written by hand here
//! because the factory itself is not a runtime class.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use windows_core::{GUID, HSTRING};

use crate::{CLASS, private, traced};

pub(crate) const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);
pub(crate) const IID_IINSPECTABLE: GUID =
    GUID::from_u128(0xaf86_e2e0_b12d_4c6a_9c5a_d7aa_6510_1e90);
pub(crate) const IID_IACTIVATION_FACTORY: GUID =
    GUID::from_u128(0x0000_0035_0000_0000_c000_0000_0000_0046);

const S_OK: i32 = 0;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

type QueryInterface = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
type ActivateInstance = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;

#[repr(C)]
pub struct Factory {
    vtbl: *const FactoryVtbl,
    refs: AtomicU32,
}

/// `IInspectable` followed by `IActivationFactory`.
#[repr(C)]
struct FactoryVtbl {
    query_interface: QueryInterface,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_iids: unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut GUID) -> i32,
    get_runtime_class_name: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_trust_level: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
    activate_instance: ActivateInstance,
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

/// Wine's factory for the same class, resolved once.
static WINE_FACTORY: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

impl Factory {
    /// A factory with one reference, owned by the caller.
    pub fn create() -> *mut c_void {
        Box::into_raw(Box::new(Factory {
            vtbl: &FACTORY_VTBL,
            refs: AtomicU32::new(1),
        }))
        .cast()
    }
}

/// The first slot of any `IInspectable` is its `QueryInterface`.
unsafe fn qer(object: *mut c_void) -> QueryInterface {
    unsafe { **(object as *const *const QueryInterface) }
}

/// Slot six of an `IActivationFactory` is `ActivateInstance`.
unsafe fn activator(object: *mut c_void) -> ActivateInstance {
    unsafe { std::mem::transmute(*(object as *const *const usize).add(6)) }
}

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    let iid = unsafe { *iid };
    if iid == IID_IUNKNOWN || iid == IID_IINSPECTABLE || iid == IID_IACTIVATION_FACTORY {
        unsafe { add_ref(this) };
        unsafe { *out = this };
        return S_OK;
    }
    if iid == private::IID {
        unsafe { *out = private::create() };
        return S_OK;
    }
    let wine = wine_factory();
    if !wine.is_null() {
        return unsafe { qer(wine)(wine, &iid, out) };
    }
    unsafe { *out = ptr::null_mut() };
    if traced() {
        eprintln!("opencore: no factory answer for {iid:?}");
    }
    E_NOINTERFACE
}

unsafe extern "system" fn activate_instance(this: *mut c_void, out: *mut *mut c_void) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    let _ = this;
    let wine = wine_factory();
    if wine.is_null() {
        unsafe { *out = ptr::null_mut() };
        return E_NOTIMPL;
    }
    unsafe { activator(wine)(wine, out) }
}

unsafe extern "system" fn get_iids(
    _this: *mut c_void,
    count: *mut u32,
    values: *mut *mut GUID,
) -> i32 {
    if count.is_null() || values.is_null() {
        return E_POINTER;
    }
    // An empty list keeps the factory off the COM allocator.
    unsafe {
        *count = 0;
        *values = ptr::null_mut();
    }
    S_OK
}

unsafe extern "system" fn get_runtime_class_name(
    _this: *mut c_void,
    value: *mut *mut c_void,
) -> i32 {
    if value.is_null() {
        return E_POINTER;
    }
    // The handle moves into the out parameter; nothing here drops it.
    unsafe { *value = std::mem::transmute::<HSTRING, *mut c_void>(HSTRING::from(CLASS)) };
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

/// Wine's `CoreApplication` factory, for the interfaces this one does not answer.
///
/// The DLL is named rather than imported: a build that runs without Wine (the tests on
/// the host) must still load, and an interface we do not know about should end in
/// `E_NOINTERFACE` only because Wine said so.
fn wine_factory() -> *mut c_void {
    let cached = WINE_FACTORY.load(Ordering::Acquire);
    if !cached.is_null() {
        return cached;
    }
    #[cfg(windows)]
    let loaded = load_wine_factory();
    #[cfg(not(windows))]
    let loaded: *mut c_void = ptr::null_mut();
    if !loaded.is_null() {
        WINE_FACTORY.store(loaded, Ordering::Release);
    }
    loaded
}

/// `twinapi.appcore.dll`'s factory for the same class, asked once per process.
#[cfg(windows)]
fn load_wine_factory() -> *mut c_void {
    let mut factory: *mut c_void = ptr::null_mut();
    let Some(get) = wine_entry(b"twinapi.appcore.dll\0", b"DllGetActivationFactory\0") else {
        return ptr::null_mut();
    };
    type GetFactory = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    let get: GetFactory = unsafe { std::mem::transmute(get) };
    let name = HSTRING::from(CLASS);
    let class = unsafe { std::mem::transmute_copy::<HSTRING, *mut c_void>(&name) };
    let hr = unsafe { get(class, &mut factory) };
    if hr < 0 {
        return ptr::null_mut();
    }
    factory
}

/// `GetProcAddress(LoadLibrary(library), name)`, or nothing when either step fails.
#[cfg(windows)]
fn wine_entry(library: &[u8], name: &[u8]) -> Option<*mut c_void> {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let module = unsafe { LoadLibraryA(library.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let entry = unsafe { GetProcAddress(module, name.as_ptr()) };
    (!entry.is_null()).then_some(entry)
}

#[cfg(test)]
pub(crate) unsafe fn release_for_test(factory: *mut c_void) {
    unsafe { release(factory) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_factory_answers_the_private_interface() {
        let factory = Factory::create();
        let mut out: *mut c_void = ptr::null_mut();
        let hr = unsafe { qer(factory)(factory, &private::IID, &mut out) };
        assert_eq!(hr, S_OK);
        assert!(!out.is_null());
        unsafe { private::release_object(out) };
        unsafe { release(factory) };
    }

    #[test]
    fn an_interface_nobody_serves_is_refused() {
        let factory = Factory::create();
        let mut out: *mut c_void = ptr::null_mut();
        let unknown = GUID::from_u128(0xdead_beef_dead_beef_dead_beef_dead_beef);
        let hr = unsafe { qer(factory)(factory, &unknown, &mut out) };
        assert_eq!(hr, E_NOINTERFACE);
        unsafe { release(factory) };
    }
}
