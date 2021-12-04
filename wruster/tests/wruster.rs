use std::env;
use std::error::Error;
use std::io::ErrorKind;
use std::io::Write;
use std::net::Shutdown;
use std::net::TcpStream;
use std::thread;
use std::time;

use wruster::handlers::{log_middleware, serve_static};
use wruster::http;
use wruster::http::Response;
use wruster::http::StatusCode;
use wruster::router;
use wruster::router::HttpHandler;
use wruster::*;

#[test]
fn accepts_connection() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();
    let handle = thread::spawn(|| {
        let routes = router::Router::new();
        let serve_dir: HttpHandler =
        // log_middleware(Box::new(move |request| serve_static("./", &request)));
        Box::new(move |_request| {
          println!("Request received");
          Response::from_status(StatusCode::OK)
        });
        routes.add("/", http::HttpMethod::POST, serve_dir);
        run_and_serve("127.0.0.1:8081", routes, Some(time::Duration::from_secs(1))).unwrap();
    });
    thread::sleep(time::Duration::from_secs(1));
    let mut client = TcpClient {
        addr: String::from("127.0.0.1:8081"),
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
    let response = Response::read_from(stream).unwrap();
    assert_eq!(response.status, StatusCode::RequestTimeOut);
    handle.join().unwrap();
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
          None => return Err(Box::new(std::io::Error::new(ErrorKind::Other, "client not connected"))),
          Some(stream) => stream
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

    pub fn close(&mut self) -> Result<(), Box<dyn Error>> {
        let stream = self.stream.as_mut().unwrap();
        stream.shutdown(Shutdown::Both)?;
        Ok(())
    }
}

impl Drop for TcpClient {
    fn drop(&mut self) {
        self.close();
    }
}
