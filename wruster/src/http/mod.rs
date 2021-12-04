use std::io;
use std::io::{prelude::*, Cursor};

use std::convert::Infallible;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::{self};

use std::str::FromStr;

pub mod errors;
pub mod headers;
mod status;
pub use self::status::StatusCode;

use crate::errors::ParseError;
use crate::errors::ParseError::{ConnectionClosed, Unknow};

use headers::*;

#[cfg(test)]
mod tests;

pub type ServerResult = Result<(), Box<dyn Error>>;

#[derive(Debug)]
pub struct Request<'a> {
    pub method: HttpMethod,
    pub uri: String,
    pub version: String,
    pub headers: HttpHeaders,
    pub body: Option<Body<'a>>,
}

impl<'a> Request<'a> {
    pub fn read_from<T: io::Read + 'a>(from: T) -> Result<Request<'a>, ParseError> {
        debug!("parsing request");
        let mut reader = io::BufReader::new(from);
        let request_line = match HttpRequestLine::read_from(&mut reader) {
            Ok(request) => request,
            Err(err) => return Err(err),
        };
        debug!("request line parsed: {:?}", request_line);
        let headers = HttpHeaders::read_from(&mut reader)?;
        debug!("headers parsed: {:?}", headers);

        let body = Body::read_from(reader, &headers)?;
        debug!("body read, length: {:?}", body);

        let request = Request {
            method: request_line.method,
            uri: request_line.uri,
            version: request_line.version,
            headers,
            body,
        };
        debug!("request parsed: {:?}", request);
        Ok(request)
    }

    pub fn read_from_str(from: &str) -> Result<Request<'_>, ParseError> {
        Request::read_from(Cursor::new(from))
    }
}

#[derive(Debug)]
struct HttpRequestLine {
    method: HttpMethod,
    uri: String,
    version: String,
}

impl HttpRequestLine {
    fn read_from<T: io::Read>(from: &mut io::BufReader<T>) -> Result<HttpRequestLine, ParseError> {
        // Request-Line   = Method SP Request-URI SP HTTP-Version CRLF
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec5.html

        let mut method = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut method) {
            return Err(Unknow(err.to_string()));
        };
        if method.is_empty() {
            return Err(ConnectionClosed);
        }
        if method.len() < 2 {
            let msg = format!("invalid request line {:?}", method);
            return Err(Unknow(msg));
        };
        let method = String::from_utf8_lossy(&method[..method.len() - 1]);
        let method = match HttpMethod::from_str(&method) {
            Err(err) => return Err(Unknow(err)),
            Ok(method) => method,
        };

        let mut uri = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut uri) {
            return Err(Unknow(err.to_string()));
        };
        if uri.len() < 2 {
            return Err(Unknow(String::from("invalid request line")));
        };
        let uri = String::from_utf8_lossy(&uri[..uri.len() - 1]);

        let mut version = Vec::new();
        if let Err(err) = from.read_until(b'\n', &mut version) {
            return Err(Unknow(err.to_string()));
        };
        if version.len() < 3 {
            return Err(Unknow(String::from("invalid request line")));
        };

        if version[version.len() - 2] != (b'\r') {
            return Err(Unknow(String::from("invalid request line")));
        }
        let version = String::from_utf8_lossy(&version[..version.len() - 2]);

        Ok(HttpRequestLine {
            method,
            uri: String::from(uri),
            version: String::from(version),
        })
    }
}

pub struct Body<'a> {
    pub content_type: mime::Mime,
    pub content_length: u64,
    pub content: Box<dyn Read + 'a>,
}

impl<'a> Body<'a> {
    pub fn write<T: io::Write>(&mut self, to: &mut T) -> ServerResult {
        let src = &mut self.content;
        if let Err(err) = io::copy(src, to) {
            return Err(Box::new(err));
        };
        Ok(())
    }

    pub fn read_from<T: io::Read + 'a>(
        from: T,
        headers: &HttpHeaders,
    ) -> Result<Option<Body<'a>>, ParseError> {
        if let Some(encoding) = headers.get("Transfer-Enconding") {
            // Transfer-Enconding entity is not supported.
            if encoding.len() != 1 {
                let msg = "invalid Transfer-Enconding header".to_string();
                return Err(Unknow(msg));
            }
            if encoding[0] != "identity" {
                let msg = format!("Transfer-Encoding: {} is not supported", encoding[0]);
                return Err(Unknow(msg));
            }
        };

        let len = match headers.get("Content-Length") {
            None => return Ok(None),
            Some(lengths) => {
                if lengths.len() != 1 {
                    let msg = String::from("invalid Content-Length header");
                    return Err(Unknow(msg));
                }
                &lengths[0]
            }
        };

        let len = match usize::from_str(len) {
            Err(err) => {
                let msg = format!("invalid Content-Length header, {}", err.to_string());
                return Err(Unknow(msg));
            }
            Ok(size) => size,
        };
        let c = from.take(len as u64);
        let body = Body {
            content: Box::new(c),
            content_length: len as u64,
            content_type: mime::TEXT_PLAIN,
        };
        Ok(Some(body))
    }

    pub fn read_from_len(
        from: impl Read + 'a,
        mtype: mime::Mime,
        len: u64,
    ) -> Result<Option<Body<'a>>, ParseError> {
        let content = Box::new(from.take(len));
        let body = Body {
            content,
            content_length: len,
            content_type: mtype,
        };
        Ok(Some(body))
    }
}

impl fmt::Debug for Body<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "content-type: {}, content-length: {}, content: ....",
            self.content_type, self.content_length
        )
    }
}

#[derive(Debug)]
pub struct Response<'a> {
    pub status: StatusCode,
    pub headers: HttpHeaders,
    pub body: Option<Body<'a>>,
}

impl<'a> Response<'a> {
    pub fn write<T: io::Write>(&mut self, to: &mut T) -> ServerResult {
        let payload = format!("HTTP/1.1 {:#}\r\n", self.status);
        if let Err(err) = to.write(payload.as_bytes()) {
            return Err(Box::new(err));
        };
        if self.body.is_none() {
            self.headers.add_header(HttpHeader {
                name: String::from("Content-Length"),
                value: String::from("0"),
            })
        }
        self.headers.write(to)?;
        if self.body.is_none() {
            return Ok(());
        }
        // TODO: handle possible error.
        let body = self.body.as_mut().unwrap();
        body.write(to)
    }

    pub fn from_status(status: StatusCode) -> Response<'a> {
        let headers = HttpHeaders::new();
        Response {
            status,
            headers,
            body: None,
        }
    }

    pub fn read_from<T: io::Read + 'a>(from: T) -> Result<Response<'a>, ParseError> {
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec6.html
        //    Status-Line
        //                    *(( general-header
        //                     | response-header
        //                     | entity-header ) CRLF)
        //                    CRLF
        //                    [ message-body ]
        debug!("parsing response");
        let mut reader = io::BufReader::new(from);
        let status_line = StatusLine::read_from(&mut reader)?;
        debug!("response status line parsed: {:?}", status_line);

        let headers = HttpHeaders::read_from(&mut reader)?;
        debug!("headers parsed: {:?}", headers);

        let body = Body::read_from(reader, &headers)?;
        debug!("body read, length: {:?}", body);

        let response = Response {
            body,
            status: status_line.status_code,
            headers,
        };
        debug!("response parsed: {:?}", response);
        Ok(response)
    }
}

impl<'a> FromStr for Response<'a> {
    type Err = Infallible;
    fn from_str(content: &str) -> Result<Response<'a>, Infallible> {
        let content = Vec::from(content);
        let resp = Response {
            status: StatusCode::OK,
            headers: HttpHeaders::new(),
            body: Some(Body {
                content_length: content.len() as u64,
                content_type: mime::TEXT_PLAIN,
                content: Box::new(Cursor::new(content)),
            }),
        };
        Ok(resp)
    }
}

#[derive(Debug)]
struct StatusLine {
    http_version: String,
    status_code: StatusCode,
    reason_phrase: String,
}

impl StatusLine {
    fn read_from<T: io::Read>(from: &mut io::BufReader<T>) -> Result<StatusLine, ParseError> {
        // Status-Line = HTTP-Version SP Status-Code SP Reason-Phrase CRLF
        let mut http_version = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut http_version) {
            return Err(Unknow(err.to_string()));
        };

        if http_version.is_empty() {
            return Err(ConnectionClosed);
        }

        let http_version = String::from_utf8_lossy(&http_version).to_string();
        Self::validate_version(&http_version)?;
        let mut status_code = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut status_code) {
            return Err(Unknow(err.to_string()));
        };
        let status_code = String::from_utf8_lossy(&status_code).to_string();
        if status_code.len() != 4 {
            return Err(Unknow(format!("invalid status code: {}", status_code)));
        };
        let status_code = match status_code.parse::<usize>() {
            Err(error) => return Err(Unknow(error.to_string())),
            Ok(code) => code,
        };
        let status_code = StatusCode::from(status_code);
        let mut reason_phrase = Vec::new();
        if let Err(err) = from.read_until(b'\n', &mut reason_phrase) {
            return Err(Unknow(err.to_string()));
        };
        if reason_phrase.len() < 3 {
            return Err(Unknow(String::from("invalid reason phrase")));
        };
        let reason_phrase =
            String::from_utf8_lossy(&reason_phrase[..reason_phrase.len() - 2]).to_string();
        Ok(StatusLine {
            http_version,
            status_code,
            reason_phrase,
        })
    }

    fn validate_version(version: &String) -> Result<(), ParseError> {
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec3.html
        // HTTP-Version   = "HTTP" "/" 1*DIGIT "." 1*DIGIT
        let parts: Vec<&str> = version.split("/").collect();
        if parts.len() != 2 {
            return Err(Unknow(format!("invalid http version: {}", version)));
        };
        if parts[0] != "HTTP" {
            return Err(Unknow(format!("invalid http version: {}", version)));
        };

        let digits_parts: Vec<&str> = parts[1].split(".").collect();
        if digits_parts.len() != 2 {
            return Err(Unknow(format!("invalid http version: {}", version)));
        }

        if let Err(error) = digits_parts[0].parse::<u8>() {
            return Err(Unknow(format!(
                "invalid http version: {} {}",
                version, error
            )));
        }

        if let Err(error) = digits_parts[1].parse::<u8>() {
            return Err(Unknow(format!(
                "invalid http version: {} {}",
                version, error
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum HttpMethod {
    GET = 0,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

impl HttpMethod {
    pub fn get_last() -> HttpMethod {
        Self::PATCH
    }
}

impl PartialEq for HttpMethod {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl Eq for HttpMethod {}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(HttpMethod::GET),
            "HEAD" => Ok(HttpMethod::HEAD),
            "POST" => Ok(HttpMethod::POST),
            "PUT" => Ok(HttpMethod::PUT),
            "DELETE" => Ok(HttpMethod::DELETE),
            "CONNECT" => Ok(HttpMethod::CONNECT),
            "OPTIONS" => Ok(HttpMethod::OPTIONS),
            "TRACE" => Ok(HttpMethod::TRACE),
            "PATCH" => Ok(HttpMethod::PATCH),
            _ => Err(String::from("invalid http method")),
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::CONNECT => write!(f, "CONNECT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::HEAD => write!(f, "HEAD"),
            HttpMethod::OPTIONS => write!(f, "OPTIONS"),
            HttpMethod::PATCH => write!(f, "PATCH"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::TRACE => write!(f, "TRACE"),
        }
    }
}

trait HttpMessageChar {
    fn is_valid_token_char(&self) -> bool;

    fn is_valid_field_content(&self) -> bool;

    fn is_valid_vchar(&self) -> bool;

    fn is_optional_white_space(&self) -> bool;
}

impl HttpMessageChar for char {
    fn is_valid_token_char(&self) -> bool {
        // We don't support non ascii chars in tokens.
        if !self.is_ascii() {
            return false;
        }
        if self.is_alphanumeric() {
            return true;
        };
        let valid_token_symbols = [
            '!', '#', '$', '%', '&', '\'', '*', '+', '-', '.', '^', '_', '`', '|', '~',
        ];
        if valid_token_symbols.contains(&self) {
            return true;
        };
        false
    }

    fn is_valid_vchar(&self) -> bool {
        // field-vchar    = VCHAR / obs-text
        if self.is_ascii_graphic() {
            return true;
        };
        if *self as u8 >= 0x80 {
            return true;
        };
        false
    }

    fn is_valid_field_content(&self) -> bool {
        self.is_valid_vchar() || self.is_optional_white_space()
    }

    fn is_optional_white_space(&self) -> bool {
        *self == ' ' || *self == '\t'
    }
}
