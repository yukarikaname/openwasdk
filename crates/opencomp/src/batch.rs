//! Commit and scoped batches.
//!
//! A batch is a completion signal, not a renderer transaction. The commit boundary
//! here is the frame loop: a commit batch closes on the next tick of the compositor's
//! clock, and a scoped batch closes when its `End`/`Close` is called. Either way the
//! `Completed` handlers are posted to the same clock and run in the frame loop, once.
//!
//! The metadata has no `Close` on `ICompositionCommitBatch`; the close a client can
//! ask for is `ICompositionScopedBatch.End`, and `Windows.Foundation.IClosable.Close`
//! on either batch object maps to that same close.

use std::sync::{Arc, Mutex, Weak};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionBatchCompletedEventArgs, CompositionBatchTypes, CompositionCommitBatch,
    CompositionScopedBatch, ICompositionBatchCompletedEventArgs,
    ICompositionBatchCompletedEventArgs_Impl, ICompositionCommitBatch,
    ICompositionCommitBatch_Impl, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5, ICompositionScopedBatch, ICompositionScopedBatch_Impl,
};
use crate::bindings::Windows::Foundation::TypedEventHandler;
use crate::bindings::Windows::Foundation::{IClosable, IClosable_Impl};
use crate::clock::{Clock, FrameListener};
use crate::object::composition_object;
use windows_core::{AgileReference, Error, HRESULT, IInspectable, Interface, Ref, Result};

const E_POINTER: HRESULT = HRESULT(0x8000_4003u32 as i32);

type Handler = TypedEventHandler<IInspectable, CompositionBatchCompletedEventArgs>;

#[windows_core::implement(ICompositionBatchCompletedEventArgs)]
struct EventArgs;

impl ICompositionBatchCompletedEventArgs_Impl for EventArgs_Impl {}

/// The two batch classes are generated `Send`/`Sync`, unlike a bare `IInspectable`,
/// which is what lets the completion run on the frame loop's thread.
#[derive(Clone)]
enum Sender {
    Commit(windows_core::Weak<CompositionCommitBatch>),
    Scoped(windows_core::Weak<CompositionScopedBatch>),
}

impl Sender {
    fn inspectable(&self) -> Result<IInspectable> {
        match self {
            Self::Commit(batch) => batch.upgrade().ok_or_else(Error::empty)?.cast(),
            Self::Scoped(batch) => batch.upgrade().ok_or_else(Error::empty)?.cast(),
        }
    }
}

struct BatchState {
    clock: Arc<Clock>,
    inner: Mutex<BatchInner>,
    sender: Mutex<Option<Sender>>,
    args: CompositionBatchCompletedEventArgs,
}

struct BatchInner {
    open: bool,
    ended: bool,
    suspended: bool,
    next_token: i64,
    handlers: Vec<(i64, AgileReference<Handler>)>,
}

impl BatchState {
    fn new(clock: Arc<Clock>) -> Result<Arc<Self>> {
        let args: ICompositionBatchCompletedEventArgs = EventArgs.into();
        let args: CompositionBatchCompletedEventArgs = args.cast()?;
        Ok(Arc::new(Self {
            clock,
            inner: Mutex::new(BatchInner {
                open: true,
                ended: false,
                suspended: false,
                next_token: 1,
                handlers: Vec::new(),
            }),
            sender: Mutex::new(None),
            args,
        }))
    }

    fn is_active(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.open && !inner.suspended
    }

    fn ended(&self) -> bool {
        self.inner.lock().unwrap().ended
    }

    /// Close once. The handlers are queued on the compositor's clock; a batch that is
    /// already closed is not closed again and does not post a second completion.
    fn close(&self) {
        let handlers = {
            let mut inner = self.inner.lock().unwrap();
            if inner.ended {
                return;
            }
            inner.ended = true;
            inner.open = false;
            core::mem::take(&mut inner.handlers)
        };
        let sender = self.sender.lock().unwrap().clone();
        let Some(sender) = sender else {
            return;
        };
        let args = self.args.clone();
        let clock = self.clock.clone();
        clock.post(move || {
            let Ok(sender) = sender.inspectable() else {
                return;
            };
            for (_, handler) in handlers {
                if let Ok(handler) = handler.resolve() {
                    invoke(&handler, &sender, &args);
                }
            }
        });
    }

    fn add(&self, handler: Handler) -> Result<i64> {
        let handler = AgileReference::new(&handler)?;
        let (token, ended) = {
            let mut inner = self.inner.lock().unwrap();
            let token = inner.next_token;
            inner.next_token += 1;
            if !inner.ended {
                inner.handlers.push((token, handler.clone()));
            }
            (token, inner.ended)
        };
        if ended {
            let sender = self.sender.lock().unwrap().clone();
            if let (Some(sender), Ok(handler)) = (sender, handler.resolve())
                && let Ok(sender) = sender.inspectable()
            {
                invoke(&handler, &sender, &self.args);
            }
        }
        Ok(token)
    }

    fn remove(&self, token: i64) {
        self.inner
            .lock()
            .unwrap()
            .handlers
            .retain(|(held, _)| *held != token);
    }

    fn suspend(&self, suspended: bool) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.ended {
            inner.suspended = suspended;
        }
    }
}

/// `state` is an `Arc<BatchState>`; the two interfaces share this one body.
fn is_active(state: &Arc<BatchState>) -> Result<bool> {
    Ok(state.is_active())
}

fn is_ended(state: &Arc<BatchState>) -> Result<bool> {
    Ok(state.ended())
}

fn completed(state: &Arc<BatchState>, handler: Ref<Handler>) -> Result<i64> {
    let handler = handler
        .cloned()
        .ok_or_else(|| Error::from_hresult(E_POINTER))?;
    state.add(handler)
}

/// The delegate vtable starts with the three `IUnknown` slots and then `Invoke`.
/// The generated table keeps those fields private, so the prefix is written here;
/// this is the same shape the ABI specifies.
#[repr(C)]
struct DelegateVtbl {
    _query_interface: usize,
    _add_ref: usize,
    _release: usize,
    invoke: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> HRESULT,
}

fn invoke(handler: &Handler, sender: &IInspectable, args: &CompositionBatchCompletedEventArgs) {
    unsafe {
        let raw = Interface::as_raw(handler);
        let table = *(raw as *const *const DelegateVtbl);
        let _ = ((*table).invoke)(raw, Interface::as_raw(sender), Interface::as_raw(args));
    }
}

struct Listener {
    state: Arc<BatchState>,
}

impl FrameListener for Listener {
    fn frame(&self) {
        self.state.close();
    }
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    IClosable,
    ICompositionCommitBatch
)]
pub struct CommitBatchObject {
    state: Arc<BatchState>,
    /// Holds the listener the clock sees only weakly.
    _listener: Arc<Listener>,
}

composition_object!(CommitBatchObject_Impl);

impl IClosable_Impl for CommitBatchObject_Impl {
    fn Close(&self) -> Result<()> {
        self.state.close();
        Ok(())
    }
}

impl ICompositionCommitBatch_Impl for CommitBatchObject_Impl {
    fn IsActive(&self) -> Result<bool> {
        is_active(&self.state)
    }

    fn IsEnded(&self) -> Result<bool> {
        is_ended(&self.state)
    }

    fn Completed(&self, handler: Ref<Handler>) -> Result<i64> {
        completed(&self.state, handler)
    }

    fn RemoveCompleted(&self, token: i64) -> Result<()> {
        self.state.remove(token);
        Ok(())
    }
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    IClosable,
    ICompositionScopedBatch
)]
pub struct ScopedBatchObject {
    state: Arc<BatchState>,
}

composition_object!(ScopedBatchObject_Impl);

impl ICompositionScopedBatch_Impl for ScopedBatchObject_Impl {
    fn IsActive(&self) -> Result<bool> {
        is_active(&self.state)
    }

    fn IsEnded(&self) -> Result<bool> {
        is_ended(&self.state)
    }

    fn End(&self) -> Result<()> {
        self.state.close();
        Ok(())
    }

    fn Resume(&self) -> Result<()> {
        self.state.suspend(false);
        Ok(())
    }

    fn Suspend(&self) -> Result<()> {
        self.state.suspend(true);
        Ok(())
    }

    fn Completed(&self, handler: Ref<Handler>) -> Result<i64> {
        completed(&self.state, handler)
    }

    fn RemoveCompleted(&self, token: i64) -> Result<()> {
        self.state.remove(token);
        Ok(())
    }
}

impl IClosable_Impl for ScopedBatchObject_Impl {
    fn Close(&self) -> Result<()> {
        self.state.close();
        Ok(())
    }
}

/// A batch that closes at the next frame.
pub fn commit(
    clock: Arc<Clock>,
    _batch_type: CompositionBatchTypes,
) -> Result<CompositionCommitBatch> {
    let state = BatchState::new(clock)?;
    let listener = Arc::new(Listener {
        state: state.clone(),
    });
    state
        .clock
        .listen(Arc::downgrade(&listener) as Weak<dyn FrameListener>);
    let object: ICompositionCommitBatch = CommitBatchObject {
        state: state.clone(),
        _listener: listener,
    }
    .into();
    let batch: CompositionCommitBatch = object.cast()?;
    *state.sender.lock().unwrap() = Some(Sender::Commit(batch.downgrade()?));
    Ok(batch)
}

/// A batch that stays open until the client ends it.
pub fn scoped(
    clock: Arc<Clock>,
    _batch_type: CompositionBatchTypes,
) -> Result<CompositionScopedBatch> {
    let state = BatchState::new(clock)?;
    let object: ICompositionScopedBatch = ScopedBatchObject {
        state: state.clone(),
    }
    .into();
    let batch: CompositionScopedBatch = object.cast()?;
    *state.sender.lock().unwrap() = Some(Sender::Scoped(batch.downgrade()?));
    Ok(batch)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::ICompositor;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    fn clock(compositor: &ICompositor) -> Arc<Clock> {
        crate::compositor::clock_of(compositor).unwrap()
    }

    #[test]
    fn a_commit_batch_closes_once_at_the_next_frame() {
        let compositor = compositor();
        let batch = compositor
            .GetCommitBatch(CompositionBatchTypes::Animation)
            .unwrap();
        assert!(batch.IsActive().unwrap());
        assert!(!batch.IsEnded().unwrap());

        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let _subscription = batch
            .Completed(move |_, _| {
                counter.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        clock(&compositor).tick(0.016);
        assert!(!batch.IsActive().unwrap());
        assert!(batch.IsEnded().unwrap());
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        // A second frame does not complete the same batch again, and a second close
        // request is not an error.
        clock(&compositor).tick(0.016);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_scoped_batch_ends_when_the_client_ends_it() {
        let compositor = compositor();
        let batch = compositor
            .CreateScopedBatch(CompositionBatchTypes::Animation)
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let _subscription = batch
            .Completed(move |_, _| {
                counter.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        batch.End().unwrap();
        batch.End().unwrap();
        assert!(batch.IsEnded().unwrap());
        assert!(!batch.IsActive().unwrap());
        assert_eq!(calls.load(Ordering::Relaxed), 0, "completion is queued");

        clock(&compositor).tick(0.016);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn suspend_makes_a_scoped_batch_inactive_without_ending_it() {
        let compositor = compositor();
        let batch = compositor
            .CreateScopedBatch(CompositionBatchTypes::Animation)
            .unwrap();
        batch.Suspend().unwrap();
        assert!(!batch.IsActive().unwrap());
        assert!(!batch.IsEnded().unwrap());
        batch.Resume().unwrap();
        assert!(batch.IsActive().unwrap());
    }
}
