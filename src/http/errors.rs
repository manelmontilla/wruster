use std::error::Error;
use std::fmt;
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub struct ParseRequestError {
    pub msg: String,
}

impl fmt::Display for ParseRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for ParseRequestError {}
