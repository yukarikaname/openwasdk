//! Loader tests: both real theme fixtures, a hand-built tree and the errors.
//!
//! The fixtures are the same XBF 2.1 images the XBF crate tests use. They are
//! resource dictionaries: a non-visual root that loads and lays out to nothing.

use openxaml::property;
use openxaml::{GridLength, Kind, LoadError, Orientation, Thickness, Value};
use xbf::model::{Element as Source, Property, Value as SourceValue};

const CONTROLS: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

const THEME: &[u8] = include_bytes!("../../xbf/tests/data/winui-theme-resources.xbf");
const THEME_SMALL: &[u8] = include_bytes!("../../xbf/tests/data/winui-theme-resources-small.xbf");

fn property(name: &str, value: SourceValue) -> Property {
    Property {
        name: name.to_string(),
        namespace: CONTROLS.to_string(),
        value,
    }
}

fn directive(name: &str, value: &str) -> Property {
    Property {
        name: name.to_string(),
        namespace: XAML.to_string(),
        value: SourceValue::String(value.to_string()),
    }
}

fn source(type_name: &str, properties: Vec<Property>, children: Vec<Source>) -> Source {
    Source {
        type_name: type_name.to_string(),
        namespace: CONTROLS.to_string(),
        properties,
        children,
        custom: None,
    }
}

#[test]
fn both_theme_fixtures_load_as_a_non_visual_root() {
    for fixture in [THEME, THEME_SMALL] {
        let file = xbf::parse(fixture).expect("fixture should parse");
        let model = file.model().expect("fixture should produce a model");
        let roots = openxaml::load(&model).expect("a dictionary is not visual");
        assert_eq!(roots.len(), 1, "one root per fixture");
        assert_eq!(roots[0].kind(), Kind::ResourceDictionary);
        let measured = roots[0]
            .measure(openxaml::Size::new(800.0, 600.0))
            .expect("a dictionary measures");
        assert_eq!((measured.width, measured.height), (0.0, 0.0));
    }
}

#[test]
fn a_stack_panel_and_its_text_block_load() {
    let text = source(
        "TextBlock",
        vec![property("Text", SourceValue::String("hello".to_string()))],
        Vec::new(),
    );
    let brush = source(
        "SolidColorBrush",
        vec![property("Color", SourceValue::Color(0xff10_2030))],
        Vec::new(),
    );
    let panel = source(
        "StackPanel",
        vec![
            directive("x:Name", "root"),
            property(
                "Orientation",
                SourceValue::Enum {
                    type_name: "Orientation".to_string(),
                    value: 1,
                },
            ),
            property("Spacing", SourceValue::Float(4.0)),
            property("Margin", SourceValue::Thickness([1.0, 2.0, 3.0, 4.0])),
            property("Background", SourceValue::Element(Box::new(brush))),
        ],
        vec![text],
    );

    let loaded = openxaml::load_root(&panel).expect("the whole tree is in the table");

    assert_eq!(loaded.kind(), Kind::StackPanel);
    assert_eq!(loaded.name(), Some("root"));
    assert_eq!(loaded.orientation(), Orientation::Horizontal);
    assert_eq!(loaded.spacing(), 4.0);
    assert_eq!(loaded.margin(), Thickness::new(1.0, 2.0, 3.0, 4.0));
    assert_eq!(loaded.get(&property::SPACING), &Value::Number(4.0));

    assert_eq!(loaded.children().len(), 1);
    let child = &loaded.children()[0];
    assert_eq!(child.kind(), Kind::TextBlock);
    assert_eq!(child.text(), "hello");
    assert_eq!(child.get(&property::TEXT), &Value::Text("hello".into()));

    let background = loaded.background().expect("a solid brush is a colour");
    assert_eq!(background[3], 1.0);
    assert!(background[0] < background[1]);
}

#[test]
fn directives_are_carried_as_names_not_properties() {
    let text = source(
        "TextBlock",
        vec![
            directive("x:Name", "label"),
            directive("x:Key", "label-key"),
        ],
        Vec::new(),
    );
    let loaded = openxaml::load_root(&text).expect("TextBlock is in the table");
    assert_eq!(loaded.name(), Some("label"));
    assert_eq!(loaded.key(), Some("label-key"));
    assert!(loaded.local("x:Name").is_none());
    assert!(loaded.local("x:Key").is_none());
}

#[test]
fn an_unsupported_property_names_itself() {
    /* A property the table has never heard of, unlike TextWrapping which is dropped. */
    let text = source(
        "TextBlock",
        vec![property("Rotation", SourceValue::Float(45.0))],
        Vec::new(),
    );
    let error = openxaml::load_root(&text).expect_err("Rotation is not in the table");
    assert_eq!(
        error,
        LoadError::UnsupportedProperty {
            element: "TextBlock".to_string(),
            name: "Rotation".to_string()
        }
    );
}

#[test]
fn a_resource_reference_leaves_the_property_at_its_default() {
    let text = source(
        "TextBlock",
        vec![property(
            "Text",
            SourceValue::Resource {
                theme: false,
                key: "BodyText".to_string(),
            },
        )],
        Vec::new(),
    );
    // No resource map: Text stays unset rather than taking the key or failing the page.
    let loaded = openxaml::load_root(&text).expect("a reference loads");
    assert_eq!(loaded.text(), "");
}

#[test]
fn grid_tracks_come_from_definition_elements() {
    let definitions = SourceValue::Elements(vec![
        source(
            "RowDefinition",
            vec![property(
                "Height",
                SourceValue::GridLength {
                    unit: 0,
                    value: 0.0,
                },
            )],
            Vec::new(),
        ),
        source(
            "RowDefinition",
            vec![property(
                "Height",
                SourceValue::GridLength {
                    unit: 2,
                    value: 1.0,
                },
            )],
            Vec::new(),
        ),
    ]);
    let grid = source(
        "Grid",
        vec![property("Grid.RowDefinitions", definitions)],
        Vec::new(),
    );

    let loaded = openxaml::load_root(&grid).expect("Grid and its tracks are in the table");
    assert_eq!(loaded.rows().len(), 2);
    assert_eq!(loaded.rows()[0], GridLength::Auto);
    assert_eq!(loaded.rows()[1], GridLength::Star(1.0));
}
