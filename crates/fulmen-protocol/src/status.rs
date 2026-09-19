//! Status ("server list ping") packets.

use std::io::{Read, Write};

use crate::Result;
use crate::codec::{read_i64, read_string, write_i64, write_string};
use crate::packet::Packet;

/// Maximum length of the status JSON string.
pub const MAX_STATUS_JSON_CHARS: usize = 32_767;

/// Serverbound status request (id 0x00, empty body).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusRequest;

impl Packet for StatusRequest {
    const ID: i32 = 0x00;

    fn write_body<W: Write>(&self, _w: &mut W) -> Result<()> {
        Ok(())
    }

    fn read_body<R: Read>(_r: &mut R) -> Result<Self> {
        Ok(Self)
    }
}

/// Clientbound status response (id 0x00). The JSON is kept as raw text; parsing it is
/// left to the caller so this crate stays dependency-free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusResponse {
    pub json: String,
}

impl Packet for StatusResponse {
    const ID: i32 = 0x00;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_string(w, &self.json, MAX_STATUS_JSON_CHARS)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            json: read_string(r, MAX_STATUS_JSON_CHARS)?,
        })
    }
}

/// Serverbound ping request (id 0x01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingRequest {
    pub payload: i64,
}

impl Packet for PingRequest {
    const ID: i32 = 0x01;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_i64(w, self.payload)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            payload: read_i64(r)?,
        })
    }
}

/// Clientbound pong response (id 0x01). The server echoes the ping payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PongResponse {
    pub payload: i64,
}

impl Packet for PongResponse {
    const ID: i32 = 0x01;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_i64(w, self.payload)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            payload: read_i64(r)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{decode, encode, read_frame};

    #[test]
    fn status_request_is_two_bytes() {
        assert_eq!(encode(&StatusRequest).unwrap(), [0x01, 0x00]);
    }

    #[test]
    fn ping_wire_format() {
        let frame = encode(&PingRequest { payload: 1 }).unwrap();
        assert_eq!(frame, [0x09, 0x01, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn status_response_roundtrip_via_frame_reader() {
        let json = r#"{"version":{"name":"x","protocol":1},"players":{"max":1,"online":0}}"#;
        let bytes = encode(&StatusResponse { json: json.into() }).unwrap();
        let payload = read_frame(&mut &bytes[..]).unwrap();
        let decoded: StatusResponse = decode(&payload).unwrap();
        assert_eq!(decoded.json, json);
    }
}
