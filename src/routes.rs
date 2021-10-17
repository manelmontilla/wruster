use atomic_refcell::AtomicRefCell;
use std::path;
use std::path::Component;
use std::sync::Arc;

use crate::trie::Trie;
use crate::{HttpMethod, Request, Response};

type Action = Box<dyn Fn(Request) -> Response + Send + Sync>;

pub struct Routes {
    routes: AtomicRefCell<Trie<MethodActions>>,
}

impl Routes {
    pub fn new() -> Routes {
        Routes {
            routes: AtomicRefCell::new(Trie::new()),
        }
    }

    pub fn add(&self, route: String, method: HttpMethod, action: Action) {
        // We priorize keeping the code of the Trie simpler over adding the
        // routes faster.
        let mut routes = self.routes.borrow_mut();
        let route_actions = match routes.move_value_out(route.as_bytes()) {
            None => MethodActions::new(),
            Some(route_actions) => route_actions,
        };
        route_actions.actions.borrow_mut()[method as usize] = Some(Arc::new(action));
        routes.add_value(&route.as_bytes(), route_actions);
    }

    pub fn get(&self, route: String, method: HttpMethod) -> Option<Arc<Action>> {
        let routes = self.routes.borrow();
        let method_actions = match routes.get_value(route.as_bytes()) {
            None => return None,
            Some(actions) => actions,
        };
        method_actions.get_action(method)
    }

    pub fn get_prefix(&self, route: String, method: HttpMethod) -> Option<Arc<Action>> {
        let routes = self.routes.borrow();
        let method_actions = match routes.get_value_prefix(route.as_bytes()) {
            None => return None,
            Some(actions) => actions,
        };
        method_actions.get_action(method)
    }
}

pub struct MethodActions {
    actions: AtomicRefCell<Vec<Option<Arc<Action>>>>,
}

impl MethodActions {
    fn new() -> MethodActions {
        let mut actions = Vec::<Option<Arc<Action>>>::new();
        for _ in 0..HttpMethod::get_last() as usize + 1 {
            actions.push(None);
        }
        MethodActions {
            actions: AtomicRefCell::new(actions),
        }
    }

    fn get_action(&self, method: HttpMethod) -> Option<Arc<Action>> {
        let actions = self.actions.borrow();
        match &actions[method as usize] {
            None => None,
            Some(action) => Some(Arc::clone(action)),
        }
    }
}

pub struct RouteAction {
    pub method: HttpMethod,
    pub action: Action,
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
