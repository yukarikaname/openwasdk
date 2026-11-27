//! `DispatcherQueueShutdownStartingEventArgs`: the object the shutdown event
//! carries. Its one method asks for a deferral, which this DLL does not provide,
//! so it answers `E_NOTIMPL` rather than handing back an object that cannot
//! defer anything.

use windows_core::{Interface, Result};

use crate::bindings::Microsoft::UI::Dispatching::{
    DispatcherQueueShutdownStartingEventArgs, IDispatcherQueueShutdownStartingEventArgs,
    IDispatcherQueueShutdownStartingEventArgs_Impl,
};
use crate::not_implemented;

#[windows_core::implement(IDispatcherQueueShutdownStartingEventArgs)]
struct Args;

impl IDispatcherQueueShutdownStartingEventArgs_Impl for Args_Impl {
    fn GetDeferral(&self) -> Result<crate::bindings::Windows::Foundation::Deferral> {
        not_implemented()
    }
}

pub(crate) fn create() -> Result<DispatcherQueueShutdownStartingEventArgs> {
    let args: IDispatcherQueueShutdownStartingEventArgs = Args.into();
    args.cast()
}
