//! Key-frame animation objects: scalar, vector2 and vector3.
//!
//! Each one is the same state behind a different `InsertKeyFrame` signature, so the
//! trait bodies are written once by a macro and the value shape is the only thing
//! that differs. The state is published in the identity table; that is how
//! `ICompositionObject.StartAnimation` finds the curve behind a client's animation.
//!
//! Parameters (`ICompositionAnimation.SetScalarParameter` and friends) are accepted
//! and dropped: they exist for expression animations, which are not implemented.

use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionAnimation, CompositionEasingFunction, ICompositionAnimation,
    ICompositionAnimationBase, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5, IKeyFrameAnimation, IScalarKeyFrameAnimation,
    IScalarKeyFrameAnimation_Impl, IVector2KeyFrameAnimation, IVector2KeyFrameAnimation_Impl,
    IVector3KeyFrameAnimation, IVector3KeyFrameAnimation_Impl, ScalarKeyFrameAnimation,
    Vector2KeyFrameAnimation, Vector3KeyFrameAnimation,
};
use crate::object::composition_object;
use crate::object::prelude::*;
use opencomp_core::{Easing, Frame, Keyframes};
use windows_core::Interface;
use windows_time::TimeSpan;

/// One hundred nanoseconds, the unit `Duration` and `DelayTime` speak.
pub(crate) fn seconds(value: TimeSpan) -> f32 {
    value.duration as f32 / 10_000_000.0
}

pub(crate) fn ticks(value: f32) -> TimeSpan {
    TimeSpan {
        duration: (value * 10_000_000.0).round() as i64,
    }
}

/// The curve a client attached to a key frame.
pub(crate) fn easing_of(easing: Ref<CompositionEasingFunction>) -> Easing {
    let Some(easing) = easing.as_ref() else {
        return Easing::Linear;
    };
    let Ok(bezier) =
        easing.cast::<crate::bindings::Microsoft::UI::Composition::ICubicBezierEasingFunction>()
    else {
        return Easing::Linear;
    };
    let (Ok(first), Ok(second)) = (bezier.ControlPoint1(), bezier.ControlPoint2()) else {
        return Easing::Linear;
    };
    Easing::CubicBezier(first.x, first.y, second.x, second.y)
}

/// The curve behind a client's animation, for `StartAnimation`.
pub fn keyframes(animation: &CompositionAnimation) -> Option<Keyframes> {
    let state = crate::identity::lookup::<_, Mutex<Keyframes>>(animation)?;
    let keyframes = state.lock().unwrap();
    Some(keyframes.clone())
}

pub(crate) fn insert(state: &Mutex<Keyframes>, time: f32, value: [f32; 4], easing: Easing) {
    let mut keyframes = state.lock().unwrap();
    let frame = Frame {
        time,
        value,
        easing,
    };
    let at = keyframes
        .frames
        .partition_point(|existing| existing.time <= time);
    keyframes.frames.insert(at, frame);
}

/// The part of `ICompositionAnimation` and `IKeyFrameAnimation` that does not depend
/// on the value type.
///
/// Every name is written out in full: a macro body resolves names where it is
/// expanded, and this one is expanded in two modules.
macro_rules! animation_shared {
    ($impl_type:ident) => {
        impl $crate::bindings::Microsoft::UI::Composition::ICompositionAnimationBase_Impl
            for $impl_type
        {
        }

        impl $crate::bindings::Microsoft::UI::Composition::ICompositionAnimation_Impl
            for $impl_type
        {
            fn ClearAllParameters(&self) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn ClearParameter(
                &self,
                _key: &::windows_core::HSTRING,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetColorParameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &$crate::bindings::Windows::UI::Color,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetMatrix3x2Parameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &::windows_numerics::Matrix3x2,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetMatrix4x4Parameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &::windows_numerics::Matrix4x4,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetQuaternionParameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &$crate::bindings::Windows::Foundation::Numerics::Quaternion,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetReferenceParameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::CompositionObject,
                >,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetScalarParameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: f32,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetVector2Parameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &::windows_numerics::Vector2,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetVector3Parameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &::windows_numerics::Vector3,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn SetVector4Parameter(
                &self,
                _key: &::windows_core::HSTRING,
                _value: &::windows_numerics::Vector4,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
        }

        impl $crate::bindings::Microsoft::UI::Composition::IKeyFrameAnimation_Impl
            for $impl_type
        {
            fn DelayTime(&self) -> ::windows_core::Result<::windows_time::TimeSpan> {
                let delay = self.state.lock().unwrap().delay.max(0.0);
                Ok($crate::animation::ticks(delay))
            }
            fn SetDelayTime(&self, value: &::windows_time::TimeSpan) -> ::windows_core::Result<()> {
                self.state.lock().unwrap().delay = $crate::animation::seconds(*value);
                Ok(())
            }
            fn Duration(&self) -> ::windows_core::Result<::windows_time::TimeSpan> {
                let duration = self.state.lock().unwrap().duration;
                Ok($crate::animation::ticks(duration))
            }
            fn SetDuration(&self, value: &::windows_time::TimeSpan) -> ::windows_core::Result<()> {
                self.state.lock().unwrap().duration = $crate::animation::seconds(*value);
                Ok(())
            }
            fn IterationBehavior(
                &self,
            ) -> ::windows_core::Result<
                $crate::bindings::Microsoft::UI::Composition::AnimationIterationBehavior,
            > {
                let iterations = self.state.lock().unwrap().iterations;
                Ok(if iterations == 0 {
                    $crate::bindings::Microsoft::UI::Composition::AnimationIterationBehavior::Forever
                } else {
                    $crate::bindings::Microsoft::UI::Composition::AnimationIterationBehavior::Count
                })
            }
            fn SetIterationBehavior(
                &self,
                value: $crate::bindings::Microsoft::UI::Composition::AnimationIterationBehavior,
            ) -> ::windows_core::Result<()> {
                if value
                    == $crate::bindings::Microsoft::UI::Composition::AnimationIterationBehavior::Forever
                {
                    self.state.lock().unwrap().iterations = 0;
                }
                Ok(())
            }
            fn IterationCount(&self) -> ::windows_core::Result<i32> {
                Ok(self.state.lock().unwrap().iterations.max(1))
            }
            fn SetIterationCount(&self, value: i32) -> ::windows_core::Result<()> {
                self.state.lock().unwrap().iterations = value.max(0);
                Ok(())
            }
            fn KeyFrameCount(&self) -> ::windows_core::Result<i32> {
                Ok(self.state.lock().unwrap().frames.len() as i32)
            }
            fn StopBehavior(
                &self,
            ) -> ::windows_core::Result<
                $crate::bindings::Microsoft::UI::Composition::AnimationStopBehavior,
            > {
                Ok(
                    $crate::bindings::Microsoft::UI::Composition::AnimationStopBehavior::LeaveCurrentValue,
                )
            }
            fn SetStopBehavior(
                &self,
                _value: $crate::bindings::Microsoft::UI::Composition::AnimationStopBehavior,
            ) -> ::windows_core::Result<()> {
                Ok(())
            }
            fn InsertExpressionKeyFrame(
                &self,
                _keyFrame: f32,
                _value: &::windows_core::HSTRING,
            ) -> ::windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn InsertExpressionKeyFrameWithEasingFunction(
                &self,
                _keyFrame: f32,
                _value: &::windows_core::HSTRING,
                _easing: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::CompositionEasingFunction,
                >,
            ) -> ::windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }
    };
}

macro_rules! insert_impl {
    ($impl_type:ident, $insert:ident, $value:ty, $pack:expr) => {
        impl $insert for $impl_type {
            fn InsertKeyFrame(&self, keyFrame: f32, value: $value) -> ::windows_core::Result<()> {
                insert(&self.state, keyFrame, $pack(value), Easing::Linear);
                Ok(())
            }

            fn InsertKeyFrameWithEasingFunction(
                &self,
                keyFrame: f32,
                value: $value,
                easingFunction: Ref<CompositionEasingFunction>,
            ) -> ::windows_core::Result<()> {
                insert(
                    &self.state,
                    keyFrame,
                    $pack(value),
                    easing_of(easingFunction),
                );
                Ok(())
            }
        }
    };
}

/// The state every animation object holds.
pub(crate) type Shared = Arc<Mutex<Keyframes>>;

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionAnimationBase,
    ICompositionAnimation,
    IKeyFrameAnimation,
    IScalarKeyFrameAnimation
)]
pub struct ScalarAnimation {
    state: Shared,
}

composition_object!(ScalarAnimation_Impl);
animation_shared!(ScalarAnimation_Impl);
insert_impl!(
    ScalarAnimation_Impl,
    IScalarKeyFrameAnimation_Impl,
    f32,
    |value: f32| [value; 4]
);

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionAnimationBase,
    ICompositionAnimation,
    IKeyFrameAnimation,
    IVector2KeyFrameAnimation
)]
pub struct Vector2Animation {
    state: Shared,
}

composition_object!(Vector2Animation_Impl);
animation_shared!(Vector2Animation_Impl);
insert_impl!(
    Vector2Animation_Impl,
    IVector2KeyFrameAnimation_Impl,
    &Vector2,
    |value: &Vector2| [value.x, value.y, 0.0, 0.0]
);

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionAnimationBase,
    ICompositionAnimation,
    IKeyFrameAnimation,
    IVector3KeyFrameAnimation
)]
pub struct Vector3Animation {
    state: Shared,
}

composition_object!(Vector3Animation_Impl);
animation_shared!(Vector3Animation_Impl);
insert_impl!(
    Vector3Animation_Impl,
    IVector3KeyFrameAnimation_Impl,
    &Vector3,
    |value: &Vector3| [value.x, value.y, value.z, 0.0]
);

fn empty() -> Shared {
    Arc::new(Mutex::new(Keyframes::default()))
}

pub fn scalar() -> ::windows_core::Result<ScalarKeyFrameAnimation> {
    let state = empty();
    let animation: IScalarKeyFrameAnimation = ScalarAnimation {
        state: state.clone(),
    }
    .into();
    crate::identity::publish(&animation, &state);
    animation.cast()
}

pub fn vector2() -> ::windows_core::Result<Vector2KeyFrameAnimation> {
    let state = empty();
    let animation: IVector2KeyFrameAnimation = Vector2Animation {
        state: state.clone(),
    }
    .into();
    crate::identity::publish(&animation, &state);
    animation.cast()
}

pub fn vector3() -> ::windows_core::Result<Vector3KeyFrameAnimation> {
    let state = empty();
    let animation: IVector3KeyFrameAnimation = Vector3Animation {
        state: state.clone(),
    }
    .into();
    crate::identity::publish(&animation, &state);
    animation.cast()
}

pub(crate) use animation_shared;
