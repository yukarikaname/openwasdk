//! `AnimationController`: pause, seek and rate control over one running animation.
//!
//! The controller is a handle on a [`Playback`]. The playback lives in the visual tree's
//! clock, so the controller does not have to be handed to the sampler separately; a
//! running animation stores the playback when it starts and the snapshot asks it for
//! the local time.

use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::bindings::Microsoft::UI::Composition::{
    AnimationController, AnimationControllerProgressBehavior, IAnimationController,
    IAnimationController_Impl,
};
use crate::clock::{Clock, Playback};
use windows_core::{Interface, Result};

/// One controller's shared state. The interface is weak because the COM object owns
/// the shared state, not the other way around.
struct ControllerShared {
    playback: Arc<Playback>,
    this: Mutex<windows_core::Weak<AnimationController>>,
}

impl ControllerShared {
    fn new(playback: Arc<Playback>) -> Self {
        Self {
            playback,
            this: Mutex::new(windows_core::Weak::new()),
        }
    }
}

#[windows_core::implement(IAnimationController)]
pub struct AnimationControllerObject {
    shared: Arc<ControllerShared>,
}

type Entry = (usize, Weak<ControllerShared>);
static LIVE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn live() -> &'static Mutex<Vec<Entry>> {
    LIVE.get_or_init(|| Mutex::new(Vec::new()))
}

fn register(playback: &Arc<Playback>, shared: &Arc<ControllerShared>) {
    let key = Arc::as_ptr(playback) as usize;
    let mut table = live().lock().unwrap();
    table.retain(|(_, weak)| weak.strong_count() > 0);
    table.retain(|(existing, _)| *existing != key);
    table.push((key, Arc::downgrade(shared)));
}

fn find(playback: &Arc<Playback>) -> Option<Arc<ControllerShared>> {
    let key = Arc::as_ptr(playback) as usize;
    let table = live().lock().unwrap();
    table
        .iter()
        .find(|(existing, _)| *existing == key)
        .and_then(|(_, weak)| weak.upgrade())
}

fn build(shared: Arc<ControllerShared>) -> Result<AnimationController> {
    let object: IAnimationController = AnimationControllerObject {
        shared: shared.clone(),
    }
    .into();
    let object: AnimationController = object.cast()?;
    if let Ok(weak) = object.downgrade() {
        *shared.this.lock().unwrap() = weak;
    }
    crate::identity::publish(&object, &shared);
    register(&shared.playback, &shared);
    Ok(object)
}

/// A controller for a new animation.
pub fn create(clock: Arc<Clock>) -> Result<AnimationController> {
    from_playback(Playback::new(clock))
}

/// A controller over an existing playback.
///
/// The same playback is always represented by one live controller when one exists; a
/// client that started an animation with a controller and asks for it back gets the
/// object it started with.
pub fn from_playback(playback: Arc<Playback>) -> Result<AnimationController> {
    if let Some(shared) = find(&playback) {
        if let Some(controller) = shared.this.lock().unwrap().upgrade() {
            return Ok(controller);
        }
        return build(shared);
    }
    build(Arc::new(ControllerShared::new(playback)))
}

/// The playback behind a controller this DLL made.
pub fn playback_of(controller: &AnimationController) -> Option<Arc<Playback>> {
    crate::identity::lookup::<_, ControllerShared>(controller).map(|shared| shared.playback.clone())
}

impl IAnimationController_Impl for AnimationControllerObject_Impl {
    fn PlaybackRate(&self) -> Result<f32> {
        Ok(self.shared.playback.playback_rate())
    }

    fn SetPlaybackRate(&self, value: f32) -> Result<()> {
        let now = self.shared.playback.clock().now();
        self.shared.playback.set_playback_rate(now, value);
        Ok(())
    }

    fn Progress(&self) -> Result<f32> {
        Ok(self.shared.playback.progress())
    }

    fn SetProgress(&self, value: f32) -> Result<()> {
        self.shared.playback.set_progress(value);
        Ok(())
    }

    fn ProgressBehavior(&self) -> Result<AnimationControllerProgressBehavior> {
        Ok(if self.shared.playback.includes_delay() {
            AnimationControllerProgressBehavior::IncludesDelayTime
        } else {
            AnimationControllerProgressBehavior::Default
        })
    }

    fn SetProgressBehavior(&self, value: AnimationControllerProgressBehavior) -> Result<()> {
        self.shared
            .playback
            .set_includes_delay(value == AnimationControllerProgressBehavior::IncludesDelayTime);
        Ok(())
    }

    fn Pause(&self) -> Result<()> {
        let now = self.shared.playback.clock().now();
        self.shared.playback.pause(now);
        Ok(())
    }

    fn Resume(&self) -> Result<()> {
        let now = self.shared.playback.clock().now();
        self.shared.playback.resume(now);
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use crate::bindings::Microsoft::UI::Composition::{
        CompositionAnimation, ICompositionObject4, ICompositionObject5, ICompositor, ICompositor8,
        IScalarKeyFrameAnimation,
    };
    use windows_core::Interface;

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    fn ramp(compositor: &ICompositor) -> CompositionAnimation {
        let animation = compositor.CreateScalarKeyFrameAnimation().unwrap();
        let scalar: IScalarKeyFrameAnimation = animation.cast().unwrap();
        scalar.InsertKeyFrame(0.0, 0.0).unwrap();
        scalar.InsertKeyFrame(1.0, 1.0).unwrap();
        animation.cast().unwrap()
    }

    #[test]
    fn a_controller_binds_a_running_animation() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let animation = ramp(&compositor);
        let controller = compositor
            .cast::<ICompositor8>()
            .unwrap()
            .CreateAnimationController()
            .unwrap();

        let object: ICompositionObject5 = sprite.cast().unwrap();
        object
            .StartAnimationWithController("Opacity", &animation, &controller)
            .unwrap();

        let object4: ICompositionObject4 = sprite.cast().unwrap();
        let returned = object4.TryGetAnimationController("Opacity").unwrap();
        assert_eq!(
            Interface::as_raw(&returned),
            Interface::as_raw(&controller),
            "the controller the animation was started with comes back"
        );
    }

    #[test]
    fn progress_seeks_the_animation_without_advancing_the_clock() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let animation = ramp(&compositor);
        let controller = compositor
            .cast::<ICompositor8>()
            .unwrap()
            .CreateAnimationController()
            .unwrap();
        let object: ICompositionObject5 = sprite.cast().unwrap();
        object
            .StartAnimationWithController("Opacity", &animation, &controller)
            .unwrap();

        controller.SetProgress(0.5).unwrap();
        let node = crate::node::resolve(&sprite).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert!((tree.opacity - 0.5).abs() < 1e-3);
    }
}
