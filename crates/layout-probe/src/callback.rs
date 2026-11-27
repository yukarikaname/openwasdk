//! A minimal COM object for `ApplicationInitializationCallback`.
//!
//! `Application.Start` keeps the callback and invokes it once the XAML core is up.
//! The object carries the closure, so the measurement can capture the output path
//! and the activation factory without globals.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::GUID;

use crate::abi::*;

type Invoke = unsafe extern "system" fn(Raw, *mut c_void) -> i32;

#[repr(C)]
struct Callback {
    vtable: *const CallbackVtable,
    refs: AtomicU32,
    run: Option<Box<dyn FnOnce()>>,
}

#[repr(C)]
struct CallbackVtable {
    query_interface: QueryInterface,
    add_ref: unsafe extern "system" fn(Raw) -> u32,
    release: ReleaseFn,
    invoke: Invoke,
}

static CALLBACK_VTABLE: CallbackVtable = CallbackVtable {
    query_interface: callback_query,
    add_ref: callback_add_ref,
    release: callback_release,
    invoke: callback_invoke,
};

unsafe extern "system" fn callback_query(this: Raw, iid: *const GUID, out: *mut Raw) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    let iid = unsafe { &*iid };
    if *iid == IID_IUNKNOWN
        || *iid == IID_AGILE_OBJECT
        || *iid == IID_APPLICATION_INITIALIZATION_CALLBACK
    {
        unsafe { *out = this };
        unsafe { callback_add_ref(this) };
        0
    } else {
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
}

unsafe extern "system" fn callback_add_ref(this: Raw) -> u32 {
    let callback = this as *mut Callback;
    unsafe { (*callback).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}

unsafe extern "system" fn callback_release(this: Raw) -> u32 {
    let callback = this as *mut Callback;
    let previous = unsafe { (*callback).refs.fetch_sub(1, Ordering::AcqRel) };
    if previous == 1 {
        unsafe { drop(Box::from_raw(callback)) };
        0
    } else {
        previous - 1
    }
}

unsafe extern "system" fn callback_invoke(this: Raw, _params: *mut c_void) -> i32 {
    let callback = unsafe { &mut *(this as *mut Callback) };
    if let Some(run) = callback.run.take() {
        run();
    }
    0
}

pub struct CallbackHandle(Raw);

impl CallbackHandle {
    pub fn new(run: impl FnOnce() + 'static) -> Self {
        let callback = Box::new(Callback {
            vtable: &CALLBACK_VTABLE,
            refs: AtomicU32::new(1),
            run: Some(Box::new(run)),
        });
        Self(Box::into_raw(callback) as Raw)
    }

    pub fn raw(&self) -> Raw {
        self.0
    }
}

impl Drop for CallbackHandle {
    fn drop(&mut self) {
        unsafe { release(self.0) };
    }
}
