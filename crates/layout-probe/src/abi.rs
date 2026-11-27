//! The small ABI surface this probe needs: GUIDs, slot order, and the handful of
//! WinRT function shapes. Every slot number used by `windows.rs` comes from the
//! metadata; the three `IInspectable` slots are always counted on top.

use std::ffi::c_void;
use std::ptr;

use windows_core::GUID;

pub type Raw = *mut c_void;

pub const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
pub const E_POINTER: i32 = 0x8000_4003u32 as i32;
pub const E_FAIL: i32 = 0x8000_4005u32 as i32;

// Stable WinRT type identifiers. The generic `IVector` has one identifier for every
// element type; the element is an `IInspectable` at the ABI.
pub const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
pub const IID_AGILE_OBJECT: GUID = GUID::from_u128(0x94ea2b94_e9cc_49e0_c0ff_ee64ca8f5b90);
pub const IID_ACTIVATION_FACTORY: GUID = GUID::from_u128(0x00000035_0000_0000_c000_000000000046);
pub const IID_APPLICATION_INITIALIZATION_CALLBACK: GUID =
    GUID::from_u128(0xd8eef1c9_1234_56f1_9963_45dd9c80a661);
pub const IID_APPLICATION_FACTORY: GUID = GUID::from_u128(0x9fd96657_5294_5a65_a1db_4fea143597da);
pub const IID_APPLICATION_STATICS: GUID = GUID::from_u128(0x4e0d09f5_4358_512c_a987_503b52848e95);
pub const IID_UIELEMENT: GUID = GUID::from_u128(0xc3c01020_320c_5cf6_9d24_d396bbfa4d8b);
pub const IID_FRAMEWORK_ELEMENT: GUID = GUID::from_u128(0xfe08f13d_dc6a_5495_ad44_c2d8d21863b0);
pub const IID_STACK_PANEL: GUID = GUID::from_u128(0x493ab00b_3a6a_5e4a_9452_407cd5197406);
pub const IID_STACK_PANEL_FACTORY: GUID = GUID::from_u128(0x64c1d388_47a2_5a74_a75b_559d151ee5ac);
pub const IID_PANEL: GUID = GUID::from_u128(0x27a1b418_56f3_525e_b883_cefed905eed3);
pub const IID_VECTOR_UIELEMENT: GUID = GUID::from_u128(0x913337e9_11a1_4345_a3a2_4e7f956e222d);
pub const IID_DISPATCHER_QUEUE_STATICS: GUID =
    GUID::from_u128(0xcd3382ea_a455_5124_b63a_ca40d34ca23c);

pub type QueryInterface = unsafe extern "system" fn(Raw, *const GUID, *mut Raw) -> i32;
pub type ReleaseFn = unsafe extern "system" fn(Raw) -> u32;
pub type ActivationFactory = unsafe extern "system" fn(*mut c_void, *mut Raw) -> i32;
pub type ActivateInstance = unsafe extern "system" fn(Raw, *mut Raw) -> i32;
pub type CreateInstance = unsafe extern "system" fn(Raw, Raw, *mut Raw, *mut Raw) -> i32;
pub type GetObject = unsafe extern "system" fn(Raw, *mut Raw) -> i32;
pub type Start = unsafe extern "system" fn(Raw, Raw) -> i32;
pub type PutF64 = unsafe extern "system" fn(Raw, f64) -> i32;
pub type Append = unsafe extern "system" fn(Raw, Raw) -> i32;
pub type Measure = unsafe extern "system" fn(Raw, Size) -> i32;
pub type Arrange = unsafe extern "system" fn(Raw, Rect) -> i32;
pub type GetSize = unsafe extern "system" fn(Raw, *mut Size) -> i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct Reading {
    pub width: f32,
    pub height: f32,
    pub child0_height: f32,
}

#[derive(Clone, Copy)]
pub struct Failure {
    pub step: &'static str,
    pub hr: i32,
}

impl Failure {
    pub const fn new(step: &'static str, hr: i32) -> Self {
        Self { step, hr }
    }

    pub fn report(self) {
        eprintln!(
            "layout-probe: {} failed: hr=0x{:08x}",
            self.step, self.hr as u32
        );
    }
}

// WinRT needs an initialized apartment before it hands out a XAML object. C++/WinRT's
// `init_apartment` ends up here for the single-threaded case.
#[link(name = "combase")]
unsafe extern "system" {
    fn RoInitialize(init_type: i32) -> i32;
}

pub fn init_apartment() -> i32 {
    const RO_INIT_SINGLETHREADED: i32 = 0;
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }
}

/// Read slot `index` from the object's vtable. The caller supplies the function type,
/// so the ABI is checked at the call site even though the index is a metadata number.
pub unsafe fn slot<T: Copy>(object: Raw, index: usize) -> T {
    let vtable = unsafe { *(object as *mut *const usize) };
    unsafe { std::mem::transmute_copy(&*vtable.add(index)) }
}

pub unsafe fn query(object: Raw, iid: GUID) -> Result<Raw, i32> {
    let mut out: Raw = ptr::null_mut();
    let function: QueryInterface = unsafe { slot(object, 0) };
    let hr = unsafe { function(object, &iid, &mut out) };
    if hr < 0 {
        Err(hr)
    } else if out.is_null() {
        Err(E_POINTER)
    } else {
        Ok(out)
    }
}

pub unsafe fn release(object: Raw) {
    if !object.is_null() {
        let function: ReleaseFn = unsafe { slot(object, 2) };
        unsafe { function(object) };
    }
}
