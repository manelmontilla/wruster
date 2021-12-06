use std::env;
use std::process;

use wruster::handlers::{log_middleware, serve_static};
use wruster::http;
use wruster::router;

use wruster::router::HttpHandler;
use wruster::*;

#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: wruster ip:port directory");
        process::exit(1);
    }
    let addr = &args[1];
    let dir = &args[2];
    let routes = router::Router::new();
    let dir = dir.clone();
    let serve_dir: HttpHandler =
        log_middleware(Box::new(move |request| serve_static(&dir, &request)));
    routes.add("/", http::HttpMethod::GET, serve_dir);
    let mut server = Server::new();
    if let Err(err) = server.run(addr, routes, None) {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    };
    if let Err(err) = server.wait() {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    };
    process::exit(0);
}
