//! Login-state packets (the parts needed for an offline-mode login).
//!
//! Packet ids follow protocol 776 (26.2) from the Minecraft Wiki and have not been checked
//! against 777 yet; they are meant to be generated from the server data reports later.

use std::io::{Read, Write};

use crate::codec::{read_bool, read_string, read_uuid, write_bool, write_string, write_uuid};
use crate::packet::Packet;
use crate::varint::{read_varint, write_varint};
use crate::{Error, Result};

pub const MAX_NAME_CHARS: usize = 16;
pub const MAX_DISCONNECT_CHARS: usize = 262_144;
const MAX_PROPERTIES: usize = 1024;

/// Serverbound Login Start (id 0x00).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginStart {
    pub name: String,
    pub uuid: u128,
}

impl Packet for LoginStart {
    const ID: i32 = 0x00;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_string(w, &self.name, MAX_NAME_CHARS)?;
        write_uuid(w, self.uuid)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            name: read_string(r, MAX_NAME_CHARS)?,
            uuid: read_uuid(r)?,
        })
    }
}

/// A profile property, usually the skin `textures`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

/// Clientbound Login Success (id 0x02): the game profile of the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSuccess {
    pub uuid: u128,
    pub username: String,
    pub properties: Vec<Property>,
    /// Session ID, added in protocol 776.
    pub session_id: u128,
}

impl Packet for LoginSuccess {
    const ID: i32 = 0x02;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_uuid(w, self.uuid)?;
        write_string(w, &self.username, MAX_NAME_CHARS)?;
        write_varint(w, self.properties.len() as i32)?;
        for p in &self.properties {
            write_string(w, &p.name, 64)?;
            write_string(w, &p.value, 32_767)?;
            write_bool(w, p.signature.is_some())?;
            if let Some(sig) = &p.signature {
                write_string(w, sig, 1024)?;
            }
        }
        write_uuid(w, self.session_id)?;
        Ok(())
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        let uuid = read_uuid(r)?;
        let username = read_string(r, MAX_NAME_CHARS)?;
        let count = read_varint(r)?;
        if count < 0 {
            return Err(Error::NegativeLength(count));
        }
        let count = count as usize;
        if count > MAX_PROPERTIES {
            return Err(Error::StringTooLong {
                len: count,
                max: MAX_PROPERTIES,
            });
        }
        let mut properties = Vec::with_capacity(count);
        for _ in 0..count {
            let name = read_string(r, 64)?;
            let value = read_string(r, 32_767)?;
            let signature = if read_bool(r)? {
                Some(read_string(r, 1024)?)
            } else {
                None
            };
            properties.push(Property {
                name,
                value,
                signature,
            });
        }
        let session_id = read_uuid(r)?;
        Ok(Self {
            uuid,
            username,
            properties,
            session_id,
        })
    }
}

/// Clientbound Set Compression (id 0x03). A negative threshold disables compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetCompression {
    pub threshold: i32,
}

impl Packet for SetCompression {
    const ID: i32 = 0x03;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_varint(w, self.threshold)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            threshold: read_varint(r)?,
        })
    }
}

/// Serverbound Login Acknowledged (id 0x03, empty). Switches to the configuration state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoginAcknowledged;

impl Packet for LoginAcknowledged {
    const ID: i32 = 0x03;

    fn write_body<W: Write>(&self, _w: &mut W) -> Result<()> {
        Ok(())
    }

    fn read_body<R: Read>(_r: &mut R) -> Result<Self> {
        Ok(Self)
    }
}

/// Clientbound Disconnect in the login state (id 0x00). The reason is JSON text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginDisconnect {
    pub reason: String,
}

impl Packet for LoginDisconnect {
    const ID: i32 = 0x00;

    fn write_body<W: Write>(&self, w: &mut W) -> Result<()> {
        write_string(w, &self.reason, MAX_DISCONNECT_CHARS)
    }

    fn read_body<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            reason: read_string(r, MAX_DISCONNECT_CHARS)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{decode, encode};

    #[test]
    fn login_start_wire_format() {
        let p = LoginStart {
            name: "Notch".into(),
            uuid: 0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10,
        };
        let frame = encode(&p).unwrap();
        let mut expected = vec![0x17, 0x00, 0x05];
        expected.extend_from_slice(b"Notch");
        expected.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(frame, expected);
        assert_eq!(decode::<LoginStart>(&frame[1..]).unwrap(), p);
    }

    #[test]
    fn login_success_roundtrip_with_properties() {
        let p = LoginSuccess {
            uuid: 42,
            username: "Fulmen".into(),
            properties: vec![
                Property {
                    name: "textures".into(),
                    value: "abc".into(),
                    signature: Some("sig".into()),
                },
                Property {
                    name: "x".into(),
                    value: "y".into(),
                    signature: None,
                },
            ],
            session_id: 99,
        };
        let frame = encode(&p).unwrap();
        assert_eq!(decode::<LoginSuccess>(&frame[1..]).unwrap(), p);
    }

    #[test]
    fn set_compression_negative_roundtrip() {
        let p = SetCompression { threshold: -1 };
        let frame = encode(&p).unwrap();
        assert_eq!(decode::<SetCompression>(&frame[1..]).unwrap(), p);
    }

    #[test]
    fn login_acknowledged_is_two_bytes() {
        assert_eq!(encode(&LoginAcknowledged).unwrap(), [0x01, 0x03]);
    }

    /// Body of a Login Success as sent by Pumpkin (protocol 777) for the player "Fulmen".
    #[test]
    fn decodes_login_success_captured_from_pumpkin() {
        let body: [u8; 40] = [
            0x0a, 0x07, 0x2f, 0x01, 0x6b, 0xdf, 0x48, 0x7c, 0x43, 0x82, 0xf9, 0xf4, 0x0a, 0xe4,
            0x01, 0x3a, 0x06, 0x46, 0x75, 0x6c, 0x6d, 0x65, 0x6e, 0x00, 0x87, 0xc6, 0x1a, 0x85,
            0xa7, 0xc6, 0x4e, 0xad, 0xb5, 0xf3, 0x3d, 0xfb, 0x49, 0xae, 0xca, 0x91,
        ];
        let p = LoginSuccess::read_body(&mut &body[..]).unwrap();
        assert_eq!(p.uuid, 0x0a072f01_6bdf_487c_4382_f9f40ae4013a);
        assert_eq!(p.username, "Fulmen");
        assert!(p.properties.is_empty());
        assert_eq!(p.session_id, 0x87c61a85_a7c6_4ead_b5f3_3dfb49aeca91);
        // And the encoder reproduces exactly the same bytes.
        let mut out = Vec::new();
        p.write_body(&mut out).unwrap();
        assert_eq!(out, body);
    }
}
