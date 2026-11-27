/*
 * Node stream decoding.
 *
 * XamlBinaryFormatWriter2.cpp:59-64 serializes the substream table first and
 * then every substream as "nodes, then line info".  The offsets in the table
 * are relative to the end of the table.  Here the line stream is only used to
 * locate the end of a node stream; line info itself is not needed to draw the
 * tree.
 */

use crate::cursor::Cursor;
use crate::custom;
use crate::error::XbfError;
use crate::metadata::Metadata;
use crate::node::{Node, NodeKind, NodeStream, Predicate};
use crate::node_value::{
    read_node_string, read_persisted, read_shared_string, read_shared_strings, read_value,
};

pub(crate) fn parse(data: &[u8], metadata: &Metadata) -> Result<Vec<NodeStream>, XbfError> {
    let mut cursor = Cursor::new(data, "node stream");
    let count = cursor.u32()?;
    if count as usize > cursor.remaining() / 8 {
        return Err(XbfError::TableTooLarge {
            table: "substream table",
            count,
            remaining: cursor.remaining(),
        });
    }

    let mut table = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let node_offset = cursor.u32()?;
        let line_offset = cursor.u32()?;
        table.push((node_offset, line_offset));
    }

    let table_end = cursor.position();
    let mut streams = Vec::with_capacity(table.len());

    for (index, (node_offset, line_offset)) in table.iter().copied().enumerate() {
        let node_start = table_end.checked_add(node_offset as usize);
        let node_end = table_end.checked_add(line_offset as usize);
        let line_end = match table.get(index + 1) {
            Some((next_node, _)) => table_end.checked_add(*next_node as usize),
            None => Some(data.len()),
        };
        let (Some(node_start), Some(node_end), Some(line_end)) = (node_start, node_end, line_end)
        else {
            return Err(XbfError::BadSubstream { index });
        };
        if node_start > node_end || node_end > line_end || line_end > data.len() {
            return Err(XbfError::BadSubstream { index });
        }

        let nodes = parse_nodes(&data[node_start..node_end], metadata, node_offset)?;
        streams.push(NodeStream {
            node_offset,
            line_offset,
            nodes,
        });
    }

    Ok(streams)
}

fn parse_nodes(data: &[u8], metadata: &Metadata, base_offset: u32) -> Result<Vec<Node>, XbfError> {
    let mut cursor = Cursor::new(data, "node");
    let mut nodes = Vec::new();
    while !cursor.is_empty() {
        let offset = cursor.position() as u32 + base_offset;
        let kind = parse_node(&mut cursor, metadata, offset)?;
        nodes.push(Node { offset, kind });
    }
    Ok(nodes)
}

fn parse_node(cursor: &mut Cursor, metadata: &Metadata, offset: u32) -> Result<NodeKind, XbfError> {
    let kind = cursor.u8()?;
    let node = match kind {
        1 => NodeKind::PushScope,
        2 => NodeKind::PopScope,
        3 => NodeKind::AddNamespace {
            namespace: read_persisted(cursor)?,
            prefix: read_node_string(cursor)?,
        },
        4 => NodeKind::PushConstant {
            value: read_value(cursor, metadata, offset)?,
        },
        5 => NodeKind::PushResolvedType {
            ty: read_persisted(cursor)?,
        },
        6 => NodeKind::PushResolvedProperty {
            property: read_persisted(cursor)?,
        },
        7 => NodeKind::SetValue {
            property: read_persisted(cursor)?,
        },
        8 => NodeKind::AddToCollection,
        9 => NodeKind::AddToDictionary,
        10 => NodeKind::AddToDictionaryWithKey {
            value: read_value(cursor, metadata, offset)?,
        },
        11 => NodeKind::CheckPeerType {
            type_name: read_node_string(cursor)?,
        },
        12 => NodeKind::SetConnectionId {
            value: read_value(cursor, metadata, offset)?,
        },
        13 => NodeKind::SetName {
            value: read_value(cursor, metadata, offset)?,
        },
        14 => NodeKind::GetResourcePropertyBag {
            value: read_value(cursor, metadata, offset)?,
        },
        15 => {
            let substream = cursor.leb_u32()?;
            /*
             * TH1 compatibility fields: two counts and one PersistedXamlNode2
             * per referenced resource.  GenXbf writes zeroes here now, but the
             * bytes still have to be consumed.
             * xamlbinaryformatsubreader2.cpp:434-446.
             */
            let static_count = cursor.leb_u32()?;
            let theme_count = cursor.leb_u32()?;
            let total = static_count as u64 + theme_count as u64;
            if total > cursor.remaining() as u64 / 2 {
                return Err(XbfError::TableTooLarge {
                    table: "custom runtime resource references",
                    count: u32::try_from(total).unwrap_or(u32::MAX),
                    remaining: cursor.remaining(),
                });
            }
            for _ in 0..total {
                let _ = cursor.u16()?;
            }
            let data = custom::parse(cursor, metadata, offset)?;
            NodeKind::SetCustomRuntimeData { substream, data }
        }
        16 => {
            /* SetResourceDictionaryItems is not emitted by current GenXbf. */
            return Err(XbfError::UnknownNodeKind { kind, offset });
        }
        17 => {
            let property = read_persisted(cursor)?;
            let substream = cursor.leb_u32()?;
            let static_count = cursor.leb_u32()?;
            let theme_count = cursor.leb_u32()?;
            let static_resources = read_shared_strings(cursor, metadata, static_count)?;
            let theme_resources = read_shared_strings(cursor, metadata, theme_count)?;
            NodeKind::SetDeferredProperty {
                property,
                substream,
                static_resources,
                theme_resources,
            }
        }
        18 => NodeKind::PushScopeAddNamespace {
            namespace: read_persisted(cursor)?,
            prefix: read_node_string(cursor)?,
        },
        19 => NodeKind::PushScopeGetValue {
            property: read_persisted(cursor)?,
        },
        20 => NodeKind::PushScopeCreateTypeBeginInit {
            ty: read_persisted(cursor)?,
        },
        21 => NodeKind::PushScopeCreateTypeWithConstantBeginInit {
            ty: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        22 => NodeKind::PushScopeCreateTypeWithTypeConvertedConstantBeginInit {
            ty: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        23 => NodeKind::CreateTypeBeginInit {
            ty: read_persisted(cursor)?,
        },
        24 => NodeKind::CreateTypeWithConstantBeginInit {
            ty: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        25 => NodeKind::CreateTypeWithTypeConvertedConstantBeginInit {
            ty: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        26 => NodeKind::SetValueConstant {
            property: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        27 => NodeKind::SetValueTypeConvertedConstant {
            property: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        28 => NodeKind::SetValueTypeConvertedResolvedProperty {
            property: read_persisted(cursor)?,
            value: read_persisted(cursor)?,
        },
        29 => NodeKind::SetValueTypeConvertedResolvedType {
            property: read_persisted(cursor)?,
            ty: read_persisted(cursor)?,
        },
        30 => NodeKind::SetValueFromStaticResource {
            property: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        31 => NodeKind::SetValueFromTemplateBinding {
            property: read_persisted(cursor)?,
            value: read_persisted(cursor)?,
        },
        32 => NodeKind::SetValueFromMarkupExtension {
            property: read_persisted(cursor)?,
        },
        33 => NodeKind::EndInitPopScope,
        34 => NodeKind::ProvideStaticResourceValue {
            value: read_value(cursor, metadata, offset)?,
        },
        35 => NodeKind::ProvideThemeResourceValue {
            value: read_value(cursor, metadata, offset)?,
        },
        36 => NodeKind::SetValueFromThemeResource {
            property: read_persisted(cursor)?,
            value: read_value(cursor, metadata, offset)?,
        },
        37 => NodeKind::EndOfStream,
        38 => NodeKind::BeginConditionalScope {
            predicate: Predicate {
                ty: read_persisted(cursor)?,
                arguments: read_shared_string(cursor, metadata)?,
            },
        },
        39 => NodeKind::EndConditionalScope,
        40 => NodeKind::EndInitProvideValuePopScope,
        _ => return Err(XbfError::UnknownNodeKind { kind, offset }),
    };
    Ok(node)
}
