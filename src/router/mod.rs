use atomic_refcell::AtomicRefCell;
use std::path;
use std::path::Component;
use std::sync::Arc;

mod trie;
use crate::{HttpMethod, Request, Response};
use trie::Trie;

type HttpHandler = Box<dyn Fn(Request) -> Response + Send + Sync>;

pub struct Router {
    routes: AtomicRefCell<Trie<MethodHandlers>>,
}

impl Router {
    pub fn new() -> Router {
        Router {
            routes: AtomicRefCell::new(Trie::new()),
        }
    }

    pub fn add(&self, route: &str, method: HttpMethod, action: HttpHandler) {
        // We priorize keeping the code of the Trie simpler over adding the
        // routes faster.
        let mut routes = self.routes.borrow_mut();
        let router_handlers = match routes.move_value_out(route.as_bytes()) {
            None => MethodHandlers::new(),
            Some(route_actions) => route_actions,
        };
        router_handlers.actions.borrow_mut()[method as usize] = Some(Arc::new(action));
        routes.add_value(&route.as_bytes(), router_handlers);
    }

    pub fn get(&self, route: &str, method: HttpMethod) -> Option<Arc<HttpHandler>> {
        let routes = self.routes.borrow();
        let method_actions = match routes.get_value(route.as_bytes()) {
            None => return None,
            Some(actions) => actions,
        };
        method_actions.get_action(method)
    }

    pub fn get_prefix(&self, route: String, method: HttpMethod) -> Option<Arc<HttpHandler>> {
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

pub struct MethodHandlers {
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

pub struct MethodHanlder {
    pub method: HttpMethod,
    pub action: HttpHandler,
}

pub trait Normalize
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
    use crate::http::{Body, headers::HttpHeaders};
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
        let action = Box::new(|req: Request| {
            let mut content = String::new();
            req.body.unwrap().content.read_to_string(&mut content).unwrap();
            Response::from_str(&content).unwrap()
        });
        routes.add("/a/b", HttpMethod::GET, action);
        let action = routes.get("/a/b", HttpMethod::GET);
        let action = action.unwrap();
        let content = "content";
        let request = Request {
            body: Some(
                Body{
                content: Box::new(Cursor::new(content)),
                content_type: mime::TEXT_PLAIN,
                content_length: content.len() as u64,
            }),
            method: HttpMethod::POST,
            uri: String::from("/"),
            version: String::from("HTTP/1.1"),
            headers: HttpHeaders::new(),
        };
        let resp = action(request);
        let mut resp_body = resp.body.unwrap();
        let mut content = Vec::<u8>::new();
        resp_body.write_content(&mut content).unwrap();
        assert_eq!(Vec::from("content"), content);
    }
}
