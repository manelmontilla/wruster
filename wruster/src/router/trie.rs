#[derive(Debug)]
pub struct Trie<T> {
    children: Vec<Option<Node<T>>>,
}

impl<T> Trie<T> {
    pub fn new() -> Self {
        let children = Node::empty_children();
        Trie { children }
    }

    pub fn add_value(&mut self, key: &[u8], value: T) {
        assert!(!key.is_empty());
        Node::add_value_to_children(&mut self.children, key, value);
    }

    pub fn get_value(&self, key: &[u8]) -> Option<&T> {
        if key.is_empty() {
            return None;
        }
        let pos = key[0] as usize;
        let children = &self.children;
        let child = match &children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.get_value(&key[1..])
    }

    pub fn move_value_out(&mut self, key: &[u8]) -> Option<T> {
        if key.is_empty() {
            return None;
        }
        let pos = key[0] as usize;
        let children = &mut self.children;
        let child = match &mut children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.move_value_out(&key[1..])
    }

    pub fn get_value_prefix<'a>(&'a self, key: &[u8]) -> Option<&T> {
        if key.is_empty() {
            return None;
        }
        let pos = key[0] as usize;
        let children = &self.children;
        let child = match &children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.get_value_prefix(&key[1..], None)
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
        for _ in 0..256 {
            children.push(None);
        }
        children
    }

    fn add_value_to_children(children: &mut Vec<Option<Node<T>>>, key: &[u8], value: T) {
        let next = key[0] as usize;
        if children[next].is_none() {
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
            children,
            value: None,
        }
    }

    fn add_value(&mut self, key: &[u8], value: T) {
        if key.is_empty() {
            self.value = Some(value);
            return;
        }
        Self::add_value_to_children(&mut self.children, key, value);
    }

    fn get_value(&self, key: &[u8]) -> Option<&T> {
        if key.is_empty() {
            return self.value.as_ref();
        }
        let pos = key[0] as usize;
        let children = &self.children;
        let child = match &children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.get_value(&key[1..])
    }

    fn get_value_prefix<'a>(&'a self, key: &[u8], prefix_value: Option<&'a T>) -> Option<&T> {
        if key.is_empty() {
            if self.value.is_none() {
                return prefix_value;
            }
            return self.value.as_ref();
        }
        let pos = key[0] as usize;
        let children = &self.children;
        let child = match &children[pos] {
            None => {
                if self.value.is_some() {
                    return self.value.as_ref();
                }
                return prefix_value;
            }
            Some(node) => node,
        };
        let next_parent = match &self.value {
            None => prefix_value,
            Some(value) => Some(value),
        };
        child.get_value_prefix(&key[1..], next_parent)
    }

    pub fn move_value_out(&mut self, key: &[u8]) -> Option<T> {
        if key.is_empty() {
            return self.value.take();
        }
        let pos = key[0] as usize;
        let children = &mut self.children;
        let child = match &mut children[pos] {
            None => return None,
            Some(node) => node,
        };
        child.move_value_out(&key[1..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trie_adds_node() {
        let mut root = Node::<&str>::new();
        let index = "/a/b/c".as_bytes();
        root.add_value(index, "a");
        assert_eq!(Some(&"a"), root.get_value("/a/b/c".as_bytes()));
    }

    #[test]
    fn trie_add_key_and_values() {
        let mut root = Trie::<Box<dyn Fn(String) -> String>>::new();
        let key = "/a/b/c".as_bytes();
        let action = |param| {
            println!("action executed with param {}", param);
            String::from(param)
        };
        root.add_value(key, Box::new(action));
        let action = root.get_value(key);
        let resp = action.unwrap()(String::from("value passed"));
        assert_eq!(resp, "value passed");
    }

    #[test]
    fn trie_find_prefix() {
        let mut root = Trie::<String>::new();
        let mut key = "/a/b/c/d".as_bytes();
        let mut value = String::from("action for route /a/b/c/d");
        root.add_value(key, value);

        key = "/a/b".as_bytes();
        value = String::from("action for route /a/b");
        root.add_value(key, value);

        let value = root.get_value_prefix("/d".as_bytes());
        assert!(value.is_none());

        let value = root.get_value_prefix("/a/b/c".as_bytes());
        assert_eq!(value.unwrap(), "action for route /a/b");

        let value = root.get_value_prefix("/a/b/c/d".as_bytes());
        assert_eq!(value.unwrap(), "action for route /a/b/c/d");
    }

    #[test]
    fn trie_find_prefix_root() {
        let mut root = Trie::<String>::new();
        let key = "/".as_bytes();
        let value = String::from("action for route /");
        root.add_value(key, value);
        let value = root.get_value_prefix("/example".as_bytes());
        assert_eq!(value.unwrap(), "action for route /");
    }
}
