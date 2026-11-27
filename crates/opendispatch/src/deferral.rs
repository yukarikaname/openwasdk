//! `DispatcherExitDeferral`: created when a caller asks `RunEventLoop` to stay
//! alive, and completed when it is done. Our loop exits on `WM_QUIT` and has no
//! exit to hold open, so `Complete` answers `E_NOTIMPL` rather than pretending
//! the deferral did something.

use windows_core::{Interface, Result};

use crate::bindings::Microsoft::UI::Dispatching::{
    DispatcherExitDeferral, IDispatcherExitDeferral, IDispatcherExitDeferral_Impl,
};
use crate::not_implemented;

#[windows_core::implement(IDispatcherExitDeferral)]
struct ExitDeferral;

impl IDispatcherExitDeferral_Impl for ExitDeferral_Impl {
    fn Complete(&self) -> Result<()> {
        not_implemented()
    }
}

pub(crate) fn create() -> Result<DispatcherExitDeferral> {
    let deferral: IDispatcherExitDeferral = ExitDeferral.into();
    deferral.cast()
}
