use std::{
    fmt::{Debug, Display},
    io::{self, Read, Write},
    net::{Shutdown, TcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};

use rustls::{self, Certificate, PrivateKey, ServerConfig, ServerConnection, StreamOwned};

use super::cancellable_stream::BaseStream;

#[cfg(test)]
mod test;

pub struct Stream {
    plain_stream: TcpStream,
    stream: Mutex<StreamOwned<ServerConnection, TcpStream>>,
}

impl Stream {
    pub fn new(
        stream: TcpStream,
        private_key: PrivateKey,
        cert: Certificate,
    ) -> Result<Self, io::Error> {
        let cert_chain = vec![cert];
        let tls_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let plain_stream = stream.try_clone()?;
        let tls_config = Arc::new(tls_config);
        let connection = ServerConnection::new(Arc::clone(&tls_config))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let stream = StreamOwned::new(connection, stream);
        let stream = Mutex::new(stream);
        Ok(Stream {
            plain_stream,
            stream,
        })
    }

    pub fn as_raw(&self) -> std::os::unix::prelude::RawFd {
        self.plain_stream.as_raw()
    }

    pub fn read_int(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut stream = self.stream.lock().unwrap();
        stream.read(buf)
    }

    pub fn write_int(&self, buf: &[u8]) -> io::Result<usize> {
        let mut stream = self.stream.lock().unwrap();
        stream.write(buf)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let stream = &self.plain_stream;
        stream.set_nonblocking(nonblocking)
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let stream = &self.plain_stream;
        stream.shutdown(how)
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let stream = &self.plain_stream;
        stream.set_read_timeout(dur)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let stream = &self.plain_stream;
        stream.set_write_timeout(dur)
    }

    pub fn flush_data(&self) -> io::Result<()> {
        let mut stream = self.stream.lock().unwrap();
        stream.flush()
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_int(buf)
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_int(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_data()
    }
}

struct ComposeError {
    a: io::Error,
    b: io::Error,
}

impl Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Multiple Errors: {} {}", &self.a, &self.b)
    }
}

impl Debug for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposeError")
            .field("a", &self.a)
            .field("b", &self.b)
            .finish()
    }
}

impl std::error::Error for ComposeError {}
