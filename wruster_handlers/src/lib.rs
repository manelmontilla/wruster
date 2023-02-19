/**
Contains a set of helpful handlers, middlewares and utilities to create
new handlers in a wruster web server.
*/
use std::fs;
use std::io::BufReader;
use std::{io, path::PathBuf};

#[macro_use]
extern crate log;

use wruster::http::headers::{Header, Headers};
use wruster::http::{Body, Request, Response, StatusCode};
use wruster::router::HttpHandler;

/**
Implements a handler that serves the files in a directory tree.

# Examples

```no_run
use wruster::router;
use wruster::http;
use wruster::Server;
use wruster_handlers::serve_static;

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
pub fn serve_static(dir: &str, request: &Request) -> Response {
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
    let content = Box::new(BufReader::new(content));
    headers.add(Header {
        name: String::from("Content-Length"),
        value: metadata.len().to_string(),
    });
    headers.add(Header {
        name: String::from("Content-Type"),
        value: mime_type.to_string(),
    });
    let body = Body::new(Some(mime_type), metadata.len(), content);
    Response {
        status: StatusCode::OK,
        headers,
        body: Some(body),
    }
}

/**
A middleware that uses the log to print the request and response to
the standard output with INFO level.

# Examples

```no_run
use std::str::FromStr;

use wruster::router;
use wruster::Server;
use wruster::http;

use wruster_handlers::log_middleware;

env_logger::init();
let addr = "localhost:8085";
let routes = router::Router::new();
let handler: router::HttpHandler = Box::new(move |_| {
    let greetings = "hello!!";
    http::Response::from_str(&greetings).unwrap()
});
let handler = log_middleware(handler);
routes.add("/", http::HttpMethod::GET, handler);
let mut server = Server::new();
server.run(addr, routes).unwrap();
server.wait().unwrap();
```
*/
pub fn log_middleware(handler: HttpHandler) -> HttpHandler {
    Box::new(move |request: &mut Request| {
        info!("request {:?}", request);
        let response = handler(request);
        info!("response {:?}", response);
        response
    })
}
