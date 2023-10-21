use std::{
    fmt::{Debug, Display},
    io::{self, BufReader, Read, Write},
    net::{Shutdown, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use rustls::{self, ServerConfig, ServerConnection, StreamOwned};

use super::cancellable_stream::BaseStream;

pub mod test_utils;

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
        let cert_chain = vec![cert.0];
        let tls_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key.0)
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

/**
Represents a Certificate that can be used in the TLS connections.
*/
pub struct Certificate(rustls::Certificate);

impl Certificate {
    /**
    Reads a certificate from the given path to a pem file.
    # Arguments

    * `path` a path to a file in pem format containing a certificate.

    # Errors

    This function will return an error if:
        * The path does not exists.
        * The format of the file is not valid.
        * There are no certificates stored in the file.
    */
    pub fn read_from(path: &str) -> io::Result<Certificate> {
        let cert_path = PathBuf::from(path);
        let file = std::fs::File::open(cert_path).map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                io::Error::new(io::ErrorKind::Other, format!("file {} not found", path))
            } else {
                err
            }
        })?;
        let mut cert_reader = std::io::BufReader::new(file);
        let cert = rustls_pemfile::certs(&mut cert_reader)?
            .iter()
            .map(|v| rustls::Certificate(v.clone()))
            .collect::<Vec<rustls::Certificate>>();
        match cert.len() {
            0 => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("no certificate found in {} ", path),
            )),
            _ => Ok(Certificate(cert[0].clone())),
        }
    }
}

impl From<&Certificate> for Vec<u8> {
    fn from(cert: &Certificate) -> Self {
        let data = cert.0.as_ref();
        Vec::from(data)
    }
}

impl Clone for Certificate {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/**
Represents a private key that can be used in the TLS connections.
*/
pub struct PrivateKey(rustls::PrivateKey);

impl PrivateKey {
    /**
    Reads a private key from the given path to a pem file.
    # Arguments

    * `path` a path to a file in pem format containing the private key.

    # Errors

    This function will return an error if:
        * The path does not exists.
        * The format of the file is not valid.
        * There are no privates keys stored in the file.
    */
    pub fn read_from(path: &str) -> io::Result<PrivateKey> {
        let keyfile = std::fs::File::open(path).map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                io::Error::new(io::ErrorKind::Other, format!("no file found in {} ", path))
            } else {
                err
            }
        })?;
        let mut reader = BufReader::new(keyfile);
        match rustls_pemfile::read_one(&mut reader)? {
            Some(rustls_pemfile::Item::RSAKey(key)) => Ok(PrivateKey(rustls::PrivateKey(key))),
            Some(rustls_pemfile::Item::PKCS8Key(key)) => Ok(PrivateKey(rustls::PrivateKey(key))),
            Some(rustls_pemfile::Item::ECKey(key)) => Ok(PrivateKey(rustls::PrivateKey(key))),
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("no private key found in {} ", path),
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("no private key found in {} ", path),
            )),
        }
    }
}

impl Clone for PrivateKey {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
