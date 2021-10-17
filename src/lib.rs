use std::collections::hash_map::HashMap;
use std::io::prelude::*;
use std::net;
use std::sync::Arc;

use mime::APPLICATION_OCTET_STREAM;

pub mod http;
pub mod routes;
mod thread_pool;
mod trie;

#[cfg(test)]
mod tests;

use http::*;
use routes::Routes;

pub fn run_and_serve<T>(addr: &str, routes: Routes) -> ServerResult {
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
            let res = handle_connection(stream, cconfig);
        };
        pool.run(Box::new(action));
    }
}

fn handle_connection(mut stream: net::TcpStream, routes: Arc<Routes>) -> ServerResult {
    let request = match Request::from(&mut stream) {
        Err(err) => return Err(Box::new(err)),
        Ok(request) => request,
    };
    println!("\nContent received:\n{:?}", request);
    let resp = Ok(Response {
        status: crate::StatusCode::Ok,
        headers: HashMap::new(),
        body: Some(Body {
            content_type: APPLICATION_OCTET_STREAM,
            content: Vec::new(),
        }),
    });
    if let Err(err) = resp {
        return Err(err);
    }
    let mut resp = resp.unwrap();
    // We don't support keep alive connections.
    resp.add_header(String::from("Connection"), String::from("Close"));
    resp.write(&mut stream).unwrap();
    stream.flush().unwrap();
    stream.shutdown(net::Shutdown::Both).unwrap();
    Ok(())
}
