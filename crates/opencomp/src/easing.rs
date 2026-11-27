//! Easing functions: the objects a client hands to an animation.
//!
//! Nothing samples them yet — `ICompositionObject.StartAnimation` still answers
//! `E_NOTIMPL` — but a client builds its easing functions before it builds the
//! animation, and a throw there would stop it earlier than necessary.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CubicBezierEasingFunction, ICompositionEasingFunction, ICompositionEasingFunction_Impl,
    ICompositionObject, ICompositionObject2, ICompositionObject4, ICompositionObject5,
    ICubicBezierEasingFunction, ICubicBezierEasingFunction_Impl, ILinearEasingFunction,
    ILinearEasingFunction_Impl, LinearEasingFunction,
};
use crate::object::composition_object;
use crate::object::prelude::*;
use windows_core::{Interface, Result};
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
