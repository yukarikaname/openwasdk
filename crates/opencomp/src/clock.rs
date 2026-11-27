//! The compositor's time source and the queue completions are posted to.
//!
//! Only the frame loop advances a clock. A batch does not end when the caller says so
//! in this compositor; it ends at the next frame, which is the only commit boundary
//! this DLL has. The completion handler is queued on the same clock and runs in the
//! frame loop after the batch is closed, so a client that waits for `Completed` sees
//! the tree the next frame will draw, not one half-way through the change.

use std::sync::{Arc, Mutex, Weak};

/// A listener the clock closes at the end of a frame.
pub trait FrameListener: Send + Sync {
    fn frame(&self);
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// The time a compositor hands to its visuals, and the work the frame loop runs.
pub struct Clock {
    state: Mutex<State>,
}

struct State {
    now: f32,
    listeners: Vec<Weak<dyn FrameListener>>,
    jobs: Vec<Job>,
}

impl Clock {
    /// A clock at time zero with nothing queued.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                now: 0.0,
                listeners: Vec::new(),
                jobs: Vec::new(),
            }),
        })
    }

    /// Seconds since the clock was created. This is the only time animations see.
    pub fn now(&self) -> f32 {
        self.state.lock().unwrap().now
    }

    /// Queue work for the frame loop. Jobs run in the order they were posted.
    pub fn post(&self, job: impl FnOnce() + Send + 'static) {
        self.state.lock().unwrap().jobs.push(Box::new(job));
    }

    /// Close `listener` on every frame. The clock keeps only a weak reference.
    pub fn listen(&self, listener: Weak<dyn FrameListener>) {
        self.state.lock().unwrap().listeners.push(listener);
    }

    /// Advance the clock, close open batches, then run what they posted.
    ///
    /// A listener that is dropped between frames disappears from the list; a batch a
    /// client released cannot keep the clock alive or be completed behind its back.
    pub fn tick(&self, delta: f32) {
        let listeners = {
            let mut state = self.state.lock().unwrap();
            state.now += delta.max(0.0);
            state
                .listeners
                .retain(|listener| listener.strong_count() > 0);
            state.listeners.clone()
        };
        for listener in listeners {
            if let Some(listener) = listener.upgrade() {
                listener.frame();
            }
        }
        let jobs = {
            let mut state = self.state.lock().unwrap();
            core::mem::take(&mut state.jobs)
        };
        for job in jobs {
            job();
        }
    }
}

/// A persistent playback state: where an animation was when it was paused, how fast
/// it should run, and the progress a controller seeks to.
///
/// It is separate from the COM object so the visual tree can sample it without
/// knowing about `IAnimationController`; `animation_controller` is the ABI around it.
pub struct Playback {
    clock: Arc<Clock>,
    state: Mutex<PlaybackState>,
}

struct PlaybackState {
    /// The clock time and the animation time at the last anchor change.
    anchor_clock: f32,
    anchor_time: f32,
    rate: f32,
    paused: bool,
    /// A progress in 0..=1 that overrides the clock, or `None` to follow it.
    progress: Option<f32>,
    last_progress: f32,
    includes_delay: bool,
}

impl Playback {
    pub fn new(clock: Arc<Clock>) -> Arc<Self> {
        Arc::new(Self {
            clock,
            state: Mutex::new(PlaybackState {
                anchor_clock: 0.0,
                anchor_time: 0.0,
                rate: 1.0,
                paused: false,
                progress: None,
                last_progress: 0.0,
                includes_delay: false,
            }),
        })
    }

    pub fn clock(&self) -> &Arc<Clock> {
        &self.clock
    }

    /// Start an animation from time zero at the clock's current instant.
    pub fn bind(&self, now: f32) {
        let mut state = self.state.lock().unwrap();
        state.anchor_clock = now;
        state.anchor_time = 0.0;
        state.paused = false;
        state.last_progress = state.progress.unwrap_or(0.0);
    }

    /// The animation's local time in seconds.
    ///
    /// `span` is the animation's duration and `delay` its delay. When progress is
    /// controlled, `IncludesDelayTime` maps the progress over both; the default maps
    /// it over the animation alone.
    pub fn local_time(&self, now: f32, span: f32, delay: f32) -> f32 {
        let mut state = self.state.lock().unwrap();
        let local = if let Some(progress) = state.progress {
            if state.includes_delay {
                progress * (span + delay)
            } else {
                delay + progress * span
            }
        } else if state.paused {
            state.anchor_time
        } else {
            state.anchor_time + (now - state.anchor_clock) * state.rate
        };
        state.last_progress = if state.includes_delay {
            if span + delay > 0.0 {
                (local / (span + delay)).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else if span > 0.0 {
            ((local - delay) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        local
    }

    pub fn playback_rate(&self) -> f32 {
        self.state.lock().unwrap().rate
    }

    pub fn set_playback_rate(&self, now: f32, rate: f32) {
        let mut state = self.state.lock().unwrap();
        if state.progress.is_none() {
            state.anchor_time = if state.paused {
                state.anchor_time
            } else {
                state.anchor_time + (now - state.anchor_clock) * state.rate
            };
            state.anchor_clock = now;
        }
        state.rate = rate;
    }

    pub fn progress(&self) -> f32 {
        let state = self.state.lock().unwrap();
        state.progress.unwrap_or(state.last_progress)
    }

    pub fn set_progress(&self, progress: f32) {
        let mut state = self.state.lock().unwrap();
        state.progress = Some(progress.clamp(0.0, 1.0));
        state.last_progress = progress.clamp(0.0, 1.0);
    }

    pub fn pause(&self, now: f32) {
        let mut state = self.state.lock().unwrap();
        if state.progress.is_none() && !state.paused {
            state.anchor_time += (now - state.anchor_clock) * state.rate;
        }
        state.anchor_clock = now;
        state.paused = true;
    }

    pub fn resume(&self, now: f32) {
        let mut state = self.state.lock().unwrap();
        state.anchor_clock = now;
        state.paused = false;
    }

    pub fn includes_delay(&self) -> bool {
        self.state.lock().unwrap().includes_delay
    }

    pub fn set_includes_delay(&self, includes: bool) {
        self.state.lock().unwrap().includes_delay = includes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Count(AtomicUsize);

    impl FrameListener for Count {
        fn frame(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn the_clock_only_moves_when_it_is_ticked() {
        let clock = Clock::new();
        assert_eq!(clock.now(), 0.0);
        clock.tick(0.25);
        assert_eq!(clock.now(), 0.25);
        clock.tick(-1.0);
        assert_eq!(clock.now(), 0.25, "time does not run backwards");
    }

    #[test]
    fn a_listener_is_closed_on_every_frame_and_then_removed() {
        let clock = Clock::new();
        let listener = Arc::new(Count(AtomicUsize::new(0)));
        clock.listen(Arc::downgrade(&listener) as Weak<dyn FrameListener>);
        clock.tick(0.016);
        clock.tick(0.016);
        assert_eq!(listener.0.load(Ordering::Relaxed), 2);

        drop(listener);
        clock.tick(0.016);
        assert!(clock.state.lock().unwrap().listeners.is_empty());
    }

    #[test]
    fn a_job_posted_before_a_tick_runs_once() {
        let clock = Clock::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        clock.post(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        clock.tick(0.016);
        clock.tick(0.016);
        assert_eq!(runs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn playback_follows_the_clock_and_can_be_paused() {
        let clock = Clock::new();
        let playback = Playback::new(clock.clone());
        playback.bind(clock.now());
        clock.tick(0.5);
        assert_eq!(playback.local_time(clock.now(), 1.0, 0.0), 0.5);

        playback.pause(clock.now());
        clock.tick(1.0);
        assert_eq!(playback.local_time(clock.now(), 1.0, 0.0), 0.5);

        playback.resume(clock.now());
        clock.tick(0.25);
        assert_eq!(playback.local_time(clock.now(), 1.0, 0.0), 0.75);
    }

    #[test]
    fn a_progress_override_ignores_the_clock() {
        let clock = Clock::new();
        let playback = Playback::new(clock.clone());
        playback.bind(clock.now());
        playback.set_progress(0.25);
        clock.tick(2.0);
        assert_eq!(playback.local_time(clock.now(), 1.0, 0.0), 0.25);
        assert!((playback.progress() - 0.25).abs() < 1e-6);

        playback.set_includes_delay(true);
        assert_eq!(playback.local_time(clock.now(), 1.0, 0.5), 0.375);
    }
}
