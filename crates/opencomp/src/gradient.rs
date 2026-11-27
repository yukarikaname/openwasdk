//! `CompositionLinearGradientBrush`, `CompositionRadialGradientBrush` and their stops.
//!
//! A brush keeps an identity and a list of stops; the renderer reads them when a sprite is
//! attached, the same snapshot shortcut `CompositionColorBrush` takes. `ColorStops` returns a
//! live `IVector`, so appending a stop before `SetBrush` reaches the tree even though the brush
//! itself is only sampled once.
//!
//! Only what the draw list can paint is real. The gradient transform properties (anchor point,
//! centre point, offset, rotation, scale, transform matrix) are not modelled, and neither is a
//! non-default gradient origin: their getters and setters answer `E_NOTIMPL` rather than
//! returning a value that changes nothing. Extend mode is Clamp and interpolation is sRGB/Auto,
//! which is what both backends do; any other choice is refused for the same reason.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionColorGradientStop, CompositionColorGradientStopCollection, CompositionColorSpace,
    CompositionGradientExtendMode, CompositionLinearGradientBrush, CompositionMappingMode,
    CompositionRadialGradientBrush, ICompositionBrush, ICompositionBrush_Impl,
    ICompositionColorGradientStop, ICompositionGradientBrush, ICompositionGradientBrush_Impl,
    ICompositionGradientBrush2, ICompositionGradientBrush2_Impl, ICompositionLinearGradientBrush,
    ICompositionLinearGradientBrush_Impl, ICompositionObject, ICompositionObject2,
    ICompositionObject4, ICompositionObject5, ICompositionRadialGradientBrush,
    ICompositionRadialGradientBrush_Impl,
};
use crate::brush::to_rgba;
use crate::gradient_collection::collection;
pub(crate) use crate::gradient_collection::stop;
use crate::object::composition_object;
use opencomp_core::{Gradient, GradientShape, GradientStop};
use windows_core::{Interface, Result};
use windows_numerics::Vector2;

/// Brush identities start at one so zero never names a brush.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

fn next_id() -> u32 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    if id == 0 { 1 } else { id }
}

/// The properties of a gradient brush that the renderer does not model.
///
/// One helper rather than a body per property keeps the stub table honest about what is
/// missing: the feature is the transform of a gradient, not each of its six spellings.
fn unsupported<T>() -> Result<T> {
    crate::object::not_implemented()
}

/// A brush's own units, before `MappingMode` turns them into node units.
#[derive(Clone, Copy)]
pub enum Shape {
    Linear { start: Vector2, end: Vector2 },
    Radial { center: Vector2, radius: Vector2 },
}

/// Everything one gradient brush keeps.
pub struct BrushState {
    id: u32,
    mapping: CompositionMappingMode,
    extend: CompositionGradientExtendMode,
    interpolation: CompositionColorSpace,
    shape: Shape,
    stops: Arc<Mutex<Vec<CompositionColorGradientStop>>>,
}

/// What `SetBrush` copies into the node. Stops stay behind an `Arc`, so reading them in a later
/// frame sees an appended stop; the shape is resolved against the sprite size at snapshot time.
#[derive(Clone)]
pub struct Spec {
    pub id: u32,
    pub mapping: CompositionMappingMode,
    pub shape: Shape,
    pub stops: Arc<Mutex<Vec<CompositionColorGradientStop>>>,
}

impl Spec {
    /// The gradient in the node's own units, or `None` when a stop cannot be read.
    pub fn resolve(&self, size: Vector2) -> Option<Gradient> {
        let point = |value: Vector2| {
            if self.mapping == CompositionMappingMode::Relative {
                [value.x * size.x, value.y * size.y]
            } else {
                [value.x, value.y]
            }
        };
        let shape = match self.shape {
            Shape::Linear { start, end } => GradientShape::Linear {
                start: point(start),
                end: point(end),
            },
            Shape::Radial { center, radius } => GradientShape::Radial {
                center: point(center),
                radius: point(radius),
            },
        };
        let stops = self.stops.lock().ok()?;
        let stops = stops.iter().filter_map(read_stop).collect();
        Some(Gradient {
            id: self.id,
            stops,
            shape,
        })
    }
}

/// One stop as the draw list holds it: linear light, the same form as a colour brush.
fn read_stop(stop: &CompositionColorGradientStop) -> Option<GradientStop> {
    let stop: &ICompositionColorGradientStop = stop;
    Some(GradientStop {
        offset: stop.Offset().ok()?,
        color: to_rgba(stop.Color().ok()?),
    })
}

/// The brush behind `brush`, if this DLL made it.
pub fn spec(brush: &crate::bindings::Microsoft::UI::Composition::CompositionBrush) -> Option<Spec> {
    let state = crate::identity::lookup::<_, Mutex<BrushState>>(brush)?;
    let state = state.lock().ok()?;
    Some(Spec {
        id: state.id,
        mapping: state.mapping,
        shape: state.shape,
        stops: state.stops.clone(),
    })
}

/// A linear gradient brush with no stops and the documented default ramp.
pub fn linear() -> Result<CompositionLinearGradientBrush> {
    let state = Arc::new(Mutex::new(BrushState {
        id: next_id(),
        mapping: CompositionMappingMode::Relative,
        extend: CompositionGradientExtendMode::Clamp,
        interpolation: CompositionColorSpace::Auto,
        // WinUI's default runs diagonally across the brush's bounds.
        shape: Shape::Linear {
            start: Vector2 { x: 0.0, y: 0.0 },
            end: Vector2 { x: 1.0, y: 1.0 },
        },
        stops: Arc::new(Mutex::new(Vec::new())),
    }));
    let object: ICompositionLinearGradientBrush = LinearGradientBrushObject {
        state: state.clone(),
    }
    .into();
    let brush: CompositionLinearGradientBrush = object.cast()?;
    crate::identity::publish(&brush, &state);
    Ok(brush)
}

/// A radial gradient brush with no stops, centred on the brush and reaching its edges.
pub fn radial() -> Result<CompositionRadialGradientBrush> {
    let state = Arc::new(Mutex::new(BrushState {
        id: next_id(),
        mapping: CompositionMappingMode::Relative,
        extend: CompositionGradientExtendMode::Clamp,
        interpolation: CompositionColorSpace::Auto,
        shape: Shape::Radial {
            center: Vector2 { x: 0.5, y: 0.5 },
            radius: Vector2 { x: 0.5, y: 0.5 },
        },
        stops: Arc::new(Mutex::new(Vec::new())),
    }));
    let object: ICompositionRadialGradientBrush = RadialGradientBrushObject {
        state: state.clone(),
    }
    .into();
    let brush: CompositionRadialGradientBrush = object.cast()?;
    crate::identity::publish(&brush, &state);
    Ok(brush)
}

/// The two brush classes share every `ICompositionGradientBrush` method.
macro_rules! gradient_brush_traits {
    ($impl_type:ident) => {
        impl ICompositionBrush_Impl for $impl_type {}

        impl ICompositionGradientBrush_Impl for $impl_type {
            fn AnchorPoint(&self) -> Result<Vector2> {
                unsupported()
            }
            fn SetAnchorPoint(&self, _value: &Vector2) -> Result<()> {
                unsupported()
            }
            fn CenterPoint(&self) -> Result<Vector2> {
                unsupported()
            }
            fn SetCenterPoint(&self, _value: &Vector2) -> Result<()> {
                unsupported()
            }
            fn ColorStops(&self) -> Result<CompositionColorGradientStopCollection> {
                let stops = self.state.lock().unwrap().stops.clone();
                collection(stops)
            }
            fn ExtendMode(&self) -> Result<CompositionGradientExtendMode> {
                Ok(self.state.lock().unwrap().extend)
            }
            fn SetExtendMode(&self, value: CompositionGradientExtendMode) -> Result<()> {
                // Both backends clamp; Wrap and Mirror would need a shader the preview does
                // not have and a D2D extend mode the draw list cannot express.
                if value != CompositionGradientExtendMode::Clamp {
                    return unsupported();
                }
                self.state.lock().unwrap().extend = value;
                Ok(())
            }
            fn InterpolationSpace(&self) -> Result<CompositionColorSpace> {
                Ok(self.state.lock().unwrap().interpolation)
            }
            fn SetInterpolationSpace(&self, value: CompositionColorSpace) -> Result<()> {
                // Auto and Rgb both name the sRGB interpolation D2D and the ramp use.
                if value != CompositionColorSpace::Auto && value != CompositionColorSpace::Rgb {
                    return unsupported();
                }
                self.state.lock().unwrap().interpolation = value;
                Ok(())
            }
            fn Offset(&self) -> Result<Vector2> {
                unsupported()
            }
            fn SetOffset(&self, _value: &Vector2) -> Result<()> {
                unsupported()
            }
            fn RotationAngle(&self) -> Result<f32> {
                unsupported()
            }
            fn SetRotationAngle(&self, _value: f32) -> Result<()> {
                unsupported()
            }
            fn RotationAngleInDegrees(&self) -> Result<f32> {
                unsupported()
            }
            fn SetRotationAngleInDegrees(&self, _value: f32) -> Result<()> {
                unsupported()
            }
            fn Scale(&self) -> Result<Vector2> {
                unsupported()
            }
            fn SetScale(&self, _value: &Vector2) -> Result<()> {
                unsupported()
            }
            fn TransformMatrix(&self) -> Result<windows_numerics::Matrix3x2> {
                unsupported()
            }
            fn SetTransformMatrix(&self, _value: &windows_numerics::Matrix3x2) -> Result<()> {
                unsupported()
            }
        }

        impl ICompositionGradientBrush2_Impl for $impl_type {
            fn MappingMode(&self) -> Result<CompositionMappingMode> {
                Ok(self.state.lock().unwrap().mapping)
            }
            fn SetMappingMode(&self, value: CompositionMappingMode) -> Result<()> {
                if value != CompositionMappingMode::Absolute
                    && value != CompositionMappingMode::Relative
                {
                    return unsupported();
                }
                self.state.lock().unwrap().mapping = value;
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
    ICompositionBrush,
    ICompositionGradientBrush,
    ICompositionGradientBrush2,
    ICompositionLinearGradientBrush
)]
pub struct LinearGradientBrushObject {
    state: Arc<Mutex<BrushState>>,
}

composition_object!(LinearGradientBrushObject_Impl);
gradient_brush_traits!(LinearGradientBrushObject_Impl);

impl ICompositionLinearGradientBrush_Impl for LinearGradientBrushObject_Impl {
    fn EndPoint(&self) -> Result<Vector2> {
        match self.state.lock().unwrap().shape {
            Shape::Linear { end, .. } => Ok(end),
            Shape::Radial { .. } => unsupported(),
        }
    }
    fn SetEndPoint(&self, value: &Vector2) -> Result<()> {
        match &mut self.state.lock().unwrap().shape {
            Shape::Linear { end, .. } => {
                *end = *value;
                Ok(())
            }
            Shape::Radial { .. } => unsupported(),
        }
    }
    fn StartPoint(&self) -> Result<Vector2> {
        match self.state.lock().unwrap().shape {
            Shape::Linear { start, .. } => Ok(start),
            Shape::Radial { .. } => unsupported(),
        }
    }
    fn SetStartPoint(&self, value: &Vector2) -> Result<()> {
        match &mut self.state.lock().unwrap().shape {
            Shape::Linear { start, .. } => {
                *start = *value;
                Ok(())
            }
            Shape::Radial { .. } => unsupported(),
        }
    }
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionBrush,
    ICompositionGradientBrush,
    ICompositionGradientBrush2,
    ICompositionRadialGradientBrush
)]
pub struct RadialGradientBrushObject {
    state: Arc<Mutex<BrushState>>,
}

composition_object!(RadialGradientBrushObject_Impl);
gradient_brush_traits!(RadialGradientBrushObject_Impl);

impl ICompositionRadialGradientBrush_Impl for RadialGradientBrushObject_Impl {
    fn EllipseCenter(&self) -> Result<Vector2> {
        match self.state.lock().unwrap().shape {
            Shape::Radial { center, .. } => Ok(center),
            Shape::Linear { .. } => unsupported(),
        }
    }
    fn SetEllipseCenter(&self, value: &Vector2) -> Result<()> {
        match &mut self.state.lock().unwrap().shape {
            Shape::Radial { center, .. } => {
                *center = *value;
                Ok(())
            }
            Shape::Linear { .. } => unsupported(),
        }
    }
    fn EllipseRadius(&self) -> Result<Vector2> {
        match self.state.lock().unwrap().shape {
            Shape::Radial { radius, .. } => Ok(radius),
            Shape::Linear { .. } => unsupported(),
        }
    }
    fn SetEllipseRadius(&self, value: &Vector2) -> Result<()> {
        match &mut self.state.lock().unwrap().shape {
            Shape::Radial { radius, .. } => {
                *radius = *value;
                Ok(())
            }
            Shape::Linear { .. } => unsupported(),
        }
    }
    fn GradientOriginOffset(&self) -> Result<Vector2> {
        // The default origin is the ellipse centre. An offset would need a second centre in
        // the draw list; the getter reports the documented default, the setter refuses.
        Ok(Vector2 { x: 0.0, y: 0.0 })
    }
    fn SetGradientOriginOffset(&self, _value: &Vector2) -> Result<()> {
        unsupported()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{
        ICompositor, ICompositor4, ICompositorWithRadialGradient, ISpriteVisual, IVisual,
    };
    use crate::bindings::Windows::UI::Color;

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    #[test]
    fn a_linear_gradient_brush_reaches_the_snapshot() {
        let compositor = compositor();
        let factory: ICompositor4 = compositor.cast().unwrap();
        let brush = factory.CreateLinearGradientBrush().unwrap();
        let gradient_brush: ICompositionGradientBrush = brush.cast().unwrap();
        let stops: windows_collections::IVector<CompositionColorGradientStop> =
            gradient_brush.ColorStops().unwrap().cast().unwrap();
        let black = factory.CreateColorGradientStopWithOffsetAndColor(
            0.0,
            Color {
                a: 255,
                r: 0,
                g: 0,
                b: 0,
            },
        );
        let white = factory.CreateColorGradientStopWithOffsetAndColor(
            1.0,
            Color {
                a: 255,
                r: 255,
                g: 255,
                b: 255,
            },
        );
        stops.Append(&black.unwrap()).unwrap();
        stops.Append(&white.unwrap()).unwrap();
        let start = Vector2 { x: 0.0, y: 0.0 };
        let end = Vector2 { x: 1.0, y: 0.0 };
        let linear: crate::bindings::Microsoft::UI::Composition::ICompositionLinearGradientBrush =
            brush.cast().unwrap();
        linear.SetStartPoint(start).unwrap();
        linear.SetEndPoint(end).unwrap();
        assert_eq!(linear.StartPoint().unwrap(), start);

        let sprite = compositor.CreateSpriteVisual().unwrap();
        let visual: IVisual = sprite.cast().unwrap();
        visual.SetSize(Vector2 { x: 40.0, y: 20.0 }).unwrap();
        let brush: crate::bindings::Microsoft::UI::Composition::CompositionBrush =
            brush.cast().unwrap();
        let sprite_visual: ISpriteVisual = sprite.cast().unwrap();
        sprite_visual.SetBrush(&brush).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        let gradient = tree.gradient.expect("a gradient");
        assert_eq!(gradient.stops.len(), 2);
        assert_eq!(gradient.stops[1].offset, 1.0);
        assert_eq!(
            gradient.shape,
            GradientShape::Linear {
                start: [0.0, 0.0],
                end: [40.0, 0.0],
            }
        );
        assert_eq!(tree.color, [1.0; 4]);
    }

    #[test]
    fn mapping_mode_absolute_leaves_the_coordinates_alone() {
        let compositor = compositor();
        let factory: ICompositorWithRadialGradient = compositor.cast().unwrap();
        let brush = factory.CreateRadialGradientBrush().unwrap();
        let mode: ICompositionGradientBrush2 = brush.cast().unwrap();
        mode.SetMappingMode(CompositionMappingMode::Absolute)
            .unwrap();
        assert_eq!(
            mode.MappingMode().unwrap(),
            CompositionMappingMode::Absolute
        );
        let radial: crate::bindings::Microsoft::UI::Composition::ICompositionRadialGradientBrush =
            brush.cast().unwrap();
        radial
            .SetEllipseCenter(Vector2 { x: 10.0, y: 20.0 })
            .unwrap();
        radial.SetEllipseRadius(Vector2 { x: 5.0, y: 5.0 }).unwrap();
        let brush: crate::bindings::Microsoft::UI::Composition::CompositionBrush =
            brush.cast().unwrap();
        let spec = spec(&brush).unwrap();
        let gradient = spec.resolve(Vector2 { x: 100.0, y: 100.0 }).unwrap();
        assert_eq!(
            gradient.shape,
            GradientShape::Radial {
                center: [10.0, 20.0],
                radius: [5.0, 5.0],
            }
        );
    }
}
