//! Exact-number checks for the layout rules.
//!
//! The numbers are hand-computed from the engine algorithms named in `src/layout.rs`; a
//! passing test says this layer implements the rule, not that WinUI produces the same
//! number. That comparison needs the numbers `scripts/gate-b.sh` collects, and the first
//! one of those -- a vertical `StackPanel` with `Spacing = 4` and three 10 px children --
//! is 38, which is also what this file asserts.

use openxaml::{Element, GridTrack, LayoutError, Orientation, Rect, Size, Thickness, Visibility};

/// `Element` has `set_width` but no builder for it, so the tests use one.
trait TapWidth {
    fn tap_width(self, width: f32) -> Self;
}

impl TapWidth for Element {
    fn tap_width(mut self, width: f32) -> Self {
        self.set_width(width);
        self
    }
}

fn ten_high_text() -> Element {
    Element::text_block("x")
        .with_font_size(20.0)
        .with_height(10.0)
}

#[test]
fn vertical_stack_adds_three_heights_and_two_spacings() {
    let mut panel = Element::stack_panel(Orientation::Vertical);
    panel.set_spacing(4.0);
    for _ in 0..3 {
        panel.push(ten_high_text());
    }

    // `CStackPanel::MeasureOverride`: sum of desired plus spacing * (n - 1).
    let measured = panel
        .measure(Size::new(100.0, f32::INFINITY))
        .expect("measure");
    assert_eq!(measured, Size::new(10.0, 38.0));

    // `CStackPanel::ArrangeOverride`: spacing advances the cursor between children
    // only, so the first starts at 0 and the last ends at the sum.
    let layout = panel
        .arrange(Rect::new(0.0, 0.0, 100.0, 38.0))
        .expect("arrange");
    assert_eq!(layout.children[0].rect.y, 0.0);
    assert_eq!(layout.children[1].rect.y, 14.0);
    assert_eq!(layout.children[2].rect.y, 28.0);
    assert_eq!(
        layout.children[2].rect.y + layout.children[2].rect.height,
        38.0
    );
}

#[test]
fn stack_child_margin_stays_inside_its_slot() {
    let mut panel = Element::stack_panel(Orientation::Vertical);
    panel.push(ten_high_text().with_margin(Thickness::new(5.0, 5.0, 5.0, 5.0)));

    // `CLayoutElement::ArrangeCore` removes the child's margin from the slot contents
    // and leaves the slot itself at the full height, so the margin is inside it.
    let measured = panel
        .measure(Size::new(100.0, f32::INFINITY))
        .expect("measure");
    assert_eq!(measured.height, 20.0);

    let layout = panel
        .arrange(Rect::new(0.0, 0.0, 100.0, 20.0))
        .expect("arrange");
    assert_eq!(layout.children[0].rect.y, 5.0);
    assert_eq!(layout.children[0].rect.height, 10.0);
}

#[test]
fn a_collapsed_child_takes_no_space_and_no_spacing() {
    let mut panel = Element::stack_panel(Orientation::Vertical);
    panel.set_spacing(4.0);
    panel.push(ten_high_text());
    panel.push(ten_high_text().with_visibility(Visibility::Collapsed));
    panel.push(ten_high_text());

    // A collapsed child is not laid out at all: two visible children leave one gap.
    let measured = panel
        .measure(Size::new(100.0, f32::INFINITY))
        .expect("measure");
    assert_eq!(measured.height, 24.0);

    let layout = panel
        .arrange(Rect::new(0.0, 0.0, 100.0, 24.0))
        .expect("arrange");
    assert_eq!(layout.children[0].rect.y, 0.0);
    // The collapsed child keeps its place in the tree and gets no rectangle.
    assert_eq!(layout.children[1].rect.width, 0.0);
    assert_eq!(layout.children[1].rect.height, 0.0);
    assert_eq!(layout.children[2].rect.y, 14.0);
}

#[test]
fn border_padding_and_thickness_shrink_the_child_slot() {
    let mut border = Element::border();
    border.set_padding(Thickness::new(8.0, 8.0, 8.0, 8.0));
    border.set_border_thickness(Thickness::new(2.0, 2.0, 2.0, 2.0));
    border.push(ten_high_text().with_height(20.0).tap_width(20.0));

    // `CBorder::HelperGetCombinedThickness`: desired chrome is thickness + padding on
    // both sides, so 20 + 2 * 8 + 2 * 2 = 40.
    let measured = border
        .measure(Size::new(f32::INFINITY, f32::INFINITY))
        .expect("measure");
    assert_eq!(measured, Size::new(40.0, 40.0));

    let layout = border
        .arrange(Rect::new(0.0, 0.0, 40.0, 40.0))
        .expect("arrange");
    assert_eq!(layout.children[0].rect, Rect::new(10.0, 10.0, 20.0, 20.0));
}

#[test]
fn border_corner_radius_does_not_change_the_child_slot() {
    let mut rounded = Element::border();
    rounded.set_padding(Thickness::new(6.0, 6.0, 6.0, 6.0));
    rounded.set_corner_radius(50.0);
    rounded.push(ten_high_text().with_height(10.0).tap_width(10.0));

    // `CornerRadius` appears in drawing and hit testing, never in measure or arrange.
    let measured = rounded
        .measure(Size::new(f32::INFINITY, f32::INFINITY))
        .expect("measure");
    assert_eq!(measured, Size::new(22.0, 22.0));
}

#[test]
fn grid_takes_pixel_and_auto_before_splitting_stars() {
    let mut grid = Element::grid();
    grid.set_columns([
        GridTrack::pixels(20.0),
        GridTrack::auto(),
        GridTrack::star(1.0),
        GridTrack::star(2.0),
    ]);
    for column in 0..4 {
        grid.push(Element::border().with_grid_column(column));
    }
    grid.children_mut()[1] = Element::text_block("abc")
        .with_font_size(20.0)
        .with_grid_column(1);

    let layout = grid
        .arrange(Rect::new(0.0, 0.0, 100.0, 24.0))
        .expect("arrange");
    // `CGrid::ResolveStar`: 20 + 30 are taken, 50 is left. The 2-star is allocated
    // first (largest weight), round(50 * 2 / 3) = 33, then the 1-star takes the 17
    // that remains.
    assert_eq!(layout.children[0].rect, Rect::new(0.0, 0.0, 20.0, 24.0));
    assert_eq!(layout.children[1].rect, Rect::new(20.0, 0.0, 30.0, 24.0));
    assert_eq!(layout.children[2].rect, Rect::new(50.0, 0.0, 17.0, 24.0));
    assert_eq!(layout.children[3].rect, Rect::new(67.0, 0.0, 33.0, 24.0));
}

#[test]
fn grid_measures_a_star_as_its_content() {
    let mut grid = Element::grid();
    grid.set_columns([GridTrack::star(1.0)]);
    grid.push(Element::border().tap_width(30.0));

    // `CGrid::GetDesiredInnerSize`: a star track's desired size is its content
    // minimum, so the share only appears at arrange time.
    let measured = grid.measure(Size::new(200.0, 200.0)).expect("measure");
    assert_eq!(measured.width, 30.0);
}

#[test]
fn grid_gives_a_star_the_space_at_arrange() {
    let mut grid = Element::grid();
    grid.set_columns([GridTrack::star(1.0)]);
    // No explicit width: a grid stretches a child into its cell.
    grid.push(Element::border());

    let layout = grid
        .arrange(Rect::new(0.0, 0.0, 200.0, 50.0))
        .expect("arrange");
    assert_eq!(layout.children[0].rect.width, 200.0);
}

#[test]
fn grid_star_rounding_absorbs_the_residual_in_allocation_order() {
    let mut grid = Element::grid();
    grid.set_columns([GridTrack::star(1.0); 3]);
    for column in 0..3 {
        grid.push(Element::border().with_grid_column(column));
    }

    let layout = grid
        .arrange(Rect::new(0.0, 0.0, 100.0, 10.0))
        .expect("arrange");
    // Three equal stars in 100 px: 33, then 33 of the 67 left, then the last 34.
    let widths: Vec<f32> = layout
        .children
        .iter()
        .map(|child| child.rect.width)
        .collect();
    // `DistributeStarSpace` allocates in order out of what is left: 100/3 rounds to 33,
    // then 67/2 is 33.5 and rounds away from zero to 34, then the last takes 33.
    assert_eq!(widths, vec![33.0, 34.0, 33.0]);
}

#[test]
fn grid_spacing_is_between_tracks_only() {
    let mut grid = Element::grid();
    grid.set_columns([GridTrack::pixels(10.0); 3]);
    grid.set_column_spacing(4.0);
    for column in 0..3 {
        grid.push(Element::border().with_grid_column(column));
    }

    let measured = grid
        .measure(Size::new(f32::INFINITY, 10.0))
        .expect("measure");
    // Two gaps of 4 between three 10 px tracks, and nothing outside them.
    assert_eq!(measured.width, 38.0);

    let layout = grid
        .arrange(Rect::new(0.0, 0.0, 38.0, 10.0))
        .expect("arrange");
    assert_eq!(layout.children[1].rect.x, 14.0);
    assert_eq!(layout.children[2].rect.x, 28.0);
}

#[test]
fn a_span_across_a_star_track_is_refused() {
    let mut grid = Element::grid();
    grid.set_columns([GridTrack::star(1.0), GridTrack::pixels(20.0)]);
    grid.push(
        Element::border()
            .with_grid_column(0)
            .with_grid_column_span(2),
    );

    // `CGrid` resolves a span through its cyclic measure groups; this layer does not
    // model them, so it refuses rather than inventing a constraint.
    let error = grid
        .measure(Size::new(100.0, 100.0))
        .expect_err("a star span is refused");
    assert_eq!(error, LayoutError::SpanAcrossStarTrack);
}

#[test]
fn a_collapsed_element_arranges_to_nothing() {
    let mut border = Element::border();
    border.set_visibility(Visibility::Collapsed);
    border.push(ten_high_text());

    let layout = border
        .arrange(Rect::new(0.0, 0.0, 50.0, 50.0))
        .expect("arrange");
    assert_eq!(layout.rect, Rect::new(0.0, 0.0, 0.0, 0.0));
    assert!(layout.children.is_empty());
}

#[test]
fn text_block_measures_from_its_placeholder_metrics() {
    let text = Element::text_block("abcd").with_font_size(20.0);

    // Four characters at 20 * 0.5 wide, one line at 20 * 1.2 high; the placeholder is
    // not a text engine, which is why the module doc lists shaping as unmodelled.
    let measured = text.measure(Size::new(200.0, 200.0)).expect("measure");
    assert_eq!(measured, Size::new(40.0, 24.0));
}
