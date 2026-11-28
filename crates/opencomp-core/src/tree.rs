//! The visual tree: what a compositor scene is, before it is drawn.
//!
//! The shape follows the WinRT one node for node, so `opencomp` can map
//! `ContainerVisual` and `SpriteVisual` onto it without a translation layer.
//!
//! Transforms compose down the tree as 2D affine matrices, so a child is placed in its parent's
//! transformed space. Clips stay axis-aligned rectangles: a rotation cannot be expressed by a
//! scissor, so a rotated draw is bounded by its own axis-aligned bounds and its clip is not
//! applied. Translation and scale, which is what a zoom uses, keep exact clipping.

use crate::TextureId;
use crate::gradient::Gradient;

/// A primitive a node paints instead of its rectangle, in the node's own units.
///
/// A rounded rectangle is the default path and stays a rectangle with radii; only the two
/// shapes a backend cannot spell that way get a variant here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// An ellipse centred on `center` with one radius per axis.
    Ellipse { center: [f32; 2], radius: [f32; 2] },
    /// A segment stroked `width` units wide, with flat ends.
    Line {
        start: [f32; 2],
        end: [f32; 2],
        width: f32,
    },
}

/// A rectangle in canvas pixels: origin, then size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// A string a node paints with its own glyphs, laid out by the renderer.
///
/// The element layer hands text over as text rather than as a bitmap: only the renderer
/// knows about the device, and a preview re-renders the same string every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub text: String,
    /// Font size in the node's own units.
    pub size: f32,
    /// Straight colour, as [`Visual::color`].
    pub color: [f32; 4],
}

/// One node of the tree. The shape is deliberately the WinRT one: an offset, a size, a colour and
/// children, so `opencomp` can hand `ContainerVisual` and `SpriteVisual` onto it one to one.
#[derive(Clone, Debug, PartialEq)]
pub struct Visual {
    /// Translation from the parent's origin.
    pub offset: [f32; 2],
    pub size: [f32; 2],
    /// Straight (unpremultiplied) colour in linear light; see [`crate::srgb`].
    pub color: [f32; 4],
    /// Multiplied down the tree, like a group's opacity.
    pub opacity: f32,
    /// Scale about `center`, in the node's own coordinates.
    pub scale: [f32; 2],
    /// Rotation in radians about `center`.
    pub rotation: f32,
    /// The point `scale` and `rotation` act around, in the node's own coordinates.
    pub center: [f32; 2],
    /// Children are clipped to this rectangle in the node's own coordinates.
    pub clip: Option<(f32, f32, f32, f32)>,
    /// A bitmap uploaded to the renderer; when set, the colour multiplies the sampled texel.
    pub image: Option<TextureId>,
    /// A gradient paint in the node's own units; when set, the colour's rgb is ignored and its
    /// alpha stays the opacity.
    pub gradient: Option<Gradient>,
    /// The part of the bitmap the rectangle covers, as `u0, v0, u1, v1`.
    pub uv: [f32; 4],
    /// Corner radii clockwise from the top-left, in the node's own units. Zero is a square
    /// corner, and a radius past half the shorter side is clamped.
    pub radius: [f32; 4],
    /// A primitive painted inside the node's rectangle; `None` fills the rectangle itself.
    pub shape: Option<Shape>,
    pub children: Vec<Visual>,
    /// When set, the node paints this string; the renderer rasterises it once and draws a quad.
    pub text: Option<Text>,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
            opacity: 1.0,
            scale: [1.0, 1.0],
            rotation: 0.0,
            center: [0.0, 0.0],
            clip: None,
            image: None,
            gradient: None,
            uv: [0.0, 0.0, 1.0, 1.0],
            radius: [0.0; 4],
            shape: None,
            children: Vec::new(),
            text: None,
        }
    }
}

impl Visual {
    /// A coloured rectangle at the parent's origin.
    pub fn rect(width: f32, height: f32, color: [f32; 4]) -> Self {
        Self {
            size: [width, height],
            color,
            ..Self::default()
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.offset = [x, y];
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Clip children to a rectangle in this node's coordinates.
    pub fn clip_to(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.clip = Some((x, y, width, height));
        self
    }

    /// Draw a bitmap from the renderer's registry across this node's rectangle.
    pub fn with_image(mut self, image: TextureId) -> Self {
        self.image = Some(image);
        self
    }

    /// Paint the node's rectangle with a gradient instead of its flat colour.
    pub fn with_gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Cover only part of the bitmap: `u0, v0` is its top-left, `u1, v1` its bottom-right.
    pub fn with_uv(mut self, uv: [f32; 4]) -> Self {
        self.uv = uv;
        self
    }

    /// Round all four corners by the same radius, in the node's own units.
    pub fn rounded(mut self, radius: f32) -> Self {
        self.radius = [radius; 4];
        self
    }

    /// Painted as `shape` inside this node's rectangle instead of as a filled rectangle.
    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = Some(shape);
        self
    }

    pub fn push(&mut self, child: Visual) -> &mut Self {
        self.children.push(child);
        self
    }
}

/// A 2D affine transform: `x' = m[0] x + m[1] y + m[2]`, `y' = m[3] x + m[4] y + m[5]`.
///
/// Six entries are enough for translation, rotation and scale. The third row of the 3x3 matrix
/// is always `[0, 0, 1]`, so it is not stored.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Affine {
    m: [f32; 6],
}

impl Affine {
    const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    };

    fn translate(x: f32, y: f32) -> Self {
        Self {
            m: [1.0, 0.0, x, 0.0, 1.0, y],
        }
    }

    fn rotate(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            m: [cos, -sin, 0.0, sin, cos, 0.0],
        }
    }

    fn scale([x, y]: [f32; 2]) -> Self {
        Self {
            m: [x, 0.0, 0.0, 0.0, y, 0.0],
        }
    }

    fn transform_point(self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            self.m[0] * x + self.m[1] * y + self.m[2],
            self.m[3] * x + self.m[4] * y + self.m[5],
        ]
    }

    /// Whether the transform maps axis-aligned rectangles to axis-aligned rectangles.
    ///
    /// A rotation by a multiple of 90 degrees also qualifies, but is treated as rotated; the
    /// only cost is a looser scissor, never a wrong pixel.
    fn axis_aligned(self) -> bool {
        self.m[1] == 0.0 && self.m[3] == 0.0
    }
}

/// `self * other` applies `other` first, the order points compose in.
impl core::ops::Mul for Affine {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let a = self.m;
        let b = other.m;
        Self {
            m: [
                a[0] * b[0] + a[1] * b[3],
                a[0] * b[1] + a[1] * b[4],
                a[0] * b[2] + a[1] * b[5] + a[2],
                a[3] * b[0] + a[4] * b[3],
                a[3] * b[1] + a[4] * b[4],
                a[3] * b[2] + a[4] * b[5] + a[5],
            ],
        }
    }
}

/// The four corners of a rectangle in local coordinates, clockwise from its top-left.
fn corners(transform: Affine, rect: Rect) -> [[f32; 2]; 4] {
    [
        transform.transform_point([rect.x, rect.y]),
        transform.transform_point([rect.x + rect.width, rect.y]),
        transform.transform_point([rect.x + rect.width, rect.y + rect.height]),
        transform.transform_point([rect.x, rect.y + rect.height]),
    ]
}

/// The axis-aligned bounds of four corners.
fn bounds(corners: [[f32; 2]; 4]) -> Rect {
    let mut left = f32::INFINITY;
    let mut top = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for [x, y] in corners {
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    Rect::new(left, top, right - left, bottom - top)
}

/// The part of both rectangles, with a zero size when they do not meet.
fn overlap(rect: Rect, clip: Rect) -> Rect {
    let left = rect.x.max(clip.x);
    let top = rect.y.max(clip.y);
    let right = (rect.x + rect.width).min(clip.x + clip.width);
    let bottom = (rect.y + rect.height).min(clip.y + clip.height);
    Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

/// A rectangle ready to draw, with the tree's transforms, translations and opacities applied.
#[derive(Clone, Debug)]
pub struct Draw {
    /// The four corners in canvas units, in the order [`corners`] produces.
    pub corners: [[f32; 2]; 4],
    /// The axis-aligned bounds of `corners`, already narrowed by the accumulated clip when the
    /// transform is axis aligned.
    pub scissor: Rect,
    pub color: [f32; 4],
    /// The bitmap to sample; `None` binds the built-in white texture.
    pub image: Option<TextureId>,
    /// A gradient in the node's own units; when set, the colour's rgb is ignored and its alpha is
    /// still the opacity.
    pub gradient: Option<Gradient>,
    /// The part of the bitmap the quad covers.
    pub uv: [f32; 4],
    /// The rectangle in the node's own units, which bounds the shader's rounded-box distance.
    pub size: [f32; 2],
    /// The four corner radii, in the node's own units and clamped to half the shorter side.
    pub radius: [f32; 4],
    /// The primitive to paint inside `corners`; `None` fills the rectangle itself.
    pub shape: Option<Shape>,
}

/// Flatten a root tree, whose parent space is the canvas, into draw order.
pub(crate) fn flatten_root(visual: &Visual, out: &mut Vec<Draw>) {
    flatten(visual, Affine::IDENTITY, 1.0, None, out);
}

/// Flatten the tree into draw order, carrying the accumulated transform, opacity and clip.
fn flatten(visual: &Visual, parent: Affine, opacity: f32, clip: Option<Rect>, out: &mut Vec<Draw>) {
    // WinRT places a visual by its offset, then rotates and scales it about its centre point.
    let local = Affine::translate(visual.center[0], visual.center[1])
        * Affine::rotate(visual.rotation)
        * Affine::scale(visual.scale)
        * Affine::translate(-visual.center[0], -visual.center[1]);
    let world = parent * Affine::translate(visual.offset[0], visual.offset[1]) * local;
    let alpha = opacity * visual.opacity;

    if visual.size[0] > 0.0 && visual.size[1] > 0.0 && alpha > 0.0 && visual.color[3] > 0.0 {
        let corners = corners(world, Rect::new(0.0, 0.0, visual.size[0], visual.size[1]));
        let bounds = bounds(corners);
        // A scissor is always a rectangle, so a rotated clip has no exact rectangle to use.
        // Restrict axis-aligned draws to the clip; for a rotated draw keep only its own bounds
        // and let the clip go, which is the limitation documented at the top of this module.
        let scissor = if world.axis_aligned() {
            clip.map_or(bounds, |clip| overlap(bounds, clip))
        } else {
            bounds
        };
        // A radius past half the shorter side would fold the outline back on itself, so clamp
        // it the way a brush does; the shader's distance then stays exact.
        let limit = visual.size[0].min(visual.size[1]) * 0.5;
        out.push(Draw {
            corners,
            scissor,
            color: [
                visual.color[0],
                visual.color[1],
                visual.color[2],
                visual.color[3] * alpha,
            ],
            image: visual.image,
            gradient: visual.gradient.clone(),
            uv: visual.uv,
            size: visual.size,
            radius: visual.radius.map(|radius| radius.clamp(0.0, limit)),
            shape: visual.shape,
        });
    }

    let clip = visual.clip.map_or(clip, |(x, y, width, height)| {
        let absolute = bounds(corners(world, Rect::new(x, y, width, height)));
        Some(match clip {
            Some(parent) => overlap(absolute, parent),
            None => absolute,
        })
    });
    for child in &visual.children {
        flatten(child, world, alpha, clip, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(transform: Affine, point: [f32; 2]) -> [f32; 2] {
        transform.transform_point(point)
    }

    #[test]
    fn an_identity_transform_leaves_points_alone() {
        assert_eq!(round_trip(Affine::IDENTITY, [3.0, 4.0]), [3.0, 4.0]);
    }

    #[test]
    fn a_translation_is_applied_before_its_parent() {
        let parent = Affine::translate(10.0, 0.0);
        let child = Affine::translate(1.0, 2.0);
        assert_eq!(round_trip(parent * child, [0.0, 0.0]), [11.0, 2.0]);
    }

    #[test]
    fn a_gradient_reaches_the_draw_list_instead_of_the_colour() {
        use crate::gradient::{Gradient, GradientShape, GradientStop};

        let gradient = Gradient {
            id: 9,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: [0.0, 0.0, 0.0, 1.0],
                },
                GradientStop {
                    offset: 1.0,
                    color: [1.0, 1.0, 1.0, 1.0],
                },
            ],
            shape: GradientShape::Linear {
                start: [0.0, 0.0],
                end: [8.0, 0.0],
            },
        };
        let visual =
            crate::Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]).with_gradient(gradient.clone());
        let draws = visual.draws();
        assert_eq!(draws.len(), 1);
        // The flat colour keeps only its alpha; the ramp is what the backend paints.
        assert_eq!(draws[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(draws[0].gradient.as_ref(), Some(&gradient));
    }

    #[test]
    fn a_shape_reaches_the_draw_list_as_a_shape() {
        let shape = Shape::Ellipse {
            center: [4.0, 4.0],
            radius: [3.0, 2.0],
        };
        let visual = crate::Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]).with_shape(shape);
        let draws = visual.draws();
        assert_eq!(draws.len(), 1);
        // The colour is still the paint; the primitive is what carries the outline.
        assert_eq!(draws[0].shape, Some(shape));
        assert_eq!(draws[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(draws[0].radius, [0.0; 4]);
    }

    #[test]
    fn a_scale_about_the_centre_keeps_the_centre_fixed() {
        let centre = [4.0, 4.0];
        let transform = Affine::translate(centre[0], centre[1])
            * Affine::scale([2.0, 2.0])
            * Affine::translate(-centre[0], -centre[1]);
        assert_eq!(round_trip(transform, centre), centre);
        assert_eq!(round_trip(transform, [0.0, 0.0]), [-4.0, -4.0]);
    }
}
