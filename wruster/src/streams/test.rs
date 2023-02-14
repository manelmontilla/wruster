use super::*;
use crate::streams::timeout_stream::TimeoutStream;
use crate::test_utils::{
    build_tls_test_client_config, load_private_key, load_test_certificate, TestTLSClient,
};

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::thread;

use std::time::Duration;
use test_utils::{get_free_port, TcpClient};

#[test]
fn shutdown_stops_reading() {
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let cstream = Arc::new(CancellableStream::new(stream).unwrap());
        let cstream2 = Arc::clone(&cstream);
        let m = AtomicBool::new(true);
        let sm = &m;
        let result = thread::scope(|s| {
            let worker_handle = s.spawn(move || {
                let mut content = Vec::new();
                let s = cstream.as_ref();
                let mut reader = BufReader::new(s);
                reader.read_until(b't', &mut content).unwrap();
                sm.store(false, Ordering::SeqCst);

                let err = reader
                    .read_until(b't', &mut content)
                    .expect_err("expected error");
                assert_eq!(err.kind(), ErrorKind::NotConnected)
            });
            while m.load(Ordering::SeqCst) {
                thread::yield_now();
            }
            let s = cstream2.as_ref();
            s.shutdown(Shutdown::Both).unwrap();
            worker_handle.join().unwrap()
        });
        result
    });

    let mut client = TcpClient::connect(addr.to_string()).unwrap();
    thread::sleep(Duration::from_secs(1));
    client.send("t".as_bytes()).unwrap();
    handle.join().unwrap();
}

#[test]
fn read_reads_data() {
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut cstream = CancellableStream::new(stream).unwrap();
        let mut reader = BufReader::new(&mut cstream);
        let mut content = Vec::new();
        reader.read_until(b' ', &mut content).unwrap();
        String::from_utf8_lossy(&content).to_string()
    });

    let mut client = TcpClient::connect(addr.to_string()).unwrap();
    thread::sleep(Duration::from_secs(1));
    client.send("test  ".as_bytes()).unwrap();
    let received = handle.join().unwrap();
    assert_eq!(received, "test ".to_string());
}

#[test]
fn read_honors_timeout() {
    //env::set_var("RUST_LOG", "debug");
    env_logger::init();
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let read_timeout = Duration::from_secs(2);
    let expected_timeout = read_timeout.clone();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut cstream = CancellableStream::new(stream).unwrap();
        cstream.set_read_timeout(Some(expected_timeout)).unwrap();
        let mut reader = BufReader::new(&mut cstream);
        let mut content = Vec::new();
        reader
            .read_until(b' ', &mut content)
            .expect_err("expected timeout")
    });

    let client = TcpClient::connect(addr.to_string()).unwrap();
    let received_err = handle.join().unwrap();
    drop(client);
    assert_eq!(received_err.kind(), io::ErrorKind::TimedOut)
}

#[test]
fn write_writes_data() {
    let data = "test ";
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let server_data = data.clone();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut cstream = CancellableStream::new(stream).unwrap();
        let data = server_data.as_bytes();
        cstream.write(&data)
    });

    let mut client = TcpClient::connect(addr.to_string()).unwrap();
    let bytes_sent = handle
        .join()
        .unwrap()
        .expect("expected data to be written correctly");
    assert_eq!(bytes_sent, data.len());

    let mut reader = BufReader::new(&mut client);
    let mut content = Vec::new();
    reader
        .read_until(b' ', &mut content)
        .expect("expect data to available");
    let content = String::from_utf8(content).expect("expect data to be valid");
    assert_eq!(content, "test ".to_string());
}

#[test]
fn test_shutdown_list() {
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let read_timeout = Duration::from_secs(3);
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let cstream = CancellableStream::new(stream).unwrap();
        let track_list = TrackedStreamList::new();
        let stream_tracked = TrackedStreamList::track(&track_list, cstream);
        let cstream2 = stream_tracked.clone();
        assert_eq!(1, track_list.len());
        let handle = thread::spawn(move || {
            let mut data = String::from_str("").unwrap();
            let mut tstream = TimeoutStream::from(stream_tracked, Some(read_timeout), None);
            tstream
                .read_to_string(&mut data)
                .expect_err("expected error reading data");
        });
        cstream2.shutdown(Shutdown::Read).unwrap();
        handle.join().unwrap();
        drop(cstream2);
        assert_eq!(0, track_list.len());
    });
    let client = TcpClient::connect(addr.to_string()).unwrap();
    handle.join().unwrap();
    drop(client)
}

#[test]
fn tls_stream_read_reads_data() {
    let port = get_free_port();
    let addr = format!("localhost:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let key = load_private_key().unwrap();
        let cert = load_test_certificate().unwrap();
        let stream = tls::Stream::new(stream, key, cert).unwrap();
        let mut cstream = CancellableStream::new(stream).unwrap();
        let mut reader = BufReader::new(&mut cstream);
        let mut content = Vec::new();
        reader.read_until(b' ', &mut content).unwrap();
        String::from_utf8_lossy(&content).to_string()
    });
    let config = build_tls_test_client_config().unwrap();
    let mut client = TestTLSClient::new("localhost", port, config).unwrap();
    client.write("test  ".as_bytes()).unwrap();
    let received = handle.join().unwrap();
    assert_eq!(received, "test ".to_string());
}

#[test]
fn test_shutdown_list_tls() {
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let read_timeout = Duration::from_secs(3);
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let key = load_private_key().unwrap();
        let cert = load_test_certificate().unwrap();
        let stream = tls::Stream::new(stream, key, cert).unwrap();
        let cstream = CancellableStream::new(stream).unwrap();
        let track_list = TrackedStreamList::new();
        let stream_tracked = TrackedStreamList::track(&track_list, cstream);
        let cstream2 = stream_tracked.clone();
        assert_eq!(1, track_list.len());
        let handle = thread::spawn(move || {
            let mut data = String::from_str("").unwrap();
            let mut tstream = TimeoutStream::from(stream_tracked, Some(read_timeout), None);
            tstream
                .read_to_string(&mut data)
                .expect_err("expected error reading data");
        });
        cstream2.shutdown(Shutdown::Read).unwrap();
        handle.join().unwrap();
        drop(cstream2);
        assert_eq!(0, track_list.len());
    });
    //let client = TcpClient::connect(addr.to_string()).unwrap();
    let config = build_tls_test_client_config().unwrap();
    let client = TestTLSClient::new("localhost", port, config).unwrap();
    handle.join().unwrap();
    drop(client)
}
