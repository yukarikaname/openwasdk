/*
 * Tests against a real XBF 2.1 image.
 *
 * The fixture is the smallest standalone XBF in the WinUI theme resources;
 * see tests/data/README.md for provenance.
 */

use xbf::{MAGIC, NodeKind, XbfError};

const FIXTURE: &[u8] = include_bytes!("data/winui-theme-resources.xbf");
const SMALLEST: &[u8] = include_bytes!("data/winui-theme-resources-small.xbf");

#[test]
fn parses_the_smallest_real_xbf() {
    let file = xbf::parse(SMALLEST).expect("smallest fixture should parse");

    assert_eq!(file.header.version.major, 2);
    assert_eq!(file.header.version.minor, 1);
    assert_eq!(file.header.metadata_size, 1146);
    assert_eq!(file.header.node_size, 214);
    assert_eq!(file.metadata.strings.len(), 11);
    assert_eq!(file.streams.len(), 2);
    assert_eq!(file.streams[0].nodes.len(), 5);
    assert_eq!(file.streams[1].nodes.len(), 21);
    assert_eq!(file.total_nodes(), 26);

    match &file.streams[0].nodes[2].kind {
        NodeKind::CreateTypeBeginInit { ty } => {
            assert_eq!(file.type_label(ty), "ResourceDictionary");
        }
        other => panic!("unexpected type node {other:?}"),
    }
    match &file.streams[0].nodes[3].kind {
        NodeKind::SetCustomRuntimeData { data, .. } => {
            assert_eq!(data.kind_name, "ResourceDictionary_v3");
            assert_eq!(data.names.len(), 9);
        }
        other => panic!("unexpected custom data node {other:?}"),
    }
}

#[test]
fn parses_real_theme_resource_dictionary() {
    let file = xbf::parse(FIXTURE).expect("fixture should parse");

    assert_eq!(file.header.version.major, 2);
    assert_eq!(file.header.version.minor, 1);
    assert_eq!(file.header.metadata_size, 1328);
    assert_eq!(file.header.node_size, 378);
    assert_eq!(file.metadata.hash.len(), 64);
    assert_eq!(file.metadata.strings.len(), 19);
    assert_eq!(file.metadata.xml_namespaces.len(), 2);
    assert!(file.metadata.assemblies.is_empty());
    assert!(file.metadata.types.is_empty());
    assert!(file.metadata.properties.is_empty());

    assert_eq!(file.streams.len(), 3);
    assert_eq!(file.streams[0].nodes.len(), 5);
    assert_eq!(file.streams[1].nodes.len(), 39);
    assert_eq!(file.streams[2].nodes.len(), 2);
    assert_eq!(file.total_nodes(), 46);

    let first = &file.streams[0].nodes[0];
    match &first.kind {
        NodeKind::PushScopeAddNamespace { namespace, prefix } => {
            assert_eq!(prefix, "x");
            assert_eq!(
                file.namespace_label(namespace),
                "http://schemas.microsoft.com/winfx/2006/xaml"
            );
        }
        other => panic!("unexpected first node {other:?}"),
    }

    match &file.streams[0].nodes[2].kind {
        NodeKind::CreateTypeBeginInit { ty } => {
            assert!(ty.trusted);
            assert_eq!(file.type_label(ty), "ResourceDictionary");
        }
        other => panic!("unexpected type node {other:?}"),
    }

    match &file.streams[0].nodes[3].kind {
        NodeKind::SetCustomRuntimeData { substream, data } => {
            assert_eq!(*substream, 1);
            assert_eq!(data.kind, 10);
            assert_eq!(data.kind_name, "ResourceDictionary_v3");
            assert_eq!(data.names.len(), 17);
            assert_eq!(data.names[0], "TreeViewItemPresenterPadding");
        }
        other => panic!("unexpected custom data node {other:?}"),
    }

    let last = file.streams[2].nodes.last().expect("last node");
    assert!(matches!(last.kind, NodeKind::PopScope));
}

#[test]
fn stable_index_tables_are_interfaced() {
    assert_eq!(xbf::stable::type_name(371), Some("ResourceDictionary"));
    assert_eq!(xbf::stable::type_name(442), Some("Grid"));
    assert_eq!(
        xbf::stable::property_name(860),
        Some("Grid_Row"),
        "property index 860 comes from StableXbfPropertyIndex"
    );
    /*
     * The tables end at StableXbfTypeCount (984) and
     * StableXbfPropertyCount (2476) entries.
     */
    assert_eq!(xbf::stable::type_name(983), Some("SplitMenuFlyoutItem"));
    assert_eq!(xbf::stable::property_name(2475), Some("Window_PlacementId"));
    assert_eq!(xbf::stable::type_name(984), None);
    assert_eq!(xbf::stable::property_name(2476), None);
    assert_eq!(xbf::stable::type_name(u16::MAX), None);
}

#[test]
fn header_parsing_reports_each_field() {
    let file = xbf::parse(FIXTURE).expect("fixture should parse");
    let hash = std::str::from_utf8(&file.metadata.hash_bytes).expect("ascii hash");
    assert_eq!(hash.len(), 64);
    assert!(hash.starts_with("E171EF"), "unexpected hash {hash}");
    assert_eq!(&FIXTURE[..4], &MAGIC);
}

#[test]
fn wrong_magic_is_typed() {
    let mut data = FIXTURE.to_vec();
    data[0] = b'Y';
    match xbf::parse(&data) {
        Err(XbfError::BadMagic(magic)) => assert_eq!(magic, [b'Y', b'B', b'F', 0]),
        other => panic!("expected BadMagic, got {other:?}"),
    }
}

#[test]
fn unsupported_major_version_is_typed() {
    let mut data = FIXTURE.to_vec();
    /* Version lives at the start of the metadata stream, file offset 12. */
    data[12..16].copy_from_slice(&1u32.to_le_bytes());
    match xbf::parse(&data) {
        Err(XbfError::UnsupportedVersion { major: 1, minor: 1 }) => {}
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn oversized_sections_are_typed() {
    let mut data = FIXTURE.to_vec();
    data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    match xbf::parse(&data) {
        Err(XbfError::BadSectionSize { section, .. }) => assert_eq!(section, "metadata"),
        other => panic!("expected BadSectionSize, got {other:?}"),
    }
}

#[test]
fn every_truncation_returns_an_error_without_panicking() {
    for fixture in [FIXTURE, SMALLEST] {
        for length in 0..fixture.len() {
            let result = xbf::parse(&fixture[..length]);
            assert!(
                result.is_err(),
                "prefix of {length} bytes unexpectedly parsed"
            );
        }
    }
}

#[test]
fn every_single_byte_corruption_does_not_panic() {
    /*
     * Flipping arbitrary payload bytes usually produces a different but still
     * valid tree, so only assert that the parser returns instead of panicking
     * or hanging.
     */
    for offset in 0..FIXTURE.len() {
        let mut data = FIXTURE.to_vec();
        data[offset] ^= 0xff;
        let _ = xbf::parse(&data);
    }
}
