use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use atomic_refcell::AtomicRefCell;
use crate::{Request, Response, StatusCode};

type Action =  Box<dyn Fn(Request) -> Response + Send + Sync>;

pub struct Routes {
    routes: AtomicRefCell<HashMap<Route, Action>>,
}

impl Routes {
    pub fn new() -> Routes {
        Routes {
            routes: AtomicRefCell::new(HashMap::new()),
        }
    }

    pub fn add(&self, route: Route, action: Action) {
        self.routes.borrow_mut().insert(route, action);
    }

    pub fn action_for_path(path: String) -> Action {

    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Route {
    pub path: String,
    pub method: HttpMethod,
}


#[derive(Debug)]
pub enum HttpMethod {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH
}

impl PartialEq for HttpMethod {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}
impl Eq for HttpMethod {}

impl Hash for HttpMethod {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

fn static_action(dir: String) -> impl Fn(Request) -> Response {
    move |req: Request| {
        let dir = &dir;
        Response::from_status(StatusCode::Ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    const request: &str = "GET / HTTP/1.1\r\nHost: ww.google.es\r\n\r\n";

    #[test]
    fn runs_an_action() {
        let handler = |req: Request| {
            println!("help");
            Response::from_status(StatusCode::Ok)
        };
        let dir = String::from("/a/b");
        let action = static_action(dir);
        println!("request {}", request);
        let mut reader = BufReader::new(request.as_bytes());
        let req = Request::from(&mut reader).unwrap();
        let response = action(req);
        println!("{:?}", response);
    }
}
