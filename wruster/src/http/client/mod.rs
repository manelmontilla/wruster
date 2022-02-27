use std::convert::From;

use std::io::Cursor;

use crate::http::*;

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    // pub fn run(self, request: Request) -> Response {

    // }
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
    fn build_request_from_str() {}
}
