// type Children<'a, T> =  [Option<T>; 256];

use std::cell::RefCell;
use std::rc;
use std::rc::Rc;

struct Trie<T> {
    children: Vec<Option<Node<T>>>,
}

impl<T> Trie<T> {
    fn new() -> Self {
        let children = Node::empty_children();
        Trie { children: children }
    }

    fn add_value(&mut self, key: &[u8], value: T) {
        assert!(key.len() > 0);
        Node::add_value_to_children(&mut self.children, key, value);
    }

    fn get_value(&self, index: &[u8]) -> Option<&T> {
        if index.len() == 0 {
            return None;
        }
        let pos = index[0] as usize;
        let children = &self.children;
        let child = match &children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.get_value(&index[1..])
    }
}

#[derive(Debug)]
struct Node<T> {
    children: Vec<Option<Node<T>>>,
    value: Option<T>,
}

impl<T> Node<T> {
    fn empty_children() -> Vec<Option<Node<T>>> {
        let mut children = Vec::new();
        for i in 0..256 {
            children.push(None);
        }
        children
    }

    fn add_value_to_children(children: &mut Vec<Option<Node<T>>>, key: &[u8], value: T) {
        let next = key[0] as usize;
        if let None = children[next] {
            let new_node = Node::<T>::new();
            children[next] = Some(new_node);
        };
        let mut child = children[next].take().unwrap();
        child.add_value(&key[1..], value);
        children[next] = Some(child);
    }

    fn new() -> Self {
        let children = Self::empty_children();
        Node {
            children: children,
            value: None,
        }
    }

    fn add_value(&mut self, key: &[u8], value: T) {
        if key.len() == 0 {
            self.value = Some(value);
            return;
        }
        Self::add_value_to_children(&mut self.children, key, value);
    }

    fn get_value(&self, index: &[u8]) -> Option<&T> {
        if index.len() == 0 {
            return self.value.as_ref();
        }
        let pos = index[0] as usize;
        let children = &self.children;
        let child = match &children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.get_value(&index[1..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Elem {
        pub path: String,
        pub method: Option<usize>,
    }

    #[test]
    fn adds_node() {
        let mut root = Node::<&str>::new();
        let index = "/a/b/c".as_bytes();
        root.add_value(index, "a");
        println!("value {:?}", root.get_value("/a".as_bytes()));
    }
}
