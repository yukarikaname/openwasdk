//! The visual tree: what a compositor scene is, before it is drawn.
//!
//! The shape follows the WinRT one node for node, so `opencomp` can map
//! `ContainerVisual` and `SpriteVisual` onto it without a translation layer.

/// A solid rectangle in canvas pixels: origin, then size.
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

    pub(crate) fn as_pixels(&self, scale: f32) -> (u32, u32, u32, u32) {
        let x = (self.x * scale).round().max(0.0) as u32;
        let y = (self.y * scale).round().max(0.0) as u32;
        let width = (self.width * scale).round().max(0.0) as u32;
        let height = (self.height * scale).round().max(0.0) as u32;
        (x, y, width, height)
    }
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
    /// Children are clipped to this rectangle in the node's own coordinates.
    pub clip: Option<(f32, f32, f32, f32)>,
    pub children: Vec<Visual>,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
            opacity: 1.0,
            clip: None,
            children: Vec::new(),
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

    pub fn push(&mut self, child: Visual) -> &mut Self {
        self.children.push(child);
        self
    }
}

/// The part of `rect` that survives `clip`, or `None` when nothing does.
fn intersect(rect: Rect, clip: (f32, f32, f32, f32)) -> Option<Rect> {
    let left = rect.x.max(clip.0);
    let top = rect.y.max(clip.1);
    let right = (rect.x + rect.width).min(clip.0 + clip.2);
    let bottom = (rect.y + rect.height).min(clip.1 + clip.3);
    (right > left && bottom > top).then(|| Rect::new(left, top, right - left, bottom - top))
}

/// A rectangle ready to draw, with the tree's translations and opacities applied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Draw {
    pub(crate) rect: Rect,
    pub(crate) color: [f32; 4],
}

/// Flatten the tree into draw order, carrying the accumulated clip rectangle in canvas pixels.
pub(crate) fn flatten(
    visual: &Visual,
    origin: (f32, f32),
    opacity: f32,
    clip: Option<(f32, f32, f32, f32)>,
    out: &mut Vec<Draw>,
) {
    let x = origin.0 + visual.offset[0];
    let y = origin.1 + visual.offset[1];
    let alpha = opacity * visual.opacity;
    let draw = Rect::new(x, y, visual.size[0], visual.size[1]);
    if visual.size[0] > 0.0 && visual.size[1] > 0.0 && alpha > 0.0 && visual.color[3] > 0.0 {
        if let Some(clip) = clip {
            if let Some(visible) = intersect(draw, clip) {
                out.push(Draw {
                    rect: visible,
                    color: [
                        visual.color[0],
                        visual.color[1],
                        visual.color[2],
                        visual.color[3] * alpha,
                    ],
                });
            }
        } else {
            out.push(Draw {
                rect: draw,
                color: [
                    visual.color[0],
                    visual.color[1],
                    visual.color[2],
                    visual.color[3] * alpha,
                ],
            });
        }
    }
    let clip = visual.clip.map_or(clip, |local| {
        let absolute = (x + local.0, y + local.1, local.2, local.3);
        Some(match clip {
            Some(parent) => {
                let left = parent.0.max(absolute.0);
                let top = parent.1.max(absolute.1);
                let right = (parent.0 + parent.2).min(absolute.0 + absolute.2);
                let bottom = (parent.1 + parent.3).min(absolute.1 + absolute.3);
                (left, top, (right - left).max(0.0), (bottom - top).max(0.0))
            }
            None => absolute,
        })
    });
    for child in &visual.children {
        flatten(child, (x, y), alpha, clip, out);
    }
}
