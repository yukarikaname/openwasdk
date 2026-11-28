//! The two public passes: measure and arrange. Kept apart from `layout.rs` so the
//! module that defines the geometry types is not also the module that dispatches them.

use opencomp_core::Rect;

use super::border::{arrange_border, measure_border};
use super::stack::{arrange_stack, horizontal_box, measure_stack, vertical_box};
use super::{Layout, LayoutError, Size, canvas, grid, layout_round};
use crate::element::{Element, Length, Panel, Role, Thickness};

/// Measure an element, applying the outer `FrameworkElement` rules and then the
/// rule for its kind.
pub fn measure(element: &Element, available: Size) -> Result<Size, LayoutError> {
    // A collapsed subtree never enters MeasureCore; it contributes nothing,
    // margin included (`CUIElement::GetIsLayoutSuspended`).
    if !element.is_visible() {
        return Ok(Size::new(0.0, 0.0));
    }

    let margin = element.margin();
    let inner_available = Size::new(
        reduce(available.width, margin.horizontal()),
        reduce(available.height, margin.vertical()),
    );

    let constraint = Size::new(
        match element.width() {
            Length::Pixels(width) => width.max(0.0),
            Length::Auto => inner_available.width,
        },
        match element.height() {
            Length::Pixels(height) => height.max(0.0),
            Length::Auto => inner_available.height,
        },
    );

    let content = measure_override(element, constraint)?;

    // `CLayoutElement::MeasureCore` rounds the content and the margin sums
    // separately before adding the margin back.
    let content_width = match element.width() {
        Length::Pixels(width) => width.max(0.0),
        Length::Auto => content.width,
    };
    let content_height = match element.height() {
        Length::Pixels(height) => height.max(0.0),
        Length::Auto => content.height,
    };
    let desired = Size::new(
        layout_round(content_width) + layout_round(margin.horizontal()),
        layout_round(content_height) + layout_round(margin.vertical()),
    );

    /*
     * The desired size is deliberately not clamped to the available one: XAML lets a child
     * ask for more room than it was offered, and a star track learns its content that way.
     * Clamping here is what collapsed a centred stack inside a star row to zero height.
     */
    Ok(desired)
}

fn measure_override(element: &Element, available: Size) -> Result<Size, LayoutError> {
    match element.kind().role() {
        Role::Text => Ok(measure_text(element)),
        // A panel measures its children its own way.
        Role::Panel(panel) => match panel {
            Panel::Stack => measure_stack(element, available),
            Panel::Grid => grid::measure(element, available),
            Panel::Canvas => canvas::measure_canvas(element, available),
        },
        // A content host carries only chrome: border thickness and padding deflate the one
        // child, and nothing else is laid out.
        Role::ContentHost => measure_border(element, available),
        // A resource dictionary, a style, an inline: nothing visual below it.
        Role::NonVisual => Ok(Size::new(0.0, 0.0)),
    }
}

fn measure_text(element: &Element) -> Size {
    let font_size = element.font_size();
    let characters = element.text().chars().count() as f32;
    Size::new(characters * font_size * 0.5, font_size * 1.2)
}

pub fn arrange(element: &Element, slot: Rect) -> Result<Layout, LayoutError> {
    if !element.is_visible() {
        return Ok(Layout {
            rect: Rect::new(slot.x, slot.y, 0.0, 0.0),
            children: Vec::new(),
        });
    }
    let desired = measure(element, Size::new(slot.width, slot.height))?;
    arrange_with_desired(element, slot, desired)
}

pub(crate) fn arrange_with_desired(
    element: &Element,
    slot: Rect,
    desired: Size,
) -> Result<Layout, LayoutError> {
    let margin = element.margin();
    let client = Rect::new(
        slot.x + margin.left,
        slot.y + margin.top,
        (slot.width - margin.horizontal()).max(0.0),
        (slot.height - margin.vertical()).max(0.0),
    );
    let desired_inner = Size::new(
        (desired.width - margin.horizontal()).max(0.0),
        (desired.height - margin.vertical()).max(0.0),
    );

    let (x, width) = horizontal_box(
        element.width(),
        element.horizontal_alignment(),
        desired_inner.width,
        client,
    );
    let (y, height) = vertical_box(
        element.height(),
        element.vertical_alignment(),
        desired_inner.height,
        client,
    );

    let local = Rect::new(0.0, 0.0, width, height);
    Ok(Layout {
        rect: Rect::new(x, y, width, height),
        children: arrange_children(element, local)?,
    })
}

fn arrange_children(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    match element.kind().role() {
        Role::Text => Ok(Vec::new()),
        Role::Panel(panel) => match panel {
            Panel::Stack => arrange_stack(element, content),
            Panel::Grid => grid::arrange(element, content),
            Panel::Canvas => canvas::arrange_canvas(element, content),
        },
        Role::ContentHost => arrange_border(element, content),
        // Nothing below a non-visual host is arranged, so nothing of it is composed.
        Role::NonVisual => Ok(Vec::new()),
    }
}

pub(crate) fn collapsed(origin: Rect) -> Layout {
    Layout {
        rect: Rect::new(origin.x, origin.y, 0.0, 0.0),
        children: Vec::new(),
    }
}

/// `CBorder::HelperDeflateRect`: subtract a thickness, never going negative.
pub(crate) fn deflate(rect: Rect, thickness: Thickness) -> Rect {
    Rect::new(
        rect.x + thickness.left,
        rect.y + thickness.top,
        (rect.width - thickness.horizontal()).max(0.0),
        (rect.height - thickness.vertical()).max(0.0),
    )
}

pub(crate) fn reduce(length: f32, amount: f32) -> f32 {
    if length.is_finite() {
        (length - amount).max(0.0)
    } else {
        f32::INFINITY
    }
}
