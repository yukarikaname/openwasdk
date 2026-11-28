//! Find XBF documents inside a PRI resource index.
//!
//! A framework package keeps its theme resources as compiled XBF inside a `.pri`, and the
//! index itself is not something this project needs to decode: an XBF document starts with
//! its own magic and the parser validates the header and the node stream. So a document is
//! "an offset where the magic occurs and the parse succeeds" -- enough to extract the theme
//! dictionary from `Microsoft.UI.Xaml.Controls.pri`, and honest about not being a general
//! PRI reader.

use crate::{MAGIC, Version, Xbf, parse};

/// One XBF document found inside a resource index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fragment {
    /// Where the document starts in the index.
    pub offset: usize,
    /// Prefix plus metadata plus node stream: the bytes the document occupies.
    pub length: usize,
    pub version: Version,
    pub substreams: usize,
    pub nodes: usize,
}

/// Scan `data` for XBF documents, in the order they appear.
pub fn scan(data: &[u8]) -> Vec<Fragment> {
    let mut found = Vec::new();
    for offset in 0..data.len().saturating_sub(MAGIC.len()) {
        if data[offset..offset + MAGIC.len()] != MAGIC {
            continue;
        }
        let Ok(file) = parse(&data[offset..]) else {
            continue;
        };
        // The header prefix is the magic and the two size fields.
        let length = 12 + file.header.metadata_size as usize + file.header.node_size as usize;
        if offset + length > data.len() {
            continue;
        }
        found.push(Fragment {
            offset,
            length,
            version: file.header.version,
            substreams: file.streams.len(),
            nodes: file.total_nodes(),
        });
    }
    found
}

/// The bytes of one fragment, for writing it out or handing it to the loader.
pub fn fragment_bytes<'a>(data: &'a [u8], fragment: &Fragment) -> &'a [u8] {
    &data[fragment.offset..fragment.offset + fragment.length]
}

/// The document, parsed again from its own bytes.
pub fn fragment_document(data: &[u8], fragment: &Fragment) -> Option<Xbf> {
    parse(fragment_bytes(data, fragment)).ok()
}

#[cfg(test)]
mod tests {
    use super::scan;

    /// The theme dictionary the crate already tests against, wrapped in junk: a resource
    /// index is a file with documents in it, not a file of documents.
    #[test]
    fn a_document_is_found_at_its_offset() {
        const THEME: &[u8] = include_bytes!("../tests/data/winui-theme-resources-small.xbf");
        let mut index = vec![0u8; 64];
        index.extend_from_slice(b"PRIJUNK");
        index.extend_from_slice(THEME);
        index.extend_from_slice(&[0u8; 32]);

        let found = scan(&index);
        assert_eq!(found.len(), 1, "one document");
        assert_eq!(found[0].offset, 64 + 7);
        assert_eq!(found[0].length, THEME.len());
        assert!(found[0].nodes > 0);
    }

    #[test]
    fn a_truncated_document_is_not_reported() {
        const THEME: &[u8] = include_bytes!("../tests/data/winui-theme-resources-small.xbf");
        let truncated = &THEME[..THEME.len() - 8];
        assert!(scan(truncated).is_empty());
    }
}
