#![allow(missing_docs)]
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Mutex;
use std::sync::{Arc, Weak};
use std::time;

use connection_pool::{Pool, PoolResource};

use crate::http::*;
use crate::streams::timeout_stream::TimeoutStream;

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

impl Deref for ClientResponse {
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
            if let Some(body) = self.response.body.as_mut() {
                body.ensure_read().consume_error(|err| {
                    error!(
                        "error ensuring body in request has completely read: {}",
                        err
                    );
                });
            }

            let pool = match pool.lock() {
                Ok(pool) => pool,
                Err(_) => {
                    error!("error getting a lock for a client connection pool");
                    return;
                }
            };

            let conn = match self.conn.try_clone() {
                Ok(conn) => conn,
                Err(err) => {
                    error!("error cloning stream {}", err);
                    return;
                }
            };
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
            match request.is_connection_persistent() {
                true => {
                    let pool = self.connection_pool.lock().map_err(HttpError::from)?;
                    match pool.get(addr) {
                        Some(conn) => conn.resource(),
                        None => Self::connect(addr).map(Arc::new)?,
                    }
                }
                false => Self::connect(addr).map(Arc::new)?,
            }
        };
        let read_timeout = DEFAULT_READ_RESPONSE_TIMEOUT;
        let write_timeout = DEFAULT_WRITE_REQUEST_TIMEOUT;

        let conn = conn.try_clone().map_err(HttpError::from)?;
        let response_conn = conn.try_clone().map_err(HttpError::from)?;
        let mut stream = TimeoutStream::from(conn, Some(read_timeout), Some(write_timeout));
        request.write(&mut stream)?;
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
            response,
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

impl Default for Client {
    fn default() -> Self {
        Self::new()
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

trait ConsumeIfError<F, E> {
    fn consume_error(self, consumer: F)
    where
        F: FnOnce(E);
}

impl<T, E, F> ConsumeIfError<F, E> for Result<T, E> {
    fn consume_error(self, consumer: F)
    where
        F: FnOnce(E),
    {
        if let Err(err) = self {
            consumer(err)
        }
    }
}
