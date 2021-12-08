use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::{io::Write, time};
use std::{net, thread};

#[macro_use]
extern crate log;

pub mod handlers;
pub mod http;
pub mod router;
mod thread_pool;
mod timeout_stream;

use http::*;
use router::{Normalize, Router};

pub const DEFAULT_IDLE_TIMEOUT: time::Duration = time::Duration::from_secs(10);
pub const DEFAULT_READ_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);
pub const DEFAULT_WRITE_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);

#[derive(Clone)]
pub struct Timeouts {
    pub idle_timeout: time::Duration,
    pub read_request_timeout: time::Duration,
    pub write_request_timeout: time::Duration,
}

pub struct Server {
    stop: Arc<AtomicBool>,
    addr: Option<String>,
    handle: Option<JoinHandle<Result<(), Box<Error>>>>,
    timeouts: Timeouts,
}

impl Server {
    pub fn new() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = None;
        let addr = None;
        let timeouts = Timeouts {
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            read_request_timeout: DEFAULT_READ_REQUEST_TIMEOUT,
            write_request_timeout: DEFAULT_WRITE_REQUEST_TIMEOUT,
        };
        Server {
            stop,
            addr,
            handle,
            timeouts,
        }
    }

    pub fn from_timeouts(timeouts: Timeouts) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = None;
        let addr = None;
        Server {
            stop,
            addr,
            handle,
            timeouts,
        }
    }

    pub fn run(&mut self, addr: &str, routes: Router) -> ServerResult {
        let listener = match net::TcpListener::bind(addr) {
            Ok(listener) => listener,
            Err(err) => return Err(Box::new(err)),
        };
        info!("listening on {}", &addr);
        let routes = Arc::new(routes);
        let mut pool = thread_pool::Pool::new(5);
        let stop = Arc::clone(&self.stop);
        let timeouts = self.timeouts.clone();

        let handle = thread::spawn(move || loop {
            let (stream, src_addr) = match listener.accept() {
                Err(err) => return Err(Box::new(err)),
                Ok(connection) => connection,
            };
            if stop.as_ref().load(Ordering::SeqCst) {
                return Ok(());
            }
            info!("accepting connection from {}", src_addr);
            let cconfig = Arc::clone(&routes);
            if let Err(err) = stream.set_read_timeout(Some(timeouts.idle_timeout)) {
                panic!("setting idle timeout for connections {}", err.to_string());
            }
            let action = move || {
                handle_conversation(stream, cconfig, src_addr);
            };
            pool.run(Box::new(action));
        });

        self.handle = Some(handle);
        self.addr = Some(String::from(addr));
        Ok(())
    }

    pub fn shutdown(self) -> ServerResult {
        if self.handle.is_none() {
            let err = Box::new(Error::new(ErrorKind::Other, "server not started"));
            return Err(err);
        }
        self.stop.as_ref().store(true, Ordering::SeqCst);
        TcpStream::connect(self.addr.unwrap()).unwrap();
        let handle = self.handle.unwrap();
        handle.join().unwrap()?;
        Ok(())
    }

    pub fn wait(self) -> ServerResult {
        if self.handle.is_none() {
            let err = Box::new(Error::new(ErrorKind::Other, "server not started"));
            return Err(err);
        }
        let handle = self.handle.unwrap();
        handle.join().unwrap()?;
        Ok(())
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

fn handle_conversation(mut stream: net::TcpStream, routes: Arc<Router>, source_addr: SocketAddr) {
    debug!("handling conversation with {}", source_addr);
    while handle_connection(&stream, Arc::clone(&routes), source_addr) {
        if let Err(err) = stream.flush() {
            error!("error flusing to: {}, {}", source_addr, err);
            return;
        }
        debug!("connection fluxed");
    }
    if let Err(err) = stream.shutdown(net::Shutdown::Both) {
        error!("error closing connection with: {}, {}", source_addr, err);
    }
    debug!("connection closed")
}

fn handle_connection(
    stream: &net::TcpStream,
    routes: Arc<Router>,
    source_addr: SocketAddr,
) -> bool {
    let connection_open: bool;
    let mut response = match Request::read_from(stream) {
        Ok(request) => {
            connection_open = is_connection_alive(&request);
            run_action(request, routes)
        }
        Err(err) => match err {
            errors::ParseError::Unknow(err) => {
                error!("error reading request, error info: {}", err);
                connection_open = false;
                Response::from_status(StatusCode::BadRequest)
            }
            errors::ParseError::Timeout => return false,
            errors::ParseError::ConnectionClosed => return false,
        },
    };
    // TODO: Handle error cloning the stream.
    let mut resp_stream = stream.try_clone().unwrap();
    if let Err(err) = response.write(&mut resp_stream) {
        error!(
            "error writing response to: {}, error info: {}",
            source_addr, err
        );
        return false;
    };
    connection_open
}

fn run_action(mut request: Request<'_>, routes: Arc<Router>) -> Response<'_> {
    let req_path = PathBuf::from(request.uri);
    let normalized = match req_path.normalize() {
        Ok(path) => path,
        Err(err) => {
            let p = req_path.to_str().unwrap_or("unble to get path");
            error!("error: parsing path {}, error info: {}", p, err);
            return Response::from_status(StatusCode::InternalServerError);
        }
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

fn is_connection_alive(request: &http::Request) -> bool {
    match request.headers.get("Connection") {
        None => false,
        Some(values) => values
            .iter()
            .any(|value| value.to_lowercase() == "keep-alive"),
    }
}
