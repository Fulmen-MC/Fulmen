//! Wire-level primitives and packets of the Minecraft: Java Edition protocol.
//!
//! This crate has no dependencies and does no I/O of its own: everything works on
//! [`std::io::Read`] / [`std::io::Write`] or plain byte slices, so it can be used from
//! blocking code, async code (via buffers) and tests alike.
//!
//! Only the uncompressed, unencrypted framing is implemented here. Compression and
//! encryption belong to the connection layer (`fulmen-net`).

#![forbid(unsafe_code)]

pub mod codec;
pub mod error;
pub mod handshake;
pub mod packet;
pub mod status;
pub mod varint;

pub use error::{Error, Result};

/// Protocol version sent in the handshake of a status request.
///
/// The protocol version is required by the Minecraft protocol. Servers may
/// ignore it, but some implementations use or reflect the value in the
/// returned status response.
pub const PROTOCOL_VERSION: i32 = -1;
