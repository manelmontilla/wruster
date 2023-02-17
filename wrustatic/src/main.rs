use std::error;
use std::fmt::Display;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use std::process;
use std::process::exit;
use std::time::Duration;

use clap::{command, Parser};

use wruster::http;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::{Server, Timeouts};
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
            let private_key_file = cli.tls_private.unwrap();
            let cert = read_certificate(cert)
                .map_err(|err| {
                    error!("{}", err);
                    exit(1)
                })
                .unwrap();
            let key = read_private_key(private_key_file)
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

fn read_certificate(path: String) -> io::Result<rustls::Certificate> {
    let cert_path = PathBuf::from(&path);
    let file = std::fs::File::open(cert_path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            io::Error::new(io::ErrorKind::Other, Error::FileNotFound(path.clone()))
        } else {
            err
        }
    })?;
    let mut cert_reader = std::io::BufReader::new(file);
    let cert = rustls_pemfile::certs(&mut cert_reader)?
        .iter()
        .map(|v| rustls::Certificate(v.clone()))
        .collect::<Vec<rustls::Certificate>>();
    match cert.len() {
        0 => Err(io::Error::new(
            io::ErrorKind::Other,
            Error::CertificateNotFound(path),
        )),
        _ => Ok(cert[0].clone()),
    }
}

fn read_private_key(path: String) -> io::Result<rustls::PrivateKey> {
    let keyfile = std::fs::File::open(&path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            io::Error::new(io::ErrorKind::Other, Error::FileNotFound(path.clone()))
        } else {
            err
        }
    })?;
    let mut reader = BufReader::new(keyfile);
    match rustls_pemfile::read_one(&mut reader)? {
        Some(rustls_pemfile::Item::RSAKey(key)) => Ok(rustls::PrivateKey(key)),
        Some(rustls_pemfile::Item::PKCS8Key(key)) => Ok(rustls::PrivateKey(key)),
        Some(rustls_pemfile::Item::ECKey(key)) => Ok(rustls::PrivateKey(key)),
        None => Err(io::Error::new(
            io::ErrorKind::Other,
            Error::PrivateKeyNotFound(path),
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::Other,
            Error::PrivateKeyNotFound(path),
        )),
    }
}

#[derive(Debug)]
enum Error {
    CertificateNotFound(String),
    PrivateKeyNotFound(String),
    FileNotFound(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CertificateNotFound(path) => {
                write!(f, "file {} does not contain any certificate", path)
            }
            Error::PrivateKeyNotFound(path) => {
                write!(f, "file {} does not contain a private key", path)
            }
            Error::FileNotFound(path) => {
                write!(f, "file {} not found", path)
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}
