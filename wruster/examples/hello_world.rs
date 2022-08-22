use env_logger::Builder;
use std::process;
use std::str::FromStr;

use log::LevelFilter;
use wruster::http;
use wruster::http::Response;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::Server;

#[macro_use]
extern crate log;

fn main() {
    Builder::new().filter_level(LevelFilter::Info).init();
    let routes = router::Router::new();
    let handler: HttpHandler = Box::new(move |_| Response::from_str("hellow world").unwrap());
    routes.add("/", http::HttpMethod::GET, handler);
    let mut server = Server::new();
    if let Err(err) = server.run("127.0.0.1:8082", routes) {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    };
    if let Err(err) = server.wait() {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    };
    process::exit(0);
}
