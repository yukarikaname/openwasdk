//! `CompositionRoundedRectangleGeometry`: the one geometry a geometric clip can cut.
//!
//! The renderer draws rounded corners as a radius on the visual, so the rounded rectangle is the
//! only geometry that maps onto it. The rest of the shape is the clip's rectangle, read from the
//! geometry's offset and size. Trim is stored and answered but never drawn: nothing strokes a
//! path here yet.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionRoundedRectangleGeometry, ICompositionGeometry, ICompositionGeometry_Impl,
    ICompositionObject, ICompositionObject2, ICompositionObject4, ICompositionObject5,
    ICompositionRoundedRectangleGeometry, ICompositionRoundedRectangleGeometry_Impl,
};
use crate::object::composition_object;
use windows_core::{Interface, Result};
use windows_numerics::Vector2;

#[derive(Default)]
struct Shape {
    offset: Vector2,
    size: Vector2,
    /// One radius per axis; the renderer's corner is a circle, so only its width is used.
    corner_radius: Vector2,
    /// Start, end and offset of the trimmed part of the outline.
    trim: [f32; 3],
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionGeometry,
    ICompositionRoundedRectangleGeometry
)]
pub struct RoundedRectangleObject {
    shape: Mutex<Shape>,
}

composition_object!(RoundedRectangleObject_Impl);

impl ICompositionGeometry_Impl for RoundedRectangleObject_Impl {
    fn TrimEnd(&self) -> Result<f32> {
        Ok(self.shape.lock().unwrap().trim[1])
    }
    fn SetTrimEnd(&self, value: f32) -> Result<()> {
        self.shape.lock().unwrap().trim[1] = value;
        Ok(())
    }
    fn TrimOffset(&self) -> Result<f32> {
        Ok(self.shape.lock().unwrap().trim[2])
    }
    fn SetTrimOffset(&self, value: f32) -> Result<()> {
        self.shape.lock().unwrap().trim[2] = value;
        Ok(())
    }
    fn TrimStart(&self) -> Result<f32> {
        Ok(self.shape.lock().unwrap().trim[0])
    }
    fn SetTrimStart(&self, value: f32) -> Result<()> {
        self.shape.lock().unwrap().trim[0] = value;
        Ok(())
    }
}

impl ICompositionRoundedRectangleGeometry_Impl for RoundedRectangleObject_Impl {
    fn CornerRadius(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().corner_radius)
    }
    fn SetCornerRadius(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().corner_radius = *value;
        Ok(())
    }
    fn Offset(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().offset)
    }
    fn SetOffset(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().offset = *value;
        Ok(())
    }
    fn Size(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().size)
    }
    fn SetSize(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().size = *value;
        Ok(())
    }
}

/// A zero-sized rounded rectangle; a client sizes it before attaching the clip.
pub(crate) fn rounded_rectangle() -> Result<CompositionRoundedRectangleGeometry> {
    let object: ICompositionRoundedRectangleGeometry = RoundedRectangleObject {
        shape: Mutex::new(Shape::default()),
    }
    .into();
    object.cast()
}
