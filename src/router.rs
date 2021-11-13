use atomic_refcell::AtomicRefCell;
use std::path;
use std::path::Component;
use std::sync::Arc;

use crate::trie::Trie;
use crate::{HttpMethod, Request, Response};

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
        actions[method as usize].as_ref().map(|action| Arc::clone(action))
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
