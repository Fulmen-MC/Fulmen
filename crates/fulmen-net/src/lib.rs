//! Connection layer for Fulmen.
//!
//! Currently this only contains a blocking server list ping ([`status::ping`]). Login,
//! encryption and compression will be added on top of `fulmen-protocol`.

#![forbid(unsafe_code)]

pub mod error;
pub mod status;

pub use error::{Error, Result};
