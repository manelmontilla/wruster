use crate::{Request, Response, StatusCode};
use atomic_refcell::AtomicRefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path;
use std::path::Component;
use crate::trie::Trie;

type Action = Box<dyn Fn(Request) -> Response + Send + Sync>;

pub struct Routes {
    routes: AtomicRefCell<Trie<Action>>,
}

impl Routes {
    pub fn new() -> Routes {
        Routes {
            routes: AtomicRefCell::new(Trie::new()),
        }
    }

    pub fn add(&self, route: Route, action: Action) {
        self.routes.borrow_mut().insert(route, action);
    }

    // pub fn action_for_path(path: String) -> Action {
    //     let path = path::PathBuf::from(path);
    //     let path = path.normalize();
    // }
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
    PATCH,
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

pub fn static_action(dir: String) -> impl Fn(Request) -> Response {
    |req: Request| Response::from_status(StatusCode::Ok)
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
