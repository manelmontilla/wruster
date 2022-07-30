use crate::http::*;
use std::net::TcpStream;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time;
use url::Url;

mod connection_pool;

use crate::timeout_stream::TimeoutStream;
use connection_pool::{Pool, PoolResource};

/// Defines the default max time for a response to be read.
pub const DEFAULT_READ_RESPONSE_TIMEOUT: time::Duration = time::Duration::from_secs(60);

/// Defines the default max time for a request to be written.
pub const DEFAULT_WRITE_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);

pub struct ClientResponse {
    response: Response,
    conn: Box<TcpStream>,
    //stream: TimeoutStream<'a, TcpStream>,
}

impl<'a> Deref for ClientResponse {
    type Target = Response;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

impl DerefMut for ClientResponse {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.response
    }
}

pub struct Client {
    connection_pool: Arc<Mutex<Pool<TcpStream>>>,
}

impl<'a> Client {
    pub fn new() -> Self {
        let connection_pool = Arc::new(Mutex::new(Pool::new(None)));
        Self { connection_pool }
    }

    pub fn run(&'a self, request: Request) -> Result<ClientResponse, HttpError> {
        todo!();
        //let conn = self.connection_pool.get(&request.uri);
        let pool = self.connection_pool.lock().unwrap();
        // Response::from_status(StatusCode::OK)
        let url = match url::Url::parse(&request.uri) {
            Ok(url) => url,
            Err(err) => return Err(HttpError::Unknown(err.to_string())),
        };
        let host = match url.host_str() {
            None => return Err(HttpError::Unknown("invalid hostname".to_string())),
            Some(host) => host,
        };
        let port = match url.port_or_known_default() {
            None => return Err(HttpError::Unknown("unknown port".to_string())),
            Some(port) => port.to_string(),
        };
        let addr = format!("{}:{}", host, port);
        let mut conn = match pool.get(&addr) {
            Some(conn) => conn,
            None => Self::connect(url)?,
        };
        let read_timeout = DEFAULT_READ_RESPONSE_TIMEOUT;
        let write_timeout = DEFAULT_WRITE_REQUEST_TIMEOUT;
        let conn = conn.resource();
        let mut stream = TimeoutStream::from(conn, Some(read_timeout), Some(write_timeout));
        if let Err(err) = request.write(&mut stream) {
            return Err(err);
        };
        let mut stream = match conn.try_clone() {
            Err(err) => return Err(HttpError::Unknown(err.to_string())),
            Ok(stream) => stream,
        };
        let mut stream = Box::new(stream);
        let response = match Response::read_from(&mut stream) {
            Ok(response) => response,
            Err(err) => return Err(err),
        };
        let pool = self.connection_pool.lock().unwrap();
        let conn = PoolResource::new(conn);
        pool.insert(&addr, conn);
        let response = ClientResponse {
            response: response,
            conn: stream,
        };
        Ok(response)
    }

    fn connect(uri: url::Url) -> Result<PoolResource<TcpStream>, HttpError> {
        let addrs = match uri.socket_addrs(|| None) {
            Ok(addrs) => addrs,
            Err(err) => return Err(HttpError::Unknown(err.to_string())),
        };
        let addr = &*addrs;
        match TcpStream::connect(addr) {
            Ok(tcp_stream) => Ok(PoolResource::new(tcp_stream)),
            Err(err) => Err(HttpError::Unknown(err.to_string())),
        }
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

    // #[test]
    // fn build_request_from_str() {
    //     let c = Arc::new(Client::new());
    //     let mut c2 = Arc::clone(&c);
    //     let handle = thread::spawn(move || {
    //         let c = &mut c2;
    //         let r = Request::read_from_str("GET / HTTP/1.1\r\n\r\n").unwrap();
    //         let mut resp = c.run(r).unwrap();
    //         let mut v: Vec<u8> = Vec::new();
    //         resp.write(&mut v).unwrap();
    //         let s = String::from_utf8(v).unwrap();
    //         println!("response {}", s);
    //     });
    //     handle.join().unwrap();
    // }
}