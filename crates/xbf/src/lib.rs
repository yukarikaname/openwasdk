/*
 * XBF (XAML Binary Format) reader.
 *
 * Ported from the MIT-licensed reference implementation in
 * microsoft/microsoft-ui-xaml:
 *
 *   dxaml/xcp/tools/XbfParser/WidgetSpinner   (C#, v2 reader)
 *   dxaml/xcp/core/Parser                    (C++, runtime reader/writer)
 *   dxaml/xcp/tools/DumpXbf                  (C#, v1 reader)
 *
 * The on-disk layout is:
 *
 *   magic "XBF\0" | metadata size | node stream size
 *   metadata: version | six table offsets | 64 byte hash | six tables
 *   node stream: substream table | substreams (nodes then line info)
 *
 * All integers are little endian.  The reader never trusts a length or an
 * offset from the file; every read is bounds checked by Cursor.
 */

mod cursor;
mod custom;
mod error;
mod header;
mod metadata;
mod node;
mod node_value;
mod node_wire;
pub mod stable;

pub use error::XbfError;
pub use header::{Header, MAGIC, Version};
pub use metadata::{
    AssemblyEntry, Metadata, PROPERTY_FLAG_CUSTOM_DEPENDENCY, PROPERTY_FLAG_IMPLICIT,
    PROPERTY_FLAG_MARKUP_DIRECTIVE, PROPERTY_FLAG_UNKNOWN, PROPERTY_FLAG_XML, PropertyEntry,
    TYPE_FLAG_MARKUP_DIRECTIVE, TYPE_FLAG_UNKNOWN, TypeEntry, TypeNamespaceEntry,
    provider_kind_name,
};
pub use node::{CustomData, Node, NodeKind, NodeStream, PersistedNode, Predicate, Value};

use std::path::Path;

/* A fully parsed XBF file. */
#[derive(Debug)]
pub struct Xbf {
    pub header: Header,
    pub metadata: Metadata,
    pub streams: Vec<NodeStream>,
}

impl Xbf {
    /* Total number of nodes across every substream. */
    pub fn total_nodes(&self) -> usize {
        self.streams.iter().map(|s| s.nodes.len()).sum()
    }

    /* Human readable name for a type reference, stable or file-local. */
    pub fn type_label(&self, node: &PersistedNode) -> String {
        if node.trusted {
            return match stable::type_name(node.id) {
                Some(name) => name.to_string(),
                None => format!("stable-type#{}", node.id),
            };
        }
        match self.metadata.type_full_name(node.id as u32) {
            Some(name) => name,
            None => format!("type#{}", node.id),
        }
    }

    /* Human readable name for a property reference, stable or file-local. */
    pub fn property_label(&self, node: &PersistedNode) -> String {
        if node.trusted {
            return match stable::property_name(node.id) {
                Some(name) => name.to_string(),
                None => format!("stable-property#{}", node.id),
            };
        }
        match self.metadata.property_full_name(node.id as u32) {
            Some(name) => name,
            None => format!("property#{}", node.id),
        }
    }

    /* Human readable XML namespace for a namespace reference. */
    pub fn namespace_label(&self, node: &PersistedNode) -> String {
        match self.metadata.xml_namespace(node.id as u32) {
            Some(name) => name.to_string(),
            None => format!("namespace#{}", node.id),
        }
    }
}

/* Parse an in-memory XBF image. */
pub fn parse(data: &[u8]) -> Result<Xbf, XbfError> {
    let (header, metadata_header) = header::parse(data)?;

    let metadata_start = header::PREFIX_SIZE;
    let metadata_end = metadata_start + header.metadata_size as usize;
    let node_end = metadata_end + header.node_size as usize;

    let metadata = metadata::parse(
        &data[metadata_start..metadata_end],
        &header.version,
        &metadata_header,
    )?;
    let streams = node::parse(&data[metadata_end..node_end], &metadata)?;

    Ok(Xbf {
        header,
        metadata,
        streams,
    })
}

/* Parse an XBF file from disk. */
pub fn parse_file(path: &Path) -> Result<Xbf, XbfError> {
    let data = std::fs::read(path)?;
    parse(&data)
}
