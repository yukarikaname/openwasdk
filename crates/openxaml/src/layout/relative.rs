//! `RelativePanel`: children anchored to the panel's edges or to a named sibling.
//!
//! WinUI resolves anchors in this order: a panel-edge flag wins, then an alignment with a
//! named sibling, then the placement relations (`RightOf`, `Below`, ...). A child whose
//! anchor is missing keeps the panel's origin, which is what the runtime does rather than
//! refusing the page.

use std::collections::HashMap;

use super::{
    Layout, LayoutError, Rect, Size, arrange_with_desired, chrome, deflate,
    measure as measure_element, reduce,
};
use crate::element::{Element, Value};

/// Where one child landed, by the name later anchors refer to.
struct Placed {
    rect: Rect,
}

pub(super) fn measure(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let placed = place(element, inner)?;
    let mut width = 0.0_f32;
    let mut height = 0.0_f32;
    for spot in placed.values() {
        width = width.max(spot.rect.x + spot.rect.width);
        height = height.max(spot.rect.y + spot.rect.height);
    }
    Ok(Size::new(
        width + chrome.horizontal(),
        height + chrome.vertical(),
    ))
}

pub(super) fn arrange(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    let inner = deflate(content, chrome(element));
    let placed = place(element, Size::new(inner.width, inner.height))?;
    let mut layouts = Vec::with_capacity(element.children().len());
    for (index, child) in element.children().iter().enumerate() {
        if !child.is_visible() {
            layouts.push(Layout {
                rect: Rect::new(inner.x, inner.y, 0.0, 0.0),
                children: Vec::new(),
            });
            continue;
        }
        let spot = placed
            .get(&key(child, index))
            .map(|spot| spot.rect)
            .unwrap_or_else(|| Rect::new(inner.x, inner.y, 0.0, 0.0));
        let rect = Rect::new(inner.x + spot.x, inner.y + spot.y, spot.width, spot.height);
        let desired = measure_element(child, Size::new(rect.width, rect.height))?;
        layouts.push(arrange_with_desired(child, rect, desired)?);
    }
    Ok(layouts)
}

/// Resolve every child's rectangle inside a panel of `inner` size. Measure and arrange share
/// this, so the two passes cannot disagree about where an anchor points.
fn place(element: &Element, inner: Size) -> Result<HashMap<String, Placed>, LayoutError> {
    let mut placed: HashMap<String, Placed> = HashMap::new();
    for (index, child) in element.children().iter().enumerate() {
        if !child.is_visible() {
            continue;
        }
        let desired = measure_element(child, inner)?;
        let width = desired.width;
        let height = desired.height;
        let (x, y) = origin(child, width, height, inner, &placed);
        placed.insert(
            key(child, index),
            Placed {
                rect: Rect::new(x, y, width, height),
            },
        );
    }
    Ok(placed)
}

fn origin(
    child: &Element,
    width: f32,
    height: f32,
    inner: Size,
    placed: &HashMap<String, Placed>,
) -> (f32, f32) {
    let x = if flag(child, "AlignLeftWithPanel") {
        0.0
    } else if flag(child, "AlignRightWithPanel") {
        inner.width - width
    } else if flag(child, "AlignHorizontalCenterWithPanel") {
        (inner.width - width) * 0.5
    } else if let Some(spot) = named(child, "AlignLeftWith", placed) {
        spot.rect.x
    } else if let Some(spot) = named(child, "AlignRightWith", placed) {
        spot.rect.x + spot.rect.width - width
    } else if let Some(spot) = named(child, "AlignHorizontalCenterWith", placed) {
        spot.rect.x + (spot.rect.width - width) * 0.5
    } else if let Some(spot) = named(child, "RightOf", placed) {
        spot.rect.x + spot.rect.width
    } else if let Some(spot) = named(child, "LeftOf", placed) {
        spot.rect.x - width
    } else {
        0.0
    };
    let y = if flag(child, "AlignTopWithPanel") {
        0.0
    } else if flag(child, "AlignBottomWithPanel") {
        inner.height - height
    } else if flag(child, "AlignVerticalCenterWithPanel") {
        (inner.height - height) * 0.5
    } else if let Some(spot) = named(child, "AlignTopWith", placed) {
        spot.rect.y
    } else if let Some(spot) = named(child, "AlignBottomWith", placed) {
        spot.rect.y + spot.rect.height - height
    } else if let Some(spot) = named(child, "AlignVerticalCenterWith", placed) {
        spot.rect.y + (spot.rect.height - height) * 0.5
    } else if let Some(spot) = named(child, "Below", placed) {
        spot.rect.y + spot.rect.height
    } else if let Some(spot) = named(child, "Above", placed) {
        spot.rect.y - height
    } else {
        0.0
    };
    (x, y)
}

fn flag(child: &Element, member: &str) -> bool {
    matches!(child.attached(member), Some(Value::Bool(true)))
}

fn named<'a>(
    child: &Element,
    member: &str,
    placed: &'a HashMap<String, Placed>,
) -> Option<&'a Placed> {
    let Value::Text(name) = child.attached(member)? else {
        return None;
    };
    placed.get(name.as_ref())
}

/// The key a child is known by: its name, or its index when it has none.
fn key(child: &Element, index: usize) -> String {
    child
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{index}"))
}
