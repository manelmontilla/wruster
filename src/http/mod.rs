use std::collections::hash_map::HashMap;
use std::io;
use std::io::{prelude::*, BufReader, Cursor};

use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;

use std::str::FromStr;
use std::string::ParseError;

pub mod errors;
pub mod headers;

use errors::*;
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
    pub fn read_from<T: io::Read + 'a>(from: T) -> Result<Request<'a>, ParseRequestError> {
        debug!("pasing request");
        let mut reader = io::BufReader::new(from);
        let request_line = match HttpRequestLine::read_from(&mut reader) {
            Ok(request) => request,
            Err(err) => return Err(err),
        };
        debug!("request line parsed: {:?}", request_line);
        let headers = HttpHeaders::read_from(&mut reader)?;
        debug!("headers parsed: {:?}", headers);
        
        let body = Body::read_from(reader, &headers)?;

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

    fn from_str<'b>(from: &'b str) -> Result<Request<'b>, ParseRequestError> {
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
    fn read_from<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<HttpRequestLine, ParseRequestError> {
        // Request-Line   = Method SP Request-URI SP HTTP-Version CRLF
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec5.html

        // Parsing the request line this way is not fast, but the objective is
        // to make it clear not performant.
        let mut method = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut method) {
            return Err(ParseRequestError {
                msg: err.to_string(),
            });
        };
        if method.len() < 2 {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        };
        let method = String::from_utf8_lossy(&method[..method.len() - 1]);
        let method = match HttpMethod::from_str(&method) {
            Err(err) => return Err(ParseRequestError { msg: err }),
            Ok(method) => method,
        };

        let mut uri = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut uri) {
            return Err(ParseRequestError {
                msg: err.to_string(),
            });
        };
        if uri.len() < 2 {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        };
        let uri = String::from_utf8_lossy(&uri[..uri.len() - 1]);

        let mut version = Vec::new();
        if let Err(err) = from.read_until(b'\n', &mut version) {
            return Err(ParseRequestError {
                msg: err.to_string(),
            });
        };
        if version.len() < 3 {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        };

        if version[version.len() - 2] != (b'\r') {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
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
        let mut header = format!("Content-Type: {}\r\n", &self.content_type);
        if let Err(err) = to.write(header.as_bytes()) {
            return Err(Box::new(err));
        };
        header = format!("Conent-Length: {}\r\n\r\n", self.content_length);
        if let Err(err) = to.write(header.as_bytes()) {
            return Err(Box::new(err));
        };
        self.write_content(to)
    }

    pub fn write_content<T: io::Write>(&mut self, to: &mut T) -> ServerResult {
        let src = &mut self.content;
        if let Err(err) = io::copy(src, to) {
            return Err(Box::new(err));
        };
        Ok(())
    }

    pub fn read_from<T: io::Read + 'a>(
        from: T,
        headers: &HttpHeaders,
    ) -> Result<Option<Body<'a>>, ParseRequestError> {
        if let Some(encoding) = headers.get("Transfer-Enconding") {
            // Transfer-Enconding entity is not supported.
            if encoding.len() != 1 {
                let msg = format!("invalid Transfer-Enconding header");
                return Err(ParseRequestError { msg });
            }
            if encoding[0] != "identity" {
                let msg = format!("Transfer-Encoding: {} is not supported", encoding[0]);
                return Err(ParseRequestError { msg });
            }
        };

        let len = match headers.get("Content-Length") {
            None => return Ok(None),
            Some(lengths) => {
                if lengths.len() != 1 {
                    let msg = String::from("invalid Content-Length header");
                    return Err(ParseRequestError { msg });
                }
                &lengths[0]
            }
        };

        let len = match usize::from_str(len) {
            Err(err) => {
                let msg = format!("invalid Content-Length header, {}", err.to_string());
                return Err(ParseRequestError { msg });
            }
            Ok(size) => size,
        };
        let c = from.take(len as u64);
        let body = Body{
            content: Box::new(c),
            content_length: len as u64,
            content_type: mime::TEXT_PLAIN,
        };
        Ok(Some(
            body
        ))
    }

    pub fn read_from_len<T: io::Read>(
        from: &'a mut T,
        mtype: mime::Mime,
        len: u64,
    ) -> Result<Option<Body>, ParseRequestError> {
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
    pub headers: HashMap<String, String>,
    pub body: Option<Body<'a>>,
}

impl<'a> Response<'a> {
    pub fn add_header(&mut self, name: String, value: String) {
        self.headers.insert(name, value);
    }

    pub fn write<T: io::Write>(&mut self, to: &mut T) -> ServerResult {
        let payload = format!("HTTP/1.1 {:#}\r\n", self.status);
        if let Err(err) = to.write(payload.as_bytes()) {
            return Err(Box::new(err));
        };
        for (name, value) in &self.headers {
            let header = format!("{}: {}\r\n", name, value);
            if let Err(err) = to.write(header.as_bytes()) {
                return Err(Box::new(err));
            };
        }
        if self.body.is_none() {
            match to.write("\r\n".as_bytes()) {
                Ok(_) => return Ok(()),
                Err(err) => return Err(Box::new(err)),
            }
        };
        let body = self.body.as_mut().unwrap();
        body.write(to)
    }

    pub fn from_status(status: StatusCode) -> Response<'a> {
        Response {
            status,
            headers: HashMap::new(),
            body: None,
        }
    }
}

impl<'a> FromStr for Response<'a> {
    type Err = ParseError;
    fn from_str(content: &str) -> Result<Response<'a>, Infallible> {
        let content = Vec::from(content);
        let resp = Response {
            status: StatusCode::Ok,
            headers: HashMap::new(),
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
pub enum StatusCode {
    Ok,
    InternalServerError,
    NotFound,
    BadRequest,
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StatusCode::Ok => write!(f, "200 OK"),
            StatusCode::InternalServerError => write!(f, "500 Internal Server Error"),
            StatusCode::NotFound => write!(f, "404 Not found"),
            &StatusCode::BadRequest => write!(f, "400 Bad Request"),
        }
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
        self == other
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
