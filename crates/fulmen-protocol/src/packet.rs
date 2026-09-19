//! Packet trait and uncompressed framing (`[length][id][body]`).

use std::io::{Cursor, Read, Write};

use crate::varint::{read_varint, varint_len, write_varint};
use crate::{Error, Result};

/// Largest allowed frame: the length prefix is at most 3 bytes (2^21 - 1).
pub const MAX_FRAME_LEN: usize = 2_097_151;

/// A single packet with a fixed id in a fixed state and direction.
pub trait Packet: Sized {
    /// Packet id (a VarInt on the wire).
    const ID: i32;

    /// Writes the body, without length prefix and id.
    fn write_body<W: Write>(&self, w: &mut W) -> Result<()>;

    /// Reads the body, without length prefix and id.
    fn read_body<R: Read>(r: &mut R) -> Result<Self>;
}

/// Encodes a packet into a complete frame: `[length][id][body]`.
pub fn encode<P: Packet>(packet: &P) -> Result<Vec<u8>> {
    let mut payload = Vec::new();
    write_varint(&mut payload, P::ID)?;
    packet.write_body(&mut payload)?;
    if payload.len() > MAX_FRAME_LEN {
        return Err(Error::FrameTooLarge {
            len: payload.len(),
            max: MAX_FRAME_LEN,
        });
    }
    let mut out = Vec::with_capacity(varint_len(payload.len() as i32) + payload.len());
    write_varint(&mut out, payload.len() as i32)?;
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Reads one frame and returns its payload (`[id][body]`), without the length prefix.
pub fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>> {
    let len = read_varint(r)?;
    if len < 0 {
        return Err(Error::NegativeLength(len));
    }
    let len = len as usize;
    if len > MAX_FRAME_LEN {
        return Err(Error::FrameTooLarge {
            len,
            max: MAX_FRAME_LEN,
        });
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Decodes a frame payload (`[id][body]`) as the packet `P`.
pub fn decode<P: Packet>(payload: &[u8]) -> Result<P> {
    let mut cur = Cursor::new(payload);
    let id = read_varint(&mut cur)?;
    if id != P::ID {
        return Err(Error::UnexpectedPacketId {
            expected: P::ID,
            got: id,
        });
    }
    let packet = P::read_body(&mut cur)?;
    let rest = payload.len() - cur.position() as usize;
    if rest != 0 {
        return Err(Error::TrailingBytes(rest));
    }
    Ok(packet)
}
