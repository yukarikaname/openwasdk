//! The private interface XAML asks the compositor for before it loads markup.
//!
//! The GUID appears nowhere in metadata. The caller queries it, calls slot 0x20 with an
//! out pointer, and passes the answer to a thread-affinity check that reads a dword at
//! +0x50; a slot that returns without writing leaves the caller with a null and it faults
//! on the next line. Every slot therefore answers with the padded stub, which carries the
//! creating thread id at that offset and records the slot it was called at.

use std::ffi::c_void;

use windows_core::{GUID, HRESULT, IUnknown, interface};

const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);
const E_POINTER: HRESULT = HRESULT(0x8000_4003u32 as i32);

pub(crate) const IID: GUID = GUID::from_u128(0xd361_6837_3ed3_484e_ac26_cf5b_e382_541e);

macro_rules! slot {
    ($name:ident, $index:expr) => {
        unsafe fn $name(&self, out: *mut *mut c_void) -> HRESULT {
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: compositor private slot {} called", $index);
            }
            if out.is_null() {
                return E_POINTER;
            }
            unsafe { *out = crate::composite::private_stub(IID) };
            E_NOTIMPL
        }
    };
}

#[interface("d3616837-3ed3-484e-ac26-cf5be382541e")]
pub unsafe trait ICompositorPrivate: IUnknown {
    unsafe fn Slot6(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot7(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot8(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot9(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot10(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot11(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot12(&self, out: *mut *mut c_void) -> HRESULT;
    unsafe fn Slot13(&self, out: *mut *mut c_void) -> HRESULT;
}

impl ICompositorPrivate_Impl for crate::compositor::Compositor_Impl {
    slot!(Slot6, 6);
    slot!(Slot7, 7);
    slot!(Slot8, 8);
    slot!(Slot9, 9);
    slot!(Slot10, 10);
    slot!(Slot11, 11);
    slot!(Slot12, 12);
    slot!(Slot13, 13);
}

/// `{fc084699-67d8-40e1-ade7-08901d84ffda}`: asked on the composition target while XAML
/// loads a window's markup. The identifier appears only in `Microsoft.ui.xaml.dll` and in
/// the SDK's `dcompi.dll`, so it is the composition bridge's private interface; our
/// replacement bridge answers the query and refuses the slots by name.
#[interface("fc084699-67d8-40e1-ade7-08901d84ffda")]
pub unsafe trait ICompositionBridgePrivate: IUnknown {
    unsafe fn Slot6(&self) -> HRESULT;
    unsafe fn Slot7(&self) -> HRESULT;
    unsafe fn Slot8(&self) -> HRESULT;
    unsafe fn Slot9(&self) -> HRESULT;
    unsafe fn Slot10(&self) -> HRESULT;
    unsafe fn Slot11(&self) -> HRESULT;
    unsafe fn Slot12(&self) -> HRESULT;
    unsafe fn Slot13(&self) -> HRESULT;
}

macro_rules! bridge_slot {
    ($name:ident, $index:expr) => {
        unsafe fn $name(&self) -> HRESULT {
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: composition bridge slot {} called", $index);
            }
            E_NOTIMPL
        }
    };
}

impl ICompositionBridgePrivate_Impl for crate::compositor::Compositor_Impl {
    bridge_slot!(Slot6, 6);
    bridge_slot!(Slot7, 7);
    bridge_slot!(Slot8, 8);
    bridge_slot!(Slot9, 9);
    bridge_slot!(Slot10, 10);
    bridge_slot!(Slot11, 11);
    bridge_slot!(Slot12, 12);
    bridge_slot!(Slot13, 13);
}
