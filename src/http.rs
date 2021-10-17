use std::collections::hash_map::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ParseError;

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

    fn from_static_content(dir: &str, request: &Request) -> ServerResultData<Response> {
        let base_path: PathBuf = PathBuf::from(dir).canonicalize().unwrap();
        let mut uri = request.uri.as_str();
        if uri.starts_with('/') {
            if uri.len() < 2 {
                println!("error reading file {:?}, error file not found", dir);
                return Ok(Response::from_status(StatusCode::NotFound));
            }
            uri = &uri[1..]
        }
        let mut path = base_path.clone();
        path.push(uri);
        let path = path.canonicalize().unwrap();
        let content = match fs::read(&path) {
            Ok(content) => content,
            Err(err) => {
                if let io::ErrorKind::NotFound = err.kind() {
                    return Ok(Response::from_status(StatusCode::NotFound));
                }
                println!("reading file {:?}, error {:?}", dir, err);
                return Ok(Response::from_status(StatusCode::InternalServerError));
            }
        };
        let mime_type = mime_guess::from_path(path).first_or_octet_stream();
        let resp = Response {
            status: StatusCode::Ok,
            headers: HashMap::new(),
            body: Some(Body {
                content_type: mime_type,
                content: content,
            }),
        };
        Ok(resp)
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
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StatusCode::Ok => write!(f, "200 OK"),
            StatusCode::InternalServerError => write!(f, "500 Internal Server Error"),
            StatusCode::NotFound => write!(f, "404 Not found"),
        }
    }
}

#[derive(Debug)]
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
    pub headers: HashMap<String, String>,
    pub content: Vec<u8>,
}

impl Request {
    pub fn from<T: io::Read>(from: &mut T) -> Result<Request, ParseRequestError> {
        let mut reader = io::BufReader::new(from);

        let request = match Request::read_request_line(&mut reader) {
            Ok(request) => request,
            Err(err) => return Err(err),
        };

        Ok(request)
    }

    pub fn from_str(from: &str) -> Result<Request, ParseRequestError> {
        let mut reader = BufReader::new(from.as_bytes());
        Request::from(&mut reader)
    }

    fn read_request_line<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<Request, ParseRequestError> {
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
        if version[version.len() - 1] != ('\n' as u8) {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        }
        if version[version.len() - 2] != ('\r' as u8) {
            return Err(ParseRequestError {
                msg: String::from("invalied request line"),
            });
        }
        let version = String::from_utf8_lossy(&version[..version.len() - 2]);

        Ok(Request {
            method: method,
            uri: String::from(uri),
            version: String::from(version),
            headers: HashMap::new(),
            content: Vec::new(),
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
