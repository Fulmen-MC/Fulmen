//! Connection layer for Fulmen.
//!
//! Contains a blocking server list ping ([`status::ping`]) and a [`Connection`] that logs in
//! (offline mode), completes the configuration phase and then runs on reader and writer
//! threads. Encryption (online mode) will be added on top of `fulmen-protocol`.

#![forbid(unsafe_code)]

pub mod connection;
pub mod error;
pub mod framing;
pub mod offline;
pub mod status;

pub use connection::{ConnectOptions, Connection};
pub use error::{Error, Result};
