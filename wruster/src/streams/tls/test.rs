use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    thread,
};

use super::test_utils::*;
use super::*;
use crate::test_utils::{get_free_port, TestTLSClient};

#[test]
fn server_receives_data() {
    let cert = load_test_certificate().unwrap();
    let key = load_test_private_key().unwrap();
    let port = get_free_port();
    let addr = format!("localhost:{}", port);
    let listener = TcpListener::bind(addr).unwrap();
    let handler = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut server_stream = Stream::new(stream, key, cert).unwrap();
        let mut reader = BufReader::new(&mut server_stream);
        let mut content = Vec::new();
        reader.read_until(b' ', &mut content).unwrap();
        String::from_utf8_lossy(&content).to_string()
    });
    let mut client = TestTLSClient::new("localhost", port).unwrap();
    client.write("test ".as_bytes()).unwrap();
    let received = handler.join().unwrap();
    assert_eq!("test ", received)
}
