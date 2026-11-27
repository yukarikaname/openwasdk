//! `StackPanel` measure and arrange: one axis, spacing between children, margins inside
//! each slot. Ported from `dxaml/xcp/core/core/elements/StackPanel.cpp`.

use super::*;

pub(super) fn measure_stack(element: &Element, available: Size) -> Result<Size, LayoutError> {
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
    let mut visible = 0usize;
    for child in element.children() {
        if !child.is_visible() {
            continue;
        }
        visible += 1;
        let desired = measure(child, constraint)?;
        if vertical {
            main += desired.height;
            cross = cross.max(desired.width);
        } else {
            main += desired.width;
            cross = cross.max(desired.height);
        }
    }
    // `CStackPanel::MeasureOverride` multiplies spacing by visible - 1.
    main += element.spacing() * visible.saturating_sub(1) as f32;

    if vertical {
        Ok(Size::new(
            cross + chrome.horizontal(),
            main + chrome.vertical(),
        ))
    } else {
        Ok(Size::new(
            main + chrome.horizontal(),
            cross + chrome.vertical(),
        ))
    }
}

pub(super) fn horizontal_box(
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

pub(super) fn vertical_box(
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

pub(super) fn arrange_stack(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
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
        if !child.is_visible() {
            // The engine arranges the collapsed child but does not advance the
            // cursor for it; a zero-size rect records the same absence.
            let origin = if vertical {
                Rect::new(inner.x, cursor, 0.0, 0.0)
            } else {
                Rect::new(cursor, inner.y, 0.0, 0.0)
            };
            layouts.push(Layout {
                rect: origin,
                children: Vec::new(),
            });
            continue;
        }
        let desired = measure(child, constraint)?;
        let slot = if vertical {
            cross = cross.max(desired.width);
            Rect::new(inner.x, cursor, cross, desired.height)
        } else {
            cross = cross.max(desired.height);
            Rect::new(cursor, inner.y, desired.width, cross)
        };
        layouts.push(arrange_with_desired(child, slot, desired)?);
        cursor += if vertical {
            desired.height
        } else {
            desired.width
        } + spacing;
    }
    Ok(layouts)
}
