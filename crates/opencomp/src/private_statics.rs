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
const E_FAIL: i32 = 0x8000_4005u32 as i32;

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
    methods: [Member; METHODS],
}

/// More than the interface is known to have: a call past the end of the real vtable would
/// otherwise go to whatever follows it in `.rdata`, which is how a missing member turns
/// into a page fault instead of a trace line.
const METHODS: usize = 16;

static VTBL: Vtbl = Vtbl {
    query_interface,
    add_ref,
    release,
    get_iids,
    get_runtime_class_name,
    get_trust_level,
    methods: [
        method::<0>,
        method::<1>,
        method::<2>,
        method::<3>,
        method::<4>,
        method::<5>,
        method::<6>,
        method::<7>,
        method::<8>,
        method::<9>,
        method::<10>,
        method::<11>,
        method::<12>,
        method::<13>,
        method::<14>,
        method::<15>,
    ],
};

/// A statics object with one reference, owned by the caller.
pub(crate) fn create() -> *mut c_void {
    Box::into_raw(Box::new(Statics {
        vtbl: &VTBL,
        refs: AtomicU32::new(1),
    }))
    .cast()
}

/// Who called the statics, as `module+offset`, so the caller can be read.
///
/// The engine does not ask the returned value for an interface; it reads it as a layout.
/// That layout is only visible at the call site, and the call site is only visible from
/// here.
fn trace_stack() {
    #[link(name = "ntdll.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn RtlCaptureStackBackTrace(
            skip: u32,
            count: u32,
            frames: *mut *mut c_void,
            hash: *mut u32,
        ) -> u16;
        fn RtlPcToFileHeader(pc: *mut c_void, base: *mut *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
    }

    let mut frames = [std::ptr::null_mut::<c_void>(); 8];
    // 0 = this function, 1 = `method`; the caller is the third frame.
    let count = unsafe { RtlCaptureStackBackTrace(2, 8, frames.as_mut_ptr(), std::ptr::null_mut()) };
    for (index, frame) in frames[..count as usize].iter().enumerate() {
        let mut base = std::ptr::null_mut();
        unsafe { RtlPcToFileHeader(*frame, &mut base) };
        let mut name = [0u16; 260];
        let written = unsafe { GetModuleFileNameW(base, name.as_mut_ptr(), name.len() as u32) };
        let name = String::from_utf16_lossy(&name[..written as usize]);
        let short = name.rsplit(['\\', '/']).next().unwrap_or("?");
        eprintln!(
            "opencomp:   frame[{index}] {short}+{:#x}",
            *frame as usize - base as usize
        );
    }
}

unsafe extern "system" fn method<const SLOT: usize>(
    _this: *mut c_void,
    first: *mut c_void,
    second: *mut c_void,
) -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!(
            "opencomp: private Compositor statics slot {SLOT} with {first:?} {second:?}"
        );
        trace_stack();
    }
    if SLOT != 0 {
        return E_NOTIMPL;
    }
    // `DCompTreeHost::EnsureDCompDevice` asks the helper for a main device and then asks
    // that device for `ICompositor`, `ICompositor2`, `ICompositor5`, `ICompositor6` and
    // `ICompositorInterop`, so what belongs here is a DComp device or a compositor.
    //
    // Which one was measured, and the answer was neither: handing back an
    // `IDCompositionDevice2`, an `IDCompositionDevice3` or our own compositor all crash in
    // the same place. The caller does not use this value on the path that reaches the fault
    // -- it faults in `CompositionHelper` because the lifted DWM core never initialised.
    // `OPENCOMP_STATICS` keeps the choice available for the next attempt.
    let want = std::env::var("OPENCOMP_STATICS").unwrap_or_else(|_| "device2".to_string());
    if want == "compositor" {
        return match crate::compositor::instance() {
            Ok(compositor) => {
                let raw = windows_core::Interface::into_raw(compositor);
                if !second.is_null() {
                    unsafe { *(second as *mut *mut c_void) = raw };
                }
                S_OK
            }
            Err(_) => E_FAIL,
        };
    }
    let iid = match want.as_str() {
        "device3" => GUID::from_u128(0x0987cb06_f916_48bf_8d35_ce7641781bd9),
        _ => GUID::from_u128(0x75f6_468d_1b8e_447c_9bc6_75fea80b5b25),
    };
    match dcomp_device(&iid) {
        Some(device) => {
            if !second.is_null() {
                unsafe { *(second as *mut *mut c_void) = device };
            }
            S_OK
        }
        None => E_FAIL,
    }
}

/// `DCompositionCreateDevice3(NULL, iid, &device)`, in the Wine that is running us.
fn dcomp_device(iid: &GUID) -> Option<*mut c_void> {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let module = unsafe { LoadLibraryA(c"dcomp.dll".as_ptr().cast()) };
    if module.is_null() {
        return None;
    }
    let entry = unsafe { GetProcAddress(module, c"DCompositionCreateDevice3".as_ptr().cast()) };
    if entry.is_null() {
        return None;
    }
    type Create = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let create: Create = unsafe { std::mem::transmute(entry) };
    let mut device: *mut c_void = std::ptr::null_mut();
    let hr = unsafe { create(std::ptr::null_mut(), iid, &mut device) };
    if hr >= 0 && !device.is_null() {
        if std::env::var_os("OPENCOMP_TRACE").is_some() {
            eprintln!("opencomp: private statics handed back a dcomp device {device:?} hr={hr:#x}");
        }
        Some(device)
    } else {
        if std::env::var_os("OPENCOMP_TRACE").is_some() {
            eprintln!("opencomp: DCompositionCreateDevice3 refused iid {iid:?} hr={hr:#x}");
        }
        None
    }
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
