//! `DispatcherQueue` itself: enqueue, the shutdown events, thread access and the
//! three `IDispatcherQueue` members that run our own loop.
//!
//! The object is a thin face over [`crate::queue::Queue`]. A task is the ABI's
//! `DispatcherQueueHandler` turned into an owned [`Handler`]; the closure the
//! queue stores captures that reference and calls the delegate when the window
//! procedure drains the queue.

use std::sync::{Arc, Mutex};

use windows_core::{Error, HRESULT, IInspectable, Interface, Ref, Result};
use windows_future::IAsyncAction;

use crate::bindings::Microsoft::UI::Dispatching::{
    DispatcherExitDeferral, DispatcherQueue, DispatcherQueueHandler, DispatcherQueuePriority,
    DispatcherQueueShutdownStartingEventArgs, DispatcherQueueTimer, DispatcherRunOptions,
    IDispatcherQueue, IDispatcherQueue_Impl, IDispatcherQueue2, IDispatcherQueue2_Impl,
    IDispatcherQueue3, IDispatcherQueue3_Impl, IDispatcherQueueStatics,
    IDispatcherQueueStatics_Impl,
};
use crate::bindings::Windows::Foundation::TypedEventHandler;
use crate::event::Handler;
use crate::platform::Platform;
use crate::queue::{Poster, Priority, Queue};
use crate::timer;

const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);
const E_FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);
const E_NOTIMPL_QUEUE: HRESULT = HRESULT(0x8000_4001u32 as i32);

/// A handler list and its token counter. Tokens are unique across every event on
/// the object, so removing the wrong kind cannot succeed by accident.
#[derive(Default)]
pub(crate) struct Events {
    next_token: i64,
    sender: Option<DispatcherQueue>,
    shutdown_starting: Vec<(i64, Handler)>,
    shutdown_completed: Vec<(i64, Handler)>,
    framework_starting: Vec<(i64, Handler)>,
    framework_completed: Vec<(i64, Handler)>,
}

impl Events {
    fn list(&mut self, kind: Kind) -> &mut Vec<(i64, Handler)> {
        match kind {
            Kind::ShutdownStarting => &mut self.shutdown_starting,
            Kind::ShutdownCompleted => &mut self.shutdown_completed,
            Kind::FrameworkStarting => &mut self.framework_starting,
            Kind::FrameworkCompleted => &mut self.framework_completed,
        }
    }

    fn add(&mut self, kind: Kind, handler: Handler) -> i64 {
        self.next_token += 1;
        let token = self.next_token;
        self.list(kind).push((token, handler));
        token
    }

    fn remove(&mut self, kind: Kind, token: i64) {
        self.list(kind).retain(|(candidate, _)| *candidate != token);
    }
}

#[derive(Clone, Copy)]
enum Kind {
    ShutdownStarting,
    ShutdownCompleted,
    FrameworkStarting,
    FrameworkCompleted,
}

#[windows_core::implement(IDispatcherQueue, IDispatcherQueue2, IDispatcherQueue3)]
struct QueueObject {
    core: Arc<Queue>,
    platform: Arc<Platform>,
    events: Arc<Mutex<Events>>,
}

/// What the controller needs from the queue it owns.
pub(crate) struct Created {
    pub queue: DispatcherQueue,
    pub events: Arc<Mutex<Events>>,
}

impl IDispatcherQueue_Impl for QueueObject_Impl {
    fn CreateTimer(&self) -> Result<DispatcherQueueTimer> {
        timer::create(self.platform.clone())
    }

    fn TryEnqueue(&self, callback: Ref<DispatcherQueueHandler>) -> Result<bool> {
        enqueue(&self.core, Priority::Normal, callback)
    }

    fn TryEnqueueWithPriority(
        &self,
        priority: DispatcherQueuePriority,
        callback: Ref<DispatcherQueueHandler>,
    ) -> Result<bool> {
        enqueue(&self.core, Priority::from_i32(priority.0), callback)
    }

    fn ShutdownStarting(
        &self,
        handler: Ref<TypedEventHandler<DispatcherQueue, DispatcherQueueShutdownStartingEventArgs>>,
    ) -> Result<i64> {
        register(&self.events, Kind::ShutdownStarting, handler)
    }

    fn RemoveShutdownStarting(&self, token: i64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .remove(Kind::ShutdownStarting, token);
        Ok(())
    }

    fn ShutdownCompleted(
        &self,
        handler: Ref<TypedEventHandler<DispatcherQueue, IInspectable>>,
    ) -> Result<i64> {
        register(&self.events, Kind::ShutdownCompleted, handler)
    }

    fn RemoveShutdownCompleted(&self, token: i64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .remove(Kind::ShutdownCompleted, token);
        Ok(())
    }
}

impl IDispatcherQueue2_Impl for QueueObject_Impl {
    fn HasThreadAccess(&self) -> Result<bool> {
        Ok(self.core.has_thread_access())
    }
}

impl IDispatcherQueue3_Impl for QueueObject_Impl {
    fn EnqueueEventLoopExit(&self) -> Result<()> {
        self.platform.quit();
        Ok(())
    }

    fn EnsureSystemDispatcherQueue(&self) -> Result<()> {
        // This queue is the thread's dispatcher queue; there is no second one to
        // install. Saying so keeps a caller that asks on our thread on this queue.
        Ok(())
    }

    fn RunEventLoop(&self) -> Result<()> {
        self.platform.run_loop(false);
        run_finished(&self.events);
        Ok(())
    }

    fn RunEventLoopWithOptions(
        &self,
        options: DispatcherRunOptions,
        deferral: Ref<DispatcherExitDeferral>,
    ) -> Result<()> {
        // A deferral holds the exit open; this DLL has no exit to hold, and
        // `Complete` on the object it would return is `E_NOTIMPL`.
        if deferral.as_ref().is_some() {
            return crate::not_implemented();
        }
        self.platform
            .run_loop(options.contains(DispatcherRunOptions::ContinueOnQuit));
        run_finished(&self.events);
        Ok(())
    }

    fn FrameworkShutdownStarting(
        &self,
        handler: Ref<TypedEventHandler<DispatcherQueue, DispatcherQueueShutdownStartingEventArgs>>,
    ) -> Result<i64> {
        register(&self.events, Kind::FrameworkStarting, handler)
    }

    fn RemoveFrameworkShutdownStarting(&self, token: i64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .remove(Kind::FrameworkStarting, token);
        Ok(())
    }

    fn FrameworkShutdownCompleted(
        &self,
        handler: Ref<TypedEventHandler<DispatcherQueue, IInspectable>>,
    ) -> Result<i64> {
        register(&self.events, Kind::FrameworkCompleted, handler)
    }

    fn RemoveFrameworkShutdownCompleted(&self, token: i64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .remove(Kind::FrameworkCompleted, token);
        Ok(())
    }
}

fn enqueue(
    core: &Arc<Queue>,
    priority: Priority,
    callback: Ref<DispatcherQueueHandler>,
) -> Result<bool> {
    let callback = callback
        .as_ref()
        .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
    let handler = Handler(callback.cast()?);
    let task = Box::new(move || handler.invoke());
    Ok(core.enqueue(priority, task))
}

fn register<TSender, TResult>(
    events: &Arc<Mutex<Events>>,
    kind: Kind,
    handler: Ref<TypedEventHandler<TSender, TResult>>,
) -> Result<i64>
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static,
{
    let handler = handler
        .as_ref()
        .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
    let handler = Handler(handler.cast()?);
    Ok(events.lock().unwrap().add(kind, handler))
}

fn raise(events: &Arc<Mutex<Events>>, kind: Kind, args: *mut std::ffi::c_void) {
    let (handlers, sender) = {
        let mut events = events.lock().unwrap();
        (events.list(kind).clone(), events.sender.clone())
    };
    let Some(sender) = sender else {
        return;
    };
    for (_, handler) in &handlers {
        handler.raise(Interface::as_raw(&sender), args);
    }
}

pub(crate) fn create(core: Arc<Queue>, platform: Arc<Platform>) -> Result<Created> {
    let events = Arc::new(Mutex::new(Events::default()));
    let object: IDispatcherQueue = QueueObject {
        core,
        platform,
        events: events.clone(),
    }
    .into();
    let queue: DispatcherQueue = object.cast()?;
    events.lock().unwrap().sender = Some(queue.clone());
    Ok(Created { queue, events })
}

/// Post the one task that raises `ShutdownStarting` and then asks the loop to
/// leave. The queue is closed first, so work enqueued after this call is refused
/// rather than overtaking the event.
pub(crate) fn request_shutdown(
    core: &Arc<Queue>,
    platform: &Arc<Platform>,
    events: &Arc<Mutex<Events>>,
) -> Result<()> {
    core.begin_shutdown();
    let platform = platform.clone();
    let events = events.clone();
    let task = Box::new(move || {
        // The event's argument is the class object itself, not its
        // `IInspectable` face: the delegate's vtable slot names the class.
        let args = crate::args::create().ok();
        let args = args
            .as_ref()
            .map_or(std::ptr::null_mut(), Interface::as_raw);
        raise(&events, Kind::ShutdownStarting, args);
        raise(&events, Kind::FrameworkStarting, args);
        platform.quit();
    });
    if core.enqueue_shutdown(task) {
        Ok(())
    } else {
        Err(Error::from_hresult(E_FAIL))
    }
}

/// `GetForCurrentThread` on a thread this DLL did not create the queue on.
pub(crate) fn not_on_this_thread() -> Error {
    Error::from_hresult(E_NOTIMPL_QUEUE)
}

/// The statics class: one method, answered from the creating thread's queue.
#[windows_core::implement(IDispatcherQueueStatics)]
pub(crate) struct QueueStatics;

impl IDispatcherQueueStatics_Impl for QueueStatics_Impl {
    fn GetForCurrentThread(&self) -> Result<DispatcherQueue> {
        crate::controller::current().ok_or_else(not_on_this_thread)
    }
}

pub(crate) fn statics() -> Result<IDispatcherQueueStatics> {
    Ok(QueueStatics.into())
}

/// An already-complete action for `ShutdownQueueAsync`.
pub(crate) fn ready() -> IAsyncAction {
    IAsyncAction::ready(Ok(()))
}

/// Raise the two completion events after our loop has left.
pub(crate) fn run_finished(events: &Arc<Mutex<Events>>) {
    raise(events, Kind::ShutdownCompleted, std::ptr::null_mut());
    raise(events, Kind::FrameworkCompleted, std::ptr::null_mut());
}
