//! `CompositionSpriteShape` and the shape list a `ShapeVisual` paints.
//!
//! A shape is read when the snapshot reaches it, the same way a visual is: the geometry becomes
//! a draw-list primitive, the fill or stroke brush becomes the paint, and the shape's own
//! transform becomes the visual's. The geometry and brushes stay live COM objects, so a client
//! that edits them between frames is seen.
//!
//! A line is stroked with `StrokeBrush` at `StrokeThickness`; an ellipse is filled with
//! `FillBrush`. The stroked outline of an ellipse is not drawn: the draw list has one paint per
//! primitive, so the stroke and the fill cannot both reach the same ellipse yet.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionBrush, CompositionGeometry, CompositionShape, CompositionSpriteShape,
    CompositionStrokeCap, CompositionStrokeDashArray, CompositionStrokeLineJoin,
    ICompositionColorBrush, ICompositionEllipseGeometry, ICompositionLineGeometry,
    ICompositionObject, ICompositionObject2, ICompositionObject4, ICompositionObject5,
    ICompositionShape, ICompositionShape_Impl, ICompositionSpriteShape,
    ICompositionSpriteShape_Impl,
};
use crate::object::{composition_object, not_implemented};
use opencomp_core::{Gradient, Shape, Visual as CoreVisual};
use windows_core::{Interface, Ref, Result};
use windows_numerics::{Matrix3x2, Vector2};

fn identity() -> Matrix3x2 {
    Matrix3x2 {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        m31: 0.0,
        m32: 0.0,
    }
}

/// Everything one sprite shape keeps. The paint and outline properties are read back as they
/// were set; only `FillBrush`, `Geometry`, `StrokeBrush` and `StrokeThickness` reach a draw.
struct ShapeState {
    geometry: Option<CompositionGeometry>,
    fill: Option<CompositionBrush>,
    stroke: Option<CompositionBrush>,
    thickness: f32,
    dash_cap: CompositionStrokeCap,
    dash_offset: f32,
    end_cap: CompositionStrokeCap,
    line_join: CompositionStrokeLineJoin,
    miter_limit: f32,
    start_cap: CompositionStrokeCap,
    non_scaling: bool,
    center: Vector2,
    offset: Vector2,
    rotation: f32,
    scale: Vector2,
    matrix: Matrix3x2,
}

impl Default for ShapeState {
    fn default() -> Self {
        Self {
            geometry: None,
            fill: None,
            stroke: None,
            thickness: 0.0,
            dash_cap: CompositionStrokeCap::Flat,
            dash_offset: 0.0,
            end_cap: CompositionStrokeCap::Flat,
            line_join: CompositionStrokeLineJoin::Miter,
            miter_limit: 10.0,
            start_cap: CompositionStrokeCap::Flat,
            non_scaling: false,
            center: Vector2::default(),
            offset: Vector2::default(),
            rotation: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
            matrix: identity(),
        }
    }
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionShape,
    ICompositionSpriteShape
)]
pub struct SpriteShapeObject {
    state: Mutex<ShapeState>,
}

composition_object!(SpriteShapeObject_Impl);

impl ICompositionShape_Impl for SpriteShapeObject_Impl {
    fn CenterPoint(&self) -> Result<Vector2> {
        Ok(self.state.lock().unwrap().center)
    }
    fn SetCenterPoint(&self, value: &Vector2) -> Result<()> {
        self.state.lock().unwrap().center = *value;
        Ok(())
    }
    fn Offset(&self) -> Result<Vector2> {
        Ok(self.state.lock().unwrap().offset)
    }
    fn SetOffset(&self, value: &Vector2) -> Result<()> {
        self.state.lock().unwrap().offset = *value;
        Ok(())
    }
    fn RotationAngle(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().rotation)
    }
    fn SetRotationAngle(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().rotation = value;
        Ok(())
    }
    fn RotationAngleInDegrees(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().rotation.to_degrees())
    }
    fn SetRotationAngleInDegrees(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().rotation = value.to_radians();
        Ok(())
    }
    fn Scale(&self) -> Result<Vector2> {
        Ok(self.state.lock().unwrap().scale)
    }
    fn SetScale(&self, value: &Vector2) -> Result<()> {
        self.state.lock().unwrap().scale = *value;
        Ok(())
    }
    fn TransformMatrix(&self) -> Result<Matrix3x2> {
        Ok(self.state.lock().unwrap().matrix)
    }
    fn SetTransformMatrix(&self, value: &Matrix3x2) -> Result<()> {
        self.state.lock().unwrap().matrix = *value;
        Ok(())
    }
}

impl ICompositionSpriteShape_Impl for SpriteShapeObject_Impl {
    fn FillBrush(&self) -> Result<CompositionBrush> {
        match self.state.lock().unwrap().fill.clone() {
            Some(brush) => Ok(brush),
            None => crate::brush::transparent(),
        }
    }
    fn SetFillBrush(&self, value: Ref<CompositionBrush>) -> Result<()> {
        self.state.lock().unwrap().fill = value.cloned();
        Ok(())
    }
    fn Geometry(&self) -> Result<CompositionGeometry> {
        match self.state.lock().unwrap().geometry.clone() {
            Some(geometry) => Ok(geometry),
            // Nullable in the ABI; the trait cannot say so.
            None => not_implemented(),
        }
    }
    fn SetGeometry(&self, value: Ref<CompositionGeometry>) -> Result<()> {
        self.state.lock().unwrap().geometry = value.cloned();
        Ok(())
    }
    fn IsStrokeNonScaling(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().non_scaling)
    }
    fn SetIsStrokeNonScaling(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().non_scaling = value;
        Ok(())
    }
    fn StrokeBrush(&self) -> Result<CompositionBrush> {
        match self.state.lock().unwrap().stroke.clone() {
            Some(brush) => Ok(brush),
            None => crate::brush::transparent(),
        }
    }
    fn SetStrokeBrush(&self, value: Ref<CompositionBrush>) -> Result<()> {
        self.state.lock().unwrap().stroke = value.cloned();
        Ok(())
    }
    fn StrokeDashArray(&self) -> Result<CompositionStrokeDashArray> {
        // The draw list has no dash pattern; a straight stroke is the only one it can paint.
        not_implemented()
    }
    fn StrokeDashCap(&self) -> Result<CompositionStrokeCap> {
        Ok(self.state.lock().unwrap().dash_cap)
    }
    fn SetStrokeDashCap(&self, value: CompositionStrokeCap) -> Result<()> {
        self.state.lock().unwrap().dash_cap = value;
        Ok(())
    }
    fn StrokeDashOffset(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().dash_offset)
    }
    fn SetStrokeDashOffset(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().dash_offset = value;
        Ok(())
    }
    fn StrokeEndCap(&self) -> Result<CompositionStrokeCap> {
        Ok(self.state.lock().unwrap().end_cap)
    }
    fn SetStrokeEndCap(&self, value: CompositionStrokeCap) -> Result<()> {
        self.state.lock().unwrap().end_cap = value;
        Ok(())
    }
    fn StrokeLineJoin(&self) -> Result<CompositionStrokeLineJoin> {
        Ok(self.state.lock().unwrap().line_join)
    }
    fn SetStrokeLineJoin(&self, value: CompositionStrokeLineJoin) -> Result<()> {
        self.state.lock().unwrap().line_join = value;
        Ok(())
    }
    fn StrokeMiterLimit(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().miter_limit)
    }
    fn SetStrokeMiterLimit(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().miter_limit = value;
        Ok(())
    }
    fn StrokeStartCap(&self) -> Result<CompositionStrokeCap> {
        Ok(self.state.lock().unwrap().start_cap)
    }
    fn SetStrokeStartCap(&self, value: CompositionStrokeCap) -> Result<()> {
        self.state.lock().unwrap().start_cap = value;
        Ok(())
    }
    fn StrokeThickness(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().thickness)
    }
    fn SetStrokeThickness(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().thickness = value;
        Ok(())
    }
}

/// A sprite shape with no geometry yet; `SetGeometry` fills it in.
pub(crate) fn sprite_shape(
    geometry: Option<CompositionGeometry>,
) -> Result<CompositionSpriteShape> {
    let object: ICompositionSpriteShape = SpriteShapeObject {
        state: Mutex::new(ShapeState {
            geometry,
            ..ShapeState::default()
        }),
    }
    .into();
    object.cast()
}

/// The paint a brush names: the flat colour, or a gradient with the colour's alpha as opacity.
fn paint(brush: Option<&CompositionBrush>, size: Vector2) -> ([f32; 4], Option<Gradient>) {
    let Some(brush) = brush else {
        return ([0.0; 4], None);
    };
    if let Some(spec) = crate::gradient::spec(brush)
        && let Some(gradient) = spec.resolve(size)
    {
        return ([1.0; 4], Some(gradient));
    }
    let color = brush
        .cast::<ICompositionColorBrush>()
        .ok()
        .and_then(|brush| brush.Color().ok())
        .map_or([0.0; 4], crate::brush::to_rgba);
    (color, None)
}

/// One shape as the draw list sees it, or `None` when it paints nothing.
pub(crate) fn visual(shape: &CompositionShape, size: Vector2) -> Option<CoreVisual> {
    let sprite = shape.cast::<ICompositionSpriteShape>().ok()?;
    let geometry = sprite.Geometry().ok()?;
    let (primitive, brush) = if let Ok(ellipse) = geometry.cast::<ICompositionEllipseGeometry>() {
        let center = ellipse.Center().ok()?;
        let radius = ellipse.Radius().ok()?;
        let primitive = Shape::Ellipse {
            center: [center.x, center.y],
            radius: [radius.x, radius.y],
        };
        (primitive, sprite.FillBrush().ok())
    } else if let Ok(line) = geometry.cast::<ICompositionLineGeometry>() {
        let start = line.Start().ok()?;
        let end = line.End().ok()?;
        let width = sprite.StrokeThickness().unwrap_or(0.0).max(0.0);
        if width <= 0.0 {
            // A line with no width covers no pixels; its fill has no area either.
            return None;
        }
        let primitive = Shape::Line {
            start: [start.x, start.y],
            end: [end.x, end.y],
            width,
        };
        (primitive, sprite.StrokeBrush().ok())
    } else {
        // A geometry the draw list has no primitive for.
        return None;
    };
    let (color, gradient) = paint(brush.as_ref(), size);
    if color[3] <= 0.0 {
        return None;
    }
    let offset = shape.Offset().ok()?;
    let center = shape.CenterPoint().ok()?;
    let scale = shape.Scale().ok()?;
    let rotation = shape.RotationAngle().ok()?;
    Some(CoreVisual {
        offset: [offset.x, offset.y],
        size: [size.x, size.y],
        color,
        opacity: 1.0,
        scale: [scale.x, scale.y],
        rotation,
        center: [center.x, center.y],
        clip: None,
        image: None,
        gradient,
        uv: [0.0, 0.0, 1.0, 1.0],
        radius: [0.0; 4],
        shape: Some(primitive),
        children: Vec::new(),
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{
        CompositionBrush, CompositionShape, ICompositor, ICompositor5, IShapeVisual, IVisual,
    };
    use crate::bindings::Windows::UI::Color;
    use windows_core::Interface;

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    fn fifth(compositor: &ICompositor) -> ICompositor5 {
        compositor.cast().unwrap()
    }

    fn brush(compositor: &ICompositor) -> CompositionBrush {
        let color = Color {
            a: 255,
            r: 255,
            g: 0,
            b: 0,
        };
        compositor
            .CreateColorBrushWithColor(color)
            .unwrap()
            .cast()
            .unwrap()
    }

    fn shape_visual(compositor: &ICompositor) -> IShapeVisual {
        let visual = fifth(compositor).CreateShapeVisual().unwrap();
        let size: IVisual = visual.cast().unwrap();
        size.SetSize(Vector2 { x: 32.0, y: 32.0 }).unwrap();
        visual.cast().unwrap()
    }

    fn draws(visual: &IShapeVisual) -> Vec<opencomp_core::Draw> {
        let node = crate::node::resolve(visual).unwrap();
        crate::node::snapshot(&node).unwrap().draws()
    }

    #[test]
    fn an_ellipse_shape_reaches_the_draw_list() {
        let compositor = compositor();
        let visual = shape_visual(&compositor);
        let geometry = fifth(&compositor).CreateEllipseGeometry().unwrap();
        geometry.SetCenter(Vector2 { x: 16.0, y: 16.0 }).unwrap();
        geometry.SetRadius(Vector2 { x: 8.0, y: 8.0 }).unwrap();
        let shape = fifth(&compositor)
            .CreateSpriteShapeWithGeometry(&geometry)
            .unwrap();
        shape.SetFillBrush(&brush(&compositor)).unwrap();
        let shape: CompositionShape = shape.cast().unwrap();
        visual.Shapes().unwrap().Append(&shape).unwrap();

        let draws = draws(&visual);
        assert_eq!(draws.len(), 1);
        assert_eq!(
            draws[0].shape,
            Some(Shape::Ellipse {
                center: [16.0, 16.0],
                radius: [8.0, 8.0],
            })
        );
        assert_eq!(draws[0].color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_line_shape_reaches_the_draw_list_with_its_stroke_width() {
        let compositor = compositor();
        let visual = shape_visual(&compositor);
        let geometry = fifth(&compositor).CreateLineGeometry().unwrap();
        geometry.SetStart(Vector2 { x: 2.0, y: 16.0 }).unwrap();
        geometry.SetEnd(Vector2 { x: 30.0, y: 16.0 }).unwrap();
        let shape = fifth(&compositor)
            .CreateSpriteShapeWithGeometry(&geometry)
            .unwrap();
        shape.SetStrokeBrush(&brush(&compositor)).unwrap();
        shape.SetStrokeThickness(3.0).unwrap();
        let shape: CompositionShape = shape.cast().unwrap();
        visual.Shapes().unwrap().Append(&shape).unwrap();

        let draws = draws(&visual);
        assert_eq!(draws.len(), 1);
        assert_eq!(
            draws[0].shape,
            Some(Shape::Line {
                start: [2.0, 16.0],
                end: [30.0, 16.0],
                width: 3.0,
            })
        );
    }

    #[test]
    fn an_unattached_geometry_still_answers_its_properties() {
        let compositor = compositor();
        let geometry = fifth(&compositor).CreateEllipseGeometry().unwrap();
        geometry.SetCenter(Vector2 { x: 3.0, y: 4.0 }).unwrap();
        geometry.SetRadius(Vector2 { x: 5.0, y: 6.0 }).unwrap();
        assert_eq!(geometry.Center().unwrap(), Vector2 { x: 3.0, y: 4.0 });
        assert_eq!(geometry.Radius().unwrap(), Vector2 { x: 5.0, y: 6.0 });
    }
}
