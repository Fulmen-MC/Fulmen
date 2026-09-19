//! Blocking server list ping.

use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use fulmen_protocol::handshake::{Handshake, NextState};
use fulmen_protocol::packet::{decode, encode, read_frame};
use fulmen_protocol::status::{PingRequest, PongResponse, StatusRequest, StatusResponse};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const STATUS_PROTOCOL_VERSION: i32 = fulmen_protocol::PROTOCOL_VERSION;

/// Result of a server list ping.
#[derive(Debug, Clone)]
pub struct StatusResult {
    pub status: ServerStatus,
    pub latency: Duration,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct ServerStatus {
    pub version: Version,
    pub players: Players,
    pub description: serde_json::Value,
    pub favicon: Option<String>,
    #[serde(rename = "enforceSecureChat")]
    pub enforce_secure_chat: Option<bool>,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Version {
    pub name: String,
    pub protocol: i32,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Players {
    pub max: u32,
    pub online: u32,
    pub sample: Vec<serde_json::Value>,
}

/// Pings `host:port` and returns the status JSON and the round-trip latency.
///
/// `timeout` applies to connecting and to every single read and write.
pub fn ping(host: &str, port: u16, timeout: Duration) -> Result<StatusResult> {
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or(Error::NoAddress)?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.set_nodelay(true)?;

    let handshake = Handshake {
        protocol_version: STATUS_PROTOCOL_VERSION,
        server_address: host.to_owned(),
        server_port: port,
        next_state: NextState::Status,
    };
    stream.write_all(&encode(&handshake)?)?;
    stream.write_all(&encode(&StatusRequest)?)?;

    let response: StatusResponse = decode(&read_frame(&mut stream)?)?;

    let payload = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()),
    )
    .unwrap_or(0);
    let started = Instant::now();
    stream.write_all(&encode(&PingRequest { payload })?)?;
    let pong: PongResponse = decode(&read_frame(&mut stream)?)?;
    let latency = started.elapsed();

    if pong.payload != payload {
        return Err(Error::PongMismatch {
            sent: payload,
            received: pong.payload,
        });
    }

    let status: ServerStatus = serde_json::from_str(&response.json)?;

    let status_result = StatusResult { status, latency };

    Ok(status_result)
}
