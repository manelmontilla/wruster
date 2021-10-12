// type Children<'a, T> =  [Option<T>; 256];

use std::cell::RefCell;
use std::rc;
use std::rc::Rc;

pub struct Trie<T> {
    children: Vec<Option<Node<T>>>,
}

impl<T> Trie<T> {
    pub fn new() -> Self {
        let children = Node::empty_children();
        Trie { children: children }
    }

    pub fn add_value(&mut self, key: &[u8], value: T) {
        assert!(key.len() > 0);
        Node::add_value_to_children(&mut self.children, key, value);
    }

    pub fn get_value(&self, index: &[u8]) -> Option<&T> {
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

    #[test]
    fn adds_node() {
        let mut root = Node::<&str>::new();
        let index = "/a/b/c".as_bytes();
        root.add_value(index, "a");
        println!("value {:?}", root.get_value("/a".as_bytes()));
    }

    #[test]
    fn trie_add_key_and_values() {
        let mut root = Trie::<Box<dyn Fn(String)->String>>::new();
        let key = "/a/b/c".as_bytes();
        let action = |param| {
           println!("action executed with param {}",param);
           String::from(param)
        };
        root.add_value(key, Box::new(action));
        let action = root.get_value(key);
        let resp = action.unwrap()(String::from("value passed"));
        assert_eq!(resp,"value passed");
    }
}
