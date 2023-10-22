/*!
Contains various types that augment a type that can act as a [Stream], e.g.: a [std::net::TcpStream].
*/
use polling::Source;
use std::io::Read;
use std::io::{self, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

pub mod cancellable_stream;
pub mod observable;
pub mod timeout_stream;
pub mod tls;

pub trait BaseStream {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
    fn shutdown(&self, how: Shutdown) -> io::Result<()>;
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
    fn as_raw(&self) -> std::os::unix::prelude::RawFd;
    fn write_buf(&self, buf: &[u8]) -> io::Result<usize>;
    fn read_buf(&self, buf: &mut [u8]) -> io::Result<usize>;
    fn flush_data(&self) -> io::Result<()>;
}

impl BaseStream for TcpStream {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.set_nonblocking(nonblocking)
    }

    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.shutdown(how)
    }

    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(dur)
    }

    fn as_raw(&self) -> std::os::unix::prelude::RawFd {
        self.raw()
    }

    fn write_buf(&self, buf: &[u8]) -> io::Result<usize> {
        let mut s = self;
        <&Self as Write>::write(&mut s, buf)
    }

    fn read_buf(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = self;
        <&Self as Read>::read(&mut s, buf)
    }

    fn flush_data(&self) -> io::Result<()> {
        let mut s = self;
        <&Self as Write>::flush(&mut s)
    }
}

impl BaseStream for tls::Stream {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.set_nonblocking(nonblocking)
    }

    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.shutdown(how)
    }

    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(dur)
    }

    fn as_raw(&self) -> std::os::unix::prelude::RawFd {
        self.as_raw()
    }

    fn write_buf(&self, buf: &[u8]) -> io::Result<usize> {
        self.write_int(buf)
    }

    fn read_buf(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_int(buf)
    }

    fn flush_data(&self) -> io::Result<()> {
        self.flush_data()
    }
}

/**
 Defines the shape type that can act as a Stream so its functionality can be extended
 by the other types in the package, e.g.: [observable::ObservableStream].
*/
pub trait Stream: Send + Sync + BaseStream {}

impl Stream for tls::Stream {}

impl Stream for TcpStream {}

#[cfg(test)]
mod test;
mod test_utils;
