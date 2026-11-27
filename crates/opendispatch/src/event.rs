//! Calls a WinRT delegate through its own vtable, and the owned wrapper the
//! queue stores.
//!
//! `windows-bindgen` gives `DispatcherQueueHandler` a public `new` but no
//! `Invoke` on the wrapper, and `TypedEventHandler`'s vtable fields are private
//! to the generated module. Both delegates have the same ABI prefix — three
//! `IUnknown` slots — so this module names the layout once and calls the fourth
//! slot.
//!
//! [`Handler`] exists because `IUnknown` is not `Send`: a queue created on a
//! dedicated thread keeps its callbacks in a structure the creating thread can
//! also reach. A WinRT delegate is apartment-agile, and the queue only ever
//! invokes it on its owner thread, which is what the two unsafe impls assert.

use std::ffi::c_void;

use windows_core::{IUnknown, Interface};

/// An owned delegate reference that can cross the queue's threads.
#[derive(Clone)]
pub(crate) struct Handler(pub IUnknown);

unsafe impl Send for Handler {}
unsafe impl Sync for Handler {}

impl Handler {
    pub(crate) fn invoke(&self) {
        // Safety: the wrapper is only built from a live delegate reference.
        unsafe { invoke_handler(&self.0) };
    }

    pub(crate) fn raise(&self, sender: *mut c_void, args: *mut c_void) {
        // Safety: the wrapper is only built from a live event delegate, and the
        // caller passes the sender and args the event carries.
        unsafe { invoke_event(&self.0, sender, args) };
    }
}

/// `DispatcherQueueHandler`: `IUnknown` plus `Invoke()`.
#[repr(C)]
struct HandlerVtbl {
    query_interface: usize,
    add_ref: usize,
    release: usize,
    invoke: unsafe extern "system" fn(*mut c_void) -> i32,
}

/// `TypedEventHandler<TSender, TResult>`: `IUnknown` plus `Invoke(sender, args)`.
#[repr(C)]
struct EventVtbl {
    query_interface: usize,
    add_ref: usize,
    release: usize,
    invoke: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
}

unsafe fn invoke_handler(handler: &IUnknown) {
    let raw = Interface::as_raw(handler);
    // Safety: the vtable is the delegate's, and the fourth slot is `Invoke`.
    let vtbl = unsafe { *(raw as *const *const HandlerVtbl) };
    unsafe { ((*vtbl).invoke)(raw) };
}

/// `args` may be null when the event has a null object argument, which is what a
/// timer tick carries.
unsafe fn invoke_event(handler: &IUnknown, sender: *mut c_void, args: *mut c_void) {
    let raw = Interface::as_raw(handler);
    // Safety: the vtable is the delegate's, and the fourth slot is `Invoke`.
    let vtbl = unsafe { *(raw as *const *const EventVtbl) };
    unsafe { ((*vtbl).invoke)(raw, sender, args) };
}
