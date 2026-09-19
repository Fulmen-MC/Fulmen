//! The handshake packet, the first packet a client sends.

use std::io::{Read, Write};

use crate::codec::{read_string, read_u16, write_string, write_u16};
use crate::packet::Packet;
use crate::varint::{read_varint, write_varint};
use crate::{Error, Result};

/// Maximum length of the server address field.
pub const MAX_ADDRESS_CHARS: usize = 255;

/// The state the connection switches to after the handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextState {
    Status = 1,
    Login = 2,
    /// Login after a server-initiated transfer (Java Edition 1.20.5 and later).
    Transfer = 3,
}

impl NextState {
    fn from_i32(v: i32) -> Result<Self> {
        match v {
            1 => Ok(Self::Status),
            2 => Ok(Self::Login),
            3 => Ok(Self::Transfer),
            other => Err(Error::InvalidNextState(other)),
        }
    }
}

/// Serverbound handshake (id 0x00).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    /// Protocol version of the client. Servers ignore it for status requests, so `-1`
    /// is fine when only pinging.
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: NextState,
}

impl Packet for Handshake {
    const ID: i32 = 0x00;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_varint(w, self.protocol_version)?;
        write_string(w, &self.server_address, MAX_ADDRESS_CHARS)?;
        write_u16(w, self.server_port)?;
        write_varint(w, self.next_state as i32)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            protocol_version: read_varint(r)?,
            server_address: read_string(r, MAX_ADDRESS_CHARS)?,
            server_port: read_u16(r)?,
            next_state: NextState::from_i32(read_varint(r)?)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{decode, encode};

    fn sample() -> Handshake {
        Handshake {
            protocol_version: 765,
            server_address: "localhost".into(),
            server_port: 25565,
            next_state: NextState::Status,
        }
    }

    #[test]
    fn handshake_wire_format() {
        let frame = encode(&sample()).unwrap();
        let mut expected = vec![
            0x10, // length: 16
            0x00, // packet id
            0xfd, 0x05, // protocol version 765
            0x09, // address length
        ];
        expected.extend_from_slice(b"localhost");
        expected.extend_from_slice(&[0x63, 0xdd, 0x01]); // port 25565, next state 1
        assert_eq!(frame, expected);
    }

    #[test]
    fn handshake_roundtrip() {
        let frame = encode(&sample()).unwrap();
        // Strip the length prefix (single byte here).
        let decoded: Handshake = decode(&frame[1..]).unwrap();
        assert_eq!(decoded, sample());
    }

    #[test]
    fn invalid_next_state_is_rejected() {
        let mut bad = sample();
        bad.next_state = NextState::Login;
        let mut frame = encode(&bad).unwrap();
        let last = frame.len() - 1;
        frame[last] = 9;
        assert!(matches!(
            decode::<Handshake>(&frame[1..]),
            Err(Error::InvalidNextState(9))
        ));
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut frame = encode(&sample()).unwrap();
        frame.push(0);
        assert!(matches!(
            decode::<Handshake>(&frame[1..]),
            Err(Error::TrailingBytes(1))
        ));
    }

    #[test]
    fn wrong_packet_id_is_rejected() {
        let mut frame = encode(&sample()).unwrap();
        frame[1] = 0x05;
        assert!(matches!(
            decode::<Handshake>(&frame[1..]),
            Err(Error::UnexpectedPacketId {
                expected: 0,
                got: 5
            })
        ));
    }
}
