use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::{io, path::PathBuf};

use crate::http::{Body, Request, Response, StatusCode};

pub fn serve_static<'a, 'b, 'c>(dir: &'b str, request: &'a Request) -> Response<'c> {
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
    Response {
        status: StatusCode::Ok,
        headers: HashMap::new(),
        body: Some(Body {
            content_length: metadata.len(),
            content_type: mime_type,
            content: Box::new(BufReader::new(content)),
        }),
    }
}
