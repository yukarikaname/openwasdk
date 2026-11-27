//! `{17b0e613-942a-422d-904c-f90dc71a7dae}`: the `CoreApplication` interface Wine lacks.
//!
//! The interface is not in the Windows SDK's contract metadata and has no documented
//! page, so the slot numbers come from two public observations rather than from a
//! definition: slot 7 returns the application-wide property set (the caller then queries
//! it for `IMap<String, Object>`), and slots 11 and 12 are the add/remove pair for
//! `FinalUnhandledErrorDetected`. Slots 8 to 10 are unknown here.
//!
//! Every slot refuses with `E_NOTIMPL` and says which one it was, because a caller that
//! receives `E_NOTIMPL` cannot report the slot and the first member XAML actually needs
//! is the whole question.

use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

use windows_core::GUID;

use crate::factory::{IID_IINSPECTABLE, IID_IUNKNOWN};
use crate::traced;

pub(crate) const IID: GUID = GUID::from_u128(0x17b0_e613_942a_422d_904c_f90d_c71a_7dae);

const S_OK: i32 = 0;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

/// The members take different parameters and none of them are known; declaring two
/// arguments keeps to the register-only part of the calling convention and lets the
/// callee ignore the rest.
type Member = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

#[repr(C)]
pub(crate) struct Private {
    vtbl: *const PrivateVtbl,
    refs: AtomicU32,
}

#[repr(C)]
struct PrivateVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_iids: unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut GUID) -> i32,
    get_runtime_class_name: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_trust_level: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
    slot_6: Member,
    slot_7: Member,
    slot_8: Member,
    slot_9: Member,
    slot_10: Member,
    slot_11: Member,
    slot_12: Member,
}

/// One refusal per slot, so a trace names the member instead of the object.
macro_rules! members {
    ($($slot:expr => $name:ident),* $(,)?) => {
        $(
            unsafe extern "system" fn $name(
                _this: *mut c_void,
                _first: *mut c_void,
                _second: *mut c_void,
            ) -> i32 {
                refuse($slot)
            }
        )*
    };
}

members! {
    6 => slot_6,
    8 => slot_8,
    9 => slot_9,
    10 => slot_10,
}

/// Slot 7 is the application-wide property set; the module comment says how it is known.
unsafe extern "system" fn slot_7(
    _this: *mut c_void,
    _first: *mut c_void,
    _second: *mut c_void,
) -> i32 {
    refuse(7)
}

/// The engine's `FinalUnhandledErrorDetected` handler and the token it was given.
///
/// Nothing fires it: the event reports an error that reached the application core, and
/// a probe that fails earlier has already printed its reason. Keeping the handler is
/// what makes the token valid for `remove_FinalUnhandledErrorDetected`.
static HANDLERS: Mutex<Vec<(i64, usize)>> = Mutex::new(Vec::new());

static NEXT_TOKEN: AtomicI64 = AtomicI64::new(1);

static PRIVATE_VTBL: PrivateVtbl = PrivateVtbl {
    query_interface,
    add_ref,
    release,
    get_iids,
    get_runtime_class_name,
    get_trust_level,
    slot_6,
    slot_7,
    slot_8,
    slot_9,
    slot_10,
    slot_11,
    slot_12,
};

/// `add_FinalUnhandledErrorDetected(handler, token)`: keep the handler, hand back a
/// token that `remove_FinalUnhandledErrorDetected` accepts.
unsafe extern "system" fn slot_11(
    _this: *mut c_void,
    handler: *mut c_void,
    token: *mut c_void,
) -> i32 {
    if handler.is_null() || token.is_null() {
        return E_POINTER;
    }
    unsafe { add_ref_of(handler) };
    let value = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut handlers) = HANDLERS.lock() {
        handlers.push((value, handler as usize));
    }
    // `EventRegistrationToken` is an `i64` in the ABI.
    unsafe { *(token as *mut i64) = value };
    if traced() {
        eprintln!("opencore: FinalUnhandledErrorDetected registered as token {value}");
    }
    S_OK
}

/// `remove_FinalUnhandledErrorDetected(token)`: release what slot 11 took.
unsafe extern "system" fn slot_12(
    _this: *mut c_void,
    token: *mut c_void,
    _unused: *mut c_void,
) -> i32 {
    let value = token as i64;
    let handler = HANDLERS.lock().ok().and_then(|mut handlers| {
        let index = handlers.iter().position(|(id, _)| *id == value)?;
        Some(handlers.remove(index).1)
    });
    match handler {
        Some(handler) => {
            unsafe { release_of(handler as *mut c_void) };
            S_OK
        }
        None => {
            if traced() {
                eprintln!("opencore: FinalUnhandledErrorDetected token {value} was not held");
            }
            S_OK
        }
    }
}

/// Add a reference to an object whose vtable this crate did not write.
unsafe fn add_ref_of(object: *mut c_void) -> u32 {
    type AddRef = unsafe extern "system" fn(*mut c_void) -> u32;
    let vtable: *const AddRef = unsafe { *(object as *const *const AddRef) };
    let add_ref = unsafe { *vtable.add(1) };
    unsafe { add_ref(object) }
}

unsafe fn release_of(object: *mut c_void) -> u32 {
    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
    let vtable: *const Release = unsafe { *(object as *const *const Release) };
    let release = unsafe { *vtable.add(2) };
    unsafe { release(object) }
}

fn refuse(slot: u32) -> i32 {
    if traced() {
        eprintln!("opencore: ICoreApplicationPrivate slot {slot} called");
    }
    E_NOTIMPL
}

/// A private-interface object with one reference, owned by the caller.
pub(crate) fn create() -> *mut c_void {
    Box::into_raw(Box::new(Private {
        vtbl: &PRIVATE_VTBL,
        refs: AtomicU32::new(1),
    }))
    .cast()
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
    // An interface pointer has no class name; a null string is the answer for one.
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
    let object = unsafe { &*(this as *const Private) };
    object.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let object = unsafe { &*(this as *const Private) };
    let left = object.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        unsafe { drop(Box::from_raw(this as *mut Private)) };
    }
    left
}

/// Drop a reference the tests hold.
#[cfg(test)]
pub(crate) unsafe fn release_object(this: *mut c_void) {
    unsafe { release(this) };
}
