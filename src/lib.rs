use std::io::prelude::*;
use std::net;
use std::path::PathBuf;
use std::sync::Arc;

pub mod actions;
pub mod http;
pub mod routes;
mod thread_pool;
mod trie;

#[cfg(test)]
mod tests;

use http::*;
use routes::{Normalize, Routes};

pub fn run_and_serve(addr: &str, routes: Routes) -> ServerResult {
    let listener_res = net::TcpListener::bind(addr);
    let listener = match listener_res {
        Ok(listener) => listener,
        Err(err) => return Err(Box::new(err)),
    };
    let config = Arc::new(routes);
    let mut pool = thread_pool::Pool::new(5);
    loop {
        let (stream, src_addr) = match listener.accept() {
            Err(err) => return Err(Box::new(err)),
            Ok(connection) => connection,
        };
        println!("\naccepting connection from {}", src_addr);
        let cconfig = Arc::clone(&config);
        let action = move || {
            handle_connection(stream, cconfig);
        };
        pool.run(Box::new(action));
    }
}

fn handle_connection(mut stream: net::TcpStream, routes: Arc<Routes>) {
    let mut response = run_action(&mut stream, routes);
    // By now, we don't support keep alive connections.
    response.add_header(String::from("Connection"), String::from("Close"));
    response.write(&mut stream).unwrap();
    stream.flush().unwrap();
    stream.shutdown(net::Shutdown::Both).unwrap();
}

fn run_action(stream: &mut net::TcpStream, routes: Arc<Routes>) -> Response {
    let mut request = match Request::from(stream) {
        Err(err) => return Response::from_status(StatusCode::InternalServerError),
        Ok(request) => request,
    };
    let req_path = PathBuf::from(request.uri);
    let normalized = match req_path.normalize() {
        Ok(path) => path,
        Err(err) => return Response::from_status(StatusCode::InternalServerError),
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
