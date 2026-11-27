//! `StepEasingFunction`: an easing function that moves in discrete steps.
//!
//! The object is a bag of properties a client sets before handing it to a key
//! frame; sampling happens later, from the copy `animation::easing_of` makes.
//! The metadata this crate binds has no jump-gap position, so the core records
//! the gap as the documented default and the four remaining properties
//! (`InitialStep`, `FinalStep` and the two single-frame flags) are kept for a
//! round trip even though the renderer's step curve does not use them yet.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    ICompositionEasingFunction, ICompositionEasingFunction_Impl, ICompositionObject,
    ICompositionObject2, ICompositionObject4, ICompositionObject5, IStepEasingFunction,
    IStepEasingFunction_Impl, StepEasingFunction,
};
use crate::object::composition_object;
use windows_core::{Interface, Result};

/// A step count of zero has no meaning; the composition API documents one as
/// the smallest legal value. Clamp in one place so the getter and the sampler
/// agree.
const MIN_STEP_COUNT: i32 = 1;

#[derive(Clone, Copy, Debug)]
struct StepState {
    final_step: i32,
    initial_step: i32,
    final_step_single_frame: bool,
    initial_step_single_frame: bool,
    step_count: i32,
}

impl Default for StepState {
    fn default() -> Self {
        Self {
            final_step: 0,
            initial_step: 0,
            final_step_single_frame: false,
            initial_step_single_frame: false,
            step_count: MIN_STEP_COUNT,
        }
    }
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionEasingFunction,
    IStepEasingFunction
)]
pub struct StepEasingObject {
    state: Mutex<StepState>,
}

composition_object!(StepEasingObject_Impl);

impl ICompositionEasingFunction_Impl for StepEasingObject_Impl {}

impl IStepEasingFunction_Impl for StepEasingObject_Impl {
    fn FinalStep(&self) -> Result<i32> {
        Ok(self.state.lock().unwrap().final_step)
    }

    fn SetFinalStep(&self, value: i32) -> Result<()> {
        self.state.lock().unwrap().final_step = value;
        Ok(())
    }

    fn InitialStep(&self) -> Result<i32> {
        Ok(self.state.lock().unwrap().initial_step)
    }

    fn SetInitialStep(&self, value: i32) -> Result<()> {
        self.state.lock().unwrap().initial_step = value;
        Ok(())
    }

    fn IsFinalStepSingleFrame(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().final_step_single_frame)
    }

    fn SetIsFinalStepSingleFrame(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().final_step_single_frame = value;
        Ok(())
    }

    fn IsInitialStepSingleFrame(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().initial_step_single_frame)
    }

    fn SetIsInitialStepSingleFrame(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().initial_step_single_frame = value;
        Ok(())
    }

    fn StepCount(&self) -> Result<i32> {
        Ok(self.state.lock().unwrap().step_count)
    }

    fn SetStepCount(&self, value: i32) -> Result<()> {
        self.state.lock().unwrap().step_count = value.max(MIN_STEP_COUNT);
        Ok(())
    }
}

pub fn create() -> Result<StepEasingFunction> {
    let easing: IStepEasingFunction = StepEasingObject {
        state: Mutex::new(StepState::default()),
    }
    .into();
    easing.cast()
}

pub fn create_with_step_count(step_count: i32) -> Result<StepEasingFunction> {
    let easing: IStepEasingFunction = StepEasingObject {
        state: Mutex::new(StepState {
            step_count: step_count.max(MIN_STEP_COUNT),
            ..StepState::default()
        }),
    }
    .into();
    easing.cast()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{CompositionEasingFunction, ICompositor2};

    fn compositor() -> ICompositor2 {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    #[test]
    fn every_step_property_round_trips() {
        let compositor = compositor();
        let function = compositor.CreateStepEasingFunction().unwrap();
        let step: IStepEasingFunction = function.cast().unwrap();

        assert_eq!(step.StepCount().unwrap(), 1);
        assert_eq!(step.InitialStep().unwrap(), 0);
        assert_eq!(step.FinalStep().unwrap(), 0);
        assert!(!step.IsInitialStepSingleFrame().unwrap());
        assert!(!step.IsFinalStepSingleFrame().unwrap());

        step.SetStepCount(6).unwrap();
        step.SetInitialStep(1).unwrap();
        step.SetFinalStep(4).unwrap();
        step.SetIsInitialStepSingleFrame(true).unwrap();
        step.SetIsFinalStepSingleFrame(true).unwrap();

        assert_eq!(step.StepCount().unwrap(), 6);
        assert_eq!(step.InitialStep().unwrap(), 1);
        assert_eq!(step.FinalStep().unwrap(), 4);
        assert!(step.IsInitialStepSingleFrame().unwrap());
        assert!(step.IsFinalStepSingleFrame().unwrap());
    }

    #[test]
    fn the_factory_keeps_the_requested_step_count() {
        let compositor = compositor();
        let function = compositor.CreateStepEasingFunctionWithStepCount(3).unwrap();
        let step: IStepEasingFunction = function.cast().unwrap();
        assert_eq!(step.StepCount().unwrap(), 3);
    }

    #[test]
    fn a_step_count_of_zero_is_raised_to_one() {
        let compositor = compositor();
        let function = compositor.CreateStepEasingFunctionWithStepCount(0).unwrap();
        let step: IStepEasingFunction = function.cast().unwrap();
        assert_eq!(step.StepCount().unwrap(), 1);
        step.SetStepCount(-2).unwrap();
        assert_eq!(step.StepCount().unwrap(), 1);
    }

    #[test]
    fn a_step_function_reaches_the_core_curve() {
        let compositor = compositor();
        let function = compositor.CreateStepEasingFunctionWithStepCount(4).unwrap();
        let easing: CompositionEasingFunction = function.cast().unwrap();
        let curve = crate::animation::easing_of(windows_core::InRef::from(&easing));
        assert_eq!(
            curve,
            opencomp_core::Easing::Step {
                steps: 4,
                jump: 0.5,
            }
        );
    }
}
