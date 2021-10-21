use std::env;
use std::process;

#[macro_use]
extern crate log;

use env_logger;

use wruster::actions;
use wruster::http;
use wruster::routes;

use wruster::run_and_serve;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("error {}", "usage: wruster ip:port directory");
        process::exit(1);
    }
    let addr = &args[1];
    let dir = &args[2];

    let routes = routes::Routes::new();
    let dir = dir.clone();
    let serve_dir = move |request| actions::serve_static(&dir, &request);
    routes.add("/", http::HttpMethod::GET, Box::new(serve_dir));
    if let Err(err) = run_and_serve(addr, routes) {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    }
    process::exit(0);
}
