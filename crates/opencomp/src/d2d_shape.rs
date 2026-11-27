//! The D2D primitives that are not a plain rectangle: an ellipse and a line.
//!
//! Both are expressed in the node's own units. The rectangle path already treats a rotated draw
//! as its axis-aligned bounds, and a shape follows that same approximation: the corners give the
//! origin and the two local axes, and a point is placed by fractions of them.

use opencomp_core::{Draw, Shape};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1Brush, ID2D1DeviceContext, ID2D1Factory1,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};

/// The canvas origin and the two local axes of a draw, in device pixels.
fn basis(draw: &Draw, scale: f32) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let origin = [draw.corners[0][0] * scale, draw.corners[0][1] * scale];
    let x = [
        (draw.corners[1][0] - draw.corners[0][0]) * scale,
        (draw.corners[1][1] - draw.corners[0][1]) * scale,
    ];
    let y = [
        (draw.corners[3][0] - draw.corners[0][0]) * scale,
        (draw.corners[3][1] - draw.corners[0][1]) * scale,
    ];
    (origin, x, y)
}

/// Map a point in the node's own units to canvas pixels through that basis.
fn point(draw: &Draw, local: [f32; 2], scale: f32) -> [f32; 2] {
    let (origin, x, y) = basis(draw, scale);
    let [width, height] = draw.size;
    let fx = if width > 0.0 { local[0] / width } else { 0.0 };
    let fy = if height > 0.0 { local[1] / height } else { 0.0 };
    [
        origin[0] + fx * x[0] + fy * y[0],
        origin[1] + fx * x[1] + fy * y[1],
    ]
}

/// Fill the draw's rectangle with `brush`, rounded or elliptical when it asks for it.
pub(crate) fn fill(
    d2d: &ID2D1DeviceContext,
    factory: &ID2D1Factory1,
    draw: &Draw,
    brush: &ID2D1Brush,
    scale: f32,
) {
    if let Some(Shape::Ellipse { center, radius }) = draw.shape {
        let [width, height] = draw.size;
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let (_, x, y) = basis(draw, scale);
        let centre = point(draw, center, scale);
        // The Win32 structs use the `windows` crate's own `windows-numerics` (0.3), a
        // different crate from the ABI's 0.100; a zeroed ellipse is one way to name its
        // point type without depending on both. The fields are public.
        let mut ellipse = D2D1_ELLIPSE::default();
        ellipse.point.X = centre[0];
        ellipse.point.Y = centre[1];
        ellipse.radiusX = radius[0] * x[0].hypot(x[1]) / width;
        ellipse.radiusY = radius[1] * y[0].hypot(y[1]) / height;
        // Safety: the factory and the context belong to the same device.
        unsafe {
            if let Ok(geometry) = factory.CreateEllipseGeometry(&ellipse) {
                d2d.FillGeometry(&geometry, brush, None::<&ID2D1Brush>);
            }
        }
        return;
    }
    let (left, top) = (draw.corners[0][0] * scale, draw.corners[0][1] * scale);
    let (right, bottom) = (draw.corners[2][0] * scale, draw.corners[2][1] * scale);
    // D2D has one radius for all four corners; the largest wins.
    let radius = draw.radius.iter().copied().fold(0.0, f32::max) * scale;
    let rect = D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    };
    // Safety: the context is alive and `brush` was made by it.
    unsafe {
        if radius > 0.0 {
            let rounded = D2D1_ROUNDED_RECT {
                rect,
                radiusX: radius,
                radiusY: radius,
            };
            d2d.FillRoundedRectangle(&rounded, brush);
        } else {
            d2d.FillRectangle(&rect, brush);
        }
    }
}

/// Stroke a line with the solid brush at the width the draw asks for.
pub(crate) fn stroke(
    d2d: &ID2D1DeviceContext,
    draw: &Draw,
    brush: &ID2D1SolidColorBrush,
    scale: f32,
) {
    let Some(Shape::Line { start, end, width }) = draw.shape else {
        return;
    };
    let from = point(draw, start, scale);
    let to = point(draw, end, scale);
    // A zeroed ellipse supplies the point type, see `fill`; the fields are public.
    let mut point0 = D2D1_ELLIPSE::default().point;
    point0.X = from[0];
    point0.Y = from[1];
    let mut point1 = point0;
    point1.X = to[0];
    point1.Y = to[1];
    // Safety: the context is alive and the brush was made by it.
    unsafe {
        d2d.DrawLine(
            point0,
            point1,
            brush,
            width * scale,
            None::<&ID2D1StrokeStyle>,
        );
    }
}
