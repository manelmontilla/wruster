use std::io::Write;
use std::net;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[macro_use]
extern crate log;

pub mod handlers;
pub mod http;
pub mod router;
mod thread_pool;

use http::*;
use router::{Normalize, Router};

pub fn run_and_serve(addr: &str, routes: Router) -> ServerResult {
    let listener = match net::TcpListener::bind(addr) {
        Ok(listener) => listener,
        Err(err) => return Err(Box::new(err)),
    };
    info!("listening on {}", &addr);
    let config = Arc::new(routes);
    let mut pool = thread_pool::Pool::new(5);
    loop {
        let (stream, src_addr) = match listener.accept() {
            Err(err) => return Err(Box::new(err)),
            Ok(connection) => connection,
        };
        info!("accepting connection from {}", src_addr);
        let cconfig = Arc::clone(&config);
        let action = move || {
            handle_conversation(stream, cconfig, src_addr);
        };
        pool.run(Box::new(action));
    }
}

fn handle_conversation(mut stream: net::TcpStream, routes: Arc<Router>, source_addr: SocketAddr) {
    debug!("handling conversation {}", source_addr);
    loop {
        if handle_connection(&stream, Arc::clone(&routes), source_addr) {
            if let Err(err) = stream.flush() {
                error!("error flusing: {}, error info: {}", source_addr, err);
                return;
            }
            debug!("connection fluxed");
            continue;
        }
        if let Err(err) = stream.shutdown(net::Shutdown::Both) {
            error!(
                "error closing  connection with: {}, error info: {}",
                source_addr, err
            );
        } else {
            debug!("connection closed")
        }
        break;
    }
}

fn handle_connection(
    stream: &net::TcpStream,
    routes: Arc<Router>,
    source_addr: SocketAddr,
) -> bool {
    let mut cont = false;
    let mut response = match read_request(&stream) {
        Ok(request) => {
            cont = is_connection_alive(&request);
            run_action(request, routes)
        }
        Err(err) => match err {
            errors::ParseRequestError::Unknow(err) => {
                error!("error reading request, error info: {}", err);
                Response::from_status(StatusCode::BadRequest)
            }
            errors::ParseRequestError::EmptyRequest => return true,
        },
    };
    // TODO: Review and handle the case when the stream returns and error when
    // cloning.
    let mut resp_stream = stream.try_clone().unwrap();
    if let Err(err) = response.write(&mut resp_stream) {
        error!(
            "error writing response to: {}, error info: {}",
            source_addr, err
        );
        return false;
    };
    cont
}

fn read_request(stream: &net::TcpStream) -> Result<Request, errors::ParseRequestError> {
    match Request::read_from(stream) {
        Err(err) => Err(err),
        Ok(request) => Ok(request),
    }
}

fn run_action<'a>(mut request: Request<'a>, routes: Arc<Router>) -> Response<'a> {
    let req_path = PathBuf::from(request.uri);
    let normalized = match req_path.normalize() {
        Ok(path) => path,
        Err(err) => {
            let p = req_path.to_str().unwrap_or("unble to get path");
            error!("error: parsing path {}, error info: {}", p, err);
            return Response::from_status(StatusCode::InternalServerError);
        }
    };

    let normalized = match normalized.to_str() {
        None => return Response::from_status(StatusCode::InternalServerError),
        Some(path) => path,
    };
    let action = match routes.get_prefix(String::from(normalized), request.method) {
        Some(action) => action,
        None => return Response::from_status(StatusCode::NotFound),
    };
    request.uri = String::from(normalized);
    action(request)
}

fn is_connection_alive(request: &http::Request) -> bool {
    match request.headers.get("Connection") {
        None => false,
        Some(values) => values.iter().any(|value| value == "keep-alive"),
    }
}
