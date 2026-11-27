//! Measure and arrange, ported from the XAML core. No WinRT, no renderer.
//!
//! Rules and the file each came from:
//! - `dxaml/xcp/dxaml/lib/StackPanel_Partial.cpp` (peer) and
//!   `dxaml/xcp/core/core/elements/StackPanel.cpp` (`MeasureOverride`,
//!   `ArrangeOverride`): infinite constraint along the stacking axis; desired
//!   is the sum of the child desired sizes plus `Spacing * (visible - 1)`;
//!   arrange advances a cursor by the child slot plus spacing, so there is no
//!   spacing before the first or after the last child. A child's slot is its
//!   desired main size and `CLayoutElement::ArrangeCore` removes the child's
//!   margin inside that slot.
//! - `dxaml/xcp/core/core/elements/Grid.cpp` (`MeasureOverride`,
//!   `MeasureCell`, `GetDesiredInnerSize`, `ArrangeOverride`, `SetFinalSize`,
//!   `ResolveStar`, `DistributeStarSpace`): a pixel track keeps its value, an
//!   auto track takes the largest child desired size, a star contributes its
//!   content in measure, and in arrange the stars divide what pixel and auto
//!   tracks leave by weight. `ValidateDefinitions` turns every star into an
//!   auto when the available size is infinite.
//! - `dxaml/xcp/core/core/elements/Border.cpp` (`MeasureOverride`,
//!   `ArrangeOverride`, `HelperGetCombinedThickness`, `HelperGetInnerRect`,
//!   `HelperDeflateRect`): border thickness and padding both reduce the child
//!   constraint and add to desired; arrange removes both from the slot,
//!   clamped at zero. `CornerRadius` is read only while drawing and hit-testing
//!   (same file), never by measure or arrange.
//! - `dxaml/xcp/core/core/elements/layoutelement.cpp` (`MeasureCore`,
//!   `ArrangeCore`): margin is removed before measuring and added back; a
//!   desired size is rounded, then clipped to available; arrange uses a slot
//!   minus margin, `Stretch` fills it and an explicit `Width`/`Height` is
//!   centred.
//! - `dxaml/xcp/core/core/elements/uielement.cpp` (`LayoutRound` ->
//!   `LayoutRoundHelper` -> `XcpRound`) and the `UseLayoutRounding` property
//!   docs (default true): a size is rounded to the nearest whole layout pixel.
//!   This layer fixes the rasterisation scale at 1, so the plateau division in
//!   `LayoutRoundHelper` is not modelled. Ties use `f32::round` (away from
//!   zero); the body of `XcpRound` is not in the MIT tree we read, so the tie
//!   rule is assumed, not verified.
//!
//! `Grid_Partial.cpp` and `Border_Partial.cpp` are absent from the current MIT
//! tree; their layout lives in the core files above. Margins do not collapse:
//! 5 px bottom + 5 px top is a 10 px gap, as in WPF.
//!
//! Not modelled:
//! - grid spans (`Grid.RowSpan`/`ColumnSpan`) and the four measure groups that
//!   break the auto-row/star-column cycle; cells outside and inside a star
//!   track are measured in two passes instead.
//! - `Grid.RowSpacing`/`ColumnSpacing`, `MinWidth`/`MaxWidth`, and the overflow
//!   redistribution in `CGrid::SetFinalSize`.
//! - `Visibility`: every child counts as visible for `Spacing`.
//! - `UseLayoutRounding = false` and rasterisation scales other than 1: this
//!   layer always rounds to whole pixels and has no property to opt out.
//! - arrange measures each child again for its slot; the engine arranges with
//!   the desired size stored by measure and only re-measures a dirty child.
//! - text: `TextBlock` is a placeholder (`font_size * 1.2` high,
//!   `font_size * 0.5` per character). `CTextBlock::MeasureOverride` is
//!   declared in `core/inc/TextBlock.h` and its metrics come from the DWrite
//!   text layout (`core/native/text/Common/WinTextCore.cpp`); `FontSize` alone
//!   cannot produce them, so this layer does not pretend it can.

use opencomp_core::Rect;

use crate::element::{Element, HorizontalAlignment, Kind, Length, Thickness, VerticalAlignment};

mod grid;

/// A desired or available size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// The result of arranging one element: its box relative to the parent, in
/// parent coordinates, and the arranged children in the same local space.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub rect: Rect,
    pub children: Vec<Layout>,
}

impl Element {
    /// Measure this element for the given available size.
    pub fn measure(&self, available: Size) -> Size {
        measure(self, available)
    }

    /// Arrange in `slot` and clear the dirty flag this pass consumed.
    pub fn arrange(&mut self, slot: Rect) -> Layout {
        let layout = arrange(self, slot);
        self.mark_layout_clean();
        layout
    }
}

/// `CUIElement::LayoutRound` at rasterisation scale 1: distances that layout
/// writes are whole pixels, halves away from zero. See the module comment.
fn layout_round(value: f32) -> f32 {
    if value.is_finite() {
        value.round()
    } else {
        value
    }
}

/// `CBorder::GetLayoutRoundedThickness`: only the drawn border is rounded;
/// padding is not drawn, so it keeps its (possibly fractional) value.
fn rounded_border(element: &Element) -> Thickness {
    let border = element.border_thickness();
    Thickness::new(
        layout_round(border.left),
        layout_round(border.top),
        layout_round(border.right),
        layout_round(border.bottom),
    )
}

/// `CBorder::HelperGetCombinedThickness`: border thickness and padding are
/// both layout chrome and both are subtracted from the child slot.
fn chrome(element: &Element) -> Thickness {
    let border = rounded_border(element);
    let padding = element.padding();
    Thickness::new(
        border.left + padding.left,
        border.top + padding.top,
        border.right + padding.right,
        border.bottom + padding.bottom,
    )
}

/// Measure an element, applying the outer `FrameworkElement` rules and then the
/// rule for its kind.
pub fn measure(element: &Element, available: Size) -> Size {
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

    let content = measure_override(element, constraint);

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
    let mut desired = Size::new(
        layout_round(content_width) + layout_round(margin.horizontal()),
        layout_round(content_height) + layout_round(margin.vertical()),
    );

    if available.width.is_finite() {
        desired.width = desired.width.min(available.width.max(0.0));
    }
    if available.height.is_finite() {
        desired.height = desired.height.min(available.height.max(0.0));
    }
    desired
}

fn measure_override(element: &Element, available: Size) -> Size {
    match element.kind() {
        Kind::TextBlock => measure_text(element),
        Kind::Border => measure_border(element, available),
        Kind::StackPanel => measure_stack(element, available),
        Kind::Grid => grid::measure(element, available),
    }
}

fn measure_text(element: &Element) -> Size {
    let font_size = element.font_size();
    let characters = element.text().chars().count() as f32;
    Size::new(characters * font_size * 0.5, font_size * 1.2)
}

fn measure_border(element: &Element, available: Size) -> Size {
    let chrome = chrome(element);
    let child_available = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    match element.children().first() {
        Some(child) => {
            let desired = measure(child, child_available);
            Size::new(
                desired.width + chrome.horizontal(),
                desired.height + chrome.vertical(),
            )
        }
        None => Size::new(chrome.horizontal(), chrome.vertical()),
    }
}

fn measure_stack(element: &Element, available: Size) -> Size {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let vertical = element.orientation() == crate::element::Orientation::Vertical;
    let constraint = if vertical {
        Size::new(inner.width, f32::INFINITY)
    } else {
        Size::new(f32::INFINITY, inner.height)
    };

    let mut main = 0.0;
    let mut cross = 0.0_f32;
    for child in element.children() {
        let desired = measure(child, constraint);
        if vertical {
            main += desired.height;
            cross = cross.max(desired.width);
        } else {
            main += desired.width;
            cross = cross.max(desired.height);
        }
    }
    let gaps = element.children().len().saturating_sub(1) as f32;
    main += element.spacing() * gaps;

    if vertical {
        Size::new(cross + chrome.horizontal(), main + chrome.vertical())
    } else {
        Size::new(main + chrome.horizontal(), cross + chrome.vertical())
    }
}

/// Arrange in `slot`, measuring first so non-stretch alignments know their size.
pub fn arrange(element: &Element, slot: Rect) -> Layout {
    let desired = measure(element, Size::new(slot.width, slot.height));
    arrange_with_desired(element, slot, desired)
}

fn arrange_with_desired(element: &Element, slot: Rect, desired: Size) -> Layout {
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
    Layout {
        rect: Rect::new(x, y, width, height),
        children: arrange_children(element, local),
    }
}

fn horizontal_box(
    width: Length,
    alignment: HorizontalAlignment,
    desired: f32,
    client: Rect,
) -> (f32, f32) {
    let arranged = match (width, alignment) {
        (Length::Pixels(pixels), HorizontalAlignment::Stretch) => pixels.max(0.0).min(client.width),
        (Length::Pixels(pixels), _) => pixels.max(0.0),
        (Length::Auto, HorizontalAlignment::Stretch) => client.width,
        (Length::Auto, _) => desired,
    };
    let x = match alignment {
        HorizontalAlignment::Left => client.x,
        HorizontalAlignment::Center => client.x + (client.width - arranged) * 0.5,
        HorizontalAlignment::Right => client.x + client.width - arranged,
        // ArrangeCore centres a Stretch element that has an explicit Width.
        HorizontalAlignment::Stretch => client.x + (client.width - arranged) * 0.5,
    };
    (x, arranged)
}

fn vertical_box(
    height: Length,
    alignment: VerticalAlignment,
    desired: f32,
    client: Rect,
) -> (f32, f32) {
    let arranged = match (height, alignment) {
        (Length::Pixels(pixels), VerticalAlignment::Stretch) => pixels.max(0.0).min(client.height),
        (Length::Pixels(pixels), _) => pixels.max(0.0),
        (Length::Auto, VerticalAlignment::Stretch) => client.height,
        (Length::Auto, _) => desired,
    };
    let y = match alignment {
        VerticalAlignment::Top => client.y,
        VerticalAlignment::Center => client.y + (client.height - arranged) * 0.5,
        VerticalAlignment::Bottom => client.y + client.height - arranged,
        VerticalAlignment::Stretch => client.y + (client.height - arranged) * 0.5,
    };
    (y, arranged)
}

fn arrange_children(element: &Element, content: Rect) -> Vec<Layout> {
    match element.kind() {
        Kind::TextBlock => Vec::new(),
        Kind::Border => arrange_border(element, content),
        Kind::StackPanel => arrange_stack(element, content),
        Kind::Grid => grid::arrange(element, content),
    }
}

fn arrange_border(element: &Element, content: Rect) -> Vec<Layout> {
    // `CBorder::HelperGetInnerRect` deflates by border thickness and then by
    // padding, each step clamped; `CornerRadius` is not part of this.
    let inner = deflate(deflate(content, rounded_border(element)), element.padding());
    let Some(child) = element.children().first() else {
        return Vec::new();
    };
    let desired = measure(child, Size::new(inner.width, inner.height));
    vec![arrange_with_desired(child, inner, desired)]
}

fn arrange_stack(element: &Element, content: Rect) -> Vec<Layout> {
    let inner = deflate(content, chrome(element));
    let vertical = element.orientation() == crate::element::Orientation::Vertical;
    let spacing = element.spacing();
    let constraint = if vertical {
        Size::new(inner.width, f32::INFINITY)
    } else {
        Size::new(f32::INFINITY, inner.height)
    };

    let mut cursor = if vertical { inner.y } else { inner.x };
    let mut cross = if vertical { inner.width } else { inner.height };
    let mut layouts = Vec::with_capacity(element.children().len());
    for child in element.children() {
        let desired = measure(child, constraint);
        let slot = if vertical {
            cross = cross.max(desired.width);
            Rect::new(inner.x, cursor, cross, desired.height)
        } else {
            cross = cross.max(desired.height);
            Rect::new(cursor, inner.y, desired.width, cross)
        };
        layouts.push(arrange_with_desired(child, slot, desired));
        cursor += if vertical {
            desired.height
        } else {
            desired.width
        } + spacing;
    }
    layouts
}

/// `CBorder::HelperDeflateRect`: subtract a thickness, never going negative.
fn deflate(rect: Rect, thickness: Thickness) -> Rect {
    Rect::new(
        rect.x + thickness.left,
        rect.y + thickness.top,
        (rect.width - thickness.horizontal()).max(0.0),
        (rect.height - thickness.vertical()).max(0.0),
    )
}

fn reduce(length: f32, amount: f32) -> f32 {
    if length.is_finite() {
        (length - amount).max(0.0)
    } else {
        f32::INFINITY
    }
}
