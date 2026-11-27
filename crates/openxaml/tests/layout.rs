//! Exact-number checks for the layout rules. The numbers are hand-computed
//! from the upstream algorithms named in `src/layout.rs`.

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

    let measured = panel.measure(Size::new(100.0, f32::INFINITY));
    assert_eq!(measured, Size::new(10.0, 38.0));

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
fn border_padding_shifts_the_child_and_reduces_both_axes() {
    let mut border = Element::border().with_padding(8.0);
    border.push(Element::border());

    let layout = border.arrange(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(layout.rect, Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(layout.children[0].rect, Rect::new(8.0, 8.0, 84.0, 84.0));
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
