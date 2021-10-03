use std::collections::hash_map::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::net;
use std::path::PathBuf;
use std::sync::Arc;

use mime::APPLICATION_OCTET_STREAM;
use mime_guess;

mod thread_pool;

pub type ServerResult = Result<(), Box<dyn Error>>;
pub type ServerResultData<T> = Result<T, Box<dyn Error>>;

pub fn run_and_serve(addr: &str, config: Config) -> ServerResult {
    let listener_res = net::TcpListener::bind(addr);
    let listener = match listener_res {
        Ok(listener) => listener,
        Err(err) => return Err(Box::new(err)),
    };
    let config = Arc::new(config);
    // let pool = sync::ThreadPool::new(5);
    loop {
        let (stream, src_addr) = match listener.accept() {
            Err(err) => return Err(Box::new(err)),
            Ok(connection) => connection,
        };
        println!("\naccepting connection from {}", src_addr);
        let cconfig = config.clone();
        // We are not waiting fot the threads to finish which is dirty.
        /*pool.exec(move || match handle_connection(stream, cconfig) {
            Err(err) => {
                println!("error handling request: {}", err);
                Some(err.to_string())
            }
            _ => None,
        });*/
    }
}

fn handle_connection(mut stream: net::TcpStream, config: Arc<Config>) -> ServerResult {
    let request = match Request::from(&mut stream) {
        Err(err) => return Err(Box::new(err)),
        Ok(request) => request,
    };
    println!("\nContent received:\n{:?}", request);
    let resp = match config.static_content.as_ref() {
        None => Ok(Response {
            status: crate::StatusCode::Ok,
            headers: HashMap::new(),
            body: Some(Body {
                content_type: APPLICATION_OCTET_STREAM,
                content: Vec::new(),
            }),
        }),
        Some(dir) => Response::from_static_content(dir, &request),
    };
    if let Err(err) = resp {
        return Err(err);
    }
    let mut resp = resp.unwrap();
    // We don't support keep alive connections.
    resp.headers
        .insert(String::from("Connection"), String::from("Close"));
    resp.write(&mut stream).unwrap();
    stream.flush().unwrap();
    stream.shutdown(net::Shutdown::Both).unwrap();
    Ok(())
}

#[derive(Debug)]
pub struct Config {
    pub static_content: Option<String>,
}

#[derive(Debug)]
struct Body {
    content_type: mime::Mime,
    content: Vec<u8>,
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
struct Response {
    status: StatusCode,
    headers: HashMap<String, String>,
    body: Option<Body>,
}

impl Response {
    fn write<T: io::Write>(&self, to: &mut T) -> ServerResult {
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
        let mut path: PathBuf = PathBuf::from(dir);
        let mut uri = request.uri.as_str();
        if uri.starts_with('/') {
            if uri.len() < 2 {
                println!("error reading file {:?}, error file not found", dir);
                return Ok(Response::from_status(StatusCode::NotFound));
            }
            uri = &uri[1..]
        }
        path.push(uri);
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

    fn from_status(status: StatusCode) -> Response {
        Response {
            status: status,
            headers: HashMap::new(),
            body: None,
        }
    }
}

#[derive(Debug)]
enum StatusCode {
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
struct ParseRequestError {
    msg: String,
}

impl fmt::Display for ParseRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for ParseRequestError {}

#[derive(Debug)]
struct Request {
    method: String,
    uri: String,
    version: String,
    headers: HashMap<String, String>,
    content: Vec<u8>,
}

impl Request {
    fn from<T: io::Read>(from: &mut T) -> Result<Request, ParseRequestError> {
        let mut reader = io::BufReader::new(from);

        let request = match Request::read_request_line(&mut reader) {
            Ok(request) => request,
            Err(err) => return Err(err),
        };

        Ok(request)
    }

    fn read_request_line<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<Request, ParseRequestError> {
        // Request-Line   = Method SP Request-URI SP HTTP-Version CRLF
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec5.html

        // Parsing the request line this way is not fast, but the objective is
        // to make it cleat not performant.
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
            method: String::from(method),
            uri: String::from(uri),
            version: String::from(version),
            headers: HashMap::new(),
            content: Vec::new(),
        })
    }
}
