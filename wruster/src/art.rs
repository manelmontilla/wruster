
use std::ops::Deref;
use std::sync::{Arc, RwLock, Weak, Mutex};
use std::collections::HashMap;
use std::io;

pub trait  TryClone where Self: Sized {
    fn try_clone(&self) -> io::Result<Self>;
}

pub struct Resource<T: TryClone> {
    resource: T,
    root: Option<Weak<RwLock<Self>>>,
    id: usize,
    next_id: usize,
    clones: HashMap<usize, Weak<RwLock<Self>>>
}

impl<T> Resource<T> where T: TryClone{
    pub fn root(elem: T) ->  Arc<RwLock<Self>> {
        let clones = HashMap::new();
        let next_id = 0;
        let root = None;
        let elem = elem;
        let id = 0;
        let res = Resource { root, resource: elem, clones, id, next_id};
        Arc::new(RwLock::new(res))
    }

    pub fn child(root:Weak<RwLock<Resource<T>>>, id: usize,  elem: T) ->  Arc<RwLock<Self>> {
        let clones = HashMap::new();
        let next_id = 0;
        let elem = elem;
        let root = Some(root);
        let res = Resource { root, resource: elem, clones, id, next_id};
        Arc::new(RwLock::new(res))
    }

    pub fn try_clone(root: &Arc<RwLock<Self>>) -> io::Result<Arc<RwLock<Self>>> {
        let mut r = root.write().unwrap();
        let clone = r.try_clone()?;
        let id = r.next_id;
        let child_weak_root = Arc::downgrade(root);
        let child_resource = Resource::child(child_weak_root, id, clone);
        let clone_resource = Arc::downgrade(&child_resource);
        match r.clones.insert(id, clone_resource) {
            Some(_) => unreachable!(),
            None => {
                 r.next_id = id + 1;
                Ok(child_resource)
            }
        }
    }

    fn remove_child(&mut self, id: usize) {
        self.clones.remove(&id);
    }

}

impl<T> Drop for Resource<T> where T: TryClone {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
             if let Some(root) = root.upgrade() {
                    // TODO: do not panic here if lock is poisoned.
                    let mut parent = root.write().unwrap();
                    parent.remove_child(self.id)
             }
        }
    }
}

impl<T> Deref for Resource<T> where T: TryClone {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

pub struct Art<T> {
    resources: Vec<T>
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    
    #[test]
    fn tracks_resources() {
        let r = Dummy{data:1};
        let root = Resource::root(r);
        let t_child = Resource::try_clone(&root).unwrap();
        let handle = thread::spawn(move || {
            let t_child_1_1 = Resource::try_clone(&t_child).unwrap();
            let t_child_1_2 = Resource::try_clone(&t_child).unwrap();
            assert!(t_child.read().unwrap().clones.len() == 2);
            drop(t_child_1_1);
            drop(t_child_1_2);
            assert!(t_child.read().unwrap().clones.len() == 0);
        });
        handle.join().unwrap();
        print!("data: {:}", *&root.read().unwrap().data);
    }

    struct Dummy {
        data: usize
    }

    impl TryClone for Dummy {
        fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Dummy{data: self.data})
    }
    }
}
