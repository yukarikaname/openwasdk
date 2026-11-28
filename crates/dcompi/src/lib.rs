// These exports exist to be loaded by a Windows process; on any other host the crate is
// empty, so `cargo check --workspace` still type-checks everything around it.
#![cfg(windows)]

//! `dcompi.dll`: the lifted compositor's activation shim.
//!
//! The engine loads this DLL by name and asks it for the composition classes and for the
//! device behind `Microsoft.UI.Composition`. The measured baseline is that the untouched
//! engine binary -- shipped as `dcompi-ms.dll` beside us -- drives the real lifted
//! compositor (`dwmcorei.dll`) and renders the Gallery window. So this shim asks the
//! engine first and takes a class over only when `OPENCOMP_MINE` names it: every
//! replacement is then an A/B against a window that is known to appear, instead of a
//! guess about a private vtable.
//!
//! `WinUIDCompositionCreateDevice3` is the device factory `Microsoft.UI.Input` reaches
//! for; our recording device answers it only under `OPENCOMP_RECORDING`, because the real
//! one is what makes the window draw.

use std::ffi::c_void;

const S_FALSE: u32 = 1;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// The untouched engine shim, shipped beside this DLL.
const ENGINE: &std::ffi::CStr = c"dcompi-ms.dll";
/// Our own composition objects, shipped beside this DLL.
const OURS: &std::ffi::CStr = c"opencomp.dll";

type GetFactory = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
type GetClass = unsafe extern "system" fn(*const c_void, *const c_void, *mut *mut c_void) -> i32;
type CreateDevice = unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> i32;

fn entry(dll: &std::ffi::CStr, name: &std::ffi::CStr) -> Option<*mut c_void> {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let module = unsafe { LoadLibraryA(dll.as_ptr().cast()) };
    if module.is_null() {
        return None;
    }
    let entry = unsafe { GetProcAddress(module, name.as_ptr().cast()) };
    if entry.is_null() { None } else { Some(entry) }
}

fn trace(args: std::fmt::Arguments) {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("dcompi: {args}");
    }
}

/// Whether `OPENCOMP_MINE` asks this shim to answer `class` itself.
fn ours_first(class: *mut c_void) -> bool {
    let Some(list) = std::env::var_os("OPENCOMP_MINE") else {
        return false;
    };
    let list = list.to_string_lossy().to_ascii_lowercase();
    let name = class_name(class).to_ascii_lowercase();
    list.split(',')
        .map(str::trim)
        .any(|item| !item.is_empty() && name.contains(item))
}

/// The engine's own activation entry point, or nothing when it is not beside us.
fn engine_factory() -> Option<GetFactory> {
    let entry = entry(ENGINE, c"DllGetActivationFactory")?;
    Some(unsafe { std::mem::transmute::<*mut c_void, GetFactory>(entry) })
}

fn our_factory() -> Option<GetFactory> {
    let entry = entry(OURS, c"DllGetActivationFactory")?;
    Some(unsafe { std::mem::transmute::<*mut c_void, GetFactory>(entry) })
}

/// The activation entry point combase calls for the classes this DLL is named for.
///
/// # Safety
/// `class` is the caller's HSTRING and `factory` is writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetActivationFactory(
    class: *mut c_void,
    factory: *mut *mut c_void,
) -> i32 {
    if class.is_null() || factory.is_null() {
        return E_POINTER;
    }
    unsafe { *factory = std::ptr::null_mut() };
    if ours_first(class) {
        if let Some(get) = our_factory() {
            let hr = unsafe { get(class, factory) };
            if hr >= 0 {
                trace(format_args!("ours: {:?}", class_name(class)));
                return hr;
            }
            trace(format_args!(
                "ours refused {:?}: {hr:#x}",
                class_name(class)
            ));
        }
    }
    if let Some(get) = engine_factory() {
        let hr = unsafe { get(class, factory) };
        if hr >= 0 {
            return hr;
        }
        trace(format_args!(
            "engine refused {:?}: {hr:#x}",
            class_name(class)
        ));
        return hr;
    }
    trace(format_args!(
        "no engine beside us for {:?}",
        class_name(class)
    ));
    CLASS_E_CLASSNOTAVAILABLE
}

/// `WinUIDCompositionCreateDevice3`: the device the input host asks the compositor for.
/// The engine's device is the one that draws; ours records instead.
///
/// # Safety
/// `out` is writable for one interface pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn WinUIDCompositionCreateDevice3(
    device: *mut c_void,
    iid: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe { *out = std::ptr::null_mut() };
    if std::env::var_os("OPENCOMP_RECORDING").is_some() {
        let Some(create) = entry(OURS, c"OpenCompRecordingDevice") else {
            return E_NOTIMPL;
        };
        type Get = unsafe extern "system" fn() -> *mut c_void;
        let get: Get = unsafe { std::mem::transmute::<*mut c_void, Get>(create) };
        let recording = unsafe { get() };
        if recording.is_null() {
            return E_NOTIMPL;
        }
        trace(format_args!("recording device for {iid:?}"));
        unsafe { *out = recording };
        return 0;
    }
    let Some(entry) = entry(ENGINE, c"WinUIDCompositionCreateDevice3") else {
        return E_NOTIMPL;
    };
    let create: CreateDevice = unsafe { std::mem::transmute::<*mut c_void, CreateDevice>(entry) };
    let hr = unsafe { create(device, iid, out) };
    trace(format_args!("engine device for {iid:?} -> {hr:#x}"));
    hr
}

/// # Safety
/// `out` is writable for one interface pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    class: *const c_void,
    iid: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe { *out = std::ptr::null_mut() };
    let Some(entry) = entry(ENGINE, c"DllGetClassObject") else {
        return CLASS_E_CLASSNOTAVAILABLE;
    };
    let get: GetClass = unsafe { std::mem::transmute::<*mut c_void, GetClass>(entry) };
    unsafe { get(class, iid, out) }
}

/// Nothing here holds a global object, so the module can always leave.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> u32 {
    S_FALSE
}

/// The class name behind the caller's HSTRING, for the trace only.
fn class_name(class: *mut c_void) -> String {
    #[link(name = "combase.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn WindowsGetStringRawBuffer(s: *mut c_void, len: *mut u32) -> *const u16;
    }
    unsafe {
        let p = WindowsGetStringRawBuffer(class, std::ptr::null_mut());
        if p.is_null() {
            return String::new();
        }
        let mut n = 0usize;
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }
}
