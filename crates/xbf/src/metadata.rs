/*
 * Metadata tables.
 *
 * Formats:
 *   String table          XamlBinaryMetadata.h, XbfMetadata.cs:55-64
 *   Assembly table        PersistedXamlAssembly, XamlBinaryMetadata.h:88-97
 *   Type namespace table  PersistedXamlTypeNamespace, :99-108
 *   Type table            PersistedXamlType, :110-131
 *   Property table        PersistedXamlProperty, :133-155
 *   XML namespace table   PersistedXamlXmlNamespace, :157-164
 *
 * Every table is a uint32 count followed by fixed size entries.  Strings are
 * a uint32 UTF-16 length, the WCHARs, and (v2.1+) a NUL.  The upstream
 * reader validates that each table starts exactly where the previous one
 * ended (XamlBinaryMetadataReader2.cpp:48-51, 108-111, ...) and so do we.
 */

use crate::cursor::Cursor;
use crate::error::XbfError;
use crate::header::{METADATA_HEADER_SIZE, MetadataHeader, Version};

pub const TYPE_FLAG_MARKUP_DIRECTIVE: u32 = 0x01;
pub const TYPE_FLAG_UNKNOWN: u32 = 0x02;

pub const PROPERTY_FLAG_XML: u32 = 0x01;
pub const PROPERTY_FLAG_MARKUP_DIRECTIVE: u32 = 0x02;
pub const PROPERTY_FLAG_IMPLICIT: u32 = 0x04;
pub const PROPERTY_FLAG_CUSTOM_DEPENDENCY: u32 = 0x08;
pub const PROPERTY_FLAG_UNKNOWN: u32 = 0x10;

#[derive(Debug)]
pub struct AssemblyEntry {
    pub provider_kind: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct TypeNamespaceEntry {
    pub assembly: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct TypeEntry {
    pub flags: u32,
    pub namespace: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct PropertyEntry {
    pub flags: u32,
    pub declaring_type: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct Metadata {
    pub version: Version,
    pub hash: String,
    pub hash_bytes: [u8; 64],
    pub strings: Vec<String>,
    pub assemblies: Vec<AssemblyEntry>,
    pub type_namespaces: Vec<TypeNamespaceEntry>,
    pub types: Vec<TypeEntry>,
    pub properties: Vec<PropertyEntry>,
    pub xml_namespaces: Vec<String>,
}

pub(crate) fn parse(
    data: &[u8],
    version: &Version,
    header: &MetadataHeader,
) -> Result<Metadata, XbfError> {
    let mut cursor = Cursor::new(data, "metadata");
    cursor.skip(METADATA_HEADER_SIZE)?;

    expect_offset("string table", header.string_table, cursor.position())?;
    let strings = read_strings(&mut cursor, version)?;
    expect_offset("assembly table", header.assembly_list, cursor.position())?;
    let assemblies = read_assemblies(&mut cursor, &strings)?;
    expect_offset(
        "type namespace table",
        header.type_namespace_list,
        cursor.position(),
    )?;
    let type_namespaces = read_type_namespaces(&mut cursor, &strings, &assemblies)?;
    expect_offset("type table", header.type_list, cursor.position())?;
    let types = read_types(&mut cursor, &strings)?;
    expect_offset("property table", header.property_list, cursor.position())?;
    let properties = read_properties(&mut cursor, &strings)?;
    expect_offset(
        "xml namespace table",
        header.xml_namespace_list,
        cursor.position(),
    )?;
    let xml_namespaces = read_xml_namespaces(&mut cursor, &strings)?;

    validate_references(&types, &properties, &type_namespaces, &xml_namespaces)?;

    Ok(Metadata {
        version: *version,
        hash: ascii_hash(&header.hash),
        hash_bytes: header.hash,
        strings,
        assemblies,
        type_namespaces,
        types,
        properties,
        xml_namespaces,
    })
}

impl Metadata {
    pub fn string(&self, index: u32) -> Option<&str> {
        self.strings.get(index as usize).map(String::as_str)
    }

    pub fn xml_namespace(&self, index: u32) -> Option<&str> {
        self.xml_namespaces.get(index as usize).map(String::as_str)
    }

    /* Resolve a file-local type id to a display name. */
    pub fn type_full_name(&self, index: u32) -> Option<String> {
        let entry = self.types.get(index as usize)?;
        if entry.flags & TYPE_FLAG_UNKNOWN != 0 {
            let ns = self.xml_namespace(entry.namespace)?;
            return Some(format!("{ns}.{}", entry.name));
        }
        if entry.flags & TYPE_FLAG_MARKUP_DIRECTIVE != 0 {
            return Some(entry.name.clone());
        }
        let ns = self.type_namespaces.get(entry.namespace as usize)?;
        if ns.name.is_empty() {
            Some(entry.name.clone())
        } else {
            Some(format!("{}.{}", ns.name, entry.name))
        }
    }

    /*
     * Namespace recorded for a file-local type.  Unknown types use their XML
     * namespace; markup directives have none and callers substitute the XAML
     * namespace; everything else is the CLR type namespace name.
     */
    pub fn type_namespace_name(&self, index: u32) -> Option<&str> {
        let entry = self.types.get(index as usize)?;
        if entry.flags & TYPE_FLAG_UNKNOWN != 0 {
            return self.xml_namespace(entry.namespace);
        }
        if entry.flags & TYPE_FLAG_MARKUP_DIRECTIVE != 0 {
            return None;
        }
        self.type_namespaces
            .get(entry.namespace as usize)
            .map(|namespace| namespace.name.as_str())
    }

    /* Namespace of a file-local property, when the metadata records one. */
    pub fn property_namespace(&self, index: u32) -> Option<&str> {
        let entry = self.properties.get(index as usize)?;
        if entry.flags & (PROPERTY_FLAG_MARKUP_DIRECTIVE | PROPERTY_FLAG_IMPLICIT) != 0 {
            return None;
        }
        self.type_namespace_name(entry.declaring_type)
    }

    /* Resolve a file-local property id to a display name. */
    pub fn property_full_name(&self, index: u32) -> Option<String> {
        let entry = self.properties.get(index as usize)?;
        if entry.flags & PROPERTY_FLAG_MARKUP_DIRECTIVE != 0 {
            return Some(format!("x:{}", entry.name));
        }
        if entry.flags & PROPERTY_FLAG_IMPLICIT != 0 {
            return Some(entry.name.clone());
        }
        let owner = self.type_full_name(entry.declaring_type)?;
        Some(format!("{}.{}", owner, entry.name))
    }
}

/* Report the provider kind name used by XamlTypeInfoProviderKind. */
pub fn provider_kind_name(kind: u32) -> &'static str {
    match kind {
        0 => "unknown",
        1 => "native",
        2 => "managed",
        3 => "system",
        4 => "parser",
        5 => "alternate",
        _ => "invalid",
    }
}

fn expect_offset(table: &'static str, offset: u64, actual: usize) -> Result<(), XbfError> {
    if offset != actual as u64 {
        return Err(XbfError::TableOffsetMismatch {
            table,
            expected: offset,
            actual,
        });
    }
    Ok(())
}

/*
 * Reject a count that cannot fit before reading it.  min_entry is the
 * smallest possible encoded entry; it keeps a corrupt count from causing a
 * huge Vec allocation.
 */
fn read_count(
    cursor: &mut Cursor,
    table: &'static str,
    min_entry: usize,
) -> Result<usize, XbfError> {
    let count = cursor.u32()?;
    let remaining = cursor.remaining();
    if min_entry != 0 && count as usize > remaining / min_entry {
        return Err(XbfError::TableTooLarge {
            table,
            count,
            remaining,
        });
    }
    Ok(count as usize)
}

fn read_string(cursor: &mut Cursor, version: &Version) -> Result<String, XbfError> {
    let units = cursor.u32()?;
    cursor.utf16(units, version.null_terminated_strings())
}

fn read_strings(cursor: &mut Cursor, version: &Version) -> Result<Vec<String>, XbfError> {
    /* Shortest entry: uint32 length and (for 2.1) a NUL. */
    let min_entry = if version.null_terminated_strings() {
        6
    } else {
        4
    };
    let count = read_count(cursor, "string table", min_entry)?;
    let mut strings = Vec::with_capacity(count);
    for _ in 0..count {
        strings.push(read_string(cursor, version)?);
    }
    Ok(strings)
}

fn string_at(strings: &[String], table: &'static str, index: u32) -> Result<String, XbfError> {
    match strings.get(index as usize) {
        Some(value) => Ok(value.clone()),
        None => Err(XbfError::BadIndex {
            table,
            index,
            count: strings.len(),
        }),
    }
}

fn read_assemblies(
    cursor: &mut Cursor,
    strings: &[String],
) -> Result<Vec<AssemblyEntry>, XbfError> {
    let count = read_count(cursor, "assembly table", 8)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let provider_kind = cursor.u32()?;
        let name = string_at(strings, "assembly table", cursor.u32()?)?;
        entries.push(AssemblyEntry {
            provider_kind,
            name,
        });
    }
    Ok(entries)
}

fn read_type_namespaces(
    cursor: &mut Cursor,
    strings: &[String],
    assemblies: &[AssemblyEntry],
) -> Result<Vec<TypeNamespaceEntry>, XbfError> {
    let count = read_count(cursor, "type namespace table", 8)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let assembly = cursor.u32()?;
        if assembly as usize >= assemblies.len() {
            return Err(XbfError::BadIndex {
                table: "type namespace table",
                index: assembly,
                count: assemblies.len(),
            });
        }
        let name = string_at(strings, "type namespace table", cursor.u32()?)?;
        entries.push(TypeNamespaceEntry { assembly, name });
    }
    Ok(entries)
}

fn read_types(cursor: &mut Cursor, strings: &[String]) -> Result<Vec<TypeEntry>, XbfError> {
    let count = read_count(cursor, "type table", 12)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = cursor.u32()?;
        let namespace = cursor.u32()?;
        let name = string_at(strings, "type table", cursor.u32()?)?;
        entries.push(TypeEntry {
            flags,
            namespace,
            name,
        });
    }
    Ok(entries)
}

fn read_properties(
    cursor: &mut Cursor,
    strings: &[String],
) -> Result<Vec<PropertyEntry>, XbfError> {
    let count = read_count(cursor, "property table", 12)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = cursor.u32()?;
        let declaring_type = cursor.u32()?;
        let name = string_at(strings, "property table", cursor.u32()?)?;
        entries.push(PropertyEntry {
            flags,
            declaring_type,
            name,
        });
    }
    Ok(entries)
}

fn read_xml_namespaces(cursor: &mut Cursor, strings: &[String]) -> Result<Vec<String>, XbfError> {
    let count = read_count(cursor, "xml namespace table", 4)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(string_at(strings, "xml namespace table", cursor.u32()?)?);
    }
    Ok(entries)
}

/*
 * The tables can only be validated once all six exist: an IsUnknown type
 * points into the XML namespace table, and every other type points into the
 * type namespace table.  XamlBinaryMetadataReader2.cpp:231-257.
 */
fn validate_references(
    types: &[TypeEntry],
    properties: &[PropertyEntry],
    type_namespaces: &[TypeNamespaceEntry],
    xml_namespaces: &[String],
) -> Result<(), XbfError> {
    for entry in types {
        if entry.flags & TYPE_FLAG_UNKNOWN != 0 {
            if entry.namespace as usize >= xml_namespaces.len() {
                return Err(XbfError::BadIndex {
                    table: "type table (xml namespace)",
                    index: entry.namespace,
                    count: xml_namespaces.len(),
                });
            }
        } else if entry.flags & TYPE_FLAG_MARKUP_DIRECTIVE == 0
            && entry.namespace as usize >= type_namespaces.len()
        {
            return Err(XbfError::BadIndex {
                table: "type table (type namespace)",
                index: entry.namespace,
                count: type_namespaces.len(),
            });
        }
    }

    for entry in properties {
        let special = entry.flags & (PROPERTY_FLAG_MARKUP_DIRECTIVE | PROPERTY_FLAG_IMPLICIT) != 0;
        if !special && entry.declaring_type as usize >= types.len() {
            return Err(XbfError::BadIndex {
                table: "property table (declaring type)",
                index: entry.declaring_type,
                count: types.len(),
            });
        }
    }
    Ok(())
}

/*
 * The hash is stored as 64 ASCII hex digits (XamlBinaryMetadataReader2.cpp:
 * 686-704).  It is not verified by the runtime reader either; it exists so
 * the loader can compare the XBF against the metadata used by GenXbf.  We
 * preserve it for callers that want to do that comparison.
 */
fn ascii_hash(bytes: &[u8; 64]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let mut out = String::with_capacity(128);
            for byte in bytes {
                out.push_str(&format!("{byte:02x}"));
            }
            out
        }
    }
}
