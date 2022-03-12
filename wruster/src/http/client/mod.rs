use std::convert::From;

use std::io::Cursor;
use std::net::TcpStream;

use crate::http::*;

mod connection_pool;

use connection_pool::Pool;

// pub struct Client {
//     connection_pool: Pool<TcpStream>
// }

// impl Client {
//     pub fn new() -> Self {
//         let connection_pool = Pool::new(|addr|{
//             TcpStream::connect(addr).unwrap()
//         });
//         Self {
//             connection_pool
//         }
//     }

//     pub fn run(self, request: Request) -> Response {

//     }
// }

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
    fn build_request_from_str() {}
}
