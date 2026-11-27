/*
 * XBF file and metadata headers.
 *
 * Layout taken from:
 *   XBFHeaderReader.cs:28-46 and XBFReader.cs:194-219
 *     magic "XBF\0", uint32 metadata size, uint32 node size, version.
 *   XamlBinaryMetadata.h:23-44 (XamlBinaryFileVersion, XbfV1/2/2_1)
 *   XamlBinaryMetadata.h:46-65 (XamlBinaryFileHeader2)
 *     six uint64 table offsets followed by a 64 byte hash.
 *   XamlBinaryMetadataStore.cpp:47-95
 *     the metadata stream begins at file offset 12 and starts with the
 *     version, so every table offset is relative to that point.
 *   XbfMetadata.cs:29-64 (the WidgetSpinner reader makes the same checks)
 *
 * Note that the first 8 bytes of the metadata stream are the same major and
 * minor version bytes that XBFHeaderReader.cs reads as part of the file
 * header; there is no separate copy.
 */

use crate::cursor::Cursor;
use crate::error::XbfError;

/* File offset where the metadata stream starts (magic + two sizes). */
pub(crate) const PREFIX_SIZE: usize = 12;

/* Version + XamlBinaryFileHeader2. */
pub(crate) const METADATA_HEADER_SIZE: usize = 8 + 6 * 8 + 64;

pub const MAGIC: [u8; 4] = [0x58, 0x42, 0x46, 0x00];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    /*
     * XBF 2.1 and later append a NUL to every metadata string.
     * XamlBinaryMetadata.h:34-38.
     */
    pub fn null_terminated_strings(&self) -> bool {
        self.major == 2 && self.minor >= 1
    }
}

#[derive(Debug)]
pub struct Header {
    pub metadata_size: u32,
    pub node_size: u32,
    pub version: Version,
}

/* The six table offsets and the 64 byte hash inside the metadata stream. */
pub(crate) struct MetadataHeader {
    pub string_table: u64,
    pub assembly_list: u64,
    pub type_namespace_list: u64,
    pub type_list: u64,
    pub property_list: u64,
    pub xml_namespace_list: u64,
    pub hash: [u8; 64],
}

pub(crate) fn parse(data: &[u8]) -> Result<(Header, MetadataHeader), XbfError> {
    let mut cursor = Cursor::new(data, "file header");

    let magic = cursor.take(4)?;
    if magic != MAGIC {
        let mut actual = [0u8; 4];
        actual.copy_from_slice(magic);
        return Err(XbfError::BadMagic(actual));
    }

    let metadata_size = cursor.u32()?;
    let node_size = cursor.u32()?;

    /* Reject impossible section sizes before slicing. */
    let metadata_end =
        PREFIX_SIZE
            .checked_add(metadata_size as usize)
            .ok_or(XbfError::BadSectionSize {
                section: "metadata",
                size: metadata_size as u64,
                available: data.len().saturating_sub(PREFIX_SIZE),
            })?;
    if metadata_end > data.len() {
        return Err(XbfError::BadSectionSize {
            section: "metadata",
            size: metadata_size as u64,
            available: data.len().saturating_sub(PREFIX_SIZE),
        });
    }
    let node_end =
        metadata_end
            .checked_add(node_size as usize)
            .ok_or(XbfError::BadSectionSize {
                section: "node stream",
                size: node_size as u64,
                available: data.len().saturating_sub(metadata_end),
            })?;
    if node_end > data.len() {
        return Err(XbfError::BadSectionSize {
            section: "node stream",
            size: node_size as u64,
            available: data.len().saturating_sub(metadata_end),
        });
    }

    /* The metadata stream starts with the version, then the header. */
    let mut meta = Cursor::new(&data[PREFIX_SIZE..metadata_end], "metadata header");
    let major = meta.u32()?;
    let minor = meta.u32()?;
    if major != 2 {
        return Err(XbfError::UnsupportedVersion { major, minor });
    }
    let version = Version { major, minor };

    let string_table = meta.u64()?;
    let assembly_list = meta.u64()?;
    let type_namespace_list = meta.u64()?;
    let type_list = meta.u64()?;
    let property_list = meta.u64()?;
    let xml_namespace_list = meta.u64()?;

    let mut hash = [0u8; 64];
    hash.copy_from_slice(meta.take(64)?);

    Ok((
        Header {
            metadata_size,
            node_size,
            version,
        },
        MetadataHeader {
            string_table,
            assembly_list,
            type_namespace_list,
            type_list,
            property_list,
            xml_namespace_list,
            hash,
        },
    ))
}
