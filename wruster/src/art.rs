
use std::ops::Deref;
use std::sync::{Arc, RwLock, Weak, Mutex};
use std::collections::HashMap;
use std::io;

use atomic_refcell::AtomicRefCell;

pub trait  TryClone where Self: Sized {
    fn try_clone(&self) -> io::Result<Self>;
}


pub struct Resource<T: TryClone> {
    elem: T,
    parent: Option<Weak<RwLock<Self>>>,
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
        let res = Resource { parent: root, elem, clones, id, next_id};
        Arc::new(RwLock::new(res))
    }

    pub fn child(root:Weak<RwLock<Resource<T>>>, id: usize,  elem: T) ->  Arc<RwLock<Self>> {
        let clones = HashMap::new();
        let next_id = 0;
        let elem = elem;
        let root = Some(root);
        let res = Resource { parent: root, elem, clones, id, next_id};
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

    fn child_dropped(&mut self, id: usize) {
        self.clones.remove(&id);
    }

}

impl<T> Drop for Resource<T> where T: TryClone {
    fn drop(&mut self) {
        if let Some(root) = self.parent.take() {
             if let Some(root) = root.upgrade() {
                    // TODO: do not panic here if lock is poisoned.
                    let mut parent = root.write().unwrap();
                    parent.child_dropped(self.id)
             }
        }
    }
}

impl<T> Deref for Resource<T> where T: TryClone {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.elem
    }
}

pub struct Art<T> {
    resources: Vec<T>
}

pub trait Dropped {
    fn dropped(&self, key: usize);
}

pub struct ResourceListItem<T, K> where K: Dropped {
    item: T,
    parent_key: usize,
    parent: Weak<K>,
}

impl<T, K>  Drop for ResourceListItem<T, K> where K: Dropped {
    fn drop(&mut self) {
       if let Some(parent) = self.parent.upgrade() {
                    parent.dropped(self.parent_key)
             }
    }
}

pub struct ResourceList<T: Sized> {
    items: RwLock<HashMap<usize, T>>
}

impl<T> ResourceList<T> where T: Sized {
    fn new() -> Self {
        return ResourceList { items: RwLock::new(HashMap::new()) }
    }
    pub fn add(&self, item: T, key: usize) {
        let mut items = self.items.write().unwrap();
        items.insert(key, item);
    }
    fn drain(&self) -> Vec<T> {
        let mut items = self.items.write().unwrap();
        items.drain().map(|(_, v)| v).collect()
    }
}


impl<T> Dropped for ResourceList<T> where T: Sized {
    fn dropped(&self, key: usize) {
        let mut items = self.items.write().unwrap();
        items.remove(&key);
    }
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

    #[test]
    fn list_tracks_resources() {
        let r = Dummy{data:1};
        let list: ResourceList<usize> = ResourceList::new();
        let list = Arc::new(list);
        list.add(1, 1);
        let parent = Arc::downgrade(&list);
        let item_root = Arc::new(
            ResourceListItem{
            item: r,
            parent: parent,
            parent_key: 1,
        });
        let item1 = Arc::clone(&item_root);
        let handle = thread::spawn(move || {
          let item2  = Arc::clone(&item1);
          drop(item2);
          drop(item1);
        });
        handle.join().unwrap();
        println!("data before {:}", list.items.read().unwrap().len());
        println!("data in item_root {:}", item_root.item.data);
        drop(item_root);
        println!("data: after {:}", list.items.read().unwrap().len());
    }
}
