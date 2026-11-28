//! `Canvas` measure and arrange: children sit at their `Canvas.Left`/`Canvas.Top`
//! offset and keep the size they ask for, so the panel never constrains them.
//! Ported from `dxaml/xcp/core/core/elements/Canvas.cpp` (`MeasureOverride` gives each
//! child an infinite slot; `ArrangeOverride` places it at the attached offset).

use super::*;

pub(super) fn measure_canvas(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let slot = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    // The engine hands every child an infinite constraint and takes the furthest extent.
    let unbounded = Size::new(f32::INFINITY, f32::INFINITY);
    let mut extent = Size::new(0.0, 0.0);
    for child in element.children() {
        if !child.is_visible() {
            continue;
        }
        let desired = measure(child, unbounded)?;
        let right = child.canvas_left() + desired.width;
        let bottom = child.canvas_top() + desired.height;
        extent = Size::new(extent.width.max(right), extent.height.max(bottom));
    }
    let _ = slot;
    Ok(Size::new(
        extent.width + chrome.horizontal(),
        extent.height + chrome.vertical(),
    ))
}

pub(super) fn arrange_canvas(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    let inner = deflate(deflate(content, rounded_border(element)), element.padding());
    let mut out = Vec::new();
    for child in element.children() {
        if !child.is_visible() {
            out.push(collapsed(inner));
            continue;
        }
        let desired = measure(child, Size::new(f32::INFINITY, f32::INFINITY))?;
        let slot = Rect::new(
            inner.x + child.canvas_left(),
            inner.y + child.canvas_top(),
            desired.width,
            desired.height,
        );
        out.push(arrange_with_desired(child, slot, desired)?);
    }
    Ok(out)
}
