use std::collections::HashMap;
use std::io;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock, Weak};

pub trait Dropped {
    fn dropped(&self, key: usize);
}

pub struct ResourceListItem<T, K>
where
    K: Dropped,
{
    item: T,
    parent_key: usize,
    parent: Weak<K>,
}

impl<T, K> Drop for ResourceListItem<T, K>
where
    K: Dropped,
{
    fn drop(&mut self) {
        if let Some(parent) = self.parent.upgrade() {
            parent.dropped(self.parent_key)
        }
    }
}

impl<T, K> Deref for ResourceListItem<T, K>
where
    K: Dropped,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T, K> DerefMut for ResourceListItem<T, K>
where
    K: Dropped,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

pub struct ResourceList<T: Sized> {
    pub items: RwLock<HashMap<usize, T>>,
}

impl<T> ResourceList<T>
where
    T: Sized,
{
    pub fn new() -> Self {
        return ResourceList {
            items: RwLock::new(HashMap::new()),
        };
    }

    pub fn track(
        list: &Arc<ResourceList<T>>,
        item: T,
    ) -> Arc<ResourceListItem<T, ResourceList<T>>> {
        let parent = Arc::downgrade(list);
        Arc::new(ResourceListItem {
            item: item,
            parent: parent,
            parent_key: 1,
        })
    }

    pub fn add(&self, item: T, key: usize) {
        let mut items = self.items.write().unwrap();
        items.insert(key, item);
    }

    pub fn drain(&self) -> Vec<T> {
        let mut items = self.items.write().unwrap();
        items.drain().map(|(_, v)| v).collect()
    }
}

impl<T> Dropped for ResourceList<T>
where
    T: Sized,
{
    fn dropped(&self, key: usize) {
        let mut items = self.items.write().unwrap();
        items.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    use super::*;

    struct Dummy {
        data: AtomicUsize,
    }

    impl Dummy {
        pub fn signal(&self) {
            self.data.store(0, Ordering::SeqCst)
        }
    }

    #[test]
    fn list_drops_tracked_resources() {
        let r = Arc::new(Dummy {
            data: AtomicUsize::new(1),
        });
        let list: ResourceList<Weak<Dummy>> = ResourceList::new();
        let list = Arc::new(list);
        list.add(Arc::downgrade(&r), 1);
        let parent = Arc::downgrade(&list);
        let item_root = Arc::new(ResourceListItem {
            item: r,
            parent: parent,
            parent_key: 1,
        });
        let item1 = Arc::clone(&item_root);
        let handle = thread::spawn(move || {
            let item2 = Arc::clone(&item1);
            drop(item2);
            drop(item1);
        });
        handle.join().unwrap();
        println!("data before {:}", list.items.read().unwrap().len());
        let _ = item_root.parent_key;
        drop(item_root);
        println!("data: after {:}", list.items.read().unwrap().len());
    }

    // #[test]
    //  fn tracks_resources() {
    //     let r = Dummy { data: 1 };
    //     let root = Resource::root(r);
    //     let t_child = Resource::try_clone(&root).unwrap();
    //     let handle = thread::spawn(move || {
    //         let t_child_1_1 = Resource::try_clone(&t_child).unwrap();
    //         let t_child_1_2 = Resource::try_clone(&t_child).unwrap();
    //         assert!(t_child.read().unwrap().clones.len() == 2);
    //         drop(t_child_1_1);
    //         drop(t_child_1_2);
    //         assert!(t_child.read().unwrap().clones.len() == 0);
    //     });
    //     handle.join().unwrap();
    //     print!("data: {:}", *&root.read().unwrap().data);
    // }
}
