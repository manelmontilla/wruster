use std::convert::From;

use std::io::Cursor;

use crate::http::*;

pub struct Client {}

// impl Client {
//     pub fn new() -> Self { Self {  } }

//     pub fn run(self, Request { method, uri, version, headers, body }: Request) -> Response {

//     }
// }

// trait RequestBuilder<'a> {
//     fn into_request(&'a self, method: HttpMethod, url: String) -> Request<'a>;
// }

// impl<'a> RequestBuilder<'a> for str {
//     fn into_request(self: &'a str, method: HttpMethod, url: String) -> Request<'a> {
//         Request {
//             version: Version::HTTP1_1.to_string(),
//             method: method,
//             uri: url,
//             headers: headers::Headers::new(),
//             body: Some(Body::from(self)),
//         }
//     }
// }

impl<'a> From<&'a str> for Body<'a> {
    fn from(from: &'a str) -> Self {
        Body {
            content_length: from.len() as u64,
            content_type: Some(mime::TEXT_PLAIN),
            content: Box::new(Cursor::new(from)),
        }
    }
}

impl<'a> IntoRequest<'a> for &'a str {
    fn into(&'a self, mime_type: mime::Mime, method: HttpMethod, url: String) -> Request<'a> {
        todo!()
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
        let r = "whatevert".into_request(HttpMethod::GET, "https:://example.com".to_string());
    }
}
