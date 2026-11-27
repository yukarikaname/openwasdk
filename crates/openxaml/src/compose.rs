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

use opencomp_core::Visual;

use crate::element::Element;
use crate::layout::Layout;

/// Build the visual tree for an element and its arranged layout.
pub fn compose(element: &Element, layout: &Layout) -> Visual {
    let color = element.background().unwrap_or([0.0; 4]);
    let mut visual =
        Visual::rect(layout.rect.width, layout.rect.height, color).at(layout.rect.x, layout.rect.y);

    for (child, child_layout) in element.children().iter().zip(&layout.children) {
        visual.push(compose(child, child_layout));
    }
    visual
}

impl Element {
    /// Compose this element with the layout `arrange` produced for it.
    pub fn compose(&self, layout: &Layout) -> Visual {
        compose(self, layout)
    }
}
