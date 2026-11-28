//! A Compositor statics interface XAML asks for and Microsoft does not document.
//!
//! `{ffcdc0ea-b528-4bfa-a315-2f11f1c5e8f9}` is reached from `WinRTExports.cpp` as a
//! `QueryInterface` target on the `Compositor` factory, and it appears in no metadata file
//! the repository has -- the App SDK's `.winmd` set does not carry it and neither does the
//! Windows SDK's contract set. What can be said about it is what a probe can see, so this
//! serves the interface and refuses the member by name instead of guessing at it.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::GUID;

/// The one interface here: a statics entry point on `Compositor`.
pub(crate) const IID: GUID = GUID::from_u128(0xffcd_c0ea_b528_4bfa_a315_2f11_f1c5_e8f9);

const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);
const IID_IINSPECTABLE: GUID = GUID::from_u128(0xaf86_e2e0_b12d_4c6a_9c5a_d7aa_6510_1e90);

const S_OK: i32 = 0;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

type Member = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

#[repr(C)]
struct Statics {
    vtbl: *const Vtbl,
    refs: AtomicU32,
}

#[repr(C)]
struct Vtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_iids: unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut GUID) -> i32,
    get_runtime_class_name: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_trust_level: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
    method: Member,
}

static VTBL: Vtbl = Vtbl {
    query_interface,
    add_ref,
    release,
    get_iids,
    get_runtime_class_name,
    get_trust_level,
    method,
};

/// A statics object with one reference, owned by the caller.
pub(crate) fn create() -> *mut c_void {
    Box::into_raw(Box::new(Statics {
        vtbl: &VTBL,
        refs: AtomicU32::new(1),
    }))
    .cast()
}

unsafe extern "system" fn method(
    _this: *mut c_void,
    first: *mut c_void,
    second: *mut c_void,
) -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp: private Compositor statics called with {first:?} {second:?}");
    }
    // The caller does not ask what it got back for an interface: it treats the value as a
    // known layout and reads it, so handing it one of ours is worse than not answering at
    // all. Refuse, and let the trace say whether the engine needed it.
    let _ = (first, second);
    E_NOTIMPL
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
    if iid == IID || iid == IID_IUNKNOWN || iid == IID_IINSPECTABLE {
        unsafe { add_ref(this) };
        unsafe { *out = this };
        return S_OK;
    }
    unsafe { *out = ptr::null_mut() };
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp: private statics object asked for {iid:?}");
    }
    E_NOINTERFACE
}

unsafe extern "system" fn get_iids(
    _this: *mut c_void,
    count: *mut u32,
    values: *mut *mut GUID,
) -> i32 {
    if count.is_null() || values.is_null() {
        return E_POINTER;
    }
    // The interface has no public definition to hand out.
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
    unsafe { *value = ptr::null_mut() };
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
    let object = unsafe { &*(this as *const Statics) };
    object.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let object = unsafe { &*(this as *const Statics) };
    let left = object.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        unsafe { drop(Box::from_raw(this as *mut Statics)) };
    }
    left
}
