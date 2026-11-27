//! `InsetClip` and `CompositionGeometricClip`: the two shapes a visual can be clipped to.
//!
//! The insets of an inset clip are copied into the visual when the clip is attached, the same
//! shortcut the brush takes. A geometric clip keeps the geometry it was built with; a rounded
//! rectangle is the one geometry the renderer understands, so it becomes the visual's corner
//! radius and its rectangle becomes the clip the children inherit.
//!
//! A clip whose insets are all zero clips nothing, which is what a client gets before it sets
//! them.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionClip, CompositionGeometricClip, CompositionGeometry, ICompositionClip,
    ICompositionClip_Impl, ICompositionClip2, ICompositionClip2_Impl, ICompositionGeometricClip,
    ICompositionGeometricClip_Impl, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5, ICompositionRoundedRectangleGeometry, IInsetClip, IInsetClip_Impl,
    InsetClip,
};
use crate::object::{composition_object, not_implemented};
use windows_core::{Interface, Ref, Result};
use windows_numerics::{Matrix3x2, Vector2};

/// The transform `ICompositionClip2` exposes on every clip.
///
/// The renderer applies a clip's insets or geometry to the visual as it is, so these
/// values are kept for a client that reads them back and are not folded into the
/// node. A clip with no transform set answers the identity, as the contract says.
struct ClipTransform {
    anchor: Vector2,
    center: Vector2,
    offset: Vector2,
    rotation: f32,
    scale: Vector2,
    matrix: Matrix3x2,
}

impl Default for ClipTransform {
    fn default() -> Self {
        Self {
            anchor: Vector2::default(),
            center: Vector2::default(),
            offset: Vector2::default(),
            rotation: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
            matrix: Matrix3x2 {
                m11: 1.0,
                m12: 0.0,
                m21: 0.0,
                m22: 1.0,
                m31: 0.0,
                m32: 0.0,
            },
        }
    }
}

/// The `ICompositionClip2` half both clip objects share. The bodies expand in this
/// module, where `ClipTransform` and the traits are in scope.
macro_rules! clip_transform {
    ($impl_type:ident) => {
        impl ICompositionClip2_Impl for $impl_type {
            fn AnchorPoint(&self) -> Result<Vector2> {
                Ok(self.transform.lock().unwrap().anchor)
            }
            fn SetAnchorPoint(&self, value: &Vector2) -> Result<()> {
                self.transform.lock().unwrap().anchor = *value;
                Ok(())
            }
            fn CenterPoint(&self) -> Result<Vector2> {
                Ok(self.transform.lock().unwrap().center)
            }
            fn SetCenterPoint(&self, value: &Vector2) -> Result<()> {
                self.transform.lock().unwrap().center = *value;
                Ok(())
            }
            fn Offset(&self) -> Result<Vector2> {
                Ok(self.transform.lock().unwrap().offset)
            }
            fn SetOffset(&self, value: &Vector2) -> Result<()> {
                self.transform.lock().unwrap().offset = *value;
                Ok(())
            }
            fn RotationAngle(&self) -> Result<f32> {
                Ok(self.transform.lock().unwrap().rotation)
            }
            fn SetRotationAngle(&self, value: f32) -> Result<()> {
                self.transform.lock().unwrap().rotation = value;
                Ok(())
            }
            fn RotationAngleInDegrees(&self) -> Result<f32> {
                Ok(self.transform.lock().unwrap().rotation.to_degrees())
            }
            fn SetRotationAngleInDegrees(&self, value: f32) -> Result<()> {
                self.transform.lock().unwrap().rotation = value.to_radians();
                Ok(())
            }
            fn Scale(&self) -> Result<Vector2> {
                Ok(self.transform.lock().unwrap().scale)
            }
            fn SetScale(&self, value: &Vector2) -> Result<()> {
                self.transform.lock().unwrap().scale = *value;
                Ok(())
            }
            fn TransformMatrix(&self) -> Result<Matrix3x2> {
                Ok(self.transform.lock().unwrap().matrix)
            }
            fn SetTransformMatrix(&self, value: &Matrix3x2) -> Result<()> {
                self.transform.lock().unwrap().matrix = *value;
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
    ICompositionClip,
    ICompositionClip2,
    IInsetClip
)]
pub struct InsetClipObject {
    /// Left, top, right, bottom.
    insets: Mutex<[f32; 4]>,
    transform: Mutex<ClipTransform>,
}

composition_object!(InsetClipObject_Impl);
clip_transform!(InsetClipObject_Impl);

impl ICompositionClip_Impl for InsetClipObject_Impl {}

impl IInsetClip_Impl for InsetClipObject_Impl {
    fn LeftInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[0])
    }

    fn SetLeftInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[0] = value;
        Ok(())
    }

    fn TopInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[1])
    }

    fn SetTopInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[1] = value;
        Ok(())
    }

    fn RightInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[2])
    }

    fn SetRightInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[2] = value;
        Ok(())
    }

    fn BottomInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[3])
    }

    fn SetBottomInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[3] = value;
        Ok(())
    }
}

/// A geometric clip: a geometry in the visual's own coordinates.
#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionClip,
    ICompositionClip2,
    ICompositionGeometricClip
)]
pub struct GeometricClipObject {
    geometry: Mutex<Option<CompositionGeometry>>,
    transform: Mutex<ClipTransform>,
}

composition_object!(GeometricClipObject_Impl);
clip_transform!(GeometricClipObject_Impl);

impl ICompositionClip_Impl for GeometricClipObject_Impl {}

impl ICompositionGeometricClip_Impl for GeometricClipObject_Impl {
    fn Geometry(&self) -> Result<CompositionGeometry> {
        match self.geometry.lock().unwrap().clone() {
            Some(geometry) => Ok(geometry),
            // The property is nullable in the ABI; the trait cannot say so.
            None => not_implemented(),
        }
    }

    fn SetGeometry(&self, value: Ref<CompositionGeometry>) -> Result<()> {
        *self.geometry.lock().unwrap() = value.cloned();
        Ok(())
    }

    // A view box maps the geometry's coordinates into the visual's; the renderer has no
    // coordinate conversion, so nothing reads one.
    fn ViewBox(&self) -> Result<crate::bindings::Microsoft::UI::Composition::CompositionViewBox> {
        not_implemented()
    }
    fn SetViewBox(
        &self,
        _value: Ref<crate::bindings::Microsoft::UI::Composition::CompositionViewBox>,
    ) -> Result<()> {
        not_implemented()
    }
}

pub fn create() -> Result<InsetClip> {
    create_with([0.0; 4])
}

pub fn create_with(insets: [f32; 4]) -> Result<InsetClip> {
    let clip: IInsetClip = InsetClipObject {
        insets: Mutex::new(insets),
        transform: Mutex::new(ClipTransform::default()),
    }
    .into();
    clip.cast()
}

/// A geometric clip with no geometry yet; `SetGeometry` fills it in.
pub fn geometric(geometry: Option<CompositionGeometry>) -> Result<CompositionGeometricClip> {
    let clip: ICompositionGeometricClip = GeometricClipObject {
        geometry: Mutex::new(geometry),
        transform: Mutex::new(ClipTransform::default()),
    }
    .into();
    clip.cast()
}

/// A clip that clips nothing, for a visual that has none.
///
/// `IVisual.Clip` is nullable in the ABI and the generated `_Impl` trait cannot say
/// so, so a client that reads it back gets this instead of an exception.
pub fn empty() -> Result<CompositionClip> {
    let clip: IInsetClip = InsetClipObject {
        insets: Mutex::new([0.0; 4]),
        transform: Mutex::new(ClipTransform::default()),
    }
    .into();
    clip.cast()
}

/// The insets of any clip object this DLL made, in left, top, right, bottom order.
pub fn insets(clip: &CompositionClip) -> Option<[f32; 4]> {
    let clip = clip.cast::<IInsetClip>().ok()?;
    Some([
        clip.LeftInset().ok()?,
        clip.TopInset().ok()?,
        clip.RightInset().ok()?,
        clip.BottomInset().ok()?,
    ])
}

/// The clip rectangle and corner radii `clip` describes inside a visual of `size`.
///
/// The renderer clips children to a rectangle and rounds the visual's own corners. An inset clip
/// therefore carries insets only, and a rounded geometric clip carries its rectangle as insets
/// and its corner radius as the visual's radius. Any other shape has nothing to draw and is
/// ignored, which leaves the visual unclipped rather than invisible.
pub fn shape(clip: &CompositionClip, size: Vector2) -> Option<([f32; 4], [f32; 4])> {
    if let Some(insets) = insets(clip) {
        return Some((insets, [0.0; 4]));
    }
    let clip = clip.cast::<ICompositionGeometricClip>().ok()?;
    let geometry = clip
        .Geometry()
        .ok()?
        .cast::<ICompositionRoundedRectangleGeometry>()
        .ok()?;
    let offset = geometry.Offset().ok()?;
    let region = geometry.Size().ok()?;
    let radius = geometry.CornerRadius().ok()?;
    let insets = [
        offset.x,
        offset.y,
        (size.x - offset.x - region.x).max(0.0),
        (size.y - offset.y - region.y).max(0.0),
    ];
    // A corner is a circle in the renderer, so an elliptical radius keeps its width.
    Some((insets, [radius.x.max(0.0); 4]))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{
        CompositionClip, ICompositionClip2, ICompositor, ICompositor5, ICompositor6, IVisual,
    };
    use windows_numerics::{Matrix3x2, Vector2};

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    #[test]
    fn a_rounded_geometric_clip_reaches_the_node() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let visual: IVisual = sprite.cast().unwrap();
        visual.SetSize(Vector2 { x: 100.0, y: 50.0 }).unwrap();

        let geometry = compositor
            .cast::<ICompositor5>()
            .unwrap()
            .CreateRoundedRectangleGeometry()
            .unwrap();
        geometry.SetOffset(Vector2 { x: 10.0, y: 5.0 }).unwrap();
        geometry.SetSize(Vector2 { x: 90.0, y: 40.0 }).unwrap();
        geometry
            .SetCornerRadius(Vector2 { x: 12.0, y: 12.0 })
            .unwrap();
        let clip = compositor
            .cast::<ICompositor6>()
            .unwrap()
            .CreateGeometricClipWithGeometry(&geometry)
            .unwrap();
        let clip: CompositionClip = clip.cast().unwrap();
        visual.SetClip(&clip).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.clip, Some((10.0, 5.0, 90.0, 40.0)));
        assert_eq!(tree.radius, [12.0; 4]);
    }

    /// The transform is the same state on either clip class; only the class differs.
    fn transform_round_trips(clip: &ICompositionClip2) {
        assert_eq!(clip.AnchorPoint().unwrap(), Vector2::default());
        assert_eq!(clip.CenterPoint().unwrap(), Vector2::default());
        assert_eq!(clip.Offset().unwrap(), Vector2::default());
        assert_eq!(clip.Scale().unwrap(), Vector2 { x: 1.0, y: 1.0 });
        assert_eq!(clip.RotationAngle().unwrap(), 0.0);
        assert_eq!(
            clip.TransformMatrix().unwrap(),
            Matrix3x2 {
                m11: 1.0,
                m12: 0.0,
                m21: 0.0,
                m22: 1.0,
                m31: 0.0,
                m32: 0.0,
            }
        );

        clip.SetAnchorPoint(Vector2 { x: 0.5, y: 0.25 }).unwrap();
        clip.SetCenterPoint(Vector2 { x: 2.0, y: 3.0 }).unwrap();
        clip.SetOffset(Vector2 { x: 4.0, y: 5.0 }).unwrap();
        clip.SetScale(Vector2 { x: 6.0, y: 7.0 }).unwrap();
        clip.SetRotationAngleInDegrees(90.0).unwrap();
        let matrix = Matrix3x2 {
            m11: 8.0,
            m12: 9.0,
            m21: 10.0,
            m22: 11.0,
            m31: 12.0,
            m32: 13.0,
        };
        clip.SetTransformMatrix(matrix).unwrap();

        assert_eq!(clip.AnchorPoint().unwrap(), Vector2 { x: 0.5, y: 0.25 });
        assert_eq!(clip.CenterPoint().unwrap(), Vector2 { x: 2.0, y: 3.0 });
        assert_eq!(clip.Offset().unwrap(), Vector2 { x: 4.0, y: 5.0 });
        assert_eq!(clip.Scale().unwrap(), Vector2 { x: 6.0, y: 7.0 });
        assert!((clip.RotationAngle().unwrap() - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((clip.RotationAngleInDegrees().unwrap() - 90.0).abs() < 1e-4);
        assert_eq!(clip.TransformMatrix().unwrap(), matrix);
    }

    #[test]
    fn both_clip_classes_keep_their_transform() {
        let compositor = compositor();
        let inset = compositor.CreateInsetClip().unwrap();
        transform_round_trips(&inset.cast().unwrap());
        let geometric = compositor
            .cast::<ICompositor6>()
            .unwrap()
            .CreateGeometricClip()
            .unwrap();
        transform_round_trips(&geometric.cast().unwrap());
    }

    #[test]
    fn a_clip_with_no_geometry_leaves_the_visual_square() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let visual: IVisual = sprite.cast().unwrap();
        visual.SetSize(Vector2 { x: 40.0, y: 40.0 }).unwrap();
        let clip = compositor
            .cast::<ICompositor6>()
            .unwrap()
            .CreateGeometricClip()
            .unwrap();
        let clip: CompositionClip = clip.cast().unwrap();
        visual.SetClip(&clip).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.clip, None);
        assert_eq!(tree.radius, [0.0; 4]);
    }
}
