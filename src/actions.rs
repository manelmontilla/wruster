use std::collections::HashMap;
use std::fs;
use std::{io, path::PathBuf};

use crate::http::{Body, Request, Response, StatusCode};

pub fn serve_static(dir: &str, request: &Request) -> Response {
    let base_path: PathBuf = PathBuf::from(dir).canonicalize().unwrap();
    let mut uri = request.uri.as_str();
    if uri.starts_with('/') {
        if uri.len() < 2 {
            return Response::from_status(StatusCode::NotFound);
        }
        uri = &uri[1..]
    }
    let mut path = base_path.clone();
    path.push(uri);
    let content = match fs::read(&path) {
        Ok(content) => content,
        Err(err) => {
            if let io::ErrorKind::NotFound = err.kind() {
                return Response::from_status(StatusCode::NotFound);
            }
            return Response::from_status(StatusCode::InternalServerError);
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
    resp
}
