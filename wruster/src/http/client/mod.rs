use crate::http::*;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Mutex;
use std::sync::{Arc, Weak};
use std::time;

use crate::timeout_stream::TimeoutStream;
use connection_pool::{Pool, PoolResource};

mod connection_pool;

#[cfg(test)]
mod tests;

/// Defines the default max time for a response to be read.
pub const DEFAULT_READ_RESPONSE_TIMEOUT: time::Duration = time::Duration::from_secs(60);

/// Defines the default max time for a request to be written.
pub const DEFAULT_WRITE_REQUEST_TIMEOUT: time::Duration = time::Duration::from_secs(60);

pub struct ClientResponse {
    response: Response,
    conn: TcpStream,
    pool: Weak<Mutex<Pool<Arc<TcpStream>>>>,
    addr: String,
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

impl Drop for ClientResponse {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            if let Some(Body) = self.response.body.as_mut() {
                // TODO: Handle possible panic here.
                Body.ensure_read().unwrap();
            }
            // TODO: Handle possible panic here.
            let pool = pool.lock().unwrap();
            // TODO: Handle possible panic here.
            let conn = self.conn.try_clone().unwrap();
            let conn = Arc::new(conn);
            pool.insert(&self.addr, PoolResource::new(conn))
        };
    }
}

pub struct Client {
    connection_pool: Arc<Mutex<Pool<Arc<TcpStream>>>>,
}

impl<'a> Client {
    pub fn new() -> Self {
        let connection_pool = Arc::new(Mutex::new(Pool::new(None)));
        Self { connection_pool }
    }

    pub fn run(&'a self, addr: &str, request: Request) -> Result<ClientResponse, HttpError> {
        let conn = {
            if request.is_connection_alive() {
                let pool = self.connection_pool.lock().unwrap();
                match pool.get(addr) {
                    Some(conn) => conn.resource(),
                    None => Self::connect(addr).map(|stream| Arc::new(stream))?,
                }
            } else {
                Self::connect(addr).map(|stream| Arc::new(stream))?
            }
        };
        let read_timeout = DEFAULT_READ_RESPONSE_TIMEOUT;
        let write_timeout = DEFAULT_WRITE_REQUEST_TIMEOUT;

        let conn = conn.try_clone().map_err(HttpError::from)?;
        let response_conn = conn.try_clone().map_err(HttpError::from)?;
        let mut stream = TimeoutStream::from(conn, Some(read_timeout), Some(write_timeout));
        if let Err(err) = request.write(&mut stream) {
            return Err(err);
        };
        stream.flush().map_err(HttpError::from)?;
        let stream = Box::new(stream);
        let response = match Response::read_from(stream) {
            Ok(response) => response,
            Err(err) => return Err(err),
        };
        // TODO: when the response does not have body we can just return back
        // the connection to the pool here.
        let response_pool = Arc::clone(&self.connection_pool);
        let response = ClientResponse {
            response: response,
            conn: response_conn,
            pool: Arc::downgrade(&response_pool),
            addr: addr.to_string(),
        };
        Ok(response)
    }

    fn connect(addr: &str) -> Result<TcpStream, HttpError> {
        let addrs = addr.to_socket_addrs().map_err(HttpError::from)?;
        let addrs = addrs.collect::<Vec<SocketAddr>>();
        TcpStream::connect(&*addrs).map_err(HttpError::from)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let pool = self.connection_pool.lock().unwrap();
        let connections = pool.drain();
        drop(pool);
        for connection in connections {
            let connection = connection.resource();
            _ = connection.shutdown(std::net::Shutdown::Both)
        }
    }
}
