//! Translate an arranged element tree into the compositor's visual tree.
//!
//! One `Visual` per element. Its offset and size are the arranged rectangle,
//! which is already relative to the parent because [`crate::arrange`] returns
//! parent-local rectangles. A `Border` paints its background and the child
//! rectangle already carries the padding, so no inset is repeated here.
//! `TextBlock` has no glyphs and therefore paints nothing; a background set on
//! it still paints, as the property asks.
//!
//! This is the only file in the crate that knows about `opencomp_core`. The
//! element and layout layers stay free of the compositor so the ABI shell can
//! use them without a GPU.

use std::collections::HashMap;

use opencomp_core::{Text, Visual};

use crate::element::Element;
use crate::layout::Layout;

/// Build the visual tree for an element and its arranged layout.
pub fn compose(element: &Element, layout: &Layout) -> Visual {
    compose_with(element, layout, &HashMap::new())
}

/// The same, with pictures the caller has already uploaded: an `Image` names a source and
/// the map says which texture that source became.
pub fn compose_with(
    element: &Element,
    layout: &Layout,
    images: &HashMap<String, opencomp_core::TextureId>,
) -> Visual {
    let color = element.background().unwrap_or([0.0; 4]);
    let mut visual =
        Visual::rect(layout.rect.width, layout.rect.height, color).at(layout.rect.x, layout.rect.y);
    // Opacity applies to the element and everything below it.
    visual.opacity = element.opacity();

    /*
     * A scrolling viewport clips what it holds. Content is arranged against the viewer's
     * size, and a page that asks for more room than the viewport has is scrolled, not drawn
     * over its neighbours -- this is the clip the runtime applies to the scroll content.
     */
    if matches!(element.kind(), crate::element::Kind::ScrollViewer) {
        visual.clip = Some((0.0, 0.0, layout.rect.width, layout.rect.height));
    }

    // A TextBlock paints its string; the renderer rasterises it, this layer only says what.
    if matches!(element.kind(), crate::element::Kind::TextBlock) && !element.text().is_empty() {
        visual.text = Some(Text {
            text: element.text().to_string(),
            size: element.font_size(),
            // The theme's text colour, from the same palette the templates use.
            color: crate::theme::palette::color("TextFillColorPrimary")
                .or_else(|| crate::theme::palette::color("SystemControlForegroundBaseHighBrush"))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]),
        });
    }

    /*
     * An Image paints its picture, which the caller uploaded under the source it named. The
     * colour multiplies the texel, so it has to be white: a transparent one would sample the
     * bitmap and then erase it.
     */
    if matches!(element.kind(), crate::element::Kind::Image)
        && let Some(texture) = images.get(element.image_source())
    {
        visual.image = Some(*texture);
        visual.uv = [0.0, 0.0, 1.0, 1.0];
        visual.color = [1.0, 1.0, 1.0, 1.0];
        /*
         * `Stretch="Uniform"`, the default: the picture keeps its aspect inside the slot and
         * is centred there. A slot the bitmap fills exactly is unchanged; an icon in a large
         * cell stays an icon.
         */
        if let Some((natural_width, natural_height)) = element.intrinsic_size()
            && natural_width > 0.0
            && natural_height > 0.0
        {
            let slot = [layout.rect.width, layout.rect.height];
            let scale = (slot[0] / natural_width).min(slot[1] / natural_height);
            let fitted = [natural_width * scale, natural_height * scale];
            visual.size = fitted;
            visual.offset[0] += (slot[0] - fitted[0]) * 0.5;
            visual.offset[1] += (slot[1] - fitted[1]) * 0.5;
        }
    }

    /*
     * A render transform is a child object, not a visual: it moves the element it belongs
     * to. Translate offsets the node, scale changes its factors about the centre the
     * transform names.
     */
    for child in element.children() {
        match child.kind() {
            crate::element::Kind::TranslateTransform => {
                let (x, y) = child.translate();
                visual.offset[0] += x;
                visual.offset[1] += y;
            }
            crate::element::Kind::ScaleTransform => {
                visual.scale = child.scale().into();
                visual.center = child.center().into();
            }
            _ => {}
        }
    }

    for (child, child_layout) in element.children().iter().zip(&layout.children) {
        if crate::animation::is_transform(child.kind()) {
            continue;
        }
        visual.push(compose_with(child, child_layout, images));
    }
    visual
}

impl Element {
    /// Compose this element with the layout `arrange` produced for it.
    pub fn compose(&self, layout: &Layout) -> Visual {
        compose(self, layout)
    }

    /// Compose with pictures: an `Image` whose source is in `images` paints that texture.
    pub fn compose_with(
        &self,
        layout: &Layout,
        images: &HashMap<String, opencomp_core::TextureId>,
    ) -> Visual {
        compose_with(self, layout, images)
    }
}
