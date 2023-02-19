use std::env;
use std::io::Cursor;
use std::process;
use std::str::FromStr;
use std::time::Duration;

use mime::Mime;
use wruster::http::HttpMethod;
use wruster::http::Request;
use wruster::http::Response;
use wruster::http::StatusCode;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::{Server, Timeouts};
use wruster_handlers::log_middleware;

#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: wrusdummy ip/host:port config_file");
        process::exit(1);
    }
    let addr = &args[1];
    let cfg_file = &args[2];
    let router = router::Router::new();
    let routes = config::Routes::from_file(cfg_file).unwrap_or_else(move |err| {
        error!("reading config file {}: {}", cfg_file, err);
        process::exit(1);
    });
    for (name, route) in routes {
        let method = HttpMethod::from_str(&route.method).unwrap_or_else(|err| {
            error!("parsing http method in route {}: {}", &name, err);
            process::exit(1);
        });
        let status = StatusCode::from(route.response.status as usize);
        let path = route.path.clone();
        let content = route.response.content.clone();
        let content_type = Mime::from_str(&route.response.content_type).unwrap_or_else(|err| {
            error!("invalid content type in route {}: {}", &name, err);
            process::exit(1);
        });
        let handler = move |request: &mut Request| -> Response {
            debug!("serving request for route {}", name);
            serve_route(
                content.clone(),
                content_type.clone(),
                status.clone(),
                request,
            )
        };
        let handler: HttpHandler = log_middleware(Box::new(handler));
        router.add(&path, method, handler);
    }
    let timeouts = Timeouts {
        write_response_timeout: Duration::from_secs(5),
        read_request_timeout: Duration::from_secs(5),
    };
    let mut server = Server::from_timeouts(timeouts);
    server.run(addr, router).unwrap_or_else(|err| {
        error!("running wruster {}", err.to_string());
        process::exit(1);
    });
    server.wait().unwrap_or_else(|err| {
        error!("error running wruster {}", err.to_string());
        process::exit(1);
    });
    process::exit(0);
}

fn serve_route(
    content: String,
    content_type: Mime,
    status: StatusCode,
    _: &mut Request,
) -> Response {
    let len = content.len() as u64;
    let mut response = Response::from_content(Cursor::new(content), len, content_type);
    response.status = status;
    response
}

mod config {
    use std::{
        collections::HashMap,
        fs::File,
        io::{self, ErrorKind},
        str::FromStr,
    };

    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
    pub struct Route {
        pub path: String,
        pub method: String,
        pub response: Response,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
    pub struct Response {
        pub status: u16,
        pub content: String,
        #[serde(alias = "type")]
        pub content_type: String,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Routes(HashMap<String, Route>);

    impl Routes {
        pub fn from_file(path: &String) -> Result<Routes, io::Error> {
            let file = File::open(path)?;
            let config: HashMap<String, Route> = serde_yaml::from_reader(file)
                .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
            Ok(Routes(config))
        }
    }

    impl FromStr for Routes {
        type Err = serde_yaml::Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let config: HashMap<String, Route> = serde_yaml::from_str(s)?;
            Ok(Routes(config))
        }
    }

    impl IntoIterator for Routes {
        type Item = (String, Route);

        type IntoIter = std::collections::hash_map::IntoIter<String, Route>;

        fn into_iter(self) -> Self::IntoIter {
            self.0.into_iter()
        }
    }
}
