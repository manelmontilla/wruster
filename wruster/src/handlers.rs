use std::fs;
use std::io::BufReader;
use std::{io, path::PathBuf};

use crate::http::headers::{Header, Headers};
use crate::http::{Body, Request, Response, StatusCode};
use crate::router::HttpHandler;

/**
Implents a handler that serves the files in a directory tree.

# Examples

```no_run
use wruster::router;
use wruster::handlers::serve_static;
use wruster::http;
use wruster::Server;

let addr = "localhost:8085";
let dir = "./";
let routes = router::Router::new();
let dir = dir.clone();
let serve_dir: router::HttpHandler = Box::new(move |request| serve_static(&dir, &request));
routes.add("/", http::HttpMethod::GET, serve_dir);
let mut server = Server::new();
server.run(addr, routes).unwrap();
server.wait().unwrap();
```
*/
pub fn serve_static(dir: &str, request: &Request) -> Response<'static> {
    let base_path: PathBuf = PathBuf::from(dir).canonicalize().unwrap();
    let mut uri = request.uri.as_str();
    if uri.starts_with('/') {
        if uri.len() < 2 {
            return Response::from_status(StatusCode::NotFound);
        }
        uri = &uri[1..]
    }
    let mut path = base_path;
    path.push(uri);

    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) => {
            if let io::ErrorKind::NotFound = err.kind() {
                return Response::from_status(StatusCode::NotFound);
            }
            return Response::from_status(StatusCode::InternalServerError);
        }
    };

    let content = match fs::File::open(&path) {
        Ok(content) => content,
        Err(err) => {
            if let io::ErrorKind::NotFound = err.kind() {
                return Response::from_status(StatusCode::NotFound);
            }
            return Response::from_status(StatusCode::InternalServerError);
        }
    };
    let mime_type = mime_guess::from_path(path).first_or_octet_stream();
    let mut headers = Headers::new();
    let body = Box::new(BufReader::new(content));
    headers.add(Header {
        name: String::from("Content-Length"),
        value: metadata.len().to_string(),
    });
    headers.add(Header {
        name: String::from("Content-Type"),
        value: mime_type.to_string(),
    });
    Response {
        status: StatusCode::OK,
        headers,
        body: Some(Body {
            content_length: metadata.len(),
            content_type: Some(mime_type),
            content: body,
        }),
    }
}

/**
A middleware that uses the log to print the request and response to
the standard output with INFO level.

# Examples

```no_run
use std::str::FromStr;

use wruster::handlers;
use wruster::router;
use wruster::Server;
use wruster::http;

env_logger::init();
let addr = "localhost:8085";
let routes = router::Router::new();
let handler: router::HttpHandler = Box::new(move |_| {
    let greetings = "hello!!";
    http::Response::from_str(&greetings).unwrap()
});
let handler = handlers::log_middleware(handler);
routes.add("/", http::HttpMethod::GET, handler);
let mut server = Server::new();
server.run(addr, routes).unwrap();
server.wait().unwrap();
```
*/
pub fn log_middleware(handler: HttpHandler) -> HttpHandler {
    Box::new(move |request: Request| {
        info!("request {:?}", request);
        let response = handler(request);
        info!("response {:?}", response);
        response
    })
}
