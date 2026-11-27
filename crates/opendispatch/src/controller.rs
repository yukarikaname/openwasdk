//! `DispatcherQueueController`: the object that owns a queue and the thread it
//! belongs to.
//!
//! `CreateOnCurrentThread` installs the queue on the caller and leaves it there;
//! the engine's own message loop pumps it from then on.
//! `CreateOnDedicatedThread` starts a thread whose only job is to run our loop,
//! and hands the controller back to the caller. Both set the thread-local
//! pointer `DispatcherQueue.GetForCurrentThread` reads.

use std::cell::RefCell;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use windows_core::{Error, HRESULT, Interface, Result};
use windows_future::IAsyncAction;

use crate::bindings::Microsoft::UI::Dispatching::{
    DispatcherQueue, DispatcherQueueController, IDispatcherQueueController,
    IDispatcherQueueController_Impl, IDispatcherQueueController2, IDispatcherQueueController2_Impl,
    IDispatcherQueueControllerStatics, IDispatcherQueueControllerStatics_Impl,
};
use crate::platform::Platform;
use crate::queue::Queue;
use crate::queue_object::{self, Events};

const E_FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);

thread_local! {
    /// The queue the current thread created. A dedicated thread sets this too,
    /// so `GetForCurrentThread` answers there as well.
    static CURRENT: RefCell<Option<DispatcherQueue>> = const { RefCell::new(None) };
}

pub(crate) fn current() -> Option<DispatcherQueue> {
    CURRENT.with(|slot| slot.borrow().clone())
}

fn set_current(queue: &DispatcherQueue) {
    CURRENT.with(|slot| *slot.borrow_mut() = Some(queue.clone()));
}

#[windows_core::implement(IDispatcherQueueController, IDispatcherQueueController2)]
struct Controller {
    core: Arc<Queue>,
    platform: Arc<Platform>,
    events: Arc<Mutex<Events>>,
    queue: DispatcherQueue,
}

impl IDispatcherQueueController_Impl for Controller_Impl {
    fn DispatcherQueue(&self) -> Result<DispatcherQueue> {
        Ok(self.queue.clone())
    }

    fn ShutdownQueueAsync(&self) -> Result<IAsyncAction> {
        queue_object::request_shutdown(&self.core, &self.platform, &self.events)?;
        Ok(queue_object::ready())
    }
}

impl IDispatcherQueueController2_Impl for Controller_Impl {
    fn ShutdownQueue(&self) -> Result<()> {
        queue_object::request_shutdown(&self.core, &self.platform, &self.events)
    }
}

/// Everything one controller creation produced. The dedicated thread needs the
/// events and the loop after the controller has been handed to the caller.
struct Instance {
    controller: DispatcherQueueController,
    events: Arc<Mutex<Events>>,
    platform: Arc<Platform>,
}

fn create_instance() -> Result<Instance> {
    let platform = Platform::create().ok_or_else(|| Error::from_hresult(E_FAIL))?;
    let core = Queue::new(crate::thread::current(), platform.clone());
    platform.set_queue(Arc::downgrade(&core));
    let created = queue_object::create(core.clone(), platform.clone())?;
    set_current(&created.queue);

    let controller: IDispatcherQueueController = Controller {
        core,
        platform: platform.clone(),
        events: created.events.clone(),
        queue: created.queue.clone(),
    }
    .into();
    let controller = controller.cast()?;
    Ok(Instance {
        controller,
        events: created.events,
        platform,
    })
}

fn create_on_dedicated_thread() -> Result<DispatcherQueueController> {
    let (sender, receiver) = mpsc::channel::<std::result::Result<DispatcherQueueController, i32>>();
    std::thread::Builder::new()
        .name("opendispatch".into())
        .spawn(move || match create_instance() {
            Ok(instance) => {
                if sender.send(Ok(instance.controller.clone())).is_err() {
                    return;
                }
                instance.platform.run_loop(false);
                queue_object::run_finished(&instance.events);
            }
            Err(error) => {
                let _ = sender.send(Err(error.code().0));
            }
        })
        .map_err(|_| Error::from_hresult(E_FAIL))?;

    match receiver.recv() {
        Ok(Ok(controller)) => Ok(controller),
        Ok(Err(code)) => Err(Error::from_hresult(HRESULT(code))),
        Err(_) => Err(Error::from_hresult(E_FAIL)),
    }
}

#[windows_core::implement(IDispatcherQueueControllerStatics)]
struct ControllerStatics;

impl IDispatcherQueueControllerStatics_Impl for ControllerStatics_Impl {
    fn CreateOnDedicatedThread(&self) -> Result<DispatcherQueueController> {
        create_on_dedicated_thread()
    }

    fn CreateOnCurrentThread(&self) -> Result<DispatcherQueueController> {
        Ok(create_instance()?.controller)
    }
}

pub(crate) fn statics() -> Result<IDispatcherQueueControllerStatics> {
    Ok(ControllerStatics.into())
}
