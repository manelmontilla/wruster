use std::path::PathBuf;
use std::str::FromStr;

use crate::http::headers::*;
use crate::http::*;
use crate::routes::*;
use crate::trie::*;

#[test]
fn normalizes_path() {
    // Returns error if the path is not absolute.
    let p = PathBuf::from("a/..");
    assert_eq!(Err("invalid path"), p.normalize());

    // Returns error if path is above the root.
    let p = PathBuf::from("/../a/..");
    assert_eq!(Err("invalid path"), p.normalize());

    // Normalizes the path properly.
    let p = PathBuf::from("/a/../b//.././");
    let res: PathBuf = [r"/"].iter().collect();
    assert_eq!(Ok(res), p.normalize());

    // Normalizes the path properly.
    let p = PathBuf::from("/a/../b/c/.././");
    let res: PathBuf = [r"/b"].iter().collect();
    assert_eq!(Ok(res), p.normalize());

    // Removes an ending separator.
    let p = PathBuf::from("/a/");
    let res: PathBuf = [r"/a"].iter().collect();
    assert_eq!(Ok(res), p.normalize());
}

#[test]
fn routes_add_and_get() {
    let routes = Routes::new();
    let action = Box::new(|req: Request| {
        let content = String::from_utf8_lossy(&req.body);
        Response::from_str(&content).unwrap()
    });
    routes.add("/a/b", HttpMethod::GET, action);
    let action = routes.get("/a/b", HttpMethod::GET);
    let action = action.unwrap();
    let request = Request {
        body: Vec::from("content"),
        method: HttpMethod::POST,
        uri: String::from("/"),
        version: String::from("HTTP/1.1"),
        headers: HttpHeaders::new(),
    };
    let resp = action(request);
    let mut resp_body = resp.body.unwrap();
    let mut content = Vec::<u8>::new();
    resp_body.write_content(&mut content).unwrap();
    assert_eq!(Vec::from("content"), content);
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
    println!("{:?}", root);
    let value = root.get_value_prefix("/example".as_bytes());
    assert_eq!(value.unwrap(), "action for route /");
}
