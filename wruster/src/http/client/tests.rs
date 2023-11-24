use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

use super::*;
use crate::http::Response;
use crate::http::StatusCode;
use crate::router;
use crate::router::HttpHandler;
use crate::*;

#[test]
fn client_write_run_post_body() {
    let handler = handler_from_check_body(|content| String::from_utf8_lossy(&content) == "test");
    let (server, addr) = run_server(handler, HttpMethod::POST, "/");

    let c = Client::new();
    let body = Body::from("test", mime::TEXT_PLAIN);
    let request = Request::from_body(body, HttpMethod::POST, "/");
    let response = c.run(&addr, request).expect("Error running request");

    assert_eq!(response.status, http::StatusCode::OK);

    server.shutdown().expect("Error shutting down server");
}

#[test]
fn client_keep_alive_reuses_connection() {
    let handler = handler_from_check_body(|content| String::from_utf8_lossy(&content) == "test");
    let (server, addr) = run_server(handler, HttpMethod::POST, "/");

    let c = Client::new();
    let body = Body::from("test", mime::TEXT_PLAIN);
    let request = Request::from_body(body, HttpMethod::POST, "/");
    let response = c.run(&addr, request).expect("Error running request");
    assert_eq!(response.status, http::StatusCode::OK);

    // Release the connection.
    drop(response);

    // Check the connection is in the connection pool.
    let connection_pool = c.connection_pool.lock().unwrap();
    let conn = connection_pool
        .get(&addr)
        .expect("Expected connection to be in the pool");
    connection_pool.insert(&addr, conn);
    drop(connection_pool);

    let body = Body::from("test", mime::TEXT_PLAIN);
    let request = Request::from_body(body, HttpMethod::POST, "/");
    let response = c.run(&addr, request).expect("Error running 2nd request");

    // Check the connection is not the in the pool.
    let connection_pool = c.connection_pool.lock().unwrap();
    let dont_exist = connection_pool.get(&addr).is_none();
    assert_eq!(dont_exist, true);
    drop(connection_pool);

    assert_eq!(response.status, http::StatusCode::OK);
    drop(response);

    // Check the connection is in the connection pool again.
    let connection_pool = c.connection_pool.lock().unwrap();
    connection_pool
        .get(&addr)
        .expect("Expected connection to be in the pool");
    drop(connection_pool);

    server.shutdown().expect("Error shutting down server");
}

fn run_server(handler: HttpHandler, method: HttpMethod, path: &str) -> (Server, String) {
    let mut server = Server::new();
    let routes = router::Router::new();
    let port = get_free_port();
    let host = "127.0.0.1";
    let addr = format!("{}:{}", host, port.to_string());
    routes.add(path, method, handler);
    server.run(&addr, routes).unwrap();
    (server, addr)
}

fn get_free_port() -> u16 {
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    TcpListener::bind(addr)
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/*
body_or(
        |body| {
            let mut content: Vec<u8> = Vec::new();
            body.content.read_to_end(&mut content).unwrap();
            let content = String::from_utf8_lossy(&content);
            if &content == "test" {
                Response::from_status(StatusCode::OK)
            } else {
                Response::from_status(StatusCode::BadRequest)
            }
        },
        || Response::from_status(StatusCode::BadRequest),
    );
*/

pub fn handler_from_check_body<T: Fn(Vec<u8>) -> bool + Send + Sync + 'static>(
    check: T,
) -> HttpHandler {
    body_or(
        move |body| {
            let mut content: Vec<u8> = Vec::new();
            body.content.read_to_end(&mut content).unwrap();
            if check(content) {
                Response::from_status(StatusCode::OK)
            } else {
                Response::from_status(StatusCode::BadRequest)
            }
        },
        || Response::from_status(StatusCode::BadRequest),
    )
}

pub fn body_or<
    T: Fn(&mut Body) -> Response + Send + Sync + 'static,
    K: Fn() -> Response + Send + Sync + 'static,
>(
    body_exist: T,
    no_body: K,
) -> HttpHandler {
    Box::new(move |request| match request.body.as_mut() {
        None => no_body(),
        Some(body) => body_exist(body),
    })
}
// #[test]
// fn build_request_from_str() {
//     let c = Arc::new(Client::new());
//     let mut c2 = Arc::clone(&c);
//     let handle = thread::spawn(move || {
//         let c = &mut c2;
//         let r = Request::read_from_str("GET / HTTP/1.1\r\n\r\n").unwrap();
//         let mut resp = c.run(r).unwrap();
//         let mut v: Vec<u8> = Vec::new();
//         resp.write(&mut v).unwrap();
//         let s = String::from_utf8(v).unwrap();
//         println!("response {}", s);
//     });
//     handle.join().unwrap();
// }
