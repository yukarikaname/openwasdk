/*
 * Wire level primitives shared by the node stream and custom runtime data
 * decoders.
 */

use crate::cursor::Cursor;
use crate::error::XbfError;
use crate::metadata::Metadata;
use crate::node::{PersistedNode, Value};

pub(crate) fn read_persisted(cursor: &mut Cursor) -> Result<PersistedNode, XbfError> {
    let raw = cursor.u16()?;
    Ok(PersistedNode {
        id: raw & 0x7fff,
        trusted: raw & 0x8000 != 0,
    })
}

/*
 * Node stream strings are a uint32 UTF-16 length followed by the code units.
 * They are never NUL terminated, even in XBF 2.1 (XamlSerializationHelper.h:
 * 104-122 only adds the terminator on the metadata stream path).
 */
pub(crate) fn read_node_string(cursor: &mut Cursor) -> Result<String, XbfError> {
    let units = cursor.u32()?;
    cursor.utf16(units, false)
}

pub(crate) fn read_shared_string(
    cursor: &mut Cursor,
    metadata: &Metadata,
) -> Result<String, XbfError> {
    let id = cursor.u16()? as u32;
    match metadata.string(id) {
        Some(value) => Ok(value.to_string()),
        None => Err(XbfError::BadIndex {
            table: "string table",
            index: id,
            count: metadata.strings.len(),
        }),
    }
}

pub(crate) fn read_shared_strings(
    cursor: &mut Cursor,
    metadata: &Metadata,
    count: u32,
) -> Result<Vec<String>, XbfError> {
    if count as usize > cursor.remaining() / 2 {
        return Err(XbfError::TableTooLarge {
            table: "resource reference list",
            count,
            remaining: cursor.remaining(),
        });
    }
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(read_shared_string(cursor, metadata)?);
    }
    Ok(values)
}

/*
 * PersistedConstantType, XamlBinaryMetadata.h:218-232.  Decoded exactly as in
 * xamlbinaryformatsubreader2.cpp:680-787.
 */
pub(crate) fn read_value(
    cursor: &mut Cursor,
    metadata: &Metadata,
    offset: u32,
) -> Result<Value, XbfError> {
    let kind = cursor.u8()?;
    let value = match kind {
        0 => Value::None,
        1 => Value::Bool(false),
        2 => Value::Bool(true),
        3 => Value::Float(cursor.f32()?),
        4 => Value::Signed(cursor.i32()?),
        5 => Value::SharedString(read_shared_string(cursor, metadata)?),
        6 => Value::Thickness([cursor.f32()?, cursor.f32()?, cursor.f32()?, cursor.f32()?]),
        7 => Value::GridLength {
            unit: cursor.u32()?,
            value: cursor.f32()?,
        },
        8 => Value::Color(cursor.u32()?),
        9 => Value::UniqueString(read_node_string(cursor)?),
        10 => Value::NullString,
        11 => Value::Enum {
            type_index: cursor.u16()?,
            value: cursor.u32()?,
        },
        _ => return Err(XbfError::UnknownConstant { kind, offset }),
    };
    Ok(value)
}
