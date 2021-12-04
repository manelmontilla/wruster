use std::error::Error;
use std::fmt;
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Unknow(String),
    ConnectionClosed,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknow(msg) => write!(f, "{}", msg),
            Self::ConnectionClosed => write!(f, "Connection Closed"),
        }
    }
}

impl Error for ParseError {}
