#![warn(missing_docs)]

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
use wruster::handlers::log_middleware;
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
   let handler: HttpHandler = log_middleware(Box::new(move |_| {
       Response::from_str("hello world").unwrap()
   }));
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

use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::{io::Write, time};
use std::{net, thread};
use std::error::Error as StdError;

#[macro_use]
extern crate log;

/// Contains a set of helpfull handlers.
pub mod handlers;
/// Contains all the types necessary for dealing with Http messages.
pub mod http;

/// Contains the router to be used in a [`Server`].
pub mod router;
mod thread_pool;
mod timeout_stream;

use http::*;
use polling::{Event, Poller};
use router::{Normalize, Router};

use crate::timeout_stream::TimeoutStream;

/// Defines the default max time for a request to be read
pub const DEFAULT_READ_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);

/// Defines the default max time for a response to be written
pub const DEFAULT_WRITE_RESPONSE_TIMEOUT: time::Duration = time::Duration::from_secs(60);


/// Defines the result type returned from the [``Server``] methods.
pub type ServerResult = Result<(), Box<dyn StdError>>;

/// Defines the timeouts used in [`Server::from_timeouts`].
#[derive(Clone)]
pub struct Timeouts {
    /// Maximun time for a request to be read
    pub read_request_timeout: time::Duration,
    /// Maximun time for a request to be written.
    pub write_response_timeout: time::Duration,
}

/// Represents a web server that can be run passing a [`router::Router`].
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
    Starts a server listening in the specified address and using the given
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
    let handler: HttpHandler = Box::new(move |request: Request| {
        let mut body = request.body.unwrap();
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

    This function will return an error if the address is wrong formatted or
    not free.
    */
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
        let mut pool = thread_pool::Pool::new(4, 20);
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
                let action_stream = match stream.try_clone() {
                    Ok(stream) => stream,
                    Err(err) => {
                        error!("error cloning stream: {}", err.to_string());
                        continue;
                    }
                };
                let action = move || {
                    handle_conversation(action_stream, cconfig, action_timeouts.clone(), src_addr);
                };

                if pool.run(Box::new(action)).is_err() {
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
    let handler: HttpHandler = Box::new(move |request: Request| {
        let mut body = request.body.unwrap();
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

    This function will return an error the error [`ErrorKind::Other`] if the server
    was not started.
    */
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
     let handler: HttpHandler = Box::new(move |request: Request| {
         let mut body = request.body.unwrap();
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

     This function will return an error the error [`ErrorKind::Other`] if the server
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

fn handle_busy(mut stream: net::TcpStream, timeouts: Timeouts, src_addr: SocketAddr) {
    debug!("sending too busy to {}", src_addr);
    let write_timeout = Some(timeouts.write_response_timeout);
    let read_timeout = Some(timeouts.read_request_timeout);
    let mut timeout_stream = TimeoutStream::from(&mut stream, read_timeout, write_timeout);
    let mut resp = Response::from_status(StatusCode::ServiceUnavailable);
    if let Err(err) = resp.write(&mut timeout_stream) {
        error!("sending too busy to {}: {}", src_addr, err.to_string())
    }
    if let Err(err) = stream.shutdown(net::Shutdown::Both) {
        error!("error closing connection with: {}, {}", src_addr, err);
    }
    debug!("connection with closed")
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
    let write_timeout = Some(timeouts.write_response_timeout);

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
            errors::HttpError::Unknown(err) => {
                error!("error reading request, error info: {}", err);
                connection_open = false;
                Response::from_status(StatusCode::BadRequest)
            }
            errors::HttpError::Timeout => return false,
            errors::HttpError::ConnectionClosed => return false,
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

fn is_connection_alive(request: &http::Request) -> bool {
    match request.headers.get("Connection") {
        None => false,
        Some(values) => values
            .iter()
            .any(|value| value.to_lowercase() == "keep-alive"),
    }
}
