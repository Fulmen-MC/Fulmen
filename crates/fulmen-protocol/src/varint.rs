//! Variable-length integers (VarInt: up to 5 bytes, VarLong: up to 10 bytes).

use std::io::{Read, Write};

use crate::{Error, Result};

pub const MAX_VARINT_BYTES: usize = 5;
pub const MAX_VARLONG_BYTES: usize = 10;

/// Reads a VarInt.
pub fn read_varint<R: Read>(r: &mut R) -> Result<i32> {
    let mut value: u32 = 0;
    for i in 0..MAX_VARINT_BYTES {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte)?;
        value |= u32::from(byte[0] & 0x7F) << (7 * i);
        if byte[0] & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(Error::VarIntTooLong)
}

/// Writes a VarInt.
pub fn write_varint<W: Write>(w: &mut W, value: i32) -> Result<()> {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            w.write_all(&[v as u8])?;
            return Ok(());
        }
        w.write_all(&[(v & 0x7F) as u8 | 0x80])?;
        v >>= 7;
    }
}

/// Number of bytes `value` occupies as a VarInt.
pub fn varint_len(value: i32) -> usize {
    let v = value as u32;
    match v {
        0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        0x4000..=0x1F_FFFF => 3,
        0x20_0000..=0x0FFF_FFFF => 4,
        _ => 5,
    }
}

/// Reads a VarLong.
pub fn read_varlong<R: Read>(r: &mut R) -> Result<i64> {
    let mut value: u64 = 0;
    for i in 0..MAX_VARLONG_BYTES {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte)?;
        value |= u64::from(byte[0] & 0x7F) << (7 * i);
        if byte[0] & 0x80 == 0 {
            return Ok(value as i64);
        }
    }
    Err(Error::VarIntTooLong)
}

/// Writes a VarLong.
pub fn write_varlong<W: Write>(w: &mut W, value: i64) -> Result<()> {
    let mut v = value as u64;
    loop {
        if v & !0x7F == 0 {
            w.write_all(&[v as u8])?;
            return Ok(());
        }
        w.write_all(&[(v & 0x7F) as u8 | 0x80])?;
        v >>= 7;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INT_CASES: &[(i32, &[u8])] = &[
        (0, &[0x00]),
        (1, &[0x01]),
        (127, &[0x7f]),
        (128, &[0x80, 0x01]),
        (255, &[0xff, 0x01]),
        (25565, &[0xdd, 0xc7, 0x01]),
        (2_097_151, &[0xff, 0xff, 0x7f]),
        (i32::MAX, &[0xff, 0xff, 0xff, 0xff, 0x07]),
        (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
        (i32::MIN, &[0x80, 0x80, 0x80, 0x80, 0x08]),
    ];

    #[test]
    fn varint_known_values() {
        for &(value, bytes) in INT_CASES {
            let mut out = Vec::new();
            write_varint(&mut out, value).unwrap();
            assert_eq!(out, bytes, "encode {value}");
            assert_eq!(varint_len(value), bytes.len(), "len {value}");
            assert_eq!(
                read_varint(&mut &bytes[..]).unwrap(),
                value,
                "decode {value}"
            );
        }
    }

    #[test]
    fn varlong_known_values() {
        let cases: &[(i64, &[u8])] = &[
            (0, &[0x00]),
            (2_147_483_647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
            (
                i64::MAX,
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f],
            ),
            (
                -1,
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
            ),
            (
                i64::MIN,
                &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
            ),
        ];
        for &(value, bytes) in cases {
            let mut out = Vec::new();
            write_varlong(&mut out, value).unwrap();
            assert_eq!(out, bytes, "encode {value}");
            assert_eq!(
                read_varlong(&mut &bytes[..]).unwrap(),
                value,
                "decode {value}"
            );
        }
    }

    #[test]
    fn varint_roundtrip_many() {
        for value in (i32::MIN..=i32::MAX)
            .step_by(65_537)
            .chain([0, 1, -1, i32::MAX, i32::MIN])
        {
            let mut out = Vec::new();
            write_varint(&mut out, value).unwrap();
            assert_eq!(read_varint(&mut &out[..]).unwrap(), value);
        }
    }

    #[test]
    fn varint_too_long_is_rejected() {
        let bytes = [0x80u8; 6];
        assert!(matches!(
            read_varint(&mut &bytes[..]),
            Err(Error::VarIntTooLong)
        ));
    }

    #[test]
    fn varint_truncated_is_io_error() {
        let bytes = [0x80u8];
        assert!(matches!(read_varint(&mut &bytes[..]), Err(Error::Io(_))));
    }
}
