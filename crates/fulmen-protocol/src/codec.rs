//! Primitive readers and writers (big-endian numbers, length-prefixed strings).

use std::io::{Read, Write};

use crate::varint::{read_varint, write_varint};
use crate::{Error, Result};

pub fn read_u16<R: Read>(r: &mut R) -> Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_be_bytes(b))
}

pub fn write_u16<W: Write>(w: &mut W, v: u16) -> Result<()> {
    Ok(w.write_all(&v.to_be_bytes())?)
}

pub fn read_i64<R: Read>(r: &mut R) -> Result<i64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(i64::from_be_bytes(b))
}

pub fn write_i64<W: Write>(w: &mut W, v: i64) -> Result<()> {
    Ok(w.write_all(&v.to_be_bytes())?)
}

/// Reads a VarInt-length-prefixed UTF-8 string of at most `max_chars` characters.
///
/// Like the vanilla implementation, the limit is counted in UTF-16 code units, and the
/// encoded byte length may not exceed `max_chars * 3`.
pub fn read_string<R: Read>(r: &mut R, max_chars: usize) -> Result<String> {
    let len = read_varint(r)?;
    if len < 0 {
        return Err(Error::NegativeLength(len));
    }
    let len = len as usize;
    let max_bytes = max_chars * 3;
    if len > max_bytes {
        return Err(Error::StringTooLong {
            len,
            max: max_bytes,
        });
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    let s = String::from_utf8(buf).map_err(|_| Error::InvalidUtf8)?;
    let chars = s.encode_utf16().count();
    if chars > max_chars {
        return Err(Error::StringTooLong {
            len: chars,
            max: max_chars,
        });
    }
    Ok(s)
}

/// Writes a VarInt-length-prefixed UTF-8 string of at most `max_chars` characters.
pub fn write_string<W: Write>(w: &mut W, s: &str, max_chars: usize) -> Result<()> {
    let chars = s.encode_utf16().count();
    if chars > max_chars {
        return Err(Error::StringTooLong {
            len: chars,
            max: max_chars,
        });
    }
    write_varint(w, s.len() as i32)?;
    w.write_all(s.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_roundtrip() {
        let mut out = Vec::new();
        write_string(&mut out, "localhost", 255).unwrap();
        assert_eq!(out[0], 9);
        assert_eq!(read_string(&mut &out[..], 255).unwrap(), "localhost");
    }

    #[test]
    fn string_unicode_roundtrip() {
        let s = "blöcke ⚡ 🧱";
        let mut out = Vec::new();
        write_string(&mut out, s, 64).unwrap();
        assert_eq!(read_string(&mut &out[..], 64).unwrap(), s);
    }

    #[test]
    fn string_too_long_is_rejected() {
        let mut out = Vec::new();
        assert!(write_string(&mut out, "abcd", 3).is_err());
        // Declared length larger than max_chars * 3.
        let mut data = Vec::new();
        write_varint(&mut data, 10).unwrap();
        data.extend_from_slice(&[b'a'; 10]);
        assert!(matches!(
            read_string(&mut &data[..], 3),
            Err(Error::StringTooLong { .. })
        ));
    }

    #[test]
    fn string_invalid_utf8_is_rejected() {
        let data = [2u8, 0xff, 0xfe];
        assert!(matches!(
            read_string(&mut &data[..], 16),
            Err(Error::InvalidUtf8)
        ));
    }

    #[test]
    fn numbers_are_big_endian() {
        let mut out = Vec::new();
        write_u16(&mut out, 25565).unwrap();
        assert_eq!(out, [0x63, 0xdd]);
        let mut out = Vec::new();
        write_i64(&mut out, 0x0102_0304_0506_0708).unwrap();
        assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(read_i64(&mut &out[..]).unwrap(), 0x0102_0304_0506_0708);
    }
}
