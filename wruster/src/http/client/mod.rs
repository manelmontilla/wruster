use std::error;
use std::net::TcpStream;

use crate::http::*;

mod connection_pool;

use connection_pool::Pool;

fn create_connection(addr: String) -> Result<TcpStream, Box<dyn std::error::Error>> {
    match TcpStream::connect(addr) {
        Ok(connetion) => Ok(connetion),
        Err(err) => Err(Box::new(err)),
    }
}

pub struct Client {
    connection_pool: Pool<TcpStream, fn(String) -> Result<TcpStream, Box<dyn std::error::Error>>>,
}

impl Client {
    pub fn new() -> Self {
        let creator: fn(String) -> Result<TcpStream, Box<dyn std::error::Error>> =
            create_connection;
        let connection_pool = Pool::new(creator);
        Self { connection_pool}
    }

    pub fn run(mut self, request: Request) -> Response {
        self.connection_pool
            .roundtrip(String::from("127.0.0.1:8081"), |mut conn| {
                conn.flush().unwrap();
                conn
            })
            .unwrap();
        Response::from_status(StatusCode::OK)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    //    #[test]
    //   fn  do_a_request() {
    //     let c = Client::new();
    //     let r = Request::read_from_str("GET / HTTP/1.1\r\n\r\n").unwrap();
    //     c.run(r);
    //   }

    #[test]
    fn build_request_from_str() {
        let c = Client::new();
        let r = Request::read_from_str("GET / HTTP/1.1\r\n\r\n").unwrap();
        c.run(r);
    }
}
