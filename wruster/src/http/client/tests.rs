use super::*;
use std::net::{SocketAddrV4, TcpListener};
use std::thread;

use std::sync::{Arc, Mutex};

use super::*;

use crate::http::headers::Header;
use crate::http::headers::Headers;
use crate::http::Response;
use crate::http::StatusCode;
use crate::router;
use crate::router::HttpHandler;
use crate::*;

#[test]
fn do_a_request() {
    let handler: HttpHandler = Box::new(move |request| {
        let mut content: Vec<u8> = Vec::new();
        request
            .body
            .as_mut()
            .unwrap()
            .content
            .read_to_end(&mut content)
            .unwrap();
        let content = String::from_utf8_lossy(&content);
        print!("payload {}", content);
        if &content == "test" {
            Response::from_status(StatusCode::OK)
        } else {
            Response::from_status(StatusCode::InternalServerError)
        }
    });
    let (server, addr) = run_server(handler, HttpMethod::POST, "/");
    let body = Body::from("test", mime::TEXT_PLAIN);
    let request = Request::from_body(body, HttpMethod::POST, "/");
    let c = Client::new();
    let response = c.run(&addr, request).expect("Error running request");
    server.shutdown().expect("Error shuting down server");
    assert_eq!(response.status, http::StatusCode::OK)
}

fn run_server(handler: HttpHandler, method: HttpMethod, path: &str) -> (Server, String) {
    let mut server = Server::new();
    let routes = router::Router::new();
    let port = get_free_port();
    let host = "127.0.0.1";
    let addr = format!("{}:{}", host, port.to_string());
    routes.add("/", http::HttpMethod::POST, handler);
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
