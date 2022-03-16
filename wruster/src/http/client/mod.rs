use std::error;
use std::net::TcpStream;
use std::sync::{self, Mutex};
use std::sync::Arc;
use std::thread;

use crate::http::*;

mod connection_pool;

use connection_pool::Pool;

pub struct Client {
    connection_pool: Arc<Mutex<Pool<TcpStream>>>,
}

impl Client {
    pub fn new() -> Self {
        let connection_pool = Arc::new(Mutex::new(Pool::new(None)));
        Self { connection_pool }
    }

    pub fn run(&self, request: Request) -> Response {
        
        // let conn = self.connection_pool.get(&request.uri);
        Response::from_status(StatusCode::Continue)
    }
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use super::*;

    //    #[test]
    //   fn  do_a_request() {
    //     let c = Client::new();
    //     let r = Request::read_from_str("GET / HTTP/1.1\r\n\r\n").unwrap();
    //     c.run(r);
    //   }

    #[test]
    fn build_request_from_str() {
        let c = Arc::new(Client::new());
        let mut c2 = Arc::clone(&c);
        let handle = thread::spawn(move || {
            let c = &mut c2;
            let r = Request::read_from_str("GET / HTTP/1.1\r\n\r\n").unwrap();
            let mut resp = c.run(r);
            let mut v: Vec<u8> = Vec::new();
            resp.write(&mut v).unwrap();
            let s = String::from_utf8(v).unwrap();
            println!("response {}", s);
        });
        handle.join().unwrap();
    }
}
