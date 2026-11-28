//! The private interface XAML asks the compositor for before it loads markup.
//!
//! The GUID appears nowhere in metadata: it is queried on the object the private statics
//! slot handed out, and the caller checks the answer's thread affinity by reading a dword
//! at +0x50. A refusal leaves the caller with a null and it faults on the next line, so
//! the interface has to exist even before its methods are known. Each slot reports
//! E_NOTIMPL and says which one was called.

use windows_core::{HRESULT, IUnknown, interface};

const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);

macro_rules! slot {
    ($name:ident, $index:expr) => {
        unsafe fn $name(&self) -> HRESULT {
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!("opencomp: compositor private slot {} called", $index);
            }
            E_NOTIMPL
        }
    };
}

#[interface("d3616837-3ed3-484e-ac26-cf5be382541e")]
pub unsafe trait ICompositorPrivate: IUnknown {
    unsafe fn Slot6(&self) -> HRESULT;
    unsafe fn Slot7(&self) -> HRESULT;
    unsafe fn Slot8(&self) -> HRESULT;
    unsafe fn Slot9(&self) -> HRESULT;
    unsafe fn Slot10(&self) -> HRESULT;
    unsafe fn Slot11(&self) -> HRESULT;
    unsafe fn Slot12(&self) -> HRESULT;
    unsafe fn Slot13(&self) -> HRESULT;
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
