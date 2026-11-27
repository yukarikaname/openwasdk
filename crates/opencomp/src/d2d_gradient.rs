//! The D2D gradient brushes a presenter caches.
//!
//! A brush is a device object keyed by the identity the draw list carries, the same shape as the
//! bitmap cache in [`crate::d2d`]. Stops only change when the WinRT brush changes them, so the
//! stop collection is built once; the geometry is updated per draw, because one brush can paint
//! sprites of different sizes under `Relative` mapping.
//!
//! Two contracts shape this: `ID2D1DeviceContext::CreateGradientStopCollection` takes stops
//! ordered by position, each in `[0, 1]`, and returns an immutable collection; and
//! `ID2D1Brush::SetTransform` maps brush space to the render target's coordinate space. The
//! stops are therefore sorted and clamped before creation, and the per-draw transform is the
//! node's local-to-device affine recovered from the draw's corners.

use std::collections::{HashMap, HashSet};

use opencomp_core::{Draw, Gradient, GradientShape, GradientStop};
use windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BRUSH_PROPERTIES, D2D1_BUFFER_PRECISION_8BPC_UNORM,
    D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT, D2D1_COLOR_SPACE_SRGB, D2D1_EXTEND_MODE_CLAMP,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, ID2D1Brush,
    ID2D1DeviceContext, ID2D1GradientStopCollection1, ID2D1LinearGradientBrush,
    ID2D1RadialGradientBrush,
};

use crate::d2d::color;

/// A device brush for one gradient brush, keyed by the identity the draw list carries.
struct CachedGradient {
    stops: Vec<GradientStop>,
    brush: GradientBrush,
}

pub(crate) enum GradientBrush {
    Linear(ID2D1LinearGradientBrush),
    Radial(ID2D1RadialGradientBrush),
}

impl Clone for GradientBrush {
    fn clone(&self) -> Self {
        match self {
            Self::Linear(brush) => Self::Linear(brush.clone()),
            Self::Radial(brush) => Self::Radial(brush.clone()),
        }
    }
}

impl GradientBrush {
    /// The common brush interface both classes derive from.
    pub(crate) fn base(&self) -> &ID2D1Brush {
        match self {
            Self::Linear(brush) => brush,
            Self::Radial(brush) => brush,
        }
    }
}

/// The device brushes a frame paints, rebuilt when a brush's stops change.
pub(crate) struct GradientCache {
    d2d: ID2D1DeviceContext,
    brushes: HashMap<u32, CachedGradient>,
}

impl GradientCache {
    pub(crate) fn new(d2d: ID2D1DeviceContext) -> Self {
        Self {
            d2d,
            brushes: HashMap::new(),
        }
    }

    /// The device brush for `gradient`, created once per brush identity.
    ///
    /// A stop change rebuilds the brush, because an `ID2D1GradientStopCollection` is immutable.
    /// The geometry is not part of the key: one brush can paint sprites of different sizes with
    /// relative coordinates, so it is set per draw by [`Self::paint`].
    pub(crate) fn brush(&mut self, gradient: &Gradient) -> Option<GradientBrush> {
        let stops = effective_stops(gradient);
        let unchanged = self
            .brushes
            .get(&gradient.id)
            .is_some_and(|cached| cached.stops == stops);
        if !unchanged {
            let device_stops: Vec<D2D1_GRADIENT_STOP> = stops
                .iter()
                .map(|stop| D2D1_GRADIENT_STOP {
                    position: stop.offset.clamp(0.0, 1.0),
                    color: color(stop.color),
                })
                .collect();
            let brush = self.create(gradient, &device_stops)?;
            self.brushes.insert(
                gradient.id,
                CachedGradient {
                    stops,
                    brush: brush.clone(),
                },
            );
        }
        self.brushes
            .get(&gradient.id)
            .map(|cached| cached.brush.clone())
    }

    /// Build one D2D gradient brush over `stops`, already in sRGB.
    fn create(&self, gradient: &Gradient, stops: &[D2D1_GRADIENT_STOP]) -> Option<GradientBrush> {
        // Safety: the slice is non-empty and ordered by position; D2D copies it. Both the pre-
        // and post-interpolation space are sRGB and the interpolation is straight alpha, which
        // is the same space the preview's CPU ramp interpolates in.
        let collection: ID2D1GradientStopCollection1 = unsafe {
            self.d2d.CreateGradientStopCollection(
                stops,
                D2D1_COLOR_SPACE_SRGB,
                D2D1_COLOR_SPACE_SRGB,
                D2D1_BUFFER_PRECISION_8BPC_UNORM,
                D2D1_EXTEND_MODE_CLAMP,
                D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
            )
        }
        .ok()?;
        let brush = match gradient.shape {
            GradientShape::Linear { start, end } => {
                let properties = linear_properties(start, end);
                // Safety: the properties outlive the call and the brush owns a reference to
                // the stop collection.
                let brush = unsafe {
                    self.d2d
                        .CreateLinearGradientBrush(&properties, None, &*collection)
                }
                .ok()?;
                GradientBrush::Linear(brush)
            }
            GradientShape::Radial { center, radius } => {
                let properties = radial_properties(center, radius);
                let brush = unsafe {
                    self.d2d
                        .CreateRadialGradientBrush(&properties, None, &*collection)
                }
                .ok()?;
                GradientBrush::Radial(brush)
            }
        };
        Some(brush)
    }

    /// Point a cached brush at this draw's ramp and opacity.
    pub(crate) fn paint(
        &self,
        brush: &GradientBrush,
        gradient: &Gradient,
        draw: &Draw,
        scale: f32,
    ) {
        match (brush, gradient.shape) {
            (GradientBrush::Linear(brush), GradientShape::Linear { start, end }) => {
                let properties = linear_properties(start, end);
                // Safety: the brush belongs to this device and is alive for the call.
                unsafe {
                    brush.SetStartPoint(properties.startPoint);
                    brush.SetEndPoint(properties.endPoint);
                }
            }
            (GradientBrush::Radial(brush), GradientShape::Radial { center, radius }) => {
                let properties = radial_properties(center, radius);
                unsafe {
                    brush.SetCenter(properties.center);
                    brush.SetGradientOriginOffset(properties.gradientOriginOffset);
                    brush.SetRadiusX(properties.radiusX);
                    brush.SetRadiusY(properties.radiusY);
                }
            }
            // A brush keeps its class, so the two shapes cannot cross.
            _ => {}
        }
        let properties = local_transform(draw, scale);
        // Safety: both classes derive from ID2D1Brush and are alive; the transform maps the
        // node's own units to device pixels. The colour's alpha is the opacity.
        unsafe {
            brush.base().SetTransform(&properties.transform);
            brush.base().SetOpacity(draw.color[3]);
        }
    }

    /// Drop every brush this frame no longer paints, and with it the D2D object.
    pub(crate) fn retain(&mut self, draws: &[Draw]) {
        let used: HashSet<u32> = draws
            .iter()
            .filter_map(|draw| draw.gradient.as_ref().map(|gradient| gradient.id))
            .collect();
        self.brushes.retain(|id, _| used.contains(id));
    }
}

/// The stops with the one-stop case widened to a flat two-stop ramp.
///
/// `CreateGradientStopCollection` takes positions in `[0, 1]`; a client that set an offset
/// outside that would otherwise make the whole brush fail to draw.
fn effective_stops(gradient: &Gradient) -> Vec<GradientStop> {
    let mut stops = gradient.sorted_stops();
    if stops.len() == 1 {
        let only = stops[0];
        stops = vec![
            GradientStop {
                offset: 0.0,
                ..only
            },
            GradientStop {
                offset: 1.0,
                ..only
            },
        ];
    }
    stops
}

fn linear_properties(start: [f32; 2], end: [f32; 2]) -> D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
    let mut properties = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES::default();
    properties.startPoint.X = start[0];
    properties.startPoint.Y = start[1];
    properties.endPoint.X = end[0];
    properties.endPoint.Y = end[1];
    properties
}

fn radial_properties(center: [f32; 2], radius: [f32; 2]) -> D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
    let mut properties = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES::default();
    properties.center.X = center[0];
    properties.center.Y = center[1];
    properties.radiusX = radius[0].max(0.0);
    properties.radiusY = radius[1].max(0.0);
    properties
}

/// The affine from the node's own units to device pixels, read back from the draw's corners.
fn local_transform(draw: &Draw, scale: f32) -> D2D1_BRUSH_PROPERTIES {
    let [width, height] = draw.size;
    let origin = draw.corners[0];
    let across = draw.corners[1];
    let down = draw.corners[3];
    let (x_x, x_y) = if width > 0.0 {
        (
            (across[0] - origin[0]) / width,
            (across[1] - origin[1]) / width,
        )
    } else {
        (1.0, 0.0)
    };
    let (y_x, y_y) = if height > 0.0 {
        (
            (down[0] - origin[0]) / height,
            (down[1] - origin[1]) / height,
        )
    } else {
        (0.0, 1.0)
    };
    let mut properties = D2D1_BRUSH_PROPERTIES::default();
    properties.transform.M11 = x_x * scale;
    properties.transform.M12 = x_y * scale;
    properties.transform.M21 = y_x * scale;
    properties.transform.M22 = y_y * scale;
    properties.transform.M31 = origin[0] * scale;
    properties.transform.M32 = origin[1] * scale;
    properties
}
