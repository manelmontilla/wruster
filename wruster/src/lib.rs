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
use polling::{Event, Poller};
use router::{Normalize, Router};

use crate::thread_pool::PoolError::{Busy, self};
use crate::timeout_stream::TimeoutStream;

pub const DEFAULT_IDLE_TIMEOUT: time::Duration = time::Duration::from_secs(10);
pub const DEFAULT_READ_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);
pub const DEFAULT_WRITE_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);

#[derive(Clone)]
pub struct Timeouts {
    pub read_request_timeout: time::Duration,
    pub write_request_timeout: time::Duration,
}

pub struct Server {
    stop: Arc<AtomicBool>,
    addr: Option<String>,
    handle: Option<JoinHandle<Result<(), Box<Error>>>>,
    poller: Option<Arc<Poller>>,
    timeouts: Timeouts,
}

impl Server {
    pub fn new() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = None;
        let poller = None;
        let addr = None;
        let timeouts = Timeouts {
            read_request_timeout: DEFAULT_READ_REQUEST_TIMEOUT,
            write_request_timeout: DEFAULT_WRITE_REQUEST_TIMEOUT,
        };
        Server {
            stop,
            addr,
            handle,
            poller,
            timeouts,
        }
    }

    pub fn from_timeouts(timeouts: Timeouts) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = None;
        let poller = None;
        let addr = None;
        Server {
            stop,
            addr,
            handle,
            poller,
            timeouts,
        }
    }

    pub fn run(&mut self, addr: &str, routes: Router) -> ServerResult {
        let listener = match net::TcpListener::bind(addr) {
            Ok(listener) => listener,
            Err(err) => return Err(Box::new(err)),
        };
        listener.set_nonblocking(true).unwrap();
        let poller = polling::Poller::new().unwrap();
        poller.add(&listener, Event::readable(1)).unwrap();
        let poller = Arc::new(poller);
        let epoller = Arc::clone(&poller);
        self.poller = Some(poller);
        let mut events = Vec::new();

        info!("listening on {}", &addr);
        let routes = Arc::new(routes);
        let mut pool = thread_pool::Pool::new(1);
        let stop = Arc::clone(&self.stop);
        let timeouts = self.timeouts.clone();
        let handle = thread::spawn(move || loop {
            events.clear();
            epoller.wait(&mut events, None)?;
            for evt in &events {
                if evt.key != 1 {
                    continue;
                }
                let (stream, src_addr) = match listener.accept() {
                    Err(err) => return Err(Box::new(err)),
                    Ok(connection) => connection,
                };
                stream.set_nonblocking(false).unwrap();
                epoller.modify(&listener, Event::readable(1)).unwrap();
                info!("accepting connection from {}", src_addr);
                let cconfig = Arc::clone(&routes);
                let action_timeouts = timeouts.clone();
                let src_addr_action = src_addr.clone();
                let action_stream = match stream.try_clone() {
                    Ok(stream) => stream,
                    Err(err) => {
                        error!("error cloning stream: {}", err.to_string());
                        continue;
                    }
                };
                let action = move || {
                    handle_conversation(action_stream, cconfig, action_timeouts.clone(), src_addr_action);
                };

                if let Err(_) = pool.run(Box::new(action)) {
                    error!("server to busy to handle connection with: {}", src_addr);
                    handle_busy(stream, timeouts.clone(), src_addr);
                }

            }
            if stop.as_ref().load(Ordering::SeqCst) {
                return Ok(());
            };
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
        self.poller.unwrap().notify()?;
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


fn handle_busy(
    mut stream: net::TcpStream,
    timeouts: Timeouts,
    src_addr: SocketAddr,
) {
    debug!("sending too busy to {}", src_addr);
    let write_timeout = Some(timeouts.write_request_timeout);
    let read_timeout = Some(timeouts.read_request_timeout);
    let mut timeout_stream = TimeoutStream::from(&mut stream, read_timeout, write_timeout);
    let mut resp = Response::from_status(StatusCode::ServiceUnavailable);
    if let Err(err) = resp.write(&mut timeout_stream) {
        error!("sending too busy to {}: {}", src_addr, err.to_string())
    }
    if let Err(err) = stream.shutdown(net::Shutdown::Both) {
        error!("error closing connection with: {}, {}", src_addr, err);
    }
    debug!("connection wuith closed")
}


fn handle_conversation(
    mut stream: net::TcpStream,
    routes: Arc<Router>,
    timeouts: Timeouts,
    source_addr: SocketAddr,
) {
    debug!("handling conversation with {}", source_addr);
    while handle_connection(
        &mut stream,
        Arc::clone(&routes),
        source_addr,
        timeouts.clone(),
    ) {
        if let Err(err) = stream.flush() {
            error!("error flushing to: {}, {}", source_addr, err);
            return;
        }
        debug!("connection flushed");
    }
    if let Err(err) = stream.shutdown(net::Shutdown::Both) {
        error!("error closing connection with: {}, {}", source_addr, err);
    }
    debug!("connection closed")
}

fn handle_connection(
    stream: &mut TcpStream,
    routes: Arc<Router>,
    source_addr: SocketAddr,
    timeouts: Timeouts,
) -> bool {
    let connection_open: bool;
    let read_timeout = Some(timeouts.read_request_timeout);
    let write_timeout = Some(timeouts.write_request_timeout);

    let mut resp_stream = match stream.try_clone() {
        Ok(stream) => stream,
        Err(err) => {
            error!("error cloning stream: {}", err.to_string());
            return false;
        }
    };

    let mut timeout_stream = TimeoutStream::from(stream, read_timeout, write_timeout);
    let mut response = match Request::read_from(&mut timeout_stream) {
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

    let mut timeout_stream = TimeoutStream::from(&mut resp_stream, read_timeout, write_timeout);
    if let Err(err) = response.write(&mut timeout_stream) {
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
