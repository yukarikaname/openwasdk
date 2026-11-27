//! `ColorKeyFrameAnimation`: the same animation with `Color` key frames.
//!
//! It is separate from the other three only because it carries one extra property,
//! the colour space to interpolate in, which this DLL ignores in favour of sRGB.

use std::sync::{Arc, Mutex};

use crate::animation::animation_shared;
use crate::animation::{easing_of, insert};
use crate::bindings::Microsoft::UI::Composition::{
    ColorKeyFrameAnimation, CompositionColorSpace, CompositionEasingFunction,
    IColorKeyFrameAnimation, IColorKeyFrameAnimation_Impl, ICompositionAnimation,
    ICompositionAnimation2, ICompositionAnimationBase, ICompositionObject, ICompositionObject2,
    ICompositionObject4, ICompositionObject5, IKeyFrameAnimation,
};
use crate::bindings::Windows::UI::Color;
use crate::object::composition_object;
use opencomp_core::{Easing, Keyframes};
use windows_core::{Interface, Ref, Result};

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionAnimationBase,
    ICompositionAnimation2,
    ICompositionAnimation,
    IKeyFrameAnimation,
    IColorKeyFrameAnimation
)]
pub struct ColorAnimation {
    state: Arc<Mutex<Keyframes>>,
    target: Mutex<String>,
}

composition_object!(ColorAnimation_Impl);
animation_shared!(ColorAnimation_Impl);

impl IColorKeyFrameAnimation_Impl for ColorAnimation_Impl {
    fn InterpolationColorSpace(&self) -> Result<CompositionColorSpace> {
        Ok(CompositionColorSpace::Auto)
    }

    fn SetInterpolationColorSpace(&self, _value: CompositionColorSpace) -> Result<()> {
        Ok(())
    }

    fn InsertKeyFrame(&self, keyFrame: f32, value: &Color) -> Result<()> {
        insert(
            &self.state,
            keyFrame,
            crate::brush::to_rgba(*value),
            Easing::Linear,
        );
        Ok(())
    }

    fn InsertKeyFrameWithEasingFunction(
        &self,
        keyFrame: f32,
        value: &Color,
        easingFunction: Ref<CompositionEasingFunction>,
    ) -> Result<()> {
        insert(
            &self.state,
            keyFrame,
            crate::brush::to_rgba(*value),
            easing_of(easingFunction),
        );
        Ok(())
    }
}

pub fn color() -> ::windows_core::Result<ColorKeyFrameAnimation> {
    let state = Arc::new(Mutex::new(Keyframes::default()));
    let animation: IColorKeyFrameAnimation = ColorAnimation {
        state: state.clone(),
        target: Mutex::new(String::new()),
    }
    .into();
    crate::identity::publish(&animation, &state);
    animation.cast()
}
