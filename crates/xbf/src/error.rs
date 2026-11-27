/*
 * Typed errors for the XBF reader.
 *
 * Nothing in this crate unwraps or indexes file data directly, so every
 * malformed input surfaces as one of these variants instead of a panic.
 */

use std::fmt;

#[derive(Debug)]
pub enum XbfError {
    /* The file could not be read from disk. */
    Io(std::io::Error),
    /* The first four bytes are not "XBF\0". */
    BadMagic([u8; 4]),
    /* A read ran past the end of its section. */
    Truncated {
        context: &'static str,
        offset: usize,
        need: usize,
        len: usize,
    },
    /* Only major version 2 is implemented. */
    UnsupportedVersion {
        major: u32,
        minor: u32,
    },
    /* The declared section size does not fit in the file. */
    BadSectionSize {
        section: &'static str,
        size: u64,
        available: usize,
    },
    /* A metadata table does not start where the previous table ended. */
    TableOffsetMismatch {
        table: &'static str,
        expected: u64,
        actual: usize,
    },
    /* A table or vector count cannot be satisfied by the remaining bytes. */
    TableTooLarge {
        table: &'static str,
        count: u32,
        remaining: usize,
    },
    /* A string or vector length is larger than the remaining input. */
    StringTooLong {
        length: u32,
        remaining: usize,
    },
    /* A table entry refers to a non-existent entry. */
    BadIndex {
        table: &'static str,
        index: u32,
        count: usize,
    },
    /* UTF-16 input contained an unpaired surrogate. */
    BadUtf16 {
        context: &'static str,
    },
    /* A 7-bit encoded integer is overlong or overflows. */
    BadLeb128 {
        offset: usize,
    },
    /* A node kind that this reader does not know how to skip. */
    UnknownNodeKind {
        kind: u8,
        offset: u32,
    },
    /* A coerced value kind that this reader does not know. */
    UnknownConstant {
        kind: u8,
        offset: u32,
    },
    /* A custom runtime data blob with an unknown type index. */
    UnknownCustomData {
        kind: u32,
        offset: u32,
    },
    /* A substream table entry points outside the node stream. */
    BadSubstream {
        index: usize,
    },
    /* The node stream cannot be represented by the typed model. */
    ModelError {
        stream: usize,
        index: usize,
        kind: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for XbfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XbfError::Io(err) => write!(f, "i/o error: {err}"),
            XbfError::BadMagic(magic) => write!(
                f,
                "bad XBF magic {:02x} {:02x} {:02x} {:02x}",
                magic[0], magic[1], magic[2], magic[3]
            ),
            XbfError::Truncated {
                context,
                offset,
                need,
                len,
            } => write!(
                f,
                "{context}: need {need} bytes at {offset}, section is {len} bytes"
            ),
            XbfError::UnsupportedVersion { major, minor } => {
                write!(f, "unsupported XBF version {major}.{minor}")
            }
            XbfError::BadSectionSize {
                section,
                size,
                available,
            } => write!(
                f,
                "{section} declares {size} bytes but only {available} are available"
            ),
            XbfError::TableOffsetMismatch {
                table,
                expected,
                actual,
            } => write!(
                f,
                "{table} starts at {actual}, expected contiguous offset {expected}"
            ),
            XbfError::TableTooLarge {
                table,
                count,
                remaining,
            } => write!(
                f,
                "{table} declares {count} entries with only {remaining} bytes left"
            ),
            XbfError::StringTooLong { length, remaining } => write!(
                f,
                "string of {length} UTF-16 units does not fit in {remaining} bytes"
            ),
            XbfError::BadIndex {
                table,
                index,
                count,
            } => write!(f, "{table} index {index} out of range (count {count})"),
            XbfError::BadUtf16 { context } => write!(f, "{context}: invalid UTF-16"),
            XbfError::BadLeb128 { offset } => {
                write!(f, "invalid 7-bit encoded integer at byte {offset}")
            }
            XbfError::UnknownNodeKind { kind, offset } => {
                write!(f, "unknown node kind {kind} at substream offset {offset}")
            }
            XbfError::UnknownConstant { kind, offset } => {
                write!(f, "unknown value kind {kind} at substream offset {offset}")
            }
            XbfError::UnknownCustomData { kind, offset } => {
                write!(
                    f,
                    "unknown custom runtime data kind {kind} at substream offset {offset}"
                )
            }
            XbfError::BadSubstream { index } => {
                write!(f, "substream table entry {index} is out of bounds")
            }
            XbfError::ModelError {
                stream,
                index,
                kind,
                reason,
            } => write!(
                f,
                "model: {reason} at substream {stream} node {index} ({kind})"
            ),
        }
    }
}

impl std::error::Error for XbfError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            XbfError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for XbfError {
    fn from(err: std::io::Error) -> Self {
        XbfError::Io(err)
    }
}
