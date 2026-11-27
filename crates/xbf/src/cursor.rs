/*
 * Bounds checked little-endian cursor.
 *
 * The upstream code reads through IPALStream/BinaryReader and relies on the
 * surrounding parse to validate lengths.  Here every accessor returns a typed
 * error instead, so a truncated or hostile file can never index out of bounds.
 */

use crate::error::XbfError;

pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    context: &'static str,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(data: &'a [u8], context: &'static str) -> Self {
        Cursor {
            data,
            pos: 0,
            context,
        }
    }

    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.pos == self.data.len()
    }

    fn need(&self, count: usize) -> Result<(), XbfError> {
        if count > self.remaining() {
            return Err(XbfError::Truncated {
                context: self.context,
                offset: self.pos,
                need: count,
                len: self.data.len(),
            });
        }
        Ok(())
    }

    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], XbfError> {
        self.need(count)?;
        let start = self.pos;
        self.pos += count;
        Ok(&self.data[start..self.pos])
    }

    pub(crate) fn skip(&mut self, count: usize) -> Result<(), XbfError> {
        self.need(count)?;
        self.pos += count;
        Ok(())
    }

    pub(crate) fn u8(&mut self) -> Result<u8, XbfError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, XbfError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, XbfError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, XbfError> {
        let bytes = self.take(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(raw))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, XbfError> {
        Ok(self.u32()? as i32)
    }

    pub(crate) fn f32(&mut self) -> Result<f32, XbfError> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub(crate) fn bool(&mut self) -> Result<bool, XbfError> {
        Ok(self.u8()? != 0)
    }

    /*
     * 7-bit encoded unsigned integer, as written by the C# BinaryWriter and
     * decoded by XamlBinaryFormatSubReader2::ReadUInt.  Unlike the .NET
     * helper we reject overlong encodings.
     */
    pub(crate) fn leb_u32(&mut self) -> Result<u32, XbfError> {
        let start = self.pos;
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.u8()?;
            if shift == 28 && byte > 0x0f {
                return Err(XbfError::BadLeb128 { offset: start });
            }
            value |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(XbfError::BadLeb128 { offset: start })
    }

    /*
     * A string stored as a UTF-16 code-unit count followed by that many
     * little-endian WCHARs.  Metadata strings for XBF 2.1+ carry a trailing
     * NUL (XamlSerializationHelper.h:68-78); node stream strings do not.
     */
    pub(crate) fn utf16(&mut self, units: u32, null_terminated: bool) -> Result<String, XbfError> {
        let count = units as usize;
        let bytes_needed = count.checked_mul(2);
        let Some(bytes_needed) = bytes_needed else {
            return Err(XbfError::StringTooLong {
                length: units,
                remaining: self.remaining(),
            });
        };
        if bytes_needed > self.remaining() {
            return Err(XbfError::StringTooLong {
                length: units,
                remaining: self.remaining(),
            });
        }
        let bytes = self.take(bytes_needed)?;
        let mut wide = Vec::with_capacity(count);
        for pair in bytes.as_chunks::<2>().0 {
            wide.push(u16::from_le_bytes([pair[0], pair[1]]));
        }
        let value = String::from_utf16(&wide).map_err(|_| XbfError::BadUtf16 {
            context: self.context,
        })?;
        if null_terminated {
            /* Consume the NUL; real files always store one here. */
            let _ = self.u16()?;
        }
        Ok(value)
    }
}
