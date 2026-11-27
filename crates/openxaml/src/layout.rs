//! Measure and arrange, ported from the XAML core. No WinRT, no renderer.
//!
//! Rules and the file each came from:
//! - `dxaml/xcp/dxaml/lib/StackPanel_Partial.cpp` (peer) and
//!   `core/elements/StackPanel.cpp` (`MeasureOverride`/`ArrangeOverride`):
//!   infinite constraint along the stacking axis, available across it; desired
//!   is the sum + `Spacing * (n - 1)` and the cross maximum; arrange advances a
//!   cursor by each desired main size plus spacing.
//! - `core/elements/Grid.cpp` (`MeasureOverride`, `ArrangeOverride`,
//!   `SetFinalSize`): pixel tracks keep their value, auto tracks take the
//!   largest child desired size, stars divide what pixels and autos leave by
//!   weight. An infinite constraint treats stars as autos.
//! - `core/elements/Border.cpp` (`MeasureOverride`/`ArrangeOverride`): measure
//!   the child with both axes reduced by `Padding`; desired is child plus
//!   padding; arrange the child in the inner rectangle.
//! - `core/elements/layoutelement.cpp` (`MeasureCore`/`ArrangeCore`): margin is
//!   removed before measuring and added back; arrange uses a slot minus margin;
//!   `Stretch` fills it and explicit `Width`/`Height` are centred; desired is
//!   clipped to available.
//! - `dxaml/xcp/dxaml/lib/FrameworkElement_Partial.cpp` and
//!   `core/elements/FrameworkElement.cpp`: alignment `Stretch`, margin zero,
//!   `Width`/`Height` auto.
//!
//! `Grid_Partial.cpp` and `Border_Partial.cpp` are absent from the current MIT
//! tree; their layout lives in the core files above.
//! Margins do not collapse: 5 px bottom + 5 px top is a 10 px gap, as in WPF.
//! `TextBlock` is a placeholder (`font_size * 1.2` high, `font_size * 0.5` per
//! character) until DWrite or HarfBuzz is wired; there is no shaping.

use opencomp_core::Rect;

use crate::element::{Element, GridLength, HorizontalAlignment, Kind, Length, VerticalAlignment};

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

    /// Arrange this element in the slot its parent gave it. The returned
    /// rectangles are relative to the parent, ready for composition.
    pub fn arrange(&self, slot: Rect) -> Layout {
        arrange(self, slot)
    }
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
    let mut desired = Size::new(
        match element.width() {
            Length::Pixels(width) => width.max(0.0),
            Length::Auto => content.width,
        } + margin.horizontal(),
        match element.height() {
            Length::Pixels(height) => height.max(0.0),
            Length::Auto => content.height,
        } + margin.vertical(),
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
        Kind::Grid => measure_grid(element, available),
    }
}

fn measure_text(element: &Element) -> Size {
    let font_size = element.font_size();
    let characters = element.text().chars().count() as f32;
    Size::new(characters * font_size * 0.5, font_size * 1.2)
}

fn measure_border(element: &Element, available: Size) -> Size {
    let padding = element.padding();
    let child_available = Size::new(
        reduce(available.width, padding.horizontal()),
        reduce(available.height, padding.vertical()),
    );
    match element.children().first() {
        Some(child) => {
            let desired = measure(child, child_available);
            Size::new(
                desired.width + padding.horizontal(),
                desired.height + padding.vertical(),
            )
        }
        None => Size::new(padding.horizontal(), padding.vertical()),
    }
}

fn measure_stack(element: &Element, available: Size) -> Size {
    let padding = element.padding();
    let inner = Size::new(
        reduce(available.width, padding.horizontal()),
        reduce(available.height, padding.vertical()),
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
        Size::new(cross + padding.horizontal(), main + padding.vertical())
    } else {
        Size::new(main + padding.horizontal(), cross + padding.vertical())
    }
}

fn measure_grid(element: &Element, available: Size) -> Size {
    let padding = element.padding();
    let inner = Size::new(
        reduce(available.width, padding.horizontal()),
        reduce(available.height, padding.vertical()),
    );
    let columns = element.columns();
    let rows = element.rows();
    let (natural_columns, natural_rows) = grid_natural(element, columns, rows, inner);

    let column_sizes = if columns.is_empty() {
        natural_columns
    } else {
        resolve(columns, inner.width, &natural_columns)
    };
    let row_sizes = if rows.is_empty() {
        natural_rows
    } else {
        resolve(rows, inner.height, &natural_rows)
    };

    let width: f32 = column_sizes.iter().sum();
    let height: f32 = row_sizes.iter().sum();
    Size::new(width + padding.horizontal(), height + padding.vertical())
}

/// Collect the largest desired size per row and column; spans are not supported.
fn grid_natural(
    element: &Element,
    columns: &[GridLength],
    rows: &[GridLength],
    inner: Size,
) -> (Vec<f32>, Vec<f32>) {
    let column_count = columns.len().max(1);
    let row_count = rows.len().max(1);
    let mut natural_columns = vec![0.0_f32; column_count];
    let mut natural_rows = vec![0.0_f32; row_count];

    for child in element.children() {
        let column = child.grid_column().min(column_count - 1);
        let row = child.grid_row().min(row_count - 1);
        let constraint = Size::new(
            track_constraint(columns, column, inner.width),
            track_constraint(rows, row, inner.height),
        );
        let desired = measure(child, constraint);
        natural_columns[column] = natural_columns[column].max(desired.width);
        natural_rows[row] = natural_rows[row].max(desired.height);
    }
    (natural_columns, natural_rows)
}

/// The constraint a cell gives its child while natural sizes are collected.
fn track_constraint(definitions: &[GridLength], index: usize, available: f32) -> f32 {
    match definitions.get(index) {
        None => available,
        Some(GridLength::Pixels(pixels)) => pixels.max(0.0),
        Some(GridLength::Auto) => f32::INFINITY,
        Some(GridLength::Star(weight)) => {
            if !available.is_finite() {
                return f32::INFINITY;
            }
            let pixels = definitions
                .iter()
                .filter_map(|definition| match definition {
                    GridLength::Pixels(pixels) => Some(pixels.max(0.0)),
                    _ => None,
                })
                .sum::<f32>();
            let stars = definitions
                .iter()
                .filter_map(|definition| match definition {
                    GridLength::Star(weight) => Some(weight.max(0.0)),
                    _ => None,
                })
                .sum::<f32>();
            if stars == 0.0 {
                0.0
            } else {
                (available - pixels).max(0.0) * weight.max(0.0) / stars
            }
        }
    }
}

/// Distribute like `CGrid::SetFinalSize` before it computes offsets.
fn resolve(definitions: &[GridLength], available: f32, natural: &[f32]) -> Vec<f32> {
    let mut sizes = vec![0.0; definitions.len().max(1)];
    let mut fixed = 0.0;
    let mut star_weight = 0.0;

    for (index, definition) in definitions.iter().enumerate() {
        let wanted = natural.get(index).copied().unwrap_or(0.0).max(0.0);
        match *definition {
            GridLength::Pixels(pixels) => {
                sizes[index] = pixels.max(0.0);
                fixed += sizes[index];
            }
            GridLength::Auto => {
                sizes[index] = wanted;
                fixed += wanted;
            }
            GridLength::Star(weight) => star_weight += weight.max(0.0),
        }
    }

    if star_weight > 0.0 {
        let left = available.is_finite().then(|| (available - fixed).max(0.0));
        for (index, definition) in definitions.iter().enumerate() {
            if let GridLength::Star(weight) = *definition {
                sizes[index] = match left {
                    Some(left) => left * weight.max(0.0) / star_weight,
                    None => natural.get(index).copied().unwrap_or(0.0).max(0.0),
                };
            }
        }
    }
    sizes
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
        Kind::Grid => arrange_grid(element, content),
    }
}

fn arrange_border(element: &Element, content: Rect) -> Vec<Layout> {
    let inner = deflate(content, element.padding());
    let Some(child) = element.children().first() else {
        return Vec::new();
    };
    let desired = measure(child, Size::new(inner.width, inner.height));
    vec![arrange_with_desired(child, inner, desired)]
}

fn arrange_stack(element: &Element, content: Rect) -> Vec<Layout> {
    let inner = deflate(content, element.padding());
    let vertical = element.orientation() == crate::element::Orientation::Vertical;
    let spacing = element.spacing();
    let constraint = if vertical {
        Size::new(inner.width, f32::INFINITY)
    } else {
        Size::new(f32::INFINITY, inner.height)
    };

    let mut cursor = if vertical { inner.y } else { inner.x };
    let mut layouts = Vec::with_capacity(element.children().len());
    for child in element.children() {
        let desired = measure(child, constraint);
        let slot = if vertical {
            Rect::new(
                inner.x,
                cursor,
                inner.width.max(desired.width),
                desired.height,
            )
        } else {
            Rect::new(
                cursor,
                inner.y,
                desired.width,
                inner.height.max(desired.height),
            )
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

fn arrange_grid(element: &Element, content: Rect) -> Vec<Layout> {
    let inner = deflate(content, element.padding());
    let columns = element.columns();
    let rows = element.rows();
    let (natural_columns, natural_rows) =
        grid_natural(element, columns, rows, Size::new(inner.width, inner.height));

    // In arrange the final size wins: an implicit track fills the Grid and a
    // star track takes the same share SetFinalSize would give it.
    let column_sizes = if columns.is_empty() {
        vec![inner.width]
    } else {
        resolve(columns, inner.width, &natural_columns)
    };
    let row_sizes = if rows.is_empty() {
        vec![inner.height]
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
        let desired = measure(child, Size::new(slot.width, slot.height));
        layouts.push(arrange_with_desired(child, slot, desired));
    }
    layouts
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

fn deflate(rect: Rect, thickness: crate::element::Thickness) -> Rect {
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
