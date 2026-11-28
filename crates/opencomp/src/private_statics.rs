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

use windows_core::{GUID, Interface as _};

/// The one interface here: a statics entry point on `Compositor`.
pub(crate) const IID: GUID = GUID::from_u128(0xffcd_c0ea_b528_4bfa_a315_2f11_f1c5_e8f9);

const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);
const IID_IINSPECTABLE: GUID = GUID::from_u128(0xaf86_e2e0_b12d_4c6a_9c5a_d7aa_6510_1e90);

const S_OK: i32 = 0;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const E_FAIL: i32 = 0x8000_4005u32 as i32;

type Member = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *const GUID,
    *mut *mut c_void,
) -> i32;

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
pub(crate) fn trace_stack() {
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
    let count =
        unsafe { RtlCaptureStackBackTrace(2, 8, frames.as_mut_ptr(), std::ptr::null_mut()) };
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
    flags: *mut c_void,
    context: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!(
            "opencomp: private Compositor statics slot {SLOT} flags {flags:?} context {context:?} iid {iid:?} out {out:?}"
        );
        trace_stack();
    }
    if SLOT != 0 {
        return E_NOTIMPL;
    }
    if out.is_null() {
        return E_POINTER;
    }
    // The caller releases whatever the slot held, asks for an object with the
    // `IDCompositionDevice2` IID and then uses the result as a WinRT compositor, so the
    // answer has to be one object carrying both sets of interfaces. See crate::composite.
    let want = std::env::var("OPENCOMP_STATICS").unwrap_or_else(|_| "composite".to_string());
    if want == "compositor" {
        return match crate::compositor::instance() {
            Ok(compositor) => {
                let raw = windows_core::Interface::into_raw(compositor);
                unsafe { *out = raw };
                S_OK
            }
            Err(_) => E_FAIL,
        };
    }
    let device_iid = if iid.is_null() {
        GUID::from_u128(0x75f6_468d_1b8e_447c_9bc6_75fe_a80b_5b25)
    } else {
        unsafe { *iid }
    };
    let Some(device) = dcomp_device(&device_iid) else {
        return E_FAIL;
    };
    if want != "composite" {
        unsafe { *out = device };
        return S_OK;
    }
    let compositor = match crate::compositor::instance() {
        Ok(compositor) => compositor,
        Err(_) => return E_FAIL,
    };
    let compositor: windows_core::IUnknown = match compositor.cast() {
        Ok(compositor) => compositor,
        Err(_) => return E_FAIL,
    };
    let composite = crate::composite::create(compositor, device);
    unsafe { *out = composite };
    S_OK
}

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

/// `DCompositionCreateDevice3` on the host. Windows refuses the NULL rendering device for
/// the device2 iid, and the v1 device does not answer that iid either, so a real D3D11
/// device is created and handed over as an IDXGIDevice, which is what the entry point wants.
fn dcomp_device(iid: &GUID) -> Option<*mut c_void> {
    const IID_IDCOMPOSITIONDEVICE: GUID =
        GUID::from_u128(0xc37e_a93a_e7aa_450d_b16f_9746_cb04_07f3);
    const IID_IDCOMPOSITIONDEVICE3: GUID =
        GUID::from_u128(0x0987_cb06_f916_48bf_8d35_ce76_4178_1bd9);
    const IID_IDXGIDEVICE: GUID = GUID::from_u128(0x54ec_77fa_1377_44e6_8c32_88fd_5f44_c84c);

    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    #[link(name = "d3d11.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn D3D11CreateDevice(
            adapter: *mut c_void,
            driver_type: u32,
            software: *mut c_void,
            flags: u32,
            levels: *const u32,
            level_count: u32,
            sdk: u32,
            device: *mut *mut c_void,
            feature_level: *mut u32,
            context: *mut *mut c_void,
        ) -> i32;
    }
    type Create = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;

    fn entry(module: *mut c_void, name: &std::ffi::CStr) -> Option<Create> {
        #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
        unsafe extern "system" {
            fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        }
        let p = unsafe { GetProcAddress(module, name.as_ptr().cast()) };
        if p.is_null() {
            None
        } else {
            type GetFactory =
                unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
            Some(unsafe { std::mem::transmute::<*mut c_void, GetFactory>(p) })
        }
    }

    let module = unsafe { LoadLibraryA(c"dcomp.dll".as_ptr().cast()) };
    if module.is_null() {
        return None;
    }
    let create3 = entry(module, c"DCompositionCreateDevice3");

    if let Some(create) = create3 {
        let mut device: *mut c_void = std::ptr::null_mut();
        let hr = unsafe { create(std::ptr::null_mut(), iid, &mut device) };
        if hr >= 0 && !device.is_null() {
            return Some(device);
        }
    }

    // The device2 entry point is the one that answers the device2 iid without a rendering
    // device; the v3 one only accepts its own generation.
    if let Some(create) = entry(module, c"DCompositionCreateDevice2") {
        let mut dev: *mut c_void = std::ptr::null_mut();
        let hr = unsafe { create(std::ptr::null_mut(), iid, &mut dev) };
        if hr >= 0 && !dev.is_null() {
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: DCompositionCreateDevice2 {iid:?} -> {dev:?}");
            }
            return Some(dev);
        }
    }

    for driver in [1u32, 5u32] {
        let mut d3d: *mut c_void = std::ptr::null_mut();
        let hr = unsafe {
            D3D11CreateDevice(
                std::ptr::null_mut(),
                driver,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                0,
                7,
                &mut d3d,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if hr < 0 || d3d.is_null() {
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: D3D11CreateDevice driver {driver} hr={hr:#x}");
            }
            continue;
        }
        let vt = unsafe { *(d3d as *mut *mut *mut c_void) };
        if vt.is_null() {
            continue;
        }
        let query: Query = unsafe { std::mem::transmute(*vt) };
        let mut dxgi: *mut c_void = std::ptr::null_mut();
        let hr = unsafe { query(d3d, &IID_IDXGIDEVICE, &mut dxgi) };
        if hr < 0 || dxgi.is_null() {
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: IDXGIDevice driver {driver} hr={hr:#x}");
            }
            continue;
        }
        if let Some(create) = create3 {
            let mut dev: *mut c_void = std::ptr::null_mut();
            let hr = unsafe { create(dxgi, iid, &mut dev) };
            if hr < 0 && std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: CreateDevice3(dxgi) driver {driver} hr={hr:#x}");
            }
            if hr >= 0 && !dev.is_null() {
                if std::env::var_os("OPENCOMP_TRACE").is_some() {
                    eprintln!(
                        "opencomp: dcomp device from D3D11 driver {driver} {iid:?} -> {dev:?}"
                    );
                }
                return Some(dev);
            }
            let mut gen3: *mut c_void = std::ptr::null_mut();
            let hr = unsafe { create(dxgi, &IID_IDCOMPOSITIONDEVICE3, &mut gen3) };
            if hr >= 0 && !gen3.is_null() {
                let vt = unsafe { *(gen3 as *mut *mut *mut c_void) };
                if !vt.is_null() {
                    let query: Query = unsafe { std::mem::transmute(*vt) };
                    let mut out: *mut c_void = std::ptr::null_mut();
                    let hr = unsafe { query(gen3, iid, &mut out) };
                    if hr >= 0 && !out.is_null() {
                        if std::env::var_os("OPENCOMP_TRACE").is_some() {
                            eprintln!("opencomp: device3 queried for {iid:?} -> {out:?}");
                        }
                        return Some(out);
                    }
                }
            }
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: device3 path refused {iid:?} hr={hr:#x}");
            }
        }
    }

    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp: recording device for {iid:?}");
    }
    Some(recording_device())
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
