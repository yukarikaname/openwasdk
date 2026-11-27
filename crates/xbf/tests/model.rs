/*
 * Tests for the typed element model.
 *
 * The fixtures are the same real XBF 2.1 images used by the wire tests; see
 * tests/data/README.md.  The synthetic streams check that a bad scope stack
 * and an unmapped node kind produce typed errors instead of panicking.
 */

use xbf::{
    Header, Metadata, Node, NodeKind, NodeStream, PersistedNode, Value as WireValue, Version, Xbf,
    XbfError, model::Value,
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

/*
 * Every PersistedConstantType kind the wire reader accepts has a model slot.
 * Push one of each through the operand stack and check the tree keeps it.
 */
#[test]
fn every_wire_value_kind_reaches_the_model() {
    assert_eq!(xbf::stable::type_name(641), Some("Orientation"));
    let grid = PersistedNode {
        id: 442,
        trusted: true,
    };
    let row = PersistedNode {
        id: 860,
        trusted: true,
    };
    let values = [
        WireValue::None,
        WireValue::Bool(false),
        WireValue::Bool(true),
        WireValue::Float(1.5),
        WireValue::Signed(-7),
        WireValue::SharedString("shared".to_string()),
        WireValue::Thickness([1.0, 2.0, 3.0, 4.0]),
        WireValue::GridLength {
            unit: 2,
            value: 1.5,
        },
        WireValue::Color(0xaabb_ccdd),
        WireValue::UniqueString("unique".to_string()),
        WireValue::NullString,
        WireValue::Enum {
            type_index: 641,
            value: 1,
        },
    ];

    let mut nodes = vec![
        Node {
            offset: 0,
            kind: NodeKind::PushScope,
        },
        Node {
            offset: 1,
            kind: NodeKind::CreateTypeBeginInit { ty: grid },
        },
    ];
    for (offset, value) in values.into_iter().enumerate() {
        nodes.push(Node {
            offset: offset as u32 * 2 + 2,
            kind: NodeKind::PushConstant { value },
        });
        nodes.push(Node {
            offset: offset as u32 * 2 + 3,
            kind: NodeKind::SetValue { property: row },
        });
    }
    nodes.push(Node {
        offset: 26,
        kind: NodeKind::PopScope,
    });

    let model = empty_file(nodes)
        .model()
        .expect("every value kind should reach the model");
    let values: Vec<&Value> = model.roots[0]
        .properties
        .iter()
        .map(|property| &property.value)
        .collect();

    assert_eq!(values.len(), 12);
    assert!(matches!(values[0], Value::None));
    assert!(matches!(values[1], Value::Bool(false)));
    assert!(matches!(values[2], Value::Bool(true)));
    assert!(matches!(values[3], Value::Float(value) if (*value - 1.5).abs() < f32::EPSILON));
    assert!(matches!(values[4], Value::Signed(-7)));
    assert!(matches!(values[5], Value::String(value) if value == "shared"));
    assert!(matches!(values[6], Value::Thickness([1.0, 2.0, 3.0, 4.0])));
    assert!(
        matches!(values[7], Value::GridLength { unit: 2, value } if (*value - 1.5).abs() < f32::EPSILON)
    );
    assert!(matches!(values[8], Value::Color(0xaabb_ccdd)));
    assert!(matches!(values[9], Value::String(value) if value == "unique"));
    assert!(matches!(values[10], Value::Null));
    assert!(
        matches!(values[11], Value::Enum { type_name, value: 1 } if type_name == "Orientation")
    );
}

/*
 * PushResolvedType and PushResolvedProperty are operands like PushConstant;
 * SetValue consumes them and the model has the Type and Property slots.
 */
#[test]
fn resolved_operands_reach_the_model_through_set_value() {
    let grid = PersistedNode {
        id: 442,
        trusted: true,
    };
    let text_block = PersistedNode {
        id: 396,
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
            kind: NodeKind::PushResolvedType { ty: text_block },
        },
        Node {
            offset: 3,
            kind: NodeKind::SetValue { property: row },
        },
        Node {
            offset: 4,
            kind: NodeKind::PushResolvedProperty { property: row },
        },
        Node {
            offset: 5,
            kind: NodeKind::SetValue { property: row },
        },
        Node {
            offset: 6,
            kind: NodeKind::PopScope,
        },
    ]);

    let model = file
        .model()
        .expect("resolved operands should reach the model");
    let values: Vec<&Value> = model.roots[0]
        .properties
        .iter()
        .map(|property| &property.value)
        .collect();

    assert!(matches!(values[0], Value::Type { name, .. } if name == "TextBlock"));
    assert!(
        matches!(values[1], Value::Property { name, .. } if name == "Grid_Row"),
        "unexpected second value {:?}",
        values[1]
    );
}

/* File offset of one node in a parsed fixture, table included. */
fn node_file_offset(file: &Xbf, stream: usize, index: usize) -> usize {
    let section = 12 + file.header.metadata_size as usize;
    let table = 4 + file.streams.len() * 8;
    section + table + file.streams[stream].nodes[index].offset as usize
}

#[test]
fn a_truncated_fixture_is_a_typed_error() {
    let cut = FIXTURE.len() - 1;
    match xbf::parse(&FIXTURE[..cut]) {
        Err(XbfError::BadSectionSize { section, .. }) => assert_eq!(section, "node stream"),
        Err(XbfError::Truncated { .. }) => {}
        other => panic!("expected a typed error for {cut} bytes, got {other:?}"),
    }
}

#[test]
fn a_flipped_node_kind_is_a_typed_error() {
    let file = xbf::parse(FIXTURE).expect("fixture should parse");
    let last = file.streams[0].nodes.len() - 1;
    let offset = node_file_offset(&file, 0, last);

    let mut data = FIXTURE.to_vec();
    data[offset] = 0xff;
    match xbf::parse(&data) {
        Err(XbfError::UnknownNodeKind { kind, .. }) => assert_eq!(kind, 0xff),
        other => panic!("expected UnknownNodeKind, got {other:?}"),
    }
}

/* EndOfStream is a sentinel; a stream that keeps going after it is corrupt. */
#[test]
fn nodes_after_end_of_stream_are_refused() {
    let file = xbf::parse(FIXTURE).expect("fixture should parse");
    let offset = node_file_offset(&file, 1, 6);

    let mut data = FIXTURE.to_vec();
    data[offset] = 37; /* EndOfStream, with the rest of the substream behind it */
    let file = xbf::parse(&data).expect("EndOfStream itself is a valid node kind");
    match file.model() {
        Err(XbfError::ModelError { kind, reason, .. }) => {
            assert_eq!(kind, "EndOfStream");
            assert!(
                reason.contains("nodes follow"),
                "unexpected reason {reason}"
            );
        }
        other => panic!("expected a trailing-node model error, got {other:?}"),
    }
}

/*
 * The last node of substream 1 closes one of its scopes.  Turning that closer
 * into EndOfStream parses, so only the model can catch that two scopes are
 * still open.  It must refuse rather than return the partial dictionary with
 * the last constant dropped.
 */
#[test]
fn a_flipped_node_kind_does_not_yield_a_partial_tree() {
    let file = xbf::parse(FIXTURE).expect("fixture should parse");
    let last = file.streams[1].nodes.len() - 1;
    let offset = node_file_offset(&file, 1, last);

    let mut data = FIXTURE.to_vec();
    data[offset] = 37; /* EndOfStream */
    let file = xbf::parse(&data).expect("EndOfStream itself is a valid node kind");
    match file.model() {
        Err(XbfError::ModelError { kind, reason, .. }) => {
            assert_eq!(kind, "end of substream");
            assert!(reason.contains("open scope"), "unexpected reason {reason}");
        }
        other => panic!("expected an unclosed-scope model error, got {other:?}"),
    }
}
