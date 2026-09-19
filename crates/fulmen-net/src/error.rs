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
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Protocol(e) => Some(e),
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
