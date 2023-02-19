use std::process;
use std::process::exit;
use std::time::Duration;

use clap::{command, Parser};

use wruster::http;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::{Certificate, PrivateKey, Server, Timeouts};
use wruster_handlers::{log_middleware, serve_static};

#[macro_use]
extern crate log;

#[derive(Parser, Debug)]
#[command(author = "manel montilla", version = "0.0.1")]
/// Static web server that exposes the files under a directory through HTTP.
struct Cli {
    /// [IP|host]:port to listen.
    addres: String,
    /// Directory to serve.
    directory: String,
    /// Path to a private key file in pem format.
    ///
    /// Indicates that the server must use the provided certificate to accept
    /// connections using TLS.
    #[arg(long, requires = "tls_cert")]
    tls_private: Option<String>,
    /// Path to a certificate file in pem format.
    ///
    /// Indicates that the server must use the provided private key to accept
    /// connections using TLS.
    #[arg(long, requires = "tls_private")]
    tls_cert: Option<String>,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let addr = cli.addres;
    let dir = cli.directory;

    let routes = router::Router::new();
    let serve_dir: HttpHandler =
        log_middleware(Box::new(move |request| serve_static(&dir, request)));
    routes.add("/", http::HttpMethod::GET, serve_dir);
    let timeouts = Timeouts {
        write_response_timeout: Duration::from_secs(5),
        read_request_timeout: Duration::from_secs(5),
    };
    let mut server = Server::from_timeouts(timeouts);
    let running = match cli.tls_cert {
        Some(cert) => {
            let cert = Certificate::read_from(&cert)
                .map_err(|err| {
                    error!("{}", err);
                    exit(1)
                })
                .unwrap();
            // The spec ensures that if the tls_cert flag is the defined the
            // tls_private must be also defined.
            let private_key_file = cli.tls_private.unwrap();
            let key = PrivateKey::read_from(&private_key_file)
                .map_err(|err| {
                    error!("{}", err);
                    exit(1)
                })
                .unwrap();
            server.run_tls(&addr, routes, key, cert)
        }
        None => server.run(&addr, routes),
    };
    if let Err(err) = running {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    }
    if let Err(err) = server.wait() {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    };
    process::exit(0);
}
