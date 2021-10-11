use crate::http::*;
use crate::routes::*;
use std::io::BufReader;
use std::path::PathBuf;

const request: &str = "GET / HTTP/1.1\r\nHost: ww.google.es\r\n\r\n";

#[test]
fn runs_an_action() {
    let handler = |req: Request| {
        println!("help");
        Response::from_status(StatusCode::Ok)
    };
    let dir = String::from("/a/b");
    let action = static_action(dir);
    println!("request {}", request);
    let mut reader = BufReader::new(request.as_bytes());
    let req = Request::from(&mut reader).unwrap();
    let response = action(req);
    println!("{:?}", response);
}
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

    // Removes and ending separator.
    let p = PathBuf::from("/a/");
    let res: PathBuf = [r"/a"].iter().collect();
    assert_eq!(Ok(res), p.normalize());
}
