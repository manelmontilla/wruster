use std::{fmt, str::FromStr};

#[derive(Debug)]
pub struct ParseVersionError {
    str: String,
}

impl ParseVersionError {
    pub fn new(parsed: &str) -> Self {
        let str = parsed.to_string();
        ParseVersionError { str }
    }
    pub fn source(&self) -> &str {
        &self.str
    }
}

impl fmt::Display for ParseVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid version string {}", self.source())
    }
}

/**
Represents the HTTP versions considered valid.
*/
#[derive(Debug, PartialEq, Eq)]
pub enum Version {
    /** HTTP version 1.0*/
    HTTP1_0,
    /** HTTP version 1.1*/
    HTTP1_1,
    /** HTTP version 2*/
    HTTP2,
}

impl FromStr for Version {
    type Err = ParseVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HTTP/1.0" => Ok(Version::HTTP1_0),
            "HTTP/1.1" => Ok(Version::HTTP1_1),
            "HTTP/2" => Ok(Version::HTTP2),
            _ => Err(ParseVersionError::new(s)),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Version::HTTP1_0 => write!(f, "HTTP/1.0"),
            Version::HTTP1_1 => write!(f, "HTTP/1.1"),
            Version::HTTP2 => write!(f, "HTTP/2"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_stores_string() {
        assert_eq!(Version::from_str("HTTP/1.0").unwrap(), Version::HTTP1_0);
        assert_eq!(Version::from_str("HTTP/2").unwrap(), Version::HTTP2);
        assert_eq!(Version::from_str("HTTP/1.1").unwrap(), Version::HTTP1_1);
        Version::from_str("error").expect_err("invalid version string error");
    }
}
