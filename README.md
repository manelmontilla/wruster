# Wruster

Wruster is a experimental web server.

Even though it´s a fully functional web server is intended to experiment and no
to be used in production.

## Example

This small program runs a web server listening at: , that accepts GET's at the
root and returns a Http Response with the payload ``Hello world``.

```rust
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

You can find a more complex example [here](wrustatic/src/main.rs).

## Objectives

- Allow to experiment with different types for managins I/O: thread per
connection, thread per request, etc..

- Include the minimun necessary components to write relatively ``low level`` web
backend machinery, think about: Reverse Proxies, Static content servers or
Http Load balancers.

- The performance is only taken into account at the amortized time complexity
level, not at the constant level.

## Design

The web server it`s composed basically of three high level components or pseudo-modules.

### HTTP Messages plumbing

Contains all the types needed to represent Http Requests and Responses, and to
read and write them from and to the wire.

### Router

Allows to the define a search routes. A route is a triplet: (path, http verb, handler)
the defines which action (Handler) must be executed when a request with the concrete
path an Http verb is received.

### Theard pool

Defines the strategy to be use to assing threads to execute the Handlers
defined in the routes.

### Server

Accepts TCP connections listening in a given address and executes the proper actions
defined in a set of routes by leveraging the other components.
