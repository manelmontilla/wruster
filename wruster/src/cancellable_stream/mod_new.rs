#![feature(negative_impls)]
use std::io::{self, Read};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use polling::Event;

pub const POOL_TIME: Duration = Duration::from_secs(3);

pub struct CancellableStream<'a> {
    stream: &'a mut TcpStream,
    poller: Arc<polling::Poller>,
    done: Arc<RwLock<bool>>,
    poller_init: bool,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl<'a> CancellableStream<'a> {
    pub fn new(
        stream: &'a mut TcpStream,
        done: Arc<RwLock<bool>>,
    ) -> io::Result<CancellableStream> {
        let poller = Arc::new(polling::Poller::new()?);
        let poller_init = false;
        stream.set_nonblocking(true)?;
        let read_timeout = None;
        let write_timeout = None;
        return Ok(CancellableStream {
            stream,
            done,
            poller,
            poller_init,
            read_timeout,
            write_timeout,
        });
    }
}

impl<'a> io::Read for CancellableStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.poller_init {
            self.poller.add(&*self.stream, Event::readable(1))?;
            self.poller_init = true;
        } else {
            self.poller.modify(&*self.stream, Event::readable(1))?
        };
        match self.poller_init {
            false => self.poller.add(&*self.stream, Event::readable(1))?,
            true => self.poller.modify(&*self.stream, Event::readable(1))?,
        }
        let mut events = Vec::new();
        if self.poller.wait(&mut events, self.read_timeout)? == 0 {
            let stop = self.done.read().unwrap();
            if  *stop == true {
                return Err(io::Error::from(io::ErrorKind::Other))
            };
        }
        
        let mut bytes_read = 0;
        let buf_len = buf.len();
        for evt in &events {
            if evt.key != 1 {
                continue;
            }
            let read_buf = &mut buf[bytes_read..buf_len];
            match self.stream.read(read_buf) {
                Ok(n) => bytes_read = bytes_read + n,
                Err(err) => {
                    self.stream.set_nonblocking(false)?;
                    return Err(err);
                }
            };
            if bytes_read == buf_len {
                break;
            }
        }
        return Ok(bytes_read);
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener};
    use std::thread::{self, Thread};

    use super::*;

    #[test]
    fn reads_data() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let read_timeout = Duration::from_secs(3);
        let expected_timeout = read_timeout.clone();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            //let stream1 = stream.try_clone().unwrap();
            let done = Arc::new(RwLock::new(false));
            let mut cstream = CancellableStream::new(&mut stream, done);
            let mut reader = BufReader::new(&mut cstream);
            let mut content = Vec::new();
            reader.read_until(b' ', &mut content).unwrap();
            String::from_utf8_lossy(&content).to_string()
        });

        let mut client = TcpClient::connect(addr.to_string()).unwrap();
        thread::sleep(Duration::from_secs(1));
        client.send("t".as_bytes()).unwrap();
        // Give time for
        //thread::park_timeout(Duration::from_secs(20));
        client.send("es".as_bytes()).unwrap();
        client.send("t ".as_bytes()).unwrap();
        client.send("this should not get read".as_bytes()).unwrap();
        let received = handle.join().unwrap();
        assert_eq!(received, "test ".to_string());
    }

    struct TcpClient {
        pub addr: String,
        stream: Option<TcpStream>,
    }

    impl TcpClient {
        pub fn connect(addr: String) -> Result<Self, Box<dyn Error>> {
            let stream = TcpStream::connect(&addr)?;
            let stream = Some(stream);
            Ok(TcpClient {
                addr: addr,
                stream: stream,
            })
        }

        pub fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
            let stream = self.stream.as_mut().unwrap();
            stream.write(data)?;
            stream.flush()?;
            Ok(())
        }

        pub fn close(&mut self) -> Result<(), Box<dyn Error>> {
            let stream = self.stream.as_mut().unwrap();
            stream.shutdown(Shutdown::Both)?;
            Ok(())
        }
    }

    impl Drop for TcpClient {
        fn drop(&mut self) {
            let _ = self.close();
        }
    }

    fn get_free_port() -> u16 {
        let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        TcpListener::bind(addr)
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }
}
