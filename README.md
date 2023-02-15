# Wruster

[![Rust](https://github.com/manelmontilla/wruster/actions/workflows/ci.yml/badge.svg)](https://github.com/manelmontilla/wruster/actions/workflows/ci.yml)

Wruster is a web server intended to experiment and learn with backend
components at the HTTP protocol level.

## Objectives

- Allow to experiment with different strategies for managing I/O: thread per
connection, thread per request, etc..

- Include the minimum necessary components to write relatively ``low level`` web
backend machinery, think about: reverse proxies, static content servers or
HTTP Load balancers.

- The performance will only be considered at the amortized time complexity
level, beyond that, it's not an objective to improve the performance of the
different components of the server.

## Status

The project is still in alfa, the public API is particularly in very
early stages and lacks access to many configuration options of some of
the components. That's also true for the documentation, that covers only the
basics for running a ``server`` and executing handlers but not a fine grain
configuration of the behavior of the Server.

## Example

This small program runs a web server listening at: 127.0.0.1:8082, accepts
GET requests at the root path, and returns a response with the payload: ``hello
world``.

```rust
use env_logger::Builder;
use std::process;
use std::str::FromStr;
use std::time::Duration;

use log::LevelFilter;
use wruster_handlers::log_middleware;
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

You can find a more complex example [here](wrustatic/src/main.rs).

## Design

The web server is composed basically of three high level components.

### HTTP Messages plumbing

Contains all the types needed to represent HTTP Messages and to
read/write them ``from`` and ``to`` the wire.

### Router

Allows to the define and search routes. A route is a triplet: (path, http verb, handler)
that defines which action (``handler``) must be executed when a request with a concrete
``path`` and ``verb`` is received.

The current router implementation is backed by a
[Trie](https://en.wikipedia.org/wiki/Trie) structure, so the cost of querying
the path of a route is O(m) where ``m`` is the length in chars of the path.

Also the router is designed to be thread safe only for querying routes,
so the operations related to creating routes must be synchronized.

### Thread pool

Defines the strategy to create and assign threads to execute the Handlers
defined in the routes.

The current implementation allows defining a ``minimum`` and a ``maximum``
number of threads. The minimum defines the number of threads that are allocated
when the pool is created. The maximum defines how many extra threads can be
allocated dynamically when the initial created threads are busy.

### Server

Accepts TCP connections listening in a given address and executes the actions
defined in a set of routes by using a thread pool and a router.

The current implementation of the server uses a thread per connection strategy
and leverages the [polling](https://github.com/smol-rs/polling) lib
to accept connections in a non blocking way.
