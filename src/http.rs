use std::collections::hash_map::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::str::FromStr;
use std::string::ParseError;

#[macro_use]
use log;

#[cfg(test)]
mod tests;

pub type ServerResult = Result<(), Box<dyn Error>>;
pub type ServerResultData<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Body {
    pub content_type: mime::Mime,
    pub content: Vec<u8>,
}

impl Body {
    fn write<T: io::Write>(&self, to: &mut T) -> ServerResult {
        let mut header = format!("Content-Type: {}\r\n", self.content_type);
        if let Err(err) = to.write(header.as_bytes()) {
            return Err(Box::new(err));
        };
        header = format!("Conent-Length: {}\r\n\r\n", self.content.len());
        if let Err(err) = to.write(header.as_bytes()) {
            return Err(Box::new(err));
        };
        if let Err(err) = to.write(&self.content) {
            return Err(Box::new(err));
        };
        Ok(())
    }
}

#[derive(Debug)]
pub struct Response {
    pub status: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Option<Body>,
}

impl Response {
    pub fn add_header(&mut self, name: String, value: String) {
        self.headers.insert(name, value);
    }

    pub fn write<T: io::Write>(&self, to: &mut T) -> ServerResult {
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
        if let None = self.body {
            match to.write("\r\n".as_bytes()) {
                Ok(_) => return Ok(()),
                Err(err) => return Err(Box::new(err)),
            }
        };
        let body = self.body.as_ref().unwrap();
        body.write(to)
    }

    pub fn from_status(status: StatusCode) -> Response {
        Response {
            status: status,
            headers: HashMap::new(),
            body: None,
        }
    }
}

impl FromStr for Response {
    type Err = ParseError;
    fn from_str(content: &str) -> Result<Response, Infallible> {
        let content = String::from(content);
        let content = Vec::from(content);
        let resp = Response {
            status: StatusCode::Ok,
            headers: HashMap::new(),
            body: Some(Body {
                content_type: mime::TEXT_PLAIN,
                content: content,
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

#[derive(Debug, PartialEq)]
pub struct ParseRequestError {
    msg: String,
}

impl fmt::Display for ParseRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for ParseRequestError {}

#[derive(Debug)]
pub struct Request {
    pub method: HttpMethod,
    pub uri: String,
    pub version: String,
    pub headers: HttpHeaders,
    pub content: Vec<u8>,
}

impl Request {
    pub fn from<T: io::Read>(from: &mut T) -> Result<Request, ParseRequestError> {
        let mut reader = io::BufReader::new(from);

        let request_line = match HttpRequestLine::read_from(&mut reader) {
            Ok(request) => request,
            Err(err) => return Err(err),
        };
        debug!("request line parsed: {:?}", request_line);
        let headers = HttpHeaders::read_from(&mut reader)?;

        let request = Request {
            content: Vec::new(),
            headers: headers,
            method: request_line.method,
            uri: request_line.uri,
            version: request_line.version,
        };
        Ok(request)
    }

    pub fn from_str(from: &str) -> Result<Request, ParseRequestError> {
        let mut reader = BufReader::new(from.as_bytes());
        Request::from(&mut reader)
    }
}

#[derive(Debug)]
pub struct HttpHeaders {
    list: Vec<(String, String)>,
}

impl HttpHeaders {
    pub fn new() -> HttpHeaders {
        HttpHeaders { list: Vec::new() }
    }
    fn read_from<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<HttpHeaders, ParseRequestError> {
        let mut headers = Self::new();
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec4.html#sec4.5:
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        debug!("parsing headers");
        loop {
            let header = HttpHeader::read_from(from)?;
            match header {
                None => {
                    break;
                }
                Some(header) => {
                    // headers.list.push((header.field_name, header.field_content));
                    headers.add_header(header);
                }
            };
        }
        debug!("headers parsed");
        Ok(headers)
    }

    fn add_header(&mut self, header: HttpHeader) {
        let name = header.field_name;
        let content = header.field_content;
        // If the header already exists append the value separated by a comma.
        // Excetp if the header is Set-Cookie:
        // https://www.rfc-editor.org/rfc/rfc7230#section-3.2.2

        match self.list.binary_search_by(|probe| probe.0.cmp(&name)) {
            Err(i) => self.list.insert(i, (name, content)),
            Ok(i) => {
                if name == "Set-Cookie" {
                    self.list.insert(i + 1, (name, content));
                    return;
                }
                let old = &mut self.list[i];
                old.1.push(',');
                old.1.push_str(&content);
            }
        };
    }
}

#[derive(Debug)]
struct HttpHeader {
    field_name: String,
    field_content: String,
}

impl HttpHeader {
    pub fn read_from<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<Option<HttpHeader>, ParseRequestError> {
        //generic-message = start-line
        //                  *(message-header CRLF)
        //                   CRLF
        // Line folding is not supported as specified in:
        // https://www.rfc-editor.org/rfc/rfc7230#section-3.2.4
        let mut line = Vec::<u8>::new();
        loop {
            let mut header_chunk = Vec::<u8>::new();
            if let Err(err) = from.read_until('\n' as u8, &mut header_chunk) {
                return Err(ParseRequestError {
                    msg: err.to_string(),
                });
            };
            line.append(&mut header_chunk);
            let len = line.len();
            if len < 2 {
                continue;
            }
            if line[len - 1] == '\n' as u8 && line[len - 2] == '\r' as u8 {
                break;
            }
        }
        HttpHeader::parse_header_line(line)
    }

    fn parse_header_line(line: Vec<u8>) -> Result<Option<HttpHeader>, ParseRequestError> {
        // header-field   = field-name ":" OWS field-value OWS
        // field-name     = token
        // field-value    = *( field-content / obs-fold )
        // field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
        // field-vchar    = VCHAR / obs-text
        assert!(line.len() >= 2);
        if line.len() == 2 {
            return Ok(None);
        };
        // Remove the \r\n at the end of the header line.
        let line = &line[..line.len() - 2];
        // Read the field-name which is a token:
        // https://www.rfc-editor.org/rfc/rfc7230#section-3.2.6
        let mut name = String::new();
        let mut i = 0;
        while i < line.len() {
            let c = line[i] as char;
            if !c.is_valid_token_char() {
                break;
            };
            name.push(c);
            i += 1;
        }
        if name.len() < 1 {
            debug!(
                "invalid header name line: {}, missing header name",
                String::from_utf8_lossy(line)
            );
            return Err(ParseRequestError {
                msg: String::from("invalid header name"),
            });
        };
        // After the token we MUST receive a colon.
        if line[i] != ':' as u8 {
            debug!(
                "invalid header line: {}, missing semicolon",
                String::from_utf8_lossy(line)
            );
            return Err(ParseRequestError {
                msg: String::from("invalid header name"),
            });
        };
        // The header value must have at least one octed.
        let mut header_value_start = i + 1;
        let header_value_length = line.len() - header_value_start;
        if header_value_length < 1 {
            debug!(
                "invalid header value line: {}",
                String::from_utf8_lossy(line)
            );
            return Err(ParseRequestError {
                msg: String::from("invalid header value"),
            });
        };
        // We don't support folding so the field-value = field-content.
        let mut field_value = String::new();
        // Skip the initial optional white space if any.
        let mut j = header_value_start;
        while j < line.len() && (line[j] as char).is_optional_white_space() {
            j += 1;
        }
        header_value_start = j;
        for j in header_value_start..line.len() {
            let c = line[j] as char;
            if !c.is_valid_field_content() {
                debug!(
                    "invalid header value line: {}, char {}, position {}",
                    String::from_utf8_lossy(line),
                    c,
                    j
                );
                return Err(ParseRequestError {
                    msg: String::from("invalid header value"),
                });
            }
            field_value.push(c);
        }

        let header = HttpHeader {
            field_name: name,
            field_content: field_value,
        };
        debug!("header parsed: {}", header);
        Ok(Some(header))
    }
}

impl fmt::Display for HttpHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.field_name, self.field_content)
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
        if let Err(err) = from.read_until(' ' as u8, &mut method) {
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
        if let Err(err) = from.read_until(' ' as u8, &mut uri) {
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
        if let Err(err) = from.read_until('\n' as u8, &mut version) {
            return Err(ParseRequestError {
                msg: err.to_string(),
            });
        };
        if version.len() < 3 {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        };

        if version[version.len() - 2] != ('\r' as u8) {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        }
        let version = String::from_utf8_lossy(&version[..version.len() - 2]);

        Ok(HttpRequestLine {
            method: method,
            uri: String::from(uri),
            version: String::from(version),
        })
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
            &HttpMethod::CONNECT => write!(f, "CONNECT"),
            &HttpMethod::DELETE => write!(f, "DELETE"),
            &HttpMethod::GET => write!(f, "GET"),
            &HttpMethod::HEAD => write!(f, "HEAD"),
            &HttpMethod::OPTIONS => write!(f, "OPTIONS"),
            &HttpMethod::PATCH => write!(f, "PATCH"),
            &HttpMethod::POST => write!(f, "POST"),
            &HttpMethod::PUT => write!(f, "PUT"),
            &HttpMethod::TRACE => write!(f, "TRACE"),
        }
    }
}

trait HttpMessageChar {
    fn is_valid_token_char(self) -> bool;

    fn is_valid_field_content(self) -> bool;

    fn is_valid_vchar(self) -> bool;

    fn is_optional_white_space(self) -> bool;
}

impl HttpMessageChar for char {
    fn is_valid_token_char(self: char) -> bool {
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
        return false;
    }

    fn is_valid_vchar(self) -> bool {
        // field-vchar    = VCHAR / obs-text
        if self.is_ascii_graphic() {
            return true;
        };
        if self as u8 >= 0x80 {
            return true;
        };
        return false;
    }

    fn is_valid_field_content(self) -> bool {
        self.is_valid_vchar() || self.is_optional_white_space()
    }

    fn is_optional_white_space(self) -> bool {
        self == ' ' || self == '\t'
    }
}
