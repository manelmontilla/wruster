use atomic_refcell::AtomicRefCell;
use std::path;
use std::path::Component;
use std::sync::Arc;

mod trie;
use crate::http::{HttpMethod, Request, Response};
use trie::Trie;

/// Defines a type alias for the Http Handlers associated with a [``Router`].
pub type HttpHandler = Box<dyn Fn(&mut Request) -> Response + Send + Sync>;

/// Router holds the Handlers that will attend a set of the http routes and methods.
pub struct Router {
    routes: AtomicRefCell<Trie<MethodHandlers>>,
}

impl Router {
    /**
    Creates a new [`Router`] empty Router.

    # Examples

    ```
    use wruster::router::Router;

    let router = Router::new();
    ```
    */
    pub fn new() -> Router {
        Router {
            routes: AtomicRefCell::new(Trie::new()),
        }
    }

    /// Adds a route; a route consists on a path, a Http verb and a handler
    /// that will attend the requests for that path and Http Verb. Note that
    /// the router will select the most concrete handler that is, at least,
    /// registered for a path that is parent of the request path. For instance:
    /// if a handler has been registered for GET's in the path "/a", a GET
    /// request to the path "/a/b" the will be attended with that Handler.
    ///
    /// # Examples
    /// TODO
    ///
    pub fn add(&self, route: &str, method: HttpMethod, action: HttpHandler) {
        // We prioritize keeping the code of the Trie simpler over adding the
        // routes faster.
        let mut routes = self.routes.borrow_mut();
        let router_handlers = match routes.move_value_out(route.as_bytes()) {
            None => MethodHandlers::new(),
            Some(route_actions) => route_actions,
        };
        router_handlers.actions.borrow_mut()[method as usize] = Some(Arc::new(action));
        routes.add_value(&route.as_bytes(), router_handlers);
    }

    #[allow(dead_code)]
    fn get(&self, route: &str, method: HttpMethod) -> Option<Arc<HttpHandler>> {
        let routes = self.routes.borrow();
        let method_actions = match routes.get_value(route.as_bytes()) {
            None => return None,
            Some(actions) => actions,
        };
        method_actions.get_action(method)
    }

    pub(crate) fn get_prefix(&self, route: String, method: HttpMethod) -> Option<Arc<HttpHandler>> {
        let routes = self.routes.borrow();
        let method_actions = match routes.get_value_prefix(route.as_bytes()) {
            None => return None,
            Some(actions) => actions,
        };
        method_actions.get_action(method)
    }
}

impl Default for Router {
    fn default() -> Self {
        Router::new()
    }
}

pub(crate) struct MethodHandlers {
    actions: AtomicRefCell<Vec<Option<Arc<HttpHandler>>>>,
}

impl MethodHandlers {
    fn new() -> MethodHandlers {
        let mut actions = Vec::<Option<Arc<HttpHandler>>>::new();
        for _ in 0..HttpMethod::get_last() as usize + 1 {
            actions.push(None);
        }
        MethodHandlers {
            actions: AtomicRefCell::new(actions),
        }
    }

    fn get_action(&self, method: HttpMethod) -> Option<Arc<HttpHandler>> {
        let actions = self.actions.borrow();
        actions[method as usize]
            .as_ref()
            .map(|action| Arc::clone(action))
    }
}

pub(crate) trait Normalize
where
    Self: std::marker::Sized,
{
    fn normalize(&self) -> Result<Self, &str>;
}

impl Normalize for path::PathBuf {
    fn normalize(&self) -> Result<Self, &str> {
        let mut normalized = path::PathBuf::new();
        if !self.has_root() {
            return Err("invalid path");
        }
        for component in self.components() {
            match component {
                Component::RootDir => normalized.push(Component::RootDir),
                Component::Prefix(_) => normalized.push(Component::RootDir),
                Component::CurDir => continue,
                Component::ParentDir => {
                    match normalized.parent() {
                        None => {
                            return Err("invalid path");
                        }
                        Some(parent) => {
                            let mut new_parent = path::PathBuf::new();
                            new_parent.push(parent);
                            normalized = new_parent;
                        }
                    };
                }
                Component::Normal(dir) => {
                    normalized.push(dir);
                }
            };
        }
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use crate::http::{headers::Headers, Body};
    use std::{io::Cursor, path::PathBuf, str::FromStr};

    use super::*;
    #[test]
    fn normalizes_path() {
        // Returns error if the path is not absolute.
        let p = PathBuf::from("a/..");
        assert_eq!(Err("invalid path"), p.normalize());

        // Returns error if path is above the root.
        let p = PathBuf::from("/../a/..");
        assert_eq!(Err("invalid path"), p.normalize());

        // Normalizes the path properly.
        let p = PathBuf::from("/a/../b//.././");
        let res: PathBuf = [r"/"].iter().collect();
        assert_eq!(Ok(res), p.normalize());

        // Normalizes the path properly.
        let p = PathBuf::from("/a/../b/c/.././");
        let res: PathBuf = [r"/b"].iter().collect();
        assert_eq!(Ok(res), p.normalize());

        // Removes an ending separator.
        let p = PathBuf::from("/a/");
        let res: PathBuf = [r"/a"].iter().collect();
        assert_eq!(Ok(res), p.normalize());
    }

    #[test]
    fn routes_add_and_get() {
        let routes = Router::new();
        let action: Box<dyn Fn(&mut Request) -> Response + Sync + Send> =
            Box::new(|req: &mut Request| {
                let mut content = String::new();
                req.body
                    .as_mut()
                    .unwrap()
                    .content
                    .read_to_string(&mut content)
                    .unwrap();
                Response::from_str(&content).unwrap()
            });
        routes.add("/a/b", HttpMethod::GET, action);
        let action = routes.get("/a/b", HttpMethod::GET);
        let action = action.unwrap();
        let content = "content";
        let body = Body::new(
            Some(mime::TEXT_PLAIN),
            content.len() as u64,
            Box::new(Cursor::new(content)),
        );
        let mut request = Request {
            body: Some(body),
            method: HttpMethod::POST,
            uri: String::from("/"),
            version: String::from("HTTP/1.1"),
            headers: Headers::new(),
        };
        let resp = action(&mut request);
        let mut resp_body = resp.body.unwrap();
        let mut content = Vec::<u8>::new();
        resp_body.write(&mut content).unwrap();
        assert_eq!(Vec::from("content"), content);
    }

    #[test]
    fn routes_most_concrete() {
        let routes = Router::new();
        let a_b_action: Box<dyn Fn(&mut Request) -> Response + Sync + Send> =
            Box::new(|_: &mut Request| Response::from_str(&"/a/b").unwrap());
        routes.add("/a/b", HttpMethod::GET, a_b_action);

        let a_action: Box<dyn Fn(&mut Request) -> Response + Sync + Send> =
            Box::new(|_: &mut Request| Response::from_str(&"/a").unwrap());
        routes.add("/a", HttpMethod::GET, a_action);

        let action = routes.get("/a/b", HttpMethod::GET);
        let action = action.unwrap();

        // Check a request to /a/b is handled by the /a/b action.
        let mut request = Request {
            body: None,
            method: HttpMethod::GET,
            uri: String::from("/a/b"),
            version: String::from("HTTP/1.1"),
            headers: Headers::new(),
        };
        let resp = action(&mut request);
        let mut resp_body = resp.body.unwrap();
        let mut content = Vec::<u8>::new();
        resp_body.write(&mut content).unwrap();
        assert_eq!(Vec::from("/a/b"), content);

        // Check a request to /a is handled by the /a action.
        let action = routes.get("/a", HttpMethod::GET);
        let action = action.unwrap();
        let mut request = Request {
            body: None,
            method: HttpMethod::GET,
            uri: String::from("/a"),
            version: String::from("HTTP/1.1"),
            headers: Headers::new(),
        };
        let resp = action(&mut request);
        let mut resp_body = resp.body.unwrap();
        let mut content = Vec::<u8>::new();
        resp_body.write(&mut content).unwrap();
        assert_eq!(Vec::from("/a"), content);
    }

    #[test]
    fn routes_disctinct_method_same_path() {
        let routes = Router::new();
        let action_body = |_: &mut Request| unimplemented!();
        let action: Box<dyn Fn(&mut Request) -> Response + Sync + Send> =
            Box::new(action_body.clone());
        routes.add("/a/b", HttpMethod::GET, action);
        let action: Box<dyn Fn(&mut Request) -> Response + Sync + Send> = Box::new(action_body);
        routes.add("/a/b", HttpMethod::POST, action);
        _ = routes.get_prefix("/a/b".into(), HttpMethod::GET).unwrap();
        _ = routes
            .get_prefix("/a/b/c".into(), HttpMethod::POST)
            .unwrap();
    }
}
