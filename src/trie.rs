// type Children<'a, T> =  [Option<T>; 256];

use std::cell::RefCell;
use std::rc::Rc;
use std::rc;

struct Trie<'a, T> {
    children: RefCell<Vec<Option<Rc<Node<'a, T>>>>>,
    values: RefCell<Vec<T>>,
}

impl<'a, T> Trie<'a, T> {
    fn new() -> Trie<'a, T> {
        let mut children = Vec::new();
        for i in 0..256 {
            children.push(None);
        }
        Trie {
            children: RefCell::new(children),
            values: RefCell::new(Vec::new()),
        }
    }

    fn add_value(&'a mut self, index: &'a [u8], value: T) {
        let mut vals = self.values.borrow_mut();
        vals.push(value);
        let node = Node::<'a, T>::new();
        self.children.borrow_mut()[0]=Some(Rc::new(node));
    }
}

struct Node<'a, T> {
    children: RefCell<Vec<Option<Rc<Node<'a, T>>>>>,
    value: RefCell<Option<&'a T>>,
}

impl<'a, T> Node<'a, T> {
    fn new() -> Self {
        let mut children = Vec::new();
        for i in 0..256 {
            children.push(None);
        }
        Node {
            children: RefCell::new(children),
            value:  RefCell::new(None),
        }
    }

    fn add_value(&self, index: &'a [u8], value: &'a T) {
      if index.len() == 1 {
         *self.value.borrow_mut() = Some(value);
         return;
      }
      let current = index[0];
      let mut children = self.children.borrow_mut();
      //let mut child = children[current as usize].as_ref();
      let child = match children[current as usize].as_ref() {
        None => {
            let new_node = Rc::new(Node::<T>::new());
            let child = Rc::clone(&new_node);
            children[current as usize] =  Some(new_node);
            child
        },
        Some(child) => Rc::clone(child)
      };
      child.add_value(&index[1..], value);
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
        let trie: Trie<&Elem> = Trie {
            children: [None; 256],
        };
    }
}
