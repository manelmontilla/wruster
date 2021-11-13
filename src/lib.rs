use std::net;
use std::path::PathBuf;
use std::sync::Arc;
use std::{io::prelude::*, net::SocketAddr};

#[macro_use]
extern crate log;

pub mod handlers;
pub mod http;
pub mod router;
mod thread_pool;

#[cfg(test)]
mod tests;

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
            handle_connection(stream, cconfig, src_addr);
        };
        pool.run(Box::new(action));
    }
}

fn handle_connection(mut stream: net::TcpStream, routes: Arc<Router>, source_addr: SocketAddr) {
    let mut response = run_action(&mut stream, routes);
    // By now, we don't support keep alive connections.
    response.add_header(String::from("Connection"), String::from("Close"));
    if let Err(err) = response.write(&mut stream) {
        error!(
            "error writing response to: {}, error info: {}",
            source_addr, err
        );
        return;
    }

    if let Err(err) = stream.flush() {
        error!(
            "error flusing stream to: {}, error info: {}",
            source_addr, err
        );
        return;
    }
    if let Err(err) = stream.shutdown(net::Shutdown::Both) {
        error!(
            "error closing  connection with: {}, error info: {}",
            source_addr, err
        );
        return;
    }
}

fn run_action(stream: &mut net::TcpStream, routes: Arc<Router>) -> Response {
    let mut request = match Request::from(stream) {
        Err(err) => {
            error!("error parsing request, error info: {}", err);
            return Response::from_status(StatusCode::BadRequest);
        }
        Ok(request) => request,
    };
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
