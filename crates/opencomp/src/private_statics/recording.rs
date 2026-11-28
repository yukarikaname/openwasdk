//! The recording device: what XAML gets when the host has no compositor to hand back.
//!
//! Every device call is accepted and every factory returns the device itself, so the engine
//! can keep building its tree. Split from `private_statics.rs` so the statics object and the
//! device are two files.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::GUID;

use super::E_POINTER;

/// The device XAML asks for when the host has no compositor to hand back: a recording
/// object that accepts every device call and returns itself for every factory, so the
/// engine can keep building its tree.
#[repr(C)]
struct RecordingDevice {
    vtbl: *const *const c_void,
    refs: AtomicU32,
}

const DEVICE_SLOTS: usize = 32;

unsafe extern "system" fn dev_ok(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: *mut c_void,
    _c: *mut c_void,
    _d: *mut c_void,
) -> i32 {
    0
}

unsafe fn add_ref_device(this: *mut c_void) {
    let device = unsafe { &*(this as *const RecordingDevice) };
    device.refs.fetch_add(1, Ordering::Relaxed);
}

unsafe extern "system" fn dev_out1(
    this: *mut c_void,
    out: *mut *mut c_void,
    _b: *mut c_void,
    _c: *mut c_void,
) -> i32 {
    if !out.is_null() {
        unsafe {
            add_ref_device(this);
            *out = this;
        }
    }
    0
}

unsafe extern "system" fn dev_out2(
    this: *mut c_void,
    _a: *mut c_void,
    out: *mut *mut c_void,
    _c: *mut c_void,
) -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp: recording device slot22 out {out:?}");
    }
    unsafe { dev_out1(this, out, std::ptr::null_mut(), std::ptr::null_mut()) }
}

unsafe extern "system" fn dev_qi(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() && !iid.is_null() {
        eprintln!("opencomp: recording device asked for {:?}", unsafe { *iid });
    }
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        add_ref_device(this);
        *out = this;
    }
    0
}

unsafe extern "system" fn dev_addref(this: *mut c_void) -> u32 {
    let device = unsafe { &*(this as *const RecordingDevice) };
    device.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn dev_release(this: *mut c_void) -> u32 {
    let device = unsafe { &*(this as *const RecordingDevice) };
    let refs = device.refs.fetch_sub(1, Ordering::Release) - 1;
    if refs == 0 {
        unsafe { drop(Box::from_raw(this as *mut RecordingDevice)) };
    }
    refs
}

pub(crate) fn recording_device() -> *mut c_void {
    use std::sync::OnceLock;
    static VTBL: OnceLock<[usize; DEVICE_SLOTS]> = OnceLock::new();
    let vtbl = VTBL.get_or_init(|| {
        let mut v = [dev_ok as *const c_void as usize; DEVICE_SLOTS];
        v[0] = dev_qi as *const c_void as usize;
        v[1] = dev_addref as *const c_void as usize;
        v[2] = dev_release as *const c_void as usize;
        v[5] = dev_out1 as *const c_void as usize;
        v[6] = dev_out1 as *const c_void as usize;
        v[7] = dev_out1 as *const c_void as usize;
        v[22] = dev_out2 as *const c_void as usize;
        v
    });
    Box::into_raw(Box::new(RecordingDevice {
        vtbl: vtbl.as_ptr() as *const *const c_void,
        refs: AtomicU32::new(1),
    }))
    .cast()
}
