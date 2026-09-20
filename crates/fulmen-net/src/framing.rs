//! Packet framing with optional zlib compression.
//!
//! Without compression a frame is `[length][id][body]`. Once the server has sent Set
//! Compression, every frame is `[length][data length][id + body]`, where the part after
//! the data length is zlib-compressed if the packet reached the threshold, and the data
//! length is `0` for packets that were sent uncompressed.

use std::io::{Cursor, Read, Write};

use fulmen_protocol::Error as ProtocolError;
use fulmen_protocol::packet::{MAX_FRAME_LEN, Packet, read_frame};
use fulmen_protocol::varint::{read_varint, varint_len, write_varint};

use crate::{Error, Result};

/// Upper bound for the size of a decompressed packet (protects against zip bombs).
pub const MAX_DECOMPRESSED_LEN: usize = 32 * 1024 * 1024;

/// A packet with its id, as it travels between the connection threads and the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub id: i32,
    pub body: Vec<u8>,
}

impl RawPacket {
    pub fn new(id: i32, body: Vec<u8>) -> Self {
        Self { id, body }
    }

    /// Builds a raw packet from a typed one.
    pub fn of<P: Packet>(packet: &P) -> Result<Self> {
        let mut body = Vec::new();
        packet.write_body(&mut body)?;
        Ok(Self { id: P::ID, body })
    }

    /// Decodes the body as `P`, checking the id and that no bytes are left over.
    pub fn decode<P: Packet>(&self) -> Result<P> {
        if self.id != P::ID {
            return Err(ProtocolError::UnexpectedPacketId {
                expected: P::ID,
                got: self.id,
            }
            .into());
        }
        let mut cur = Cursor::new(&self.body[..]);
        let packet = P::read_body(&mut cur)?;
        let rest = self.body.len() - cur.position() as usize;
        if rest != 0 {
            return Err(ProtocolError::TrailingBytes(rest).into());
        }
        Ok(packet)
    }

    fn payload(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(varint_len(self.id) + self.body.len());
        write_varint(&mut out, self.id)?;
        out.extend_from_slice(&self.body);
        Ok(out)
    }
}

/// Compression setting of a connection: `None` means disabled.
pub type Threshold = Option<usize>;

/// Converts the value of a Set Compression packet into a [`Threshold`].
pub fn threshold_from_packet(value: i32) -> Threshold {
    usize::try_from(value).ok()
}

/// Writes one packet, compressing it if it reaches the threshold.
pub fn write_packet<W: Write>(w: &mut W, packet: &RawPacket, threshold: Threshold) -> Result<()> {
    let payload = packet.payload()?;
    let mut frame_body = Vec::new();
    match threshold {
        None => frame_body = payload,
        Some(t) if payload.len() >= t => {
            write_varint(&mut frame_body, payload.len() as i32)?;
            let mut enc =
                flate2::write::ZlibEncoder::new(&mut frame_body, flate2::Compression::default());
            enc.write_all(&payload)?;
            enc.finish()?;
        }
        Some(_) => {
            write_varint(&mut frame_body, 0)?;
            frame_body.extend_from_slice(&payload);
        }
    }
    if frame_body.len() > MAX_FRAME_LEN {
        return Err(ProtocolError::FrameTooLarge {
            len: frame_body.len(),
            max: MAX_FRAME_LEN,
        }
        .into());
    }
    let mut out = Vec::with_capacity(varint_len(frame_body.len() as i32) + frame_body.len());
    write_varint(&mut out, frame_body.len() as i32)?;
    out.extend_from_slice(&frame_body);
    w.write_all(&out)?;
    w.flush()?;
    Ok(())
}

/// Reads one packet, decompressing it if compression is enabled.
pub fn read_packet<R: Read>(r: &mut R, compression: bool) -> Result<RawPacket> {
    let frame = read_frame(r)?;
    let payload = if compression {
        let mut cur = Cursor::new(&frame[..]);
        let data_len = read_varint(&mut cur)?;
        let rest = &frame[cur.position() as usize..];
        if data_len == 0 {
            rest.to_vec()
        } else {
            let data_len = usize::try_from(data_len).map_err(|_| Error::BadCompression)?;
            if data_len > MAX_DECOMPRESSED_LEN {
                return Err(Error::BadCompression);
            }
            let mut out = Vec::with_capacity(data_len);
            // Read one byte more than announced to notice a length mismatch.
            let mut dec = flate2::read::ZlibDecoder::new(rest).take(data_len as u64 + 1);
            dec.read_to_end(&mut out)
                .map_err(|_| Error::BadCompression)?;
            if out.len() != data_len {
                return Err(Error::BadCompression);
            }
            out
        }
    } else {
        frame
    };
    let mut cur = Cursor::new(&payload[..]);
    let id = read_varint(&mut cur)?;
    let body = payload[cur.position() as usize..].to_vec();
    Ok(RawPacket { id, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn uncompressed_roundtrip() {
        let p = RawPacket::new(0x2c, vec![1, 2, 3]);
        let mut out = Vec::new();
        write_packet(&mut out, &p, None).unwrap();
        assert_eq!(out, [0x04, 0x2c, 1, 2, 3]);
        assert_eq!(read_packet(&mut &out[..], false).unwrap(), p);
    }

    #[test]
    fn small_packet_with_compression_enabled_has_zero_data_length() {
        let p = RawPacket::new(0x2c, vec![1, 2, 3]);
        let mut out = Vec::new();
        write_packet(&mut out, &p, Some(256)).unwrap();
        assert_eq!(out, [0x05, 0x00, 0x2c, 1, 2, 3]);
        assert_eq!(read_packet(&mut &out[..], true).unwrap(), p);
    }

    #[test]
    fn large_packet_is_compressed_and_roundtrips() {
        let p = RawPacket::new(0x2c, vec![b'a'; 300]);
        let mut out = Vec::new();
        write_packet(&mut out, &p, Some(256)).unwrap();
        assert!(
            out.len() < 60,
            "expected a small compressed frame, got {}",
            out.len()
        );
        assert_eq!(read_packet(&mut &out[..], true).unwrap(), p);
    }

    /// Frame produced independently with Python's zlib: id 0x2c followed by 300 x 'a'.
    #[test]
    fn decodes_frame_compressed_by_another_implementation() {
        let frame = hex("0fad02789cd3491c054403000c7471d9");
        let p = read_packet(&mut &frame[..], true).unwrap();
        assert_eq!(p, RawPacket::new(0x2c, vec![b'a'; 300]));
    }

    #[test]
    fn wrong_data_length_is_rejected() {
        // Announces 10 bytes, but the zlib stream holds 301.
        let mut frame = hex("0fad02789cd3491c054403000c7471d9");
        frame[1] = 0x0a; // data length varint (first byte of the second field)
        frame.remove(2); // shorten the varint from two bytes to one
        frame[0] -= 1;
        assert!(matches!(
            read_packet(&mut &frame[..], true),
            Err(Error::BadCompression)
        ));
    }

    #[test]
    fn negative_set_compression_disables_compression() {
        assert_eq!(threshold_from_packet(-1), None);
        assert_eq!(threshold_from_packet(0), Some(0));
        assert_eq!(threshold_from_packet(256), Some(256));
    }
}
