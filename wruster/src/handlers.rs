use std::fs;
use std::io::BufReader;
use std::{io, path::PathBuf};

use crate::http::headers::{Header, Headers};
use crate::http::{Body, Request, Response, StatusCode};
use crate::router::HttpHandler;

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
    headers.add_header(Header {
        name: String::from("Content-Length"),
        value: metadata.len().to_string(),
    });
    headers.add_header(Header {
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

pub fn log_middleware(handler: HttpHandler) -> HttpHandler {
    Box::new(move |request: Request| {
        info!("request {:?}", request);
        let response = handler(request);
        info!("response: {:?}", response);
        response
    })
}
