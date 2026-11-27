//! Dependency-property rules, tested without a window: defaults, local values,
//! inheritance and the dirty flag layout consumes.

use openxaml::property::{self, ValueType};
use openxaml::{Element, Orientation, PropertyError, Rect, Value};

#[test]
fn an_unset_property_reads_its_documented_default() {
    let text = Element::text_block("x");

    assert_eq!(
        text.get(&property::WIDTH),
        &Value::Length(openxaml::Length::Auto)
    );
    assert_eq!(
        text.get(&property::HORIZONTAL_ALIGNMENT),
        &Value::HorizontalAlignment(openxaml::HorizontalAlignment::Stretch)
    );
    assert_eq!(text.get(&property::FONT_SIZE), &Value::Number(14.0));
    assert_eq!(text.font_size(), 14.0);
}

#[test]
fn a_local_value_wins_over_the_default() {
    let mut text = Element::text_block("x");
    text.set_width(40.0);

    assert_eq!(
        text.get(&property::WIDTH),
        &Value::Length(openxaml::Length::Pixels(40.0))
    );
    assert_eq!(text.width(), openxaml::Length::Pixels(40.0));
}

#[test]
fn inheritance_stops_at_a_local_value_in_the_middle() {
    let mut root = Element::stack_panel(Orientation::Vertical);
    root.set_font_size(32.0);

    let mut middle = Element::border();
    middle.push(Element::text_block("x"));
    root.push(middle);

    // Two levels down sees the root value.
    assert_eq!(root.font_size(), 32.0);
    assert_eq!(root.children()[0].font_size(), 32.0);
    assert_eq!(root.children()[0].children()[0].font_size(), 32.0);

    // A local value in the middle wins there and restarts inheritance.
    root.children_mut()[0].set_font_size(20.0);
    assert_eq!(root.font_size(), 32.0);
    assert_eq!(root.children()[0].font_size(), 20.0);
    assert_eq!(root.children()[0].children()[0].font_size(), 20.0);
}

#[test]
fn a_wrong_type_set_is_an_error_and_changes_nothing() {
    let mut text = Element::text_block("x");
    let error = text
        .set(&property::WIDTH, Value::text("wide"))
        .expect_err("Width is a Length, not text");

    assert_eq!(
        error,
        PropertyError {
            property: "Width",
            expected: ValueType::Length,
            actual: ValueType::Text,
        }
    );
    assert_eq!(
        text.get(&property::WIDTH),
        &Value::Length(openxaml::Length::Auto)
    );
    assert!(text.local("Width").is_none());
}

#[test]
fn a_set_after_arrange_dirties_the_ancestor_chain() {
    let mut root = Element::stack_panel(Orientation::Vertical);
    root.push(Element::border());
    root.children_mut()[0].push(Element::text_block("x"));

    let _ = root.arrange(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert!(!root.needs_layout());
    assert!(!root.children()[0].children()[0].needs_layout());

    // A leaf change is visible from the root as well as the leaf.
    root.children_mut()[0].children_mut()[0].set_margin(openxaml::Thickness::all(2.0));
    assert!(root.needs_layout());
    assert!(root.children()[0].needs_layout());
    assert!(root.children()[0].children()[0].needs_layout());

    // An inherited change dirties the whole subtree it reaches.
    let _ = root.arrange(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_font_size(30.0);
    assert!(root.needs_layout());
    assert!(root.children()[0].children()[0].needs_layout());
}
