#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

/*!
Experimental simple web sever that includes a http
parser, a router and a server.

# Examples
```rust no_run
use env_logger::Builder;
use std::process;
use std::str::FromStr;
use std::time::Duration;

use log::LevelFilter;
use wruster::http;
use wruster::http::Response;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::{Server, Timeouts};

#[macro_use]
extern crate log;

fn main() {
   Builder::new().filter_level(LevelFilter::Info).init();
   let routes = router::Router::new();
   let handler: HttpHandler = Box::new(move |_| {
       Response::from_str("hello world").unwrap()
   });
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
```
*/

use std::error::Error as StdError;
use std::io::{self, Error, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::{io::Write, time};
use std::{net, thread};

#[macro_use]
extern crate log;
use http::errors::HttpError;
use http::*;
use polling::{Event, Poller};
use router::{Normalize, Router};
pub use streams::tls::{Certificate, PrivateKey};
use streams::{
    cancellable_stream::CancellableStream,
    observable::{ObservedStream, ObservedStreamList},
    timeout_stream::TimeoutStream,
    tls, Stream,
};
/// Contains all the types necessary for dealing with Http messages.
pub mod http;
/// Contains the router to be used in a [`Server`].
pub mod router;

/// Contains support functions for tests.
pub mod test_utils;

mod streams;
mod thread_pool;

/// Defines the default max time for a request to be read
pub const DEFAULT_READ_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(30);

/// Defines the default max time for a response to be written
pub const DEFAULT_WRITE_RESPONSE_TIMEOUT: time::Duration = time::Duration::from_secs(30);

/// Defines the result type returned from the [Server] methods.
pub type ServerResult = Result<(), Box<dyn StdError>>;

/// Defines the timeouts used in [Server::from_timeouts] method.
#[derive(Clone)]
pub struct Timeouts {
    /// maximum time for a request to be read
    pub read_request_timeout: time::Duration,
    /// maximum time for a request to be written.
    pub write_response_timeout: time::Duration,
}

/// Represents a web server that can be run by passing a [router::Router].
pub struct Server {
    stop: Arc<AtomicBool>,
    addr: Option<String>,

    handle: Option<JoinHandle<Result<(), Box<Error>>>>,
    poller: Option<Arc<Poller>>,
    timeouts: Timeouts,
}

impl Server {
    /**
    Returns a Server using the default
    [read][`DEFAULT_READ_REQUEST_TIMEOUT`] and
    [write][`DEFAULT_WRITE_RESPONSE_TIMEOUT`] timeouts.

    # Examples:

    ```rust
    use wruster::Server;
    let server = Server::new();
    ```
    */
    pub fn new() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = None;
        let poller = None;
        let addr = None;
        let timeouts = Timeouts {
            read_request_timeout: DEFAULT_READ_REQUEST_TIMEOUT,
            write_response_timeout: DEFAULT_WRITE_RESPONSE_TIMEOUT,
        };
        Server {
            stop,
            addr,
            handle,
            poller,
            timeouts,
        }
    }

    /**
    Returns a server configured with the given [Timeouts].

    # Arguments

    * `timeouts` - A [Timeouts] struct

    # Examples

    ```
    use wruster;
    let timeouts = wruster::Timeouts {
           read_request_timeout: wruster::DEFAULT_READ_REQUEST_TIMEOUT,
           write_response_timeout: wruster::DEFAULT_WRITE_RESPONSE_TIMEOUT,
    };
    let server = wruster::Server::from_timeouts(timeouts);
    ```
    */
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

    /**
    Starts a server listening on the specified address and using the given
    [`Router`], it returns the control immediately to caller.

    # Arguments

    * `addr` a string slice specifying the address to listen on, format: "hostname:port"

    * `routes` a [`Router`] with the routes the server must serve.

    # Examples

    ```no_run
    use std::str::FromStr;

    use wruster::Server;
    use wruster::http::{self, Request};
    use wruster::http::Response;
    use wruster::router;
    use wruster::router::HttpHandler;
    let routes = router::Router::new();
    let handler: HttpHandler = Box::new(move |request: &mut Request| {
        let body = &mut request.body.as_mut().unwrap();
        let mut name = String::new();
        body.content.read_to_string(&mut name).unwrap();
        let greetings = format!("hello {}!!", name);
        Response::from_str(&greetings).unwrap()
    });
    routes.add("/", http::HttpMethod::GET, handler);
    let mut server = Server::new();
    server.run("127.0.0.1:8082", routes).unwrap();
    server.wait().unwrap();
    ```

    # Errors

    This function will return an error if:
    * The address is wrong formatted or not free.
    * The server is already started.
    */
    pub fn run(&mut self, addr: &str, routes: Router) -> ServerResult {
        self.start(addr, routes, move |stream: TcpStream| {
            CancellableStream::new(stream)
        })
    }

    /**
     Starts a server that accepts TLS connections by listening on the specified address and using the given
     [`Router`], it returns the control immediately to caller.

     # Arguments
     * `addr` a string slice specifying the address to listen on, format: "hostname:port"

     * `routes` a [`Router`] with the routes the server must serve.

     * `key` the private key to use in TLS connections.

     * `cert` the certificate to use in TLS connections.

     # Examples

     ```no_run
     use std::str::FromStr;

     use wruster::{Server, Certificate, PrivateKey};
     use wruster::http::{self, Request};
     use wruster::http::Response;
     use wruster::router;
     use wruster::router::HttpHandler;
     let routes = router::Router::new();
     let handler: HttpHandler = Box::new(move |request: &mut Request| {
         let body = &mut request.body.as_mut().unwrap();
         let mut name = String::new();
         body.content.read_to_string(&mut name).unwrap();
         let greetings = format!("hello {}!!", name);
         Response::from_str(&greetings).unwrap()
     });
     routes.add("/", http::HttpMethod::GET, handler);
     let cert = Certificate::read_from("certificate.perm").unwrap();
     let key = PrivateKey::read_from("private_key.perm").unwrap();
     let mut server = Server::new();
     server.run_tls("127.0.0.1:8082", routes, key, cert).unwrap();
     server.wait().unwrap();
     ```
    # Errors

    This function will return an error if:
    * The address is wrong formatted or not free.
    * The server is already started.

     */
    pub fn run_tls(
        &mut self,
        addr: &str,
        routes: Router,
        key: PrivateKey,
        cert: Certificate,
    ) -> ServerResult {
        self.start(addr, routes, move |stream: TcpStream| {
            let stream = tls::Stream::new(stream, key.clone(), cert.clone()).unwrap();
            CancellableStream::new(stream)
        })
    }

    fn start<T: Stream + Send + Sync + 'static, F>(
        &mut self,
        addr: &str,
        routes: Router,
        stream_builder: F,
    ) -> ServerResult
    where
        F: Fn(TcpStream) -> io::Result<CancellableStream<T>> + Send + 'static,
    {
        if self.poller.is_some() {
            return Err(Box::new(Error::new(
                ErrorKind::Other,
                "server already started",
            )));
        }

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
        info!("listening on {}", &addr);
        let routes = Arc::new(routes);
        let execunits = match thread::available_parallelism() {
            Ok(units) => {
                info!("system reported {} available execution units", units);
                usize::from(units)
            }
            Err(err) => {
                let default_value = 2;
                error!(
                    "error getting available run units: {}, using default value: {}",
                    err.to_string(),
                    default_value
                );
                default_value
            }
        };

        let stop = Arc::clone(&self.stop);
        let timeouts = self.timeouts.clone();

        let handle = thread::spawn(move || {
            Self::accept_connections(
                timeouts,
                stop,
                listener,
                execunits,
                epoller,
                routes,
                stream_builder,
            )
        });

        self.handle = Some(handle);
        self.addr = Some(String::from(addr));
        Ok(())
    }

    fn accept_connections<F, T: Stream + Send + Sync + 'static>(
        timeouts: Timeouts,
        stop: Arc<AtomicBool>,
        listener: TcpListener,
        execunits: usize,
        epoller: Arc<Poller>,
        routes: Arc<Router>,
        stream_builder: F,
    ) -> Result<(), Box<Error>>
    where
        F: Fn(TcpStream) -> io::Result<CancellableStream<T>> + Send + 'static,
    {
        let mut events = Vec::new();
        let mut pool = thread_pool::Pool::new(execunits, 100);
        let active_streams = ObservedStreamList::new();
        loop {
            debug!("tracked streams {}", active_streams.len());
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
                epoller.modify(&listener, Event::readable(1)).unwrap();
                info!("accepting connection from {}", src_addr);
                let cconfig = Arc::clone(&routes);
                let action_timeouts = timeouts.clone();

                let stream = stream_builder(stream);
                let action_stream = match stream {
                    Ok(stream) => stream,
                    Err(err) => {
                        error!("error cloning stream: {}", err.to_string());
                        continue;
                    }
                };
                let action_stream = ObservedStreamList::track(&active_streams, action_stream);
                let local_action_stream = action_stream.clone();
                let action = move || {
                    handle_conversation(action_stream, cconfig, action_timeouts.clone(), src_addr);
                };

                if pool.run(Box::new(action)).is_err() {
                    error!("server too busy to handle connection with: {}", src_addr);
                    handle_busy(local_action_stream, timeouts.clone(), src_addr);
                }
            }
            if stop.as_ref().load(Ordering::SeqCst) {
                let pending = active_streams.drain();
                for p in pending {
                    match p.upgrade() {
                        Some(p) => {
                            match p.shutdown(net::Shutdown::Both) {
                                Ok(()) => debug!("pending active connection closed"),
                                Err(err) => {
                                    error!("error closing a pending active connection {}", err)
                                }
                            };
                        }
                        None => debug!("pending active connection already dropped"),
                    }
                }
                break;
            };
        }
        info!("server stopped accepting connections");
        Ok(())
    }

    /**
    Forces the server to gracefully shutdown by stop accepting new
    connections. It waits until the ongoing requests are processed.

    # Examples

    ```no_run
    use std::str::FromStr;

    use wruster::Server;
    use wruster::http::{self, Request};
    use wruster::http::Response;
    use wruster::router;
    use wruster::router::HttpHandler;
    let routes = router::Router::new();
    let handler: HttpHandler = Box::new(move |request: &mut Request| {
        let body = &mut request.body.as_mut().unwrap();
        let mut name = String::new();
        body.content.read_to_string(&mut name).unwrap();
        let greetings = format!("hello {}!!", name);
        Response::from_str(&greetings).unwrap()
    });
    routes.add("/", http::HttpMethod::GET, handler);
    let mut server = Server::new();
    server.run("127.0.0.1:8082", routes).unwrap();
    server.shutdown().unwrap();
    ```

    # Errors

    This function will return an error type [`ErrorKind::Other`] if the server
    was not started.
    */
    pub fn shutdown(self) -> ServerResult {
        let handle = match self.handle {
            None => {
                let err = Box::new(Error::new(ErrorKind::Other, "server not started"));
                return Err(err);
            }
            Some(handle) => handle,
        };
        self.stop.as_ref().store(true, Ordering::SeqCst);
        self.poller.unwrap().notify()?;

        match handle.join() {
            Ok(result) => match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    let err = Box::new(Error::new(ErrorKind::Other, error.to_string()));
                    Err(err)
                }
            },
            Err(err) => {
                error!("error waiting for stopping accepting connections {:?}", err);
                let err = Box::new(Error::new(
                    ErrorKind::Other,
                    "error waiting for accepting connections",
                ));
                Err(err)
            }
        }
    }

    /**

     Blocks the current thread until a call to [`Self::shutdown()`] is done.

     # Examples

    ```no_run
     use std::str::FromStr;

     use wruster::Server;
     use wruster::http::{self, Request};
     use wruster::http::Response;
     use wruster::router;
     use wruster::router::HttpHandler;
     let routes = router::Router::new();
     let handler: HttpHandler = Box::new(move |request: &mut Request| {
        let body = &mut request.body.as_mut().unwrap();
        let mut name = String::new();
        body.content.read_to_string(&mut name).unwrap();
         let greetings = format!("hello {}!!", name);
         Response::from_str(&greetings).unwrap()
     });
     routes.add("/", http::HttpMethod::GET, handler);
     let mut server = Server::new();
     server.run("127.0.0.1:8082", routes).unwrap();
     server.wait().unwrap();
     ```

     # Errors

     This function will return the error [`ErrorKind::Other`] if the server
     was not started.
     */
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

fn handle_busy<T>(stream: ObservedStream<T>, timeouts: Timeouts, src_addr: SocketAddr)
where
    T: Stream,
{
    debug!("sending too busy to {}", src_addr);
    let write_timeout = Some(timeouts.write_response_timeout);
    let read_timeout = Some(timeouts.read_request_timeout);
    let shutdown_stream = stream.clone();
    let mut timeout_stream = TimeoutStream::from(stream, read_timeout, write_timeout);
    let mut resp = Response::from_status(StatusCode::ServiceUnavailable);
    if let Err(err) = resp.write(&mut timeout_stream) {
        error!("sending too busy to {}: {}", src_addr, err.to_string())
    }
    let s = &*shutdown_stream;
    if let Err(err) = s.shutdown(net::Shutdown::Both) {
        error!("error closing connection with: {}, {}", src_addr, err);
    }
    debug!("connection with closed")
}

fn handle_conversation<T>(
    mut stream: ObservedStream<T>,
    routes: Arc<Router>,
    timeouts: Timeouts,
    source_addr: SocketAddr,
) where
    T: Stream + 'static,
{
    debug!("handling conversation with {}", source_addr);
    let mut connection_open = true;
    while connection_open {
        let handle_stream = stream.clone();
        connection_open = handle_connection(
            handle_stream,
            Arc::clone(&routes),
            source_addr,
            timeouts.clone(),
        );
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

fn handle_connection<T>(
    stream: ObservedStream<T>,
    routes: Arc<Router>,
    source_addr: SocketAddr,
    timeouts: Timeouts,
) -> bool
where
    T: Stream + 'static,
{
    let connection_open: bool;
    let read_timeout = Some(timeouts.read_request_timeout);
    let write_timeout = Some(timeouts.write_response_timeout);

    let resp_stream = stream.clone();
    let timeout_stream = TimeoutStream::from(stream, read_timeout, write_timeout);

    let (request, mut response) = match Request::read_from(timeout_stream) {
        Ok(mut request) => {
            connection_open = is_connection_persistent(&request);
            let response = run_action(&mut request, routes);
            (Some(request), response)
        }
        Err(err) => match err {
            HttpError::InvalidRequest(err) => {
                error!("error reading request, error info: {}", err);
                connection_open = false;
                let response = Response::from_status(StatusCode::BadRequest);
                (None, response)
            }
            err => {
                debug!("error reading request {:?}", err);
                return false;
            }
        },
    };

    // Ensure the request body (if any) is read.
    if let Some(mut request) = request {
        let body = request.body.as_mut();
        if let Some(body) = body {
            if let Err(err) = body.ensure_read() {
                error!("error reading request body, error info: {}", err);
                return false;
            }
        }
    }

    // Write the response.
    let mut timeout_stream = TimeoutStream::from(resp_stream, read_timeout, write_timeout);
    if let Err(err) = response.write(&mut timeout_stream) {
        error!(
            "error writing response to: {}, error info: {}",
            source_addr, err
        );
        return false;
    };
    connection_open
}

fn run_action(request: &mut Request, routes: Arc<Router>) -> Response {
    let req_path = PathBuf::from(request.uri.clone());
    let normalized = match req_path.normalize() {
        Ok(path) => path,
        Err(err) => {
            let p = req_path.to_str().unwrap_or("unable to get path");
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

/**
Evaluates if a request requires a connection to be [persistent](https://httpwg.org/specs/rfc7230.html#rfc.section.6.3).
*/
fn is_connection_persistent(request: &http::Request) -> bool {
    let value = match request.headers.get("Connection") {
        None => "".to_string(),
        Some(values) => values[0].to_lowercase(),
    };
    if value == "close" {
        return false;
    }

    if request.version == "HTTP/1.1" || request.version == "HTTP/2" {
        return true;
    };

    if request.version == "HTTP/1.0" && value == "keep-alive" {
        return true;
    };
    false
}
