//! `CompositionAnimationGroup`: several animations started by one call.
//!
//! The group is a container, not a curve of its own. `ICompositionObject2`'s
//! `StartAnimationGroup` is what gives it meaning: every animation it holds is
//! started on the property named by that animation's `Target`.
//!
//! `ICompositionAnimationGroup` has no indexer, and the only other interface
//! that would expose the elements is `IIterable`, which this DLL does not
//! implement. The list is therefore published in the identity table, the same
//! way animation key frames are, so `StartAnimationGroup` can find it from the
//! COM pointer the ABI hands over.

use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionAnimation, CompositionAnimationGroup, ICompositionAnimation2,
    ICompositionAnimationBase, ICompositionAnimationBase_Impl, ICompositionAnimationGroup,
    ICompositionAnimationGroup_Impl, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5,
};
use crate::object::composition_object;
use windows_core::{Interface, Ref, Result};

/// The list every clone of the group shares with the identity table.
type Shared = Arc<Mutex<Vec<CompositionAnimation>>>;

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionAnimationBase,
    ICompositionAnimationGroup
)]
pub struct AnimationGroupObject {
    animations: Shared,
}

composition_object!(AnimationGroupObject_Impl);

impl ICompositionAnimationBase_Impl for AnimationGroupObject_Impl {}

impl ICompositionAnimationGroup_Impl for AnimationGroupObject_Impl {
    fn Count(&self) -> Result<i32> {
        Ok(self.animations.lock().unwrap().len() as i32)
    }

    fn Add(&self, value: Ref<CompositionAnimation>) -> Result<()> {
        // A null animation cannot be started later; refusing it here would throw
        // at the caller for no gain, so it is simply not added.
        if let Some(animation) = value.cloned() {
            self.animations.lock().unwrap().push(animation);
        }
        Ok(())
    }

    fn Remove(&self, value: Ref<CompositionAnimation>) -> Result<()> {
        let Some(animation) = value.as_ref() else {
            return Ok(());
        };
        let mut animations = self.animations.lock().unwrap();
        // A group is a list, not a set: remove the first match and leave any
        // duplicate the client added deliberately.
        if let Some(index) = animations.iter().position(|held| held == animation) {
            animations.remove(index);
        }
        Ok(())
    }

    fn RemoveAll(&self) -> Result<()> {
        self.animations.lock().unwrap().clear();
        Ok(())
    }
}

pub fn create() -> Result<CompositionAnimationGroup> {
    let animations = Arc::new(Mutex::new(Vec::new()));
    let group: ICompositionAnimationGroup = AnimationGroupObject {
        animations: animations.clone(),
    }
    .into();
    crate::identity::publish(&group, &animations);
    group.cast()
}

/// Start every animation a group made by this DLL holds.
///
/// An animation without a `Target`, or with a property the renderer does not
/// know, is skipped: `StartAnimation` behaves the same way for a name it does
/// not parse. A group from another implementation has no list to read, so that
/// case reports `E_NOTIMPL` rather than pretending it started something.
pub(crate) fn start(node: &crate::node::NodeRef, group: &ICompositionAnimationBase) -> Result<()> {
    let Some(animations) = animations_of(group) else {
        return crate::object::not_implemented();
    };
    for (property, keyframes) in animations {
        crate::node::start_animation(node, property, keyframes);
    }
    Ok(())
}

/// Stop every animation a group made by this DLL started.
///
/// Stopping follows the same target names as starting, so the two calls always
/// address the same node property. A foreign group has no list and reports
/// `E_NOTIMPL`, exactly as `start` does.
pub(crate) fn stop(node: &crate::node::NodeRef, group: &ICompositionAnimationBase) -> Result<()> {
    let Some(animations) = animations_of(group) else {
        return crate::object::not_implemented();
    };
    for (property, _) in animations {
        crate::node::stop_animation(node, property);
    }
    Ok(())
}

/// The targets and curves a group holds, or `None` for a foreign group.
///
/// The list is copied out and the lock released before touching the node: a
/// client may hold the group lock while it waits for an animation to start.
fn animations_of(
    group: &ICompositionAnimationBase,
) -> Option<Vec<(crate::node::Property, opencomp_core::Keyframes)>> {
    let state = crate::identity::lookup::<_, Mutex<Vec<CompositionAnimation>>>(group)?;
    let animations = state.lock().unwrap().clone();
    Some(
        animations
            .into_iter()
            .filter_map(|animation| {
                let target = animation
                    .cast::<ICompositionAnimation2>()
                    .ok()
                    .and_then(|animation| animation.Target().ok())
                    .unwrap_or_default();
                let property = crate::node::Property::parse(&target)?;
                let keyframes = crate::animation::keyframes(&animation)?;
                Some((property, keyframes))
            })
            .collect(),
    )
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{
        ICompositionObject2, ICompositor, ICompositor2, IScalarKeyFrameAnimation,
    };

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    fn compositor2() -> ICompositor2 {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    #[test]
    fn a_group_counts_and_removes_its_animations() {
        let compositor = compositor();
        let group = compositor2().CreateAnimationGroup().unwrap();
        let group: ICompositionAnimationGroup = group.cast().unwrap();
        assert_eq!(group.Count().unwrap(), 0);

        let animation = compositor.CreateScalarKeyFrameAnimation().unwrap();
        let animation: CompositionAnimation = animation.cast().unwrap();
        group.Add(&animation).unwrap();
        group.Add(&animation).unwrap();
        assert_eq!(group.Count().unwrap(), 2);

        group.Remove(&animation).unwrap();
        assert_eq!(group.Count().unwrap(), 1);

        group.RemoveAll().unwrap();
        assert_eq!(group.Count().unwrap(), 0);
    }

    #[test]
    fn starting_a_group_uses_each_animations_target() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let group = compositor2().CreateAnimationGroup().unwrap();
        let group: ICompositionAnimationGroup = group.cast().unwrap();

        let opacity = compositor.CreateScalarKeyFrameAnimation().unwrap();
        let target: ICompositionAnimation2 = opacity.cast().unwrap();
        target.SetTarget("Opacity").unwrap();
        assert_eq!(target.Target().unwrap(), "Opacity");
        let keyframes: IScalarKeyFrameAnimation = opacity.cast().unwrap();
        keyframes.InsertKeyFrame(0.0, 0.25).unwrap();
        let opacity: CompositionAnimation = opacity.cast().unwrap();
        group.Add(&opacity).unwrap();

        let base: ICompositionAnimationBase = group.cast().unwrap();
        let object: ICompositionObject2 = sprite.cast().unwrap();
        object.StartAnimationGroup(&base).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.opacity, 0.25);
    }

    #[test]
    fn stopping_a_group_stops_what_it_started() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let group = compositor2().CreateAnimationGroup().unwrap();
        let group: ICompositionAnimationGroup = group.cast().unwrap();

        let opacity = compositor.CreateScalarKeyFrameAnimation().unwrap();
        let target: ICompositionAnimation2 = opacity.cast().unwrap();
        target.SetTarget("Opacity").unwrap();
        let keyframes: IScalarKeyFrameAnimation = opacity.cast().unwrap();
        keyframes.InsertKeyFrame(0.0, 0.25).unwrap();
        let opacity: CompositionAnimation = opacity.cast().unwrap();
        group.Add(&opacity).unwrap();

        let base: ICompositionAnimationBase = group.cast().unwrap();
        let object: ICompositionObject2 = sprite.cast().unwrap();
        object.StartAnimationGroup(&base).unwrap();
        let node = crate::node::resolve(&sprite).unwrap();
        assert_eq!(node.lock().unwrap().animations.len(), 1);

        object.StopAnimationGroup(&base).unwrap();
        assert!(node.lock().unwrap().animations.is_empty());
        // `LeaveCurrentValue` keeps the sampled value on the node.
        assert_eq!(crate::node::snapshot(&node).unwrap().opacity, 0.25);
    }

    #[test]
    fn an_animation_without_a_target_is_skipped() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let group = compositor2().CreateAnimationGroup().unwrap();
        let group: ICompositionAnimationGroup = group.cast().unwrap();

        let animation = compositor.CreateScalarKeyFrameAnimation().unwrap();
        let animation: CompositionAnimation = animation.cast().unwrap();
        group.Add(&animation).unwrap();

        let base: ICompositionAnimationBase = group.cast().unwrap();
        let object: ICompositionObject2 = sprite.cast().unwrap();
        object.StartAnimationGroup(&base).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        assert_eq!(crate::node::snapshot(&node).unwrap().opacity, 1.0);
    }
}
