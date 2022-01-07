use std::io;
use std::io::{prelude::*, Cursor};

use std::convert::Infallible;
use std::fmt;
use std::fmt::Debug;
use std::str::FromStr;

/// Contains the definition of the errors used in the Http module.
pub mod errors;
/// Contains all the types needed to read and write Http headers.
pub mod headers;
/// Contains the definition of all the standard Http status code.
pub mod status;
pub use self::status::StatusCode;

use crate::errors::HttpError;
use crate::errors::HttpError::{ConnectionClosed, Timeout, Unknown};

use headers::*;

#[cfg(test)]
mod tests;

/// Defines the returned by the methods and functions of this module.
pub type HttpResult<T> = Result<T, HttpError>;

/// Represents a Http Request.
#[derive(Debug)]
pub struct Request<'a> {
    /// The [``HttpMethod``] of the request.
    pub method: HttpMethod,
    /// The uri of the request.
    pub uri: String,
    /// The version of the request.
    pub version: String,
    /// The headers of the request.
    pub headers: Headers,
    /// The body of the request, if any.
    pub body: Option<Body<'a>>,
}

impl<'a> Request<'a> {
    /**
    Reads a request from an HTTP message in a type implementing [`io::Read`] according to
    the spec: https://datatracker.ietf.org/doc/html/rfc7230.

    # Examples.

    TODO

    # Errors

    Returns a [``HttpError``] if there is any problem reading from ``from`` or the message
    does not conform to the spec: https://datatracker.ietf.org/doc/html/rfc7230.
    */
    pub fn read_from<T: io::Read + 'a>(from: T) -> HttpResult<Request<'a>> {
        debug!("parsing request");
        let mut reader = io::BufReader::new(from);
        let request_line = match HttpRequestLine::read_from(&mut reader) {
            Ok(request) => request,
            Err(err) => return Err(err),
        };
        debug!("request line parsed: {:?}", request_line);
        let headers = Headers::read_from(&mut reader)?;
        debug!("headers parsed: {:?}", headers);

        let body = Body::read_from(reader, &headers)?;
        debug!("body read: {:?}", body);

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

    /**
    Reads a request from a string.

    # Examples

    ```
    use wruster::http::Request;

    let str_req = "GET / HTTP/1.1\r\n\r\n";
    let req = Request::read_from_str(str_req).unwrap();
    ```

    # Errors
    Returns a [``HttpError``] if there is any problem reading from ``from`` or the message
    does not conform to the spec: https://datatracker.ietf.org/doc/html/rfc7230.
    */
    pub fn read_from_str(from: &str) -> Result<Request<'_>, HttpError> {
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
    fn read_from<T: io::Read>(from: &mut io::BufReader<T>) -> Result<HttpRequestLine, HttpError> {
        // Request-Line   = Method SP Request-URI SP HTTP-Version CRLF
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec5.html

        let mut method = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut method) {
            let err = match err.kind() {
                io::ErrorKind::WouldBlock => Err(Timeout),
                _ => Err(Unknown(err.to_string())),
            };
            return err;
        };
        if method.is_empty() {
            return Err(ConnectionClosed);
        }
        if method.len() < 2 {
            let msg = format!("invalid request line {:?}", method);
            return Err(Unknown(msg));
        };
        let method = String::from_utf8_lossy(&method[..method.len() - 1]);
        let method = match HttpMethod::from_str(&method) {
            Err(err) => return Err(Unknown(err)),
            Ok(method) => method,
        };

        let mut uri = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut uri) {
            return Err(Unknown(err.to_string()));
        };
        if uri.len() < 2 {
            return Err(Unknown(String::from("invalid request line")));
        };

        let uri = String::from_utf8_lossy(&uri[..uri.len() - 1]);

        let mut version = Vec::new();
        if let Err(err) = from.read_until(b'\n', &mut version) {
            return Err(Unknown(err.to_string()));
        };
        if version.len() < 3 {
            return Err(Unknown(String::from("invalid request line")));
        };

        if version[version.len() - 2] != (b'\r') {
            return Err(Unknown(String::from("invalid request line")));
        }
        let version = String::from_utf8_lossy(&version[..version.len() - 2]);

        Ok(HttpRequestLine {
            method,
            uri: String::from(uri),
            version: String::from(version),
        })
    }
}

/// Body holds the body part of an Http Message.
pub struct Body<'a> {
    /// The content type of body.
    pub content_type: Option<mime::Mime>,
    /// The length, in bytes, of the body.
    pub content_length: u64,
    /// The content of the body, if any.
    pub content: Box<dyn Read + 'a>,
}

impl<'a> Body<'a> {
    /**
    Writes the content of body to a type implementing the [``io::Write``] trait.

    # Examples

    ```
    use std::io::Cursor;
    use wruster::http::Body;

    let content = "content";
    let mut body = Body {
        content: Box::new(Cursor::new(content)),
        content_type: Some(mime::TEXT_PLAIN),
        content_length: content.len() as u64,
    };
    let mut to: Vec<u8> = Vec::new();
    body.write(&mut to).unwrap();
    let got_content = String::from_utf8(to).unwrap();
    assert_eq!(content, &got_content)
    ```

    # Errors

    This function will return an error if there is any error writing
    to the ``to`` paramerer.
    */
    pub fn write<T: io::Write>(&mut self, to: &mut T) ->  HttpResult<()> {
        let src = &mut self.content;
        if let Err(err) = io::copy(src, to) {
            return Err(HttpError::Unknown(err.to_string()));
        };
        Ok(())
    }

    /**

    Reads the body of a Http message given the Headers of the message and
    a type implementing the [`io::Read`] trait that contains content of the
    body. The method assumes that the content and the headers follow the spec
    https://datatracker.ietf.org/doc/html/rfc7230#page-27. By now, the method only
    supports the ``Content-Length`` header and not ``Transfer-Encoding`` header.

    # Examples

    TODO

    # Errors

    This function will return an error if the ``Headers`` parameter contains a
    ``Transfer-Encoding`` header or if it contains more that one value a ``Content-Length``
    header.
    */
    pub fn read_from<T: io::Read + 'a>(
        from: T,
        headers: &Headers,
    ) -> Result<Option<Body<'a>>, HttpError> {
        if let Some(encoding) = headers.get("Transfer-Enconding") {
            // Transfer-Encoding entity is not supported.
            if encoding.len() != 1 {
                let msg = "invalid Transfer-Enconding header".to_string();
                return Err(Unknown(msg));
            }
            if encoding[0] != "identity" {
                let msg = format!("Transfer-Encoding: {} is not supported", encoding[0]);
                return Err(Unknown(msg));
            }
        };

        let len = match headers.get("Content-Length") {
            None => return Ok(None),
            Some(lengths) => {
                if lengths.len() != 1 {
                    let msg = String::from("invalid Content-Length header");
                    return Err(Unknown(msg));
                }
                &lengths[0]
            }
        };

        let len = match usize::from_str(len) {
            Err(err) => {
                let msg = format!("invalid Content-Length header, {}", err.to_string());
                return Err(Unknown(msg));
            }
            Ok(size) => size,
        };
        if len == 0 {
            return Ok(None);
        }
        let content_type = match headers.get("Content-Type") {
            None => None,
            Some(types) => {
                if types.is_empty() {
                    let msg = format!("invalid Content-Type header, {:?}", types);
                    return Err(Unknown(msg));
                };
                let mtype: mime::Mime = match types[0].parse() {
                    Ok(t) => t,
                    Err(err) => {
                        let msg = format!(
                            "invalid Content-Type header, {:?}, {}",
                            types,
                            err.to_string()
                        );
                        return Err(Unknown(msg));
                    }
                };
                Some(mtype)
            }
        };
        let c = from.take(len as u64);
        let body = Body {
            content: Box::new(c),
            content_type,
            content_length: len as u64,
        };
        Ok(Some(body))
    }
}

impl fmt::Debug for Body<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "content-type: {:?}, content-length: {}, content: ....",
            self.content_type, self.content_length
        )
    }
}

/// Represents a Http Response.
#[derive(Debug)]
pub struct Response<'a> {
    /// The http [``StatusCode``] of the response.
    pub status: StatusCode,
    /// The [``Headers``] of the response.
    pub headers: Headers,
    /// The body, if any, of the response.
    pub body: Option<Body<'a>>,
}

impl<'a> Response<'a> {
    /**
    Writes a [``Response``] to a type implementing the [``io::Write``] trait.

    # Examples

    ```
    use std::io::Cursor;
    use wruster::http::headers::{Header, Headers};
    use wruster::http::{Body, Response, StatusCode};

       let content = "#wruster";
    let body = Body {
    content: Box::new(Cursor::new(content)),
    content_type: Some(mime::TEXT_PLAIN),
    content_length: content.len() as u64,
    };

    let mut headers = Headers::new();
    headers.add(Header {
    name: String::from("Content-Length"),
    value: String::from("8"),
    });
    let mut response = Response {
    status: StatusCode::OK,
    headers: headers,
    body: Some(body),
    };

    let mut to: Vec<u8> = Vec::new();
    response.write(&mut to).unwrap();
    ```

    # Errors

    This function will return an error if there is any error writing
    to the ``to`` paramerer.
    */
    pub fn write<T: io::Write>(&mut self, to: &mut T) -> HttpResult<()> {
        let payload = format!("HTTP/1.1 {:#}\r\n", self.status);
        if let Err(err) = to.write(payload.as_bytes()) {
           return Err(HttpError::Unknown(err.to_string()));
        };
        if self.body.is_none() {
            self.headers.add(Header {
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


    /// Creates a Request with the given http [``StatusCode``].
    ///
    /// # Examples
    ///
    /// ```
    /// use wruster::http::Response;
    /// use wruster::http::status::StatusCode;
    /// let response = Response::from_status(StatusCode::OK);
    /// ```
    pub fn from_status(status: StatusCode) -> Response<'a> {
        let headers = Headers::new();
        Response {
            status,
            headers,
            body: None,
        }
    }

    /**
    Reads a response from an HTTP message in a type implementing [`io::Read`] according to
    the spec: https://datatracker.ietf.org/doc/html/rfc7230.

    # Examples.

    TODO

    # Errors

    Returns a [``HttpError``] if there is any problem reading from ``from`` or the message
    does not conform to the spec: https://datatracker.ietf.org/doc/html/rfc7230.
    */
    pub fn read_from<T: io::Read + 'a>(from: T) -> Result<Response<'a>, HttpError> {
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

        let headers = Headers::read_from(&mut reader)?;
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
            headers: Headers::new(),
            body: Some(Body {
                content_length: content.len() as u64,
                content_type: Some(mime::TEXT_PLAIN),
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
    fn read_from<T: io::Read>(from: &mut io::BufReader<T>) -> Result<StatusLine, HttpError> {
        // Status-Line = HTTP-Version SP Status-Code SP Reason-Phrase CRLF
        let mut http_version = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut http_version) {
            let err = match err.kind() {
                io::ErrorKind::WouldBlock => Err(Timeout),
                _ => Err(Unknown(err.to_string())),
            };
            return err;
        };

        if http_version.is_empty() {
            return Err(ConnectionClosed);
        }

        let mut http_version = String::from_utf8_lossy(&http_version).to_string();
        http_version = http_version.trim_end().to_string();
        Self::validate_version(&http_version)?;
        let mut status_code = Vec::new();
        if let Err(err) = from.read_until(b' ', &mut status_code) {
            return Err(Unknown(err.to_string()));
        };
        let mut status_code = String::from_utf8_lossy(&status_code).to_string();
        if status_code.len() != 4 {
            return Err(Unknown(format!("invalid status code: {}", status_code)));
        };
        status_code = status_code.trim_end().to_string();
        let status_code = match status_code.parse::<usize>() {
            Err(error) => return Err(Unknown(error.to_string())),
            Ok(code) => code,
        };
        let status_code = StatusCode::from(status_code);
        let mut reason_phrase = Vec::new();
        if let Err(err) = from.read_until(b'\n', &mut reason_phrase) {
            return Err(Unknown(err.to_string()));
        };
        if reason_phrase.len() < 3 {
            return Err(Unknown(String::from("invalid reason phrase")));
        };
        let reason_phrase =
            String::from_utf8_lossy(&reason_phrase[..reason_phrase.len() - 2]).to_string();
        Ok(StatusLine {
            http_version,
            status_code,
            reason_phrase,
        })
    }

    fn validate_version(version: &str) -> Result<(), HttpError> {
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec3.html
        // HTTP-Version   = "HTTP" "/" 1*DIGIT "." 1*DIGIT
        let parts: Vec<&str> = version.split('/').collect();
        if parts.len() != 2 {
            return Err(Unknown(format!("invalid http version: {}", version)));
        };
        if parts[0] != "HTTP" {
            return Err(Unknown(format!("invalid http version: {}", version)));
        };

        let digits_parts: Vec<&str> = parts[1].split('.').collect();
        if digits_parts.len() != 2 {
            return Err(Unknown(format!("invalid http version: {}", version)));
        }

        if let Err(error) = digits_parts[0].parse::<u8>() {
            return Err(Unknown(format!(
                "invalid http version: {} {}",
                version, error
            )));
        }

        if let Err(error) = digits_parts[1].parse::<u8>() {
            return Err(Unknown(format!(
                "invalid http version: {} {}",
                version, error
            )));
        }
        Ok(())
    }
}
#[allow(missing_docs)]
/// Contains a variant per each Http Method.
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
    /// The [``HttpMethod``] variants are represented using a [``u16``], this
    /// methods returns the variant with the highest value.
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

trait MessageChar {
    fn is_valid_token_char(&self) -> bool;

    fn is_valid_field_content(&self) -> bool;

    fn is_valid_vchar(&self) -> bool;

    fn is_optional_white_space(&self) -> bool;
}

impl MessageChar for char {
    fn is_valid_token_char(&self) -> bool {
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
