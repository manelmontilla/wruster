use std::error::Error;
use std::fmt;
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
/// Defines the possible errors generated when reading and parsing a Request or a Response.
pub enum HttpError {
    /// It's generated when any not controlled error is encountered when
    /// parsing a Request or a Response, more info about the error is stores
    /// in the inner String of the variant.
    Unknown(String),
    /// It is generated when the connection is closed while waiting to star
    /// reading a request.
    ConnectionClosed,
    /// It is generated when the maximun allowed time to read a request, or
    /// write a response has been exceed.
    Timeout,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(msg) => write!(f, "{}", msg),
            Self::ConnectionClosed => write!(f, "Connection Closed"),
            Self::Timeout => write!(f, "operation timeout"),
        }
    }
}

impl Error for HttpError {}
