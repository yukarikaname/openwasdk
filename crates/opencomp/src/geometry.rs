//! The geometry objects a client can attach to a clip or a sprite shape.
//!
//! The renderer draws rounded corners as a radius on the visual, so the rounded rectangle is the
//! only geometry that maps onto a clip. An ellipse and a line have a draw path of their own: they
//! keep their outline here, and a `CompositionSpriteShape` reads it into the draw list. Trim is
//! stored and answered but never drawn: nothing strokes a trimmed path here yet.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionEllipseGeometry, CompositionLineGeometry, CompositionRoundedRectangleGeometry,
    ICompositionEllipseGeometry, ICompositionEllipseGeometry_Impl, ICompositionGeometry,
    ICompositionGeometry_Impl, ICompositionLineGeometry, ICompositionLineGeometry_Impl,
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

/// The trim half every geometry in this module answers.
macro_rules! geometry_trim {
    ($impl_type:ident) => {
        impl ICompositionGeometry_Impl for $impl_type {
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
    };
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
geometry_trim!(RoundedRectangleObject_Impl);

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

/// An ellipse: a centre and one radius per axis, in the shape's own units.
#[derive(Default)]
struct Ellipse {
    center: Vector2,
    radius: Vector2,
    trim: [f32; 3],
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionGeometry,
    ICompositionEllipseGeometry
)]
pub struct EllipseObject {
    shape: Mutex<Ellipse>,
}

composition_object!(EllipseObject_Impl);
geometry_trim!(EllipseObject_Impl);

impl ICompositionEllipseGeometry_Impl for EllipseObject_Impl {
    fn Center(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().center)
    }
    fn SetCenter(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().center = *value;
        Ok(())
    }
    fn Radius(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().radius)
    }
    fn SetRadius(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().radius = *value;
        Ok(())
    }
}

/// A line: its two endpoints, in the shape's own units.
#[derive(Default)]
struct Line {
    start: Vector2,
    end: Vector2,
    trim: [f32; 3],
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionGeometry,
    ICompositionLineGeometry
)]
pub struct LineObject {
    shape: Mutex<Line>,
}

composition_object!(LineObject_Impl);
geometry_trim!(LineObject_Impl);

impl ICompositionLineGeometry_Impl for LineObject_Impl {
    fn Start(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().start)
    }
    fn SetStart(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().start = *value;
        Ok(())
    }
    fn End(&self) -> Result<Vector2> {
        Ok(self.shape.lock().unwrap().end)
    }
    fn SetEnd(&self, value: &Vector2) -> Result<()> {
        self.shape.lock().unwrap().end = *value;
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

/// An ellipse at the origin with no radii; a client sets both before attaching it.
pub(crate) fn ellipse() -> Result<CompositionEllipseGeometry> {
    let object: ICompositionEllipseGeometry = EllipseObject {
        shape: Mutex::new(Ellipse::default()),
    }
    .into();
    object.cast()
}

/// A line from the origin to the origin; a client sets both endpoints.
pub(crate) fn line() -> Result<CompositionLineGeometry> {
    let object: ICompositionLineGeometry = LineObject {
        shape: Mutex::new(Line::default()),
    }
    .into();
    object.cast()
}
