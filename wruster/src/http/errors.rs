use std::error::Error;
use std::fmt::Debug;
use std::sync::PoisonError;
use std::{fmt, io};

#[derive(Debug, Eq, PartialEq)]
/// Defines the possible errors generated when reading and parsing a Request or a Response.
pub enum HttpError {
    /// It's generated when any not controlled error is encountered when
    /// parsing a Request or a Response. Additional info about the error is
    /// stored in the inner String of the variant.
    Unknown(String),
    /// It's generated when the connection is closed while reading o writing
    /// reading a request.
    ConnectionClosed,
    /// It's generated when the maximum allowed time to read/write a request or
    /// response has been exceed.
    Timeout,
    /// It's generated when a syntactic error is found while reading a request.
    InvalidRequest(String),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(msg) => write!(f, "{}", msg),
            Self::ConnectionClosed => write!(f, "connection closed"),
            Self::Timeout => write!(f, "operation timeout"),
            HttpError::InvalidRequest(msg) => write!(f, "invalid request: {}", msg),
        }
    }
}

impl Error for HttpError {}

impl From<io::Error> for HttpError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::NotConnected => HttpError::ConnectionClosed,
            io::ErrorKind::TimedOut => HttpError::Timeout,
            _ => HttpError::Unknown(err.to_string()),
        }
    }
}

impl<T> From<PoisonError<T>> for HttpError {
    fn from(err: PoisonError<T>) -> Self {
        HttpError::Unknown(err.to_string())
    }
}
