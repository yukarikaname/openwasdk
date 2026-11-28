//! Measure and arrange, ported from the XAML core. No WinRT, no renderer.
//!
//! Rules and the file each came from:
//! - `dxaml/xcp/core/core/elements/StackPanel.cpp` (peer) and
//!   `dxaml/xcp/core/core/elements/StackPanel.cpp` (`MeasureOverride`,
//!   `ArrangeOverride`): infinite constraint along the stacking axis; desired
//!   is the sum of the child desired sizes plus `Spacing * (visible - 1)`, where
//!   `visible` counts only children whose `IsVisible` is true; arrange advances
//!   a cursor by the child slot plus spacing, so there is no spacing before the
//!   first or after the last visible child. A collapsed child is placed in a
//!   zero-size rect and never advances the cursor. A child's slot is its desired
//!   main size and `CLayoutElement::ArrangeCore` removes the child's margin
//!   inside that slot.
//! - `dxaml/xcp/core/core/elements/Grid.cpp` (`MeasureOverride`, `MeasureCell`,
//!   `GetAvailableSizeForRange`, `GetDesiredInnerSize`, `ValidateDefinitions`,
//!   `ArrangeOverride`, `SetFinalSize`, `ResolveStar`, `DistributeStarSpace`,
//!   `EnsureMinSizeInDefinitionRange`, `SortDefinitionsForStarSizeDistribution`,
//!   `SortDefinitionsForSpanMaxSizeDistribution`,
//!   `SortDefinitionsForOverflowSizeDistribution`): a pixel track keeps its
//!   value clamped to its Min/Max, an auto track takes the largest child
//!   desired size, a star contributes its content in measure, and in arrange
//!   the stars divide what pixel and auto tracks leave by weight. `RowSpacing`
//!   and `ColumnSpacing` are removed from the available size before that
//!   division and are added only between tracks. A star's resolved size is
//!   clamped to `[effectiveMin, userMax]`; the clamp happens in allocation order
//!   (lowest `maxSize / weight` first), so a track that hits its Max leaves the
//!   remainder for the later stars and a track forced up by its Min takes space
//!   from them. Each star is allocated once. After the stars, if the total still
//!   exceeds the final size, `SetFinalSize`'s overflow pass shrinks every track
//!   once in ascending `(size - effectiveMin)` order. `ValidateDefinitions`
//!   turns every star into an auto when the available size is infinite.
//! - `dxaml/xcp/core/core/elements/Grid.cpp` spans: `GetRowSpan`/`GetColumnSpan`
//!   clamp to the remaining tracks; `MeasureCell` measures a spanning child
//!   against `GetAvailableSizeForRange` (the sum of the spanned tracks plus the
//!   spacing between them, or infinity when the range contains an auto and no
//!   star track); `RegisterSpan` plus `EnsureMinSizeInDefinitionRange` spread
//!   the child's desired size back over the spanned tracks; `ArrangeOverride`
//!   places it in `GetFinalSizeForRange`. This layer refuses a span that covers
//!   a star track with [`LayoutError::SpanAcrossStarTrack`]; the engine resolves
//!   those cells through the cyclic measure groups (groups 2', 2'' in
//!   `CGrid::MeasureOverride`) that this layer does not model.
//! - `dxaml/xcp/core/core/elements/Border.cpp` (`MeasureOverride`,
//!   `ArrangeOverride`, `HelperGetCombinedThickness`, `HelperGetInnerRect`,
//!   `HelperDeflateRect`): border thickness and padding both reduce the child
//!   constraint and add to desired; arrange removes both from the slot,
//!   clamped at zero. `CornerRadius` is read only while drawing and hit-testing
//!   (same file), never by measure or arrange. A collapsed child contributes
//!   nothing to desired and gets a zero-size rect.
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
//! - `dxaml/xcp/core/core/elements/uielement.h`: `GetIsLayoutSuspended` is
//!   `IsCollapsed`, and `uielement.cpp`'s `MeasureInternal` returns before
//!   `MeasureCore` while suspended. The zero desired size that follows is
//!   inferred from `CLayoutElement::GetActualWidth`'s comment (a collapsed
//!   element has no layout storage and reports 0); the reset itself lives in a
//!   visibility file absent from the local tree, so that step is unverified.
//!
//! `Grid_Partial.cpp` and `Border_Partial.cpp` are absent from the current MIT
//! tree; their layout lives in the core files above. Margins do not collapse:
//! 5 px bottom + 5 px top is a 10 px gap, as in WPF.
//!
//! Not modelled:
//! - the four measure groups of `CGrid::MeasureOverride` (`ValidateCells`):
//!   cells outside a star track are measured first, then the stars are
//!   resolved, then cells inside a star track are measured. The engine's
//!   group-2'/group-2'' cycle handling is not here, which is why a span across
//!   a star track is an error rather than a number.
//! - the `SortDefinitionsForSpanPreferredDistribution` branch of
//!   `EnsureMinSizeInDefinitionRange`: for a span with no star track every
//!   definition's `GetPreferredSize` is its effective min (auto returns the min
//!   by definition; a pixel track's measure and min are the same clamped
//!   value), so `rangePreferred == rangeMin` and that branch cannot be reached.
//!   The source is cited rather than exercised.
//! - `GRID_STARVALUE_MAX`, the overflow cap on star weights, is defined in
//!   `Grid.h`, which is not in the local tree; weights are used as given.
//! - `UseLayoutRounding = false` and rasterisation scales other than 1: this
//!   layer always rounds to whole pixels and has no property to opt out.
//! - arrange measures each child again for its slot; the engine arranges with
//!   the desired size stored by measure and only re-measures a dirty child.
//! - text: `TextBlock` is a placeholder (`font_size * 1.2` high,
//!   `font_size * 0.5` per character). `CTextBlock::MeasureOverride` is
//!   declared in `core/inc/TextBlock.h` and its metrics come from the DWrite
//!   text layout (`core/native/text/Common/WinTextCore.cpp`); `FontSize` alone
//!   cannot produce them, so this layer does not pretend it can.
//!
//! The numbers are this port's; conformance to the engine needs the measurements
//! `docs/composition-baseline.md` records.

use std::fmt;

use opencomp_core::Rect;

use crate::element::{Element, HorizontalAlignment, Length, Thickness, VerticalAlignment};

mod border;
mod grid;
mod stack;

mod canvas;

/// Why a layout pass could not follow an engine rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// A `Grid` child spans more than one track and the span covers a star
    /// track. `CGrid::MeasureOverride` resolves such cells through the cyclic
    /// measure groups, which this layer does not model; refusing is preferable
    /// to inventing a constraint.
    SpanAcrossStarTrack,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpanAcrossStarTrack => {
                write!(formatter, "a Grid span across a star track is not modelled")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

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
    pub fn measure(&self, available: Size) -> Result<Size, LayoutError> {
        measure(self, available)
    }

    /// Arrange in `slot` and clear the dirty flag this pass consumed.
    pub fn arrange(&mut self, slot: Rect) -> Result<Layout, LayoutError> {
        let layout = arrange(self, slot)?;
        self.mark_layout_clean();
        Ok(layout)
    }
}

/// `CUIElement::LayoutRound` at rasterisation scale 1: distances that layout
/// writes are whole pixels, halves away from zero. See the module comment.
pub(super) fn layout_round(value: f32) -> f32 {
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
pub(super) fn chrome(element: &Element) -> Thickness {
    let border = rounded_border(element);
    let padding = element.padding();
    Thickness::new(
        border.left + padding.left,
        border.top + padding.top,
        border.right + padding.right,
        border.bottom + padding.bottom,
    )
}

mod panels;
mod pass;
mod relative;

pub use pass::{arrange, measure};
pub(crate) use pass::{arrange_with_desired, collapsed, deflate, reduce};
