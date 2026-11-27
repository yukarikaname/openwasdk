//! Exact-number checks for the layout rules. The numbers are hand-computed
//! from the engine algorithms named in `src/layout.rs`; a passing test says this
//! layer implements the rule, not that WinUI produces the same number. That
//! comparison needs the numbers `scripts/gate-b.sh` collects.

use opencomp_core::Rect;
use openxaml::{Element, GridLength, Orientation, Size, Thickness};

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
    let measured = panel.measure(Size::new(100.0, f32::INFINITY));
    assert_eq!(measured, Size::new(10.0, 38.0));

    // `CStackPanel::ArrangeOverride`: spacing advances the cursor between
    // children only, so the first starts at 0 and the last ends at the sum.
    let layout = panel.arrange(Rect::new(0.0, 0.0, 100.0, 38.0));
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
    let child = ten_high_text().with_margin(Thickness::new(10.0, 0.0, 10.0, 0.0));
    let mut panel = Element::stack_panel(Orientation::Vertical);
    panel.push(child);

    let layout = panel.arrange(Rect::new(0.0, 0.0, 100.0, 10.0));
    // The slot is the panel width; `CLayoutElement::ArrangeCore` removes the
    // child's margin inside that slot instead of shrinking the slot.
    assert_eq!(layout.children[0].rect, Rect::new(10.0, 0.0, 80.0, 10.0));
}

#[test]
fn grid_gives_the_auto_column_its_desired_width() {
    let auto = Element::text_block("abc").with_font_size(20.0);
    let star = Element::border();

    let mut grid = Element::grid();
    grid.set_columns([GridLength::Star(1.0), GridLength::Auto]);
    grid.push(star.with_grid_column(0));
    grid.push(auto.with_grid_column(1));

    let layout = grid.arrange(Rect::new(0.0, 0.0, 100.0, 50.0));
    assert_eq!(layout.children[0].rect, Rect::new(0.0, 0.0, 70.0, 50.0));
    assert_eq!(layout.children[1].rect, Rect::new(70.0, 0.0, 30.0, 50.0));
}

#[test]
fn grid_takes_pixel_and_auto_before_splitting_stars() {
    let mut grid = Element::grid();
    grid.set_columns([
        GridLength::Pixels(20.0),
        GridLength::Auto,
        GridLength::Star(1.0),
        GridLength::Star(2.0),
    ]);
    for column in 0..4 {
        grid.push(Element::border().with_grid_column(column));
    }
    grid.children_mut()[1] = Element::text_block("abc")
        .with_font_size(20.0)
        .with_grid_column(1);

    let layout = grid.arrange(Rect::new(0.0, 0.0, 100.0, 24.0));
    // `CGrid::ResolveStar`: 20 + 30 are taken, 50 is left. The 2-star is
    // allocated first (largest weight), round(50 * 2 / 3) = 33, then the
    // 1-star takes the 17 that remains.
    assert_eq!(layout.children[0].rect, Rect::new(0.0, 0.0, 20.0, 24.0));
    assert_eq!(layout.children[1].rect, Rect::new(20.0, 0.0, 30.0, 24.0));
    assert_eq!(layout.children[2].rect, Rect::new(50.0, 0.0, 17.0, 24.0));
    assert_eq!(layout.children[3].rect, Rect::new(67.0, 0.0, 33.0, 24.0));
}

#[test]
fn grid_measures_a_star_as_content_and_arranges_it_as_a_share() {
    let mut grid = Element::grid();
    grid.set_columns([GridLength::Auto, GridLength::Star(1.0)]);
    grid.push(
        Element::text_block("abc")
            .with_font_size(20.0)
            .with_grid_column(0),
    );
    grid.push(
        Element::text_block("ab")
            .with_font_size(20.0)
            .with_grid_column(1),
    );

    // `CGrid::GetDesiredInnerSize`: a star contributes its content (20), so
    // the desired width is 30 + 20 under either constraint. An infinite
    // constraint turns the star into an auto (`ValidateDefinitions`).
    assert_eq!(
        grid.measure(Size::new(f32::INFINITY, f32::INFINITY)),
        Size::new(50.0, 24.0)
    );
    assert_eq!(grid.measure(Size::new(200.0, 24.0)), Size::new(50.0, 24.0));

    // In arrange (`SetFinalSize`) the star takes what is left of the final
    // size, so the same tree fills 200.
    let layout = grid.arrange(Rect::new(0.0, 0.0, 200.0, 24.0));
    assert_eq!(layout.children[0].rect, Rect::new(0.0, 0.0, 30.0, 24.0));
    assert_eq!(layout.children[1].rect, Rect::new(30.0, 0.0, 170.0, 24.0));
}

#[test]
fn grid_star_rounding_absorbs_the_residual_in_allocation_order() {
    let mut grid = Element::grid();
    grid.set_columns([GridLength::Star(1.0); 3]);
    for column in 0..3 {
        grid.push(Element::border().with_grid_column(column));
    }

    let layout = grid.arrange(Rect::new(0.0, 0.0, 100.0, 10.0));
    // `CUIElement::LayoutRound` on each `DistributeStarSpace` share:
    // round(100 / 3) = 33, round(67 / 2) = round(33.5) = 34 (away from zero),
    // then 33. The rounded total stays 100.
    assert_eq!(layout.children[0].rect, Rect::new(0.0, 0.0, 33.0, 10.0));
    assert_eq!(layout.children[1].rect, Rect::new(33.0, 0.0, 34.0, 10.0));
    assert_eq!(layout.children[2].rect, Rect::new(67.0, 0.0, 33.0, 10.0));
}

#[test]
fn border_border_thickness_and_padding_both_shrink_the_child_slot() {
    let mut border = Element::border()
        .with_border_thickness(2.0)
        .with_padding(8.0);
    border.push(Element::border());

    // `CBorder::HelperGetCombinedThickness` adds both to the desired size...
    assert_eq!(
        border.measure(Size::new(100.0, 100.0)),
        Size::new(20.0, 20.0)
    );

    // ...and `HelperGetInnerRect` removes both from the child slot.
    let layout = border.arrange(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(layout.children[0].rect, Rect::new(10.0, 10.0, 80.0, 80.0));
}

#[test]
fn border_corner_radius_does_not_change_the_child_slot() {
    let plain = Element::border()
        .with_border_thickness(2.0)
        .with_padding(8.0);
    let rounded = Element::border()
        .with_border_thickness(2.0)
        .with_padding(8.0)
        .with_corner_radius(20.0);
    for mut border in [plain, rounded] {
        border.push(Element::border());
        let layout = border.arrange(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(layout.children[0].rect, Rect::new(10.0, 10.0, 80.0, 80.0));
    }
}

#[test]
fn border_thinner_than_padding_does_not_go_negative() {
    let mut border = Element::border()
        .with_border_thickness(1.0)
        .with_padding(10.0);
    border.push(Element::border());

    // 15 - 1 - 10 is negative; `HelperDeflateRect` clamps each axis at zero.
    let layout = border.arrange(Rect::new(0.0, 0.0, 15.0, 15.0));
    assert_eq!(layout.children[0].rect, Rect::new(11.0, 11.0, 0.0, 0.0));
}

#[test]
fn margins_between_siblings_add_instead_of_collapsing() {
    let first = ten_high_text().with_margin(Thickness::new(0.0, 0.0, 0.0, 5.0));
    let second = ten_high_text().with_margin(Thickness::new(0.0, 5.0, 0.0, 0.0));

    let mut panel = Element::stack_panel(Orientation::Vertical);
    panel.push(first);
    panel.push(second);

    // 15 + 15: each margin is part of its own child's desired size.
    assert_eq!(
        panel.measure(Size::new(100.0, f32::INFINITY)),
        Size::new(10.0, 30.0)
    );

    let layout = panel.arrange(Rect::new(0.0, 0.0, 100.0, 30.0));
    assert_eq!(layout.children[0].rect, Rect::new(0.0, 0.0, 100.0, 10.0));
    // 10 + 5 + 5, not max(5, 5) as CSS margin collapsing would give.
    assert_eq!(layout.children[1].rect, Rect::new(0.0, 20.0, 100.0, 10.0));
}

#[test]
fn text_block_measures_with_the_placeholder_formula() {
    let mut text = Element::text_block("abcd").with_font_size(20.0);

    // Four characters at 20 * 0.5 wide, one line at 20 * 1.2 high.
    assert_eq!(text.measure(Size::new(200.0, 200.0)), Size::new(40.0, 24.0));

    let layout = text.arrange(Rect::new(0.0, 0.0, 40.0, 24.0));
    assert_eq!(layout.rect, Rect::new(0.0, 0.0, 40.0, 24.0));
}

#[test]
fn compose_carries_background_offset_size_and_children() {
    let background = [0.1, 0.2, 0.3, 1.0];
    let mut border = Element::border()
        .with_padding(8.0)
        .with_background(background);
    border.push(Element::border());

    let layout = border.arrange(Rect::new(10.0, 20.0, 100.0, 100.0));
    let visual = border.compose(&layout);

    assert_eq!(visual.offset, [10.0, 20.0]);
    assert_eq!(visual.size, [100.0, 100.0]);
    assert_eq!(visual.color, background);
    assert_eq!(visual.children.len(), 1);
    assert_eq!(visual.children[0].offset, [8.0, 8.0]);
    assert_eq!(visual.children[0].size, [84.0, 84.0]);
    assert_eq!(visual.children[0].color, [0.0; 4]);
}
