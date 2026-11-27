//! Grid track resolution: measure from content, arrange from the final size.
//!
//! The rules and their upstream functions are documented in `layout.rs`; the
//! names below repeat the engine function each helper ports:
//! `MeasureCell`, `GetAvailableSizeForRange`, `GetDesiredInnerSize`,
//! `ResolveStar`, `DistributeStarSpace`, `EnsureMinSizeInDefinitionRange` and
//! `SetFinalSize`, all in `dxaml/xcp/core/core/elements/Grid.cpp`.

use opencomp_core::Rect;

use super::{
    Layout, LayoutError, Size, arrange_with_desired, chrome, deflate, measure as measure_element,
    reduce,
};
use crate::element::Element;

mod track;

use track::Tracks;

pub(super) fn measure(element: &Element, available: Size) -> Result<Size, LayoutError> {
    let chrome = chrome(element);
    let inner = Size::new(
        reduce(available.width, chrome.horizontal()),
        reduce(available.height, chrome.vertical()),
    );
    let (mut columns, mut rows) = axes(element, inner);

    natural(element, &mut columns, &mut rows)?;

    let column_count = element.columns().len().max(1);
    let row_count = element.rows().len().max(1);
    let width = columns.desired() + element.column_spacing() * (column_count - 1) as f32;
    let height = rows.desired() + element.row_spacing() * (row_count - 1) as f32;
    Ok(Size::new(
        width + chrome.horizontal(),
        height + chrome.vertical(),
    ))
}

pub(super) fn arrange(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    let inner = deflate(content, chrome(element));
    let (mut columns, mut rows) = axes(element, Size::new(inner.width, inner.height));
    let column_spacing = element.column_spacing();
    let row_spacing = element.row_spacing();

    natural(element, &mut columns, &mut rows)?;
    columns.set_final_size();
    rows.set_final_size();

    let column_offsets = columns.offsets();
    let row_offsets = rows.offsets();
    let mut layouts = Vec::with_capacity(element.children().len());
    for child in element.children() {
        let (column, column_span) = columns.range(child.grid_column(), child.grid_column_span());
        let (row, row_span) = rows.range(child.grid_row(), child.grid_row_span());

        // `CGrid::ArrangeOverride`: the offset carries the spacing before the
        // first track, the size carries the spacing inside the span.
        let x = inner.x + column_offsets[column] + column_spacing * column as f32;
        let y = inner.y + row_offsets[row] + row_spacing * row as f32;
        let width =
            columns.range_size(column, column_span) + column_spacing * (column_span - 1) as f32;
        let height = rows.range_size(row, row_span) + row_spacing * (row_span - 1) as f32;
        let slot = Rect::new(x, y, width, height);

        if !child.is_visible() {
            // A collapsed child leaves the cell empty and changes no track.
            layouts.push(Layout {
                rect: Rect::new(x, y, 0.0, 0.0),
                children: Vec::new(),
            });
            continue;
        }
        let desired = measure_element(child, Size::new(slot.width, slot.height))?;
        layouts.push(arrange_with_desired(child, slot, desired)?);
    }
    Ok(layouts)
}

/// Build both axes. The spacing between tracks is subtracted from the size the
/// tracks share (`CGrid::MeasureOverride`, `CGrid::ArrangeOverride`).
fn axes(element: &Element, inner: Size) -> (Tracks, Tracks) {
    let column_count = element.columns().len().max(1);
    let row_count = element.rows().len().max(1);
    let column_space = reduce(
        inner.width,
        element.column_spacing() * (column_count - 1) as f32,
    );
    let row_space = reduce(inner.height, element.row_spacing() * (row_count - 1) as f32);
    (
        Tracks::new(element.columns(), element.column_spacing(), column_space),
        Tracks::new(element.rows(), element.row_spacing(), row_space),
    )
}

/// `CGrid::MeasureOverride`: cells outside a star track first, then a star
/// resolution, then the cells inside a star track. The engine's four groups are
/// collapsed into those two passes; the difference is documented in `layout.rs`.
fn natural(element: &Element, columns: &mut Tracks, rows: &mut Tracks) -> Result<(), LayoutError> {
    for star_pass in [false, true] {
        for child in element.children() {
            if !child.is_visible() {
                continue;
            }
            let (column, column_span) =
                columns.range(child.grid_column(), child.grid_column_span());
            let (row, row_span) = rows.range(child.grid_row(), child.grid_row_span());
            let in_star =
                columns.range_has_star(column, column_span) || rows.range_has_star(row, row_span);
            if in_star != star_pass {
                continue;
            }
            // A span over a star track is the engine's cyclic-measure case.
            if column_span > 1 && columns.range_has_star(column, column_span) {
                return Err(LayoutError::SpanAcrossStarTrack);
            }
            if row_span > 1 && rows.range_has_star(row, row_span) {
                return Err(LayoutError::SpanAcrossStarTrack);
            }

            let constraint = Size::new(
                columns.cell_constraint(column, column_span),
                rows.cell_constraint(row, row_span),
            );
            let desired = measure_element(child, constraint)?;
            columns.contribute(column, column_span, desired.width);
            rows.contribute(row, row_span, desired.height);
        }
        if !star_pass {
            columns.resolve_stars();
            rows.resolve_stars();
        }
    }
    Ok(())
}
