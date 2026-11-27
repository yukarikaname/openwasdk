//! Easing functions: the objects a client hands to an animation.
//!
//! A key frame carries its easing function as a curve the animation sampler applies
//! between that frame and the next. `StartAnimation` resolves the object here, so a
//! function a client built is the curve its animation actually runs.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    BackEasingFunction, BounceEasingFunction, CircleEasingFunction, CompositionEasingFunctionMode,
    Compositor, CubicBezierEasingFunction, ElasticEasingFunction, ExponentialEasingFunction,
    ICompositionEasingFunction, ICompositionEasingFunction_Impl, ICompositionEasingFunctionStatics,
    ICompositionEasingFunctionStatics_Impl, ICompositionObject, ICompositionObject2,
    ICompositionObject4, ICompositionObject5, ICubicBezierEasingFunction,
    ICubicBezierEasingFunction_Impl, ILinearEasingFunction, ILinearEasingFunction_Impl,
    LinearEasingFunction, PowerEasingFunction, SineEasingFunction, StepEasingFunction,
};
use crate::object::composition_object;
use windows_core::{Interface, Ref, Result};
use windows_numerics::Vector2;

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionEasingFunction,
    ILinearEasingFunction
)]
pub struct LinearEasingObject;

composition_object!(LinearEasingObject_Impl);

impl ICompositionEasingFunction_Impl for LinearEasingObject_Impl {}

impl ILinearEasingFunction_Impl for LinearEasingObject_Impl {}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionEasingFunction,
    ICubicBezierEasingFunction
)]
pub struct CubicBezierObject {
    control_points: Mutex<(Vector2, Vector2)>,
}

composition_object!(CubicBezierObject_Impl);

impl ICompositionEasingFunction_Impl for CubicBezierObject_Impl {}

impl ICubicBezierEasingFunction_Impl for CubicBezierObject_Impl {
    fn ControlPoint1(&self) -> Result<Vector2> {
        Ok(self.control_points.lock().unwrap().0)
    }

    fn ControlPoint2(&self) -> Result<Vector2> {
        Ok(self.control_points.lock().unwrap().1)
    }
}

pub fn linear() -> Result<LinearEasingFunction> {
    let easing: ILinearEasingFunction = LinearEasingObject.into();
    easing.cast()
}

pub fn cubic_bezier(
    control_point1: Vector2,
    control_point2: Vector2,
) -> Result<CubicBezierEasingFunction> {
    let easing: ICubicBezierEasingFunction = CubicBezierObject {
        control_points: Mutex::new((control_point1, control_point2)),
    }
    .into();
    easing.cast()
}

/// The static entry points, reached without a compositor object in hand.
///
/// `CompositionEasingFunction.Create…` is what the engine calls for its own transitions
/// before any object of ours exists, so the interface has to answer through a factory
/// rather than through an activated instance. The two curves this crate can build are
/// built; the rest refuse rather than pretend.
#[windows_core::implement(ICompositionEasingFunctionStatics)]
pub struct EasingStatics;

impl ICompositionEasingFunctionStatics_Impl for EasingStatics_Impl {
    fn CreateCubicBezierEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        control_point1: &Vector2,
        control_point2: &Vector2,
    ) -> Result<CubicBezierEasingFunction> {
        cubic_bezier(*control_point1, *control_point2)
    }

    fn CreateLinearEasingFunction(&self, _owner: Ref<Compositor>) -> Result<LinearEasingFunction> {
        linear()
    }

    fn CreateStepEasingFunction(&self, _owner: Ref<Compositor>) -> Result<StepEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateStepEasingFunctionWithStepCount(
        &self,
        _owner: Ref<Compositor>,
        _step_count: i32,
    ) -> Result<StepEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateBackEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
        _amplitude: f32,
    ) -> Result<BackEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateBounceEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
        _bounces: i32,
        _bounciness: f32,
    ) -> Result<BounceEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateCircleEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
    ) -> Result<CircleEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateElasticEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
        _oscillations: i32,
        _springiness: f32,
    ) -> Result<ElasticEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateExponentialEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
        _exponent: f32,
    ) -> Result<ExponentialEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreatePowerEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
        _power: f32,
    ) -> Result<PowerEasingFunction> {
        crate::compositor::not_implemented()
    }

    fn CreateSineEasingFunction(
        &self,
        _owner: Ref<Compositor>,
        _mode: CompositionEasingFunctionMode,
    ) -> Result<SineEasingFunction> {
        crate::compositor::not_implemented()
    }
}

/// The statics object the factory hands out for this class.
pub(crate) fn statics() -> Result<ICompositionEasingFunctionStatics> {
    let statics: ICompositionEasingFunctionStatics = EasingStatics.into();
    statics.cast()
}
