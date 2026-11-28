//! Item panels: wrap, uniform grid, viewbox, and the span-carrying wrap grid.
//!
//! The first three are WPF rules and the last is WinUI's: children are measured against a
//! cell and then placed by index, wrapping when the line is full. They share one file
//! because they share that shape of answer.

use super::{
    Layout, LayoutError, Rect, Size, arrange_with_desired, chrome, deflate,
    measure as measure_element, reduce,
};
use crate::element::{Element, Orientation};

/// One child's placement inside a panel's inner rectangle.
struct Slot {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

pub(super) fn measure_wrap(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let horizontal = element.orientation() == Orientation::Horizontal;
    let (item_width, item_height) = element.item_size();
    let limit = if horizontal {
        inner.width
    } else {
        inner.height
    };
    let mut slots = Vec::new();
    let (main, cross) = walk(
        element,
        horizontal,
        item_width,
        item_height,
        limit,
        &mut slots,
    )?;
    let (width, height) = if horizontal {
        (main, cross)
    } else {
        (cross, main)
    };
    Ok(Size::new(
        width + chrome.horizontal(),
        height + chrome.vertical(),
    ))
}

pub(super) fn arrange_wrap(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    let inner = deflate(content, chrome(element));
    let horizontal = element.orientation() == Orientation::Horizontal;
    let (item_width, item_height) = element.item_size();
    let limit = if horizontal {
        inner.width
    } else {
        inner.height
    };
    let mut slots = Vec::new();
    walk(
        element,
        horizontal,
        item_width,
        item_height,
        limit,
        &mut slots,
    )?;
    let mut layouts = Vec::with_capacity(element.children().len());
    let mut index = 0;
    for child in element.children() {
        if !child.is_visible() {
            layouts.push(collapsed(inner));
            continue;
        }
        let slot = &slots[index];
        index += 1;
        let rect = Rect::new(inner.x + slot.x, inner.y + slot.y, slot.width, slot.height);
        let desired = measure_element(child, Size::new(rect.width, rect.height))?;
        layouts.push(arrange_with_desired(child, rect, desired)?);
    }
    Ok(layouts)
}

/// Walk the children once, filling `slots` in the panel's inner coordinates and returning
/// the extent along the main and cross axes.
fn walk(
    element: &Element,
    horizontal: bool,
    item_width: Option<f32>,
    item_height: Option<f32>,
    limit: f32,
    slots: &mut Vec<Slot>,
) -> Result<(f32, f32), LayoutError> {
    let mut main: f32 = 0.0;
    let mut cross: f32 = 0.0;
    let mut line_main: f32 = 0.0;
    let mut line_cross: f32 = 0.0;
    for child in element.children() {
        if !child.is_visible() {
            continue;
        }
        let desired = measure_element(
            child,
            Size::new(
                item_width.unwrap_or(f32::INFINITY),
                item_height.unwrap_or(f32::INFINITY),
            ),
        )?;
        let width = item_width.unwrap_or(desired.width);
        let height = item_height.unwrap_or(desired.height);
        let (step, thick) = if horizontal {
            (width, height)
        } else {
            (height, width)
        };
        if line_main > 0.0 && limit.is_finite() && line_main + step > limit {
            main = main.max(line_main);
            cross += line_cross;
            line_main = 0.0;
            line_cross = 0.0;
        }
        let slot = if horizontal {
            Slot {
                x: line_main,
                y: cross,
                width,
                height,
            }
        } else {
            Slot {
                x: cross,
                y: line_main,
                width,
                height,
            }
        };
        slots.push(slot);
        line_main += step;
        line_cross = line_cross.max(thick);
    }
    main = main.max(line_main);
    cross += line_cross;
    Ok((main, cross))
}

pub(super) fn measure_uniform(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let (columns, rows) = grid_shape(element);
    let mut cell = Size::new(0.0, 0.0);
    for child in element.children() {
        if !child.is_visible() {
            continue;
        }
        let desired = measure_element(child, inner)?;
        cell.width = cell.width.max(desired.width);
        cell.height = cell.height.max(desired.height);
    }
    Ok(Size::new(
        cell.width * columns as f32 + chrome.horizontal(),
        cell.height * rows as f32 + chrome.vertical(),
    ))
}

pub(super) fn arrange_uniform(
    element: &Element,
    content: Rect,
) -> Result<Vec<Layout>, LayoutError> {
    let inner = deflate(content, chrome(element));
    let (columns, rows) = grid_shape(element);
    let cell_width = inner.width / columns as f32;
    let cell_height = inner.height / rows as f32;
    let first = element.count(&crate::property::FIRST_COLUMN).unwrap_or(0);
    let mut layouts = Vec::with_capacity(element.children().len());
    let mut index = 0;
    for child in element.children() {
        if !child.is_visible() {
            layouts.push(collapsed(inner));
            continue;
        }
        let position = index + first;
        index += 1;
        let column = position % columns;
        let row = (position / columns) % rows;
        let rect = Rect::new(
            inner.x + column as f32 * cell_width,
            inner.y + row as f32 * cell_height,
            cell_width,
            cell_height,
        );
        let desired = measure_element(child, Size::new(rect.width, rect.height))?;
        layouts.push(arrange_with_desired(child, rect, desired)?);
    }
    Ok(layouts)
}

/// A viewbox gives its one child the whole slot; keeping the ratio is the composition
/// step's job, not layout's.
pub(super) fn measure_viewbox(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let desired = match element.children().first() {
        Some(child) if child.is_visible() => measure_element(child, inner)?,
        _ => Size::new(0.0, 0.0),
    };
    Ok(Size::new(
        desired.width + chrome.horizontal(),
        desired.height + chrome.vertical(),
    ))
}

pub(super) fn arrange_viewbox(
    element: &Element,
    content: Rect,
) -> Result<Vec<Layout>, LayoutError> {
    let inner = deflate(content, chrome(element));
    let Some(child) = element.children().first() else {
        return Ok(Vec::new());
    };
    if !child.is_visible() {
        return Ok(vec![collapsed(inner)]);
    }
    let desired = measure_element(child, Size::new(inner.width, inner.height))?;
    Ok(vec![arrange_with_desired(child, inner, desired)?])
}

/// The columns and rows a uniform grid wants: as asked, or squared off from the child count.
fn grid_shape(element: &Element) -> (usize, usize) {
    let count = element
        .children()
        .iter()
        .filter(|child| child.is_visible())
        .count()
        .max(1);
    let columns = element
        .count(&crate::property::COLUMNS)
        .unwrap_or_else(|| (count as f64).sqrt().ceil() as usize)
        .max(1)
        .min(count);
    let rows = element
        .count(&crate::property::ROWS)
        .unwrap_or(count.div_ceil(columns))
        .max(1);
    (columns, rows)
}

fn collapsed(inner: Rect) -> Layout {
    Layout {
        rect: Rect::new(inner.x, inner.y, 0.0, 0.0),
        children: Vec::new(),
    }
}
