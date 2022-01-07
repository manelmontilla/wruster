use std::error::Error;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream};
use std::thread;
use std::time::{self, Duration};

use wruster::http::headers::Header;
use wruster::http::headers::Headers;
use wruster::http::Response;
use wruster::http::StatusCode;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::*;

#[test]
fn server_closes_connection_when_timeout() {
    let timeouts = Timeouts {
        read_request_timeout: Duration::from_secs(1),
        write_response_timeout: Duration::from_secs(1),
    };
    let mut server = Server::from_timeouts(timeouts);
    let routes = router::Router::new();
    let serve_dir: HttpHandler = Box::new(move |_| Response::from_status(StatusCode::OK));
    routes.add("/", http::HttpMethod::POST, serve_dir);
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port.to_string());
    server.run(&addr, routes).unwrap();

    thread::sleep(time::Duration::from_secs(1));
    let mut client = TcpClient {
        addr: addr.to_string(),
        stream: None,
    };
    let request = "POST / HTTP/1.1\r\n\
Connection: Keep-Alive\r\n\
Content-Length: 4\r\n\
\r\n\
test";
    client.connect().unwrap();
    client.send(request.as_bytes()).unwrap();
    let stream = client.stream().unwrap();
    let _ = Response::read_from(stream).unwrap();

    // From here after 1 sec, the connection with the client must be closed.
    thread::sleep(time::Duration::from_secs(2));
    assert_eq!(client.is_closed(), true);
    server.shutdown().unwrap()
}

#[test]
fn server_handles_requests() {
    let mut server = Server::new();
    let routes = router::Router::new();
    let handler: HttpHandler = Box::new(move |request| {
        let mut content: Vec<u8> = Vec::new();
        request
            .body
            .unwrap()
            .content
            .read_to_end(&mut content)
            .unwrap();
        let content = String::from_utf8_lossy(&content);
        print!("content {}", content);
        if &content == "test" {
            Response::from_status(StatusCode::OK)
        } else {
            Response::from_status(StatusCode::InternalServerError)
        }
    });
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port.to_string());
    routes.add("/", http::HttpMethod::POST, handler);
    server.run(&addr, routes).unwrap();

    thread::sleep(time::Duration::from_secs(1));
    let mut client = TcpClient {
        addr: addr,
        stream: None,
    };
    let request = "POST / HTTP/1.1\r\n\
Content-Length: 4\r\n\
\r\n\
test";
    client.connect().unwrap();
    client.send(request.as_bytes()).unwrap();
    let stream = client.stream().unwrap();
    let response = Response::read_from(stream).unwrap();
    assert_eq!(response.status, StatusCode::OK);
    let got_headers = response
        .headers
        .iter()
        .collect::<Vec<(&String, &Vec<String>)>>();
    let mut want_headers = Headers::new();
    let header = Header {
        name: "Content-Length".to_string(),
        value: "0".to_string(),
    };
    want_headers.add(header);
    let want_headers = want_headers
        .iter()
        .collect::<Vec<(&String, &Vec<String>)>>();
    assert_eq!(got_headers, want_headers);
    assert!(response.body.is_none());
    server.shutdown().unwrap()
}

#[test]
fn server_shutdowns() {
    let mut server = Server::new();
    let routes = router::Router::new();
    let port = get_free_port();
    let addr = format!("127.0.0.1:{}", port.to_string());
    server.run(&addr, routes).unwrap();
    thread::sleep(time::Duration::from_secs(2));
    server.shutdown().unwrap()
}

struct TcpClient {
    pub addr: String,
    stream: Option<TcpStream>,
}

impl TcpClient {
    pub fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        let stream = TcpStream::connect(&self.addr)?;
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stream(&mut self) -> Result<TcpStream, Box<dyn Error>> {
        let stream = match &self.stream {
            None => {
                return Err(Box::new(std::io::Error::new(
                    io::ErrorKind::Other,
                    "client not connected",
                )))
            }
            Some(stream) => stream,
        };
        let stream = stream.try_clone()?;
        Ok(stream)
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let stream = self.stream.as_mut().unwrap();
        stream.write(data)?;
        stream.flush()?;
        Ok(())
    }

    pub fn is_closed(&mut self) -> bool {
        let stream = self.stream.as_mut().expect("call connect first");
        let mut buf = [0; 1];
        stream.set_nonblocking(true).unwrap();
        let err = match stream.peek(&mut buf) {
            Err(err) => err,
            Ok(n) => match n {
                0 => return true,
                _ => return false,
            },
        };
        err.kind() == io::ErrorKind::WouldBlock
    }

    pub fn close(&mut self) -> Result<(), Box<dyn Error>> {
        let stream = self.stream.as_mut().unwrap();
        stream.shutdown(Shutdown::Both)?;
        Ok(())
    }
}

impl Drop for TcpClient {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn get_free_port() -> u16 {
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    TcpListener::bind(addr)
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
