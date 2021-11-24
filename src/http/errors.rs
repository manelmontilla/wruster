use std::error::Error;
use std::fmt;
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub enum ParseRequestError {
    Unknow(String),
    EmptyRequest,
}

impl fmt::Display for ParseRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
           Self::Unknow(msg) => write!(f, "{}", msg),
           Self::EmptyRequest => write!(f, "empty request"),
        }
    }
}

impl Error for ParseRequestError {}
