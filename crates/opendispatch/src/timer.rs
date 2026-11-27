//! `DispatcherQueueTimer` on top of Win32's `SetTimer`.
//!
//! A timer lives on the queue's hidden window. `Start` registers a slot with the
//! platform and arms a timer id; `WM_TIMER` finds the slot, runs the `Tick`
//! handlers and disarms the id unless the timer repeats. The handler list is
//! shared through an `Arc<Mutex<..>>` so the window procedure can reach it
//! without holding a reference to the COM object.

use std::sync::{Arc, Mutex};

use windows_core::{Error, HRESULT, IInspectable, Interface, Result};
use windows_time::TimeSpan;

use crate::bindings::Microsoft::UI::Dispatching::{
    DispatcherQueueTimer, IDispatcherQueueTimer, IDispatcherQueueTimer_Impl,
};
use crate::event::Handler;
use crate::platform::Platform;

const E_FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);
const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

/// The state the window procedure and the object both reach.
pub(crate) struct TimerState {
    /// `TimeSpan::duration`, in 100 ns ticks.
    interval: i64,
    running: bool,
    repeating: bool,
    id: usize,
    handlers: Vec<(i64, Handler)>,
    next_token: i64,
    sender: Option<DispatcherQueueTimer>,
}

pub(crate) type TimerSlot = Arc<Mutex<TimerState>>;

/// Raise one tick. Returns whether the timer should keep running.
pub(crate) fn fire(slot: &TimerSlot) -> bool {
    let (handlers, repeating, sender) = {
        let mut state = slot.lock().unwrap();
        if !state.running {
            return false;
        }
        if !state.repeating {
            state.running = false;
        }
        (
            state.handlers.clone(),
            state.repeating,
            state.sender.clone(),
        )
    };
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!(
            "opendispatch: tick with {} handler(s), sender {}",
            handlers.len(),
            sender.is_some()
        );
    }
    if let Some(sender) = sender {
        for (_, handler) in &handlers {
            let hr = handler.raise(Interface::as_raw(&sender), std::ptr::null_mut());
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!(
                    "opendispatch: tick raised to {} handler(s), hr=0x{:08x}",
                    handlers.len(),
                    hr as u32
                );
            }
        }
    }
    repeating
}

#[windows_core::implement(IDispatcherQueueTimer)]
struct Timer {
    platform: Arc<Platform>,
    state: TimerSlot,
}

impl IDispatcherQueueTimer_Impl for Timer_Impl {
    fn Interval(&self) -> Result<TimeSpan> {
        Ok(TimeSpan {
            duration: self.state.lock().unwrap().interval,
        })
    }

    fn SetInterval(&self, value: &TimeSpan) -> Result<()> {
        if value.duration < 0 {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        let restart = {
            let mut state = self.state.lock().unwrap();
            state.interval = value.duration;
            if state.running {
                let id = state.id;
                state.running = false;
                state.id = 0;
                Some(id)
            } else {
                None
            }
        };
        if let Some(id) = restart {
            self.platform.stop_timer(id);
            start(&self.platform, &self.state)?;
        }
        Ok(())
    }

    fn IsRunning(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().running)
    }

    fn IsRepeating(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().repeating)
    }

    fn SetIsRepeating(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().repeating = value;
        Ok(())
    }

    fn Start(&self) -> Result<()> {
        start(&self.platform, &self.state)
    }

    fn Stop(&self) -> Result<()> {
        let id = {
            let mut state = self.state.lock().unwrap();
            let id = std::mem::replace(&mut state.id, 0);
            state.running = false;
            id
        };
        if id != 0 {
            self.platform.stop_timer(id);
        }
        Ok(())
    }

    fn Tick(
        &self,
        handler: windows_core::Ref<
            crate::bindings::Windows::Foundation::TypedEventHandler<
                DispatcherQueueTimer,
                IInspectable,
            >,
        >,
    ) -> Result<i64> {
        let handler = handler
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let handler = Handler(handler.cast()?);
        let mut state = self.state.lock().unwrap();
        let token = state.next_token;
        state.next_token += 1;
        state.handlers.push((token, handler));
        Ok(token)
    }

    fn RemoveTick(&self, token: i64) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .handlers
            .retain(|(candidate, _)| *candidate != token);
        Ok(())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let id = {
            let mut state = self.state.lock().unwrap();
            state.running = false;
            std::mem::replace(&mut state.id, 0)
        };
        if id != 0 {
            self.platform.stop_timer(id);
        }
    }
}

fn start(platform: &Platform, state: &TimerSlot) -> Result<()> {
    let (already, milliseconds) = {
        let state = state.lock().unwrap();
        (state.running, milliseconds(state.interval))
    };
    if already {
        return Ok(());
    }
    let id = platform
        .start_timer(state.clone(), milliseconds)
        .ok_or_else(|| Error::from_hresult(E_FAIL))?;
    let mut state = state.lock().unwrap();
    state.id = id;
    state.running = true;
    Ok(())
}

/// `TimeSpan` is 100 ns; `SetTimer` is milliseconds, and zero means "system
/// default", which is not what an interval of one tick asked for.
fn milliseconds(ticks: i64) -> u32 {
    let milliseconds = ticks / 10_000;
    milliseconds.clamp(1, u32::MAX as i64) as u32
}

pub(crate) fn create(platform: Arc<Platform>) -> Result<DispatcherQueueTimer> {
    let state: TimerSlot = Arc::new(Mutex::new(TimerState {
        interval: 10_000_000,
        running: false,
        repeating: true,
        id: 0,
        handlers: Vec::new(),
        next_token: 1,
        sender: None,
    }));
    let timer: IDispatcherQueueTimer = Timer {
        platform,
        state: state.clone(),
    }
    .into();
    let timer: DispatcherQueueTimer = timer.cast()?;
    // The sender the handlers receive is the timer itself, so the object has to
    // exist before its own state can name it.
    state.lock().unwrap().sender = Some(timer.clone());
    Ok(timer)
}
