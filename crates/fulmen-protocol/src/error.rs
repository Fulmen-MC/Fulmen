use std::{fmt, io};

/// Errors produced while reading or writing protocol data.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The underlying reader or writer failed (including unexpected end of data).
    Io(io::Error),
    /// A VarInt or VarLong used more bytes than allowed.
    VarIntTooLong,
    /// A length prefix was negative.
    NegativeLength(i32),
    /// A packet frame exceeded the maximum allowed size.
    FrameTooLarge { len: usize, max: usize },
    /// A string exceeded its maximum length.
    StringTooLong { len: usize, max: usize },
    /// A string was not valid UTF-8.
    InvalidUtf8,
    /// The handshake requested a next state we do not know.
    InvalidNextState(i32),
    /// A frame carried a different packet id than expected.
    UnexpectedPacketId { expected: i32, got: i32 },
    /// A packet body had unread bytes left over.
    TrailingBytes(usize),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::VarIntTooLong => write!(f, "VarInt/VarLong is too long"),
            Self::NegativeLength(n) => write!(f, "negative length prefix: {n}"),
            Self::FrameTooLarge { len, max } => {
                write!(f, "packet frame too large: {len} bytes (max {max})")
            }
            Self::StringTooLong { len, max } => {
                write!(f, "string too long: {len} (max {max})")
            }
            Self::InvalidUtf8 => write!(f, "string is not valid UTF-8"),
            Self::InvalidNextState(n) => write!(f, "invalid handshake next state: {n}"),
            Self::UnexpectedPacketId { expected, got } => {
                write!(
                    f,
                    "unexpected packet id {got:#04x} (expected {expected:#04x})"
                )
            }
            Self::TrailingBytes(n) => write!(f, "{n} unread bytes left in packet body"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
