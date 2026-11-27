/*
 * Custom runtime data blobs embedded in SetCustomRuntimeData nodes.
 *
 * The payloads are not needed to walk the XML tree, but their exact length is
 * needed to reach the following nodes, so they are decoded and the useful
 * names (resource keys, visual state names, deferred element name) are kept
 * for the dump.
 *
 * Ported from the WidgetSpinner reference reader:
 *   XBF/ResourceDictionaryCustomRuntimeData.cs
 *   XBF/StyleCustomRuntimeData.cs
 *   XBF/DeferredElementCustomRuntimeData.cs
 *   XBF/VisualStateGroupCollectionCustomRuntimeData.cs
 */

use crate::cursor::Cursor;
use crate::error::XbfError;
use crate::metadata::Metadata;
use crate::node::CustomData;
use crate::node_value::{read_persisted, read_shared_string, read_value};

pub(crate) fn parse(
    cursor: &mut Cursor,
    metadata: &Metadata,
    offset: u32,
) -> Result<CustomData, XbfError> {
    let kind = cursor.leb_u32()?;
    let mut names = Vec::new();

    match kind {
        371 => read_resource_dictionary_v1(cursor, metadata, &mut names)?,
        7 => read_resource_dictionary_v2(cursor, metadata, &mut names)?,
        10 => read_resource_dictionary_v3(cursor, metadata, &mut names)?,
        12 => read_resource_dictionary_v4(cursor, metadata, &mut names)?,
        2 | 8 => read_style(cursor, metadata, false)?,
        11 => read_style(cursor, metadata, true)?,
        745 => read_deferred_element(cursor, metadata, kind, &mut names)?,
        6 | 9 => read_deferred_element(cursor, metadata, kind, &mut names)?,
        420 | 1 | 3 | 4 | 5 => read_visual_state_groups(cursor, metadata, kind, &mut names)?,
        _ => return Err(XbfError::UnknownCustomData { kind, offset }),
    }

    Ok(CustomData {
        kind,
        kind_name: kind_name(kind),
        names,
    })
}

fn kind_name(kind: u32) -> &'static str {
    match kind {
        371 => "ResourceDictionary_v1",
        7 => "ResourceDictionary_v2",
        10 => "ResourceDictionary_v3",
        12 => "ResourceDictionary_v4",
        2 => "Style_v1",
        8 => "Style_v2",
        11 => "Style_v3",
        745 => "DeferredElement_v1",
        6 => "DeferredElement_v2",
        9 => "DeferredElement_v3",
        420 => "VisualStateGroupCollection_v1",
        1 => "VisualStateGroupCollection_v2",
        3 => "VisualStateGroupCollection_v3",
        4 => "VisualStateGroupCollection_v4",
        5 => "VisualStateGroupCollection_v5",
        _ => "unknown",
    }
}

/* Read a 7-bit vector count, bounded by the bytes that are actually left. */
fn read_count(cursor: &mut Cursor, table: &'static str) -> Result<usize, XbfError> {
    let count = cursor.leb_u32()?;
    if count as usize > cursor.remaining() {
        return Err(XbfError::TableTooLarge {
            table,
            count,
            remaining: cursor.remaining(),
        });
    }
    Ok(count as usize)
}

fn read_kv_vec(cursor: &mut Cursor, metadata: &Metadata) -> Result<Vec<String>, XbfError> {
    let count = read_count(cursor, "resource key vector")?;
    let mut keys = Vec::new();
    for _ in 0..count {
        keys.push(read_shared_string(cursor, metadata)?);
        let _ = cursor.leb_u32()?;
    }
    Ok(keys)
}

fn read_string_vec(cursor: &mut Cursor, metadata: &Metadata) -> Result<Vec<String>, XbfError> {
    let count = read_count(cursor, "string vector")?;
    let mut values = Vec::new();
    for _ in 0..count {
        values.push(read_shared_string(cursor, metadata)?);
    }
    Ok(values)
}

fn read_predicates(cursor: &mut Cursor, metadata: &Metadata) -> Result<(), XbfError> {
    let count = read_count(cursor, "predicate vector")?;
    for _ in 0..count {
        let _ = read_persisted(cursor)?;
        let _ = read_shared_string(cursor, metadata)?;
    }
    Ok(())
}

fn read_conditional_objects(cursor: &mut Cursor, metadata: &Metadata) -> Result<(), XbfError> {
    let count = read_count(cursor, "conditional object vector")?;
    for _ in 0..count {
        let _ = cursor.leb_u32()?;
        read_predicates(cursor, metadata)?;
    }
    Ok(())
}

fn read_conditional_keys(cursor: &mut Cursor, metadata: &Metadata) -> Result<(), XbfError> {
    let count = read_count(cursor, "conditional key vector")?;
    for _ in 0..count {
        let _ = read_shared_string(cursor, metadata)?;
        let tokens = read_count(cursor, "conditional token vector")?;
        for _ in 0..tokens {
            let _ = cursor.leb_u32()?;
        }
    }
    Ok(())
}

/* ResourceDictionary_v1: ResourceDictionaryCustomRuntimeData.cs:39-68. */
fn read_resource_dictionary_v1(
    cursor: &mut Cursor,
    metadata: &Metadata,
    names: &mut Vec<String>,
) -> Result<(), XbfError> {
    names.extend(read_kv_vec(cursor, metadata)?);
    names.extend(read_string_vec(cursor, metadata)?);
    names.extend(read_kv_vec(cursor, metadata)?);
    let _ = read_string_vec(cursor, metadata)?;
    let _ = read_string_vec(cursor, metadata)?;
    Ok(())
}

/* ResourceDictionary_v2: ResourceDictionaryCustomRuntimeData.cs:70-94. */
fn read_resource_dictionary_v2(
    cursor: &mut Cursor,
    metadata: &Metadata,
    names: &mut Vec<String>,
) -> Result<(), XbfError> {
    names.extend(read_kv_vec(cursor, metadata)?);
    names.extend(read_string_vec(cursor, metadata)?);
    names.extend(read_kv_vec(cursor, metadata)?);
    let _ = read_string_vec(cursor, metadata)?;
    Ok(())
}

/* ResourceDictionary_v3: ResourceDictionaryCustomRuntimeData.cs:96-143. */
fn read_resource_dictionary_v3(
    cursor: &mut Cursor,
    metadata: &Metadata,
    names: &mut Vec<String>,
) -> Result<(), XbfError> {
    names.extend(read_kv_vec(cursor, metadata)?);
    names.extend(read_string_vec(cursor, metadata)?);
    names.extend(read_kv_vec(cursor, metadata)?);
    read_conditional_keys(cursor, metadata)?;
    read_conditional_keys(cursor, metadata)?;
    read_conditional_objects(cursor, metadata)?;
    Ok(())
}

/* ResourceDictionary_v4: ResourceDictionaryCustomRuntimeData.cs:144-199. */
fn read_resource_dictionary_v4(
    cursor: &mut Cursor,
    metadata: &Metadata,
    names: &mut Vec<String>,
) -> Result<(), XbfError> {
    let count = read_count(cursor, "resource map")?;
    for _ in 0..count {
        names.push(read_shared_string(cursor, metadata)?);
        let _ = cursor.u64()?;
        let _ = cursor.leb_u32()?;
    }

    names.extend(read_string_vec(cursor, metadata)?);

    let count = read_count(cursor, "conditional resource map")?;
    for _ in 0..count {
        let _ = read_shared_string(cursor, metadata)?;
        let _ = cursor.u64()?;
        let tokens = read_count(cursor, "conditional token vector")?;
        for _ in 0..tokens {
            let _ = cursor.leb_u32()?;
        }
    }

    read_conditional_objects(cursor, metadata)
}

/* Style v1-v3: StyleCustomRuntimeData.cs:133-170. */
fn read_style(
    cursor: &mut Cursor,
    metadata: &Metadata,
    has_conditionals: bool,
) -> Result<(), XbfError> {
    let count = read_count(cursor, "style setter vector")?;
    for _ in 0..count {
        read_setter(cursor, metadata)?;
    }
    if has_conditionals {
        read_conditional_objects(cursor, metadata)?;
    }
    Ok(())
}

/* StyleSetterEssence.Deserialize, StyleCustomRuntimeData.cs:68-117. */
fn read_setter(cursor: &mut Cursor, metadata: &Metadata) -> Result<(), XbfError> {
    let flags = cursor.leb_u32()?;

    if flags & 0x40 == 0 {
        if flags & 0x10 != 0 {
            let _ = read_persisted(cursor)?;
        } else {
            let _ = read_shared_string(cursor, metadata)?;
            let _ = read_persisted(cursor)?;
        }
    }

    if flags & 0x04 != 0 {
        let _ = read_shared_string(cursor, metadata)?;
    } else if flags & 0x20 != 0 {
        let _ = read_value(cursor, metadata, 0)?;
    } else if flags & (0x02 | 0x01 | 0x08 | 0x40) != 0 {
        /* Static/theme/object/self tokens are all one 7-bit offset. */
        let _ = cursor.leb_u32()?;
    }
    Ok(())
}

/* DeferredElement v1-v3: DeferredElementCustomRuntimeData.cs:25-53. */
fn read_deferred_element(
    cursor: &mut Cursor,
    metadata: &Metadata,
    kind: u32,
    names: &mut Vec<String>,
) -> Result<(), XbfError> {
    names.push(read_shared_string(cursor, metadata)?);
    if kind == 745 {
        return Ok(());
    }

    let count = read_count(cursor, "deferred property vector")?;
    for _ in 0..count {
        let _ = read_persisted(cursor)?;
        let _ = read_value(cursor, metadata, 0)?;
    }
    if kind != 6 {
        let _ = cursor.bool()?;
    }
    Ok(())
}

/* VisualStateGroupCollection v1-v5, VisualStateGroupCollectionCustomRuntimeData.cs. */
fn read_visual_state_groups(
    cursor: &mut Cursor,
    metadata: &Metadata,
    kind: u32,
    names: &mut Vec<String>,
) -> Result<(), XbfError> {
    let count = read_count(cursor, "visual state group map")?;
    for _ in 0..count {
        let _ = cursor.leb_u32()?;
    }

    let count = read_count(cursor, "visual state vector")?;
    for _ in 0..count {
        names.push(read_shared_string(cursor, metadata)?);
        let _ = cursor.leb_u32()?;
        let _ = cursor.bool()?;
        if kind != 420 {
            read_token_vector(cursor)?;
            read_int_vector_vector(cursor)?;
            if kind != 1 {
                read_token_vector(cursor)?;
                read_token_vector(cursor)?;
                if kind != 3 && kind != 4 {
                    read_token_vector(cursor)?;
                }
            }
        }
    }

    let count = read_count(cursor, "visual state group vector")?;
    for _ in 0..count {
        names.push(read_shared_string(cursor, metadata)?);
        let _ = cursor.bool()?;
        let _ = cursor.leb_u32()?;
    }

    let count = read_count(cursor, "visual transition vector")?;
    for _ in 0..count {
        let _ = read_shared_string(cursor, metadata)?;
        let _ = read_shared_string(cursor, metadata)?;
        let _ = cursor.leb_u32()?;
    }

    let _ = cursor.bool()?;

    let count = read_count(cursor, "transition lookup")?;
    for _ in 0..count {
        let _ = cursor.leb_u32()?;
        let _ = cursor.leb_u32()?;
        let _ = cursor.leb_u32()?;
    }
    let count = read_count(cursor, "default transition vector")?;
    for _ in 0..count {
        let _ = cursor.leb_u32()?;
    }
    let _ = cursor.leb_u32()?;

    if kind == 5 {
        let _ = read_string_vec(cursor, metadata)?;
    }
    Ok(())
}

fn read_token_vector(cursor: &mut Cursor) -> Result<(), XbfError> {
    let count = read_count(cursor, "token vector")?;
    for _ in 0..count {
        let _ = cursor.leb_u32()?;
    }
    Ok(())
}

fn read_int_vector_vector(cursor: &mut Cursor) -> Result<(), XbfError> {
    let count = read_count(cursor, "integer vector vector")?;
    for _ in 0..count {
        read_token_vector(cursor)?;
    }
    Ok(())
}
