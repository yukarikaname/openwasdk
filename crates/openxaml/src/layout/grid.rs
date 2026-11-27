//! Grid track resolution: measure from content, arrange from the final size.
//!
//! The rules and their upstream functions are documented in `layout.rs`; the
//! names below repeat the engine function each helper ports:
//! `MeasureCell`, `GetDesiredInnerSize`, `ResolveStar`, `DistributeStarSpace`
//! and `SetFinalSize`, all in `dxaml/xcp/core/core/elements/Grid.cpp`.

use opencomp_core::Rect;

use super::{
    Layout, Size, arrange_with_desired, chrome, deflate, layout_round, measure as measure_element,
    reduce,
};
use crate::element::{Element, GridLength};

pub(super) fn measure(element: &Element, available: Size) -> Size {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let columns = element.columns();
    let rows = element.rows();
    let (natural_columns, natural_rows) = natural(element, columns, rows, inner);

    let width: f32 = desired_tracks(columns, &natural_columns).iter().sum();
    let height: f32 = desired_tracks(rows, &natural_rows).iter().sum();
    Size::new(width + chrome.horizontal(), height + chrome.vertical())
}

pub(super) fn arrange(element: &Element, content: Rect) -> Vec<Layout> {
    let inner = deflate(content, chrome(element));
    let columns = element.columns();
    let rows = element.rows();
    let (natural_columns, natural_rows) =
        natural(element, columns, rows, Size::new(inner.width, inner.height));

    // In arrange the final size wins: an implicit track fills the Grid and a
    // star track takes the share `SetFinalSize` gives it.
    let column_sizes = if columns.is_empty() {
        vec![inner.width.max(0.0)]
    } else {
        resolve(columns, inner.width, &natural_columns)
    };
    let row_sizes = if rows.is_empty() {
        vec![inner.height.max(0.0)]
    } else {
        resolve(rows, inner.height, &natural_rows)
    };
    let column_offsets = offsets(&column_sizes);
    let row_offsets = offsets(&row_sizes);

    let mut layouts = Vec::with_capacity(element.children().len());
    for child in element.children() {
        let column = child.grid_column().min(column_sizes.len() - 1);
        let row = child.grid_row().min(row_sizes.len() - 1);
        let slot = Rect::new(
            inner.x + column_offsets[column],
            inner.y + row_offsets[row],
            column_sizes[column],
            row_sizes[row],
        );
        let desired = measure_element(child, Size::new(slot.width, slot.height));
        layouts.push(arrange_with_desired(child, slot, desired));
    }
    layouts
}

/// `CGrid::GetDesiredInnerSize`: a pixel track contributes its value, an auto
/// track and a star track contribute their largest child desired size. A star
/// is therefore auto-like in measure even when the available size is finite;
/// it only becomes a share during arrange (`CGrid::SetFinalSize`).
fn desired_tracks(definitions: &[GridLength], natural: &[f32]) -> Vec<f32> {
    if definitions.is_empty() {
        return vec![natural.first().copied().unwrap_or(0.0)];
    }
    definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| match *definition {
            GridLength::Pixels(pixels) => layout_round(pixels.max(0.0)),
            GridLength::Auto | GridLength::Star(_) => {
                natural.get(index).copied().unwrap_or(0.0).max(0.0)
            }
        })
        .collect()
}

/// `CGrid::MeasureOverride` and `CGrid::MeasureCell`: collect the largest
/// desired size per track. A cell in an auto track is measured freely, a pixel
/// cell with the pixel value, a star cell with its share of what the other
/// tracks leave. Cells in a star track go second so the auto sizes exist.
fn natural(
    element: &Element,
    columns: &[GridLength],
    rows: &[GridLength],
    inner: Size,
) -> (Vec<f32>, Vec<f32>) {
    let column_count = columns.len().max(1);
    let row_count = rows.len().max(1);
    let mut natural_columns = vec![0.0_f32; column_count];
    let mut natural_rows = vec![0.0_f32; row_count];

    for star_cell in [false, true] {
        for child in element.children() {
            let column = child.grid_column().min(column_count - 1);
            let row = child.grid_row().min(row_count - 1);
            let in_star =
                star_track(columns, column, inner.width) || star_track(rows, row, inner.height);
            if in_star != star_cell {
                continue;
            }
            let constraint = Size::new(
                cell_constraint(columns, column, inner.width, &natural_columns),
                cell_constraint(rows, row, inner.height, &natural_rows),
            );
            let desired = measure_element(child, constraint);
            natural_columns[column] = natural_columns[column].max(desired.width);
            natural_rows[row] = natural_rows[row].max(desired.height);
        }
    }
    (natural_columns, natural_rows)
}

/// `CGrid::ValidateDefinitions`: with an infinite constraint a star is an auto,
/// so it is measured freely and contributes its content to desired.
fn star_track(definitions: &[GridLength], index: usize, available: f32) -> bool {
    matches!(definitions.get(index), Some(GridLength::Star(_))) && available.is_finite()
}

/// `CGrid::MeasureCell`: the constraint one cell gives its child.
fn cell_constraint(
    definitions: &[GridLength],
    index: usize,
    available: f32,
    natural: &[f32],
) -> f32 {
    match definitions.get(index) {
        None => available,
        Some(GridLength::Auto) => f32::INFINITY,
        Some(GridLength::Pixels(pixels)) => layout_round(pixels.max(0.0)),
        Some(GridLength::Star(weight)) => {
            if !available.is_finite() {
                return f32::INFINITY;
            }
            let weights = star_weight(definitions);
            if weights == 0.0 {
                0.0
            } else {
                (layout_round(available) - fixed_tracks(definitions, natural)).max(0.0)
                    * weight.max(0.0)
                    / weights
            }
        }
    }
}

/// `CGrid::ResolveStar`: pixel and auto tracks come off the available size
/// before the stars share the remainder.
fn fixed_tracks(definitions: &[GridLength], natural: &[f32]) -> f32 {
    definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| match *definition {
            GridLength::Pixels(pixels) => layout_round(pixels.max(0.0)),
            GridLength::Auto => natural.get(index).copied().unwrap_or(0.0).max(0.0),
            GridLength::Star(_) => 0.0,
        })
        .sum()
}

fn star_weight(definitions: &[GridLength]) -> f32 {
    definitions
        .iter()
        .map(|definition| match *definition {
            GridLength::Star(weight) => weight.max(0.0),
            _ => 0.0,
        })
        .sum()
}

/// `CGrid::SetFinalSize` and `CGrid::DistributeStarSpace`: a pixel keeps its
/// layout-rounded value, an auto track its content, and the stars divide the
/// remainder. `SortDefinitionsForStarSizeDistribution` orders the allocation by
/// `maxSize / weight`, which with no Min/Max values is largest weight first.
/// Each share is rounded to a layout pixel and clamped to the track's content.
fn resolve(definitions: &[GridLength], available: f32, natural: &[f32]) -> Vec<f32> {
    let mut sizes = vec![0.0_f32; definitions.len().max(1)];
    if definitions.is_empty() {
        sizes[0] = available.max(0.0);
        return sizes;
    }

    let mut stars = Vec::new();
    for (index, definition) in definitions.iter().enumerate() {
        match *definition {
            GridLength::Pixels(pixels) => sizes[index] = layout_round(pixels.max(0.0)),
            GridLength::Auto => {
                sizes[index] = natural.get(index).copied().unwrap_or(0.0).max(0.0);
            }
            GridLength::Star(weight) => stars.push((index, weight.max(0.0))),
        }
    }

    stars.sort_by(|left, right| right.1.total_cmp(&left.1));
    let distributed = available.is_finite();
    let mut left =
        (layout_round(available) - layout_round(fixed_tracks(definitions, natural))).max(0.0);
    let mut remaining: f32 = stars.iter().map(|star| star.1).sum();
    for (index, weight) in stars {
        let share = if distributed && weight > 0.0 && remaining > 0.0 {
            layout_round(left * weight / remaining)
        } else {
            0.0
        };
        let size = share.max(natural.get(index).copied().unwrap_or(0.0).max(0.0));
        sizes[index] = size;
        left = (left - size).max(0.0);
        remaining -= weight;
    }
    sizes
}

fn offsets(sizes: &[f32]) -> Vec<f32> {
    let mut offsets = Vec::with_capacity(sizes.len());
    let mut offset = 0.0;
    for size in sizes {
        offsets.push(offset);
        offset += size;
    }
    offsets
}
