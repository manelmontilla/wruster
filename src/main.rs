use std::env;
use std::process;
// use web_server::config::Config;
// use web_server::run_and_serve;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("error {}", "usage: web_server ip:port");
        process::exit(1);
    }
    let addr = &args[1];
    let dir = &args[2];
    // let config = Config {
    //     static_content: Some(dir.clone()),
    // };
    // if let Err(err) = run_and_serve(addr, config) {caergo
    //     eprintln!("error {}", err.to_string());
    //     process::exit(1);
    // }
    // process::exit(0);
}
