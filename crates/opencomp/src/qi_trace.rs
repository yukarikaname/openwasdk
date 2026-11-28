//! Diagnose a QueryInterface the engine makes on an object we built.
//!
//! An object from `#[implement]` answers `E_NOINTERFACE` for everything outside its list,
//! and that answer is what a stowed exception hides: the caller knows the IID, we never
//! see it. Replacing the object's first vtable slot with a logger that forwards to the
//! original turns the next failure into a name. Diagnostic only: nothing calls this unless
//! `OPENCOMP_QI_TRACE` is set.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows_core::GUID;

type QueryInterface = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;

/// One vtable per implemented type, so the first original seen is the one to forward to.
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn logged(this: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> i32 {
    if let Some(iid) = unsafe { iid.as_ref() } {
        eprintln!("opencomp: object QI {iid:?}");
    }
    let original: QueryInterface =
        unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Relaxed)) };
    unsafe { original(this, iid, out) }
}

/// Watch one object. Safe to call on any interface pointer whose first field is its vtable.
///
/// # Safety
/// `object` must be a live COM interface pointer for the lifetime of the process.
pub(crate) unsafe fn watch(object: *mut c_void) {
    if object.is_null() || std::env::var_os("OPENCOMP_QI_TRACE").is_none() {
        return;
    }
    // Enough for IUnknown, IInspectable and every composition interface these objects carry.
    const SLOTS: usize = 48;
    let table = unsafe { *(object as *const *const *const c_void) };
    if table.is_null() {
        return;
    }
    let mut copy = Vec::with_capacity(SLOTS);
    for slot in 0..SLOTS {
        copy.push(unsafe { *table.add(slot) });
    }
    if ORIGINAL
        .compare_exchange(0, copy[0] as usize, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        // A second type's table: forward to its own original, not the first one's.
        let _ = ORIGINAL.swap(copy[0] as usize, Ordering::Relaxed);
    }
    copy[0] = logged as *const c_void;
    let leaked = Box::leak(copy.into_boxed_slice());
    unsafe { *(object as *mut *const *const c_void) = leaked.as_ptr() };
}
