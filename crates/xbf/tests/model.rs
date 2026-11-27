/*
 * Tests for the typed element model.
 *
 * The fixtures are the same real XBF 2.1 images used by the wire tests; see
 * tests/data/README.md.  The synthetic streams check that a bad scope stack
 * and an unmapped node kind produce typed errors instead of panicking.
 */

use xbf::{
    Header, Metadata, Node, NodeKind, NodeStream, PersistedNode, Version, Xbf, XbfError,
    model::Value,
};

const FIXTURE: &[u8] = include_bytes!("data/winui-theme-resources.xbf");
const SMALLEST: &[u8] = include_bytes!("data/winui-theme-resources-small.xbf");

fn empty_file(nodes: Vec<Node>) -> Xbf {
    Xbf {
        header: Header {
            metadata_size: 0,
            node_size: 0,
            version: Version { major: 2, minor: 1 },
        },
        metadata: Metadata {
            version: Version { major: 2, minor: 1 },
            hash: String::new(),
            hash_bytes: [0; 64],
            strings: Vec::new(),
            assemblies: Vec::new(),
            type_namespaces: Vec::new(),
            types: Vec::new(),
            properties: Vec::new(),
            xml_namespaces: Vec::new(),
        },
        streams: vec![NodeStream {
            node_offset: 0,
            line_offset: 1,
            nodes,
        }],
    }
}

#[test]
fn both_fixtures_have_a_resource_dictionary_root() {
    for fixture in [FIXTURE, SMALLEST] {
        let file = xbf::parse(fixture).expect("fixture should parse");
        let model = file.model().expect("fixture should produce a model");

        assert_eq!(model.roots.len(), 1, "expected exactly one root");
        assert_eq!(model.roots[0].type_name, "ResourceDictionary");
        assert_eq!(
            model.roots[0].namespace,
            "http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        );
        assert_eq!(
            model.roots[0].custom.as_deref(),
            Some("ResourceDictionary_v3")
        );
    }
}

#[test]
fn smallest_fixture_yields_the_nine_resource_keys() {
    let file = xbf::parse(SMALLEST).expect("smallest fixture should parse");
    let model = file
        .model()
        .expect("smallest fixture should produce a model");
    let root = &model.roots[0];

    assert_eq!(root.children.len(), 9);
    let keys: Vec<&str> = root
        .children
        .iter()
        .flat_map(|child| child.properties.iter())
        .filter(|property| property.name == "x:Key")
        .filter_map(|property| match &property.value {
            Value::String(key) => Some(key.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(keys.len(), 9);
    /*
     * Keys carry the stream offset of the value node, so the first child is
     * the item height, not the padding that happens to be first in the blob.
     */
    assert_eq!(keys[0], "TimePickerFlyoutPresenterItemHeight");
    assert_eq!(keys[3], "TimePickerFlyoutPresenterItemPadding");
    assert_eq!(keys[8], "DatePickerFlyoutPresenterMonthPadding");

    let mut sorted = keys.clone();
    sorted.sort_unstable();
    let mut expected = [
        "DatePickerFlyoutPresenterAcceptDismissHostGridHeight",
        "DatePickerFlyoutPresenterHighlightHeight",
        "DatePickerFlyoutPresenterItemHeight",
        "DatePickerFlyoutPresenterItemPadding",
        "DatePickerFlyoutPresenterMonthPadding",
        "TimePickerFlyoutPresenterAcceptDismissHostGridHeight",
        "TimePickerFlyoutPresenterHighlightHeight",
        "TimePickerFlyoutPresenterItemHeight",
        "TimePickerFlyoutPresenterItemPadding",
    ];
    expected.sort_unstable();
    assert_eq!(sorted, expected);
}

#[test]
fn larger_fixture_keeps_the_style_and_its_target_type() {
    let file = xbf::parse(FIXTURE).expect("fixture should parse");
    let model = file.model().expect("fixture should produce a model");
    let root = &model.roots[0];

    assert_eq!(root.children.len(), 17);
    let style = &root.children[0];
    assert_eq!(style.type_name, "Style");
    assert_eq!(style.custom.as_deref(), Some("Style_v3"));
    let target = style
        .properties
        .iter()
        .find(|property| property.name == "Style_TargetType")
        .expect("style should set TargetType");
    match &target.value {
        Value::Type { name, .. } => assert_eq!(name, "TextBlock"),
        other => panic!("unexpected target type {other:?}"),
    }

    let key = style
        .properties
        .iter()
        .find(|property| property.name == "x:Key")
        .expect("style should have an implicit key");
    match &key.value {
        Value::String(name) => assert_eq!(name, "Microsoft.UI.Xaml.Controls.TextBlock"),
        other => panic!("unexpected implicit key {other:?}"),
    }
}

#[test]
fn text_and_json_render_the_same_tree() {
    let file = xbf::parse(SMALLEST).expect("smallest fixture should parse");
    let model = file
        .model()
        .expect("smallest fixture should produce a model");

    let text = model.to_text();
    assert!(text.starts_with("ResourceDictionary ["));
    assert!(text.contains("custom=ResourceDictionary_v3"));
    assert!(text.contains("Double.Value = 32"));
    assert!(text.contains("Double.x:Key = \"TimePickerFlyoutPresenterItemHeight\""));

    let json = model.to_json();
    assert!(json.starts_with("{\"roots\":["));
    assert!(json.ends_with("]}\n"));
    assert!(json.contains("\"type\":\"ResourceDictionary\""));
    assert!(json.contains("\"name\":\"x:Key\""));
    assert!(json.contains("\"kind\":\"string\""));
}

#[test]
fn property_element_attaches_to_the_element_on_top() {
    let grid = PersistedNode {
        id: 442,
        trusted: true,
    };
    let row = PersistedNode {
        id: 860,
        trusted: true,
    };
    let file = empty_file(vec![
        Node {
            offset: 0,
            kind: NodeKind::PushScope,
        },
        Node {
            offset: 1,
            kind: NodeKind::CreateTypeBeginInit { ty: grid },
        },
        Node {
            offset: 2,
            kind: NodeKind::PushScopeGetValue { property: row },
        },
        Node {
            offset: 3,
            kind: NodeKind::CreateTypeBeginInit { ty: grid },
        },
        Node {
            offset: 4,
            kind: NodeKind::PopScope,
        },
        Node {
            offset: 5,
            kind: NodeKind::PopScope,
        },
    ]);

    let model = file.model().expect("property scope should produce a model");
    let root = &model.roots[0];
    assert_eq!(root.type_name, "Grid");
    assert!(root.children.is_empty());
    let property = root
        .properties
        .iter()
        .find(|property| property.name == "Grid_Row")
        .expect("property element should set Grid_Row");
    match &property.value {
        Value::Element(child) => assert_eq!(child.type_name, "Grid"),
        other => panic!("expected an element value, got {other:?}"),
    }
}

#[test]
fn closing_a_scope_that_was_never_opened_is_an_error() {
    let file = empty_file(vec![Node {
        offset: 0,
        kind: NodeKind::PopScope,
    }]);

    match file.model() {
        Err(XbfError::ModelError {
            stream,
            index,
            kind,
            reason,
        }) => {
            assert_eq!(stream, 0);
            assert_eq!(index, 0);
            assert_eq!(kind, "PopScope");
            assert!(reason.contains("no open scope"));
        }
        other => panic!("expected a model error, got {other:?}"),
    }
}

#[test]
fn an_unmapped_node_kind_names_its_index_and_kind() {
    let file = empty_file(vec![
        Node {
            offset: 0,
            kind: NodeKind::PushScope,
        },
        Node {
            offset: 1,
            kind: NodeKind::GetResourcePropertyBag {
                value: xbf::Value::None,
            },
        },
        Node {
            offset: 2,
            kind: NodeKind::PopScope,
        },
    ]);

    match file.model() {
        Err(XbfError::ModelError { index, kind, .. }) => {
            assert_eq!(index, 1);
            assert_eq!(kind, "GetResourcePropertyBag");
        }
        other => panic!("expected a model error, got {other:?}"),
    }
}
