use std::{fmt, io};

/// Errors of the connection layer.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Io(io::Error),
    Protocol(fulmen_protocol::Error),
    /// The host name did not resolve to any address.
    NoAddress,
    /// The pong payload did not match the ping payload.
    PongMismatch {
        sent: i64,
        received: i64,
    },
    /// The server ended the connection with a reason (JSON text in the login state).
    Disconnected(String),
    /// The server wants encryption, which needs online mode (not implemented yet).
    EncryptionRequired,
    /// The server sent something we cannot handle yet.
    Unsupported(&'static str),
    /// A decompressed packet was larger than allowed or its length field did not match.
    BadCompression,
    /// The connection was closed and no more packets can be sent or received.
    Closed,
    /// The status response was not valid JSON of the expected shape.
    Json(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::NoAddress => write!(f, "host did not resolve to any address"),
            Self::PongMismatch { sent, received } => {
                write!(
                    f,
                    "pong payload {received} does not match ping payload {sent}"
                )
            }
            Self::Disconnected(reason) => write!(f, "disconnected by server: {reason}"),
            Self::EncryptionRequired => {
                write!(
                    f,
                    "server requires encryption (online mode is not implemented yet)"
                )
            }
            Self::Unsupported(what) => write!(f, "not supported yet: {what}"),
            Self::BadCompression => write!(f, "invalid compressed packet"),
            Self::Closed => write!(f, "connection closed"),
            Self::Json(e) => write!(f, "invalid status JSON: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Protocol(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<fulmen_protocol::Error> for Error {
    fn from(e: fulmen_protocol::Error) -> Self {
        Self::Protocol(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
