//! `Border` measure and arrange: border thickness and padding both deflate the child
//! slot, each clamped at zero, and `CornerRadius` never enters layout.
//! Ported from `dxaml/xcp/core/core/elements/Border.cpp`.

use super::stack::{horizontal_box, vertical_box};
use super::*;

pub(super) fn measure_border(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let child_available = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let desired = match element.children().first() {
        Some(child) if child.is_visible() => measure(child, child_available)?,
        _ => Size::new(0.0, 0.0),
    };
    Ok(Size::new(
        desired.width + chrome.horizontal(),
        desired.height + chrome.vertical(),
    ))
}

pub(super) fn arrange_border(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    // `CBorder::HelperGetInnerRect` deflates by border thickness and then by
    // padding, each step clamped; `CornerRadius` is not part of this.
    let inner = deflate(deflate(content, rounded_border(element)), element.padding());
    let Some(child) = element.children().first() else {
        return Ok(Vec::new());
    };
    if !child.is_visible() {
        return Ok(vec![collapsed(inner)]);
    }
    let desired = measure(child, Size::new(inner.width, inner.height))?;
    /*
     * `HorizontalContentAlignment` and its vertical pair place the one child: Stretch, the
     * default, fills the slot; anything else arranges the child at its own size.
     */
    let (horizontal, vertical) = element.content_alignment();
    let (x, width) = horizontal_box(Length::Auto, horizontal, desired.width, inner);
    let (y, height) = vertical_box(Length::Auto, vertical, desired.height, inner);
    let slot = Rect::new(x, y, width, height);
    Ok(vec![arrange_with_desired(child, slot, desired)?])
}
