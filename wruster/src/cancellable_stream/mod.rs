use std::cell::RefCell;
use std::io;
use std::net::TcpStream;

use std::sync::{Arc, RwLock};
use std::time::Duration;

use polling::Event;


pub struct CancellableStream<'a> {
    stream: &'a mut TcpStream,
    poller: Arc<polling::Poller>,
    done: Arc<RwLock<bool>>,
    poller_init: bool,
    read_timeout: RefCell<Option<Duration>>,
    write_timeout: RefCell<Option<Duration>>,
}

impl<'a> CancellableStream<'a> {
    pub fn new(
        stream: &'a mut TcpStream,
        done: Arc<RwLock<bool>>,
    ) -> io::Result<CancellableStream> {
        let poller = Arc::new(polling::Poller::new()?);
        let poller_init = false;
        stream.set_nonblocking(true)?;
        let read_timeout = RefCell::new(None);
        let write_timeout = RefCell::new(None);
        return Ok(CancellableStream {
            stream,
            done,
            poller,
            poller_init,
            read_timeout,
            write_timeout,
        });
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let mut read_timeout = self.read_timeout.borrow_mut();
        *read_timeout = dur;
        Ok(())
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let mut write_timeout = self.write_timeout.borrow_mut();
        *write_timeout = dur;
        Ok(())
    }
}

impl<'a> io::Read for CancellableStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.poller_init {
            false => {
                self.poller.add(&*self.stream, Event::readable(1))?;
                self.poller_init = true;
            },
            true => self.poller.modify(&*self.stream, Event::readable(1))?,
        }
        let mut events = Vec::new();
        let timeout = &self.read_timeout.borrow_mut().clone();
        if self.poller.wait(&mut events, *timeout)? == 0 {
            let stop = self.done.read().unwrap();
            if *stop == true {
                return Err(io::Error::from(io::ErrorKind::Other));
            };
            // TODO: Actually we could be here not only because the timeout
            // passed without any IO operation, but also because the OS
            // returned no events spuriously, so we should check ourselves if
            // the timeout period has passed, and if not, retry the wait.
            return Err(io::Error::from(io::ErrorKind::TimedOut));
        }

        let mut bytes_read = 0;
        let buf_len = buf.len();
        for evt in &events {
            if evt.key != 1 {
                continue;
            }
            let read_buf = &mut buf[bytes_read..];
            match self.stream.read(read_buf) {
                Ok(n) => bytes_read = bytes_read + n,
                Err(err) => {
                    self.stream.set_nonblocking(false)?;
                    return Err(err);
                }
            };
            // TODO: Actually this is not correct, we should read all the
            // events returned by wait, even if we end up reading more bytes
            // than the len of the buffer provider by the caller.
            if bytes_read == buf_len {
                break;
            }
        }
        Ok(bytes_read)
    }
}

impl<'a> io::Write for CancellableStream<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.poller_init {
            false => {
                self.poller.add(&*self.stream, Event::writable(1))?;
                self.poller_init = true;
            },
            true => self.poller.modify(&*self.stream, Event::writable(1))?,
        }
        let mut events = Vec::new();
        let timeout = &self.write_timeout.borrow_mut().clone();
        let mut bytes_written = 0;
        let buf_len = buf.len();
        while bytes_written < buf_len {
                if self.poller.wait(&mut events, *timeout)? == 0 {
                    let stop = self.done.read().unwrap();
                    if *stop == true {
                        return Err(io::Error::from(io::ErrorKind::Other));
                    };
                    // TODO: Actually we could be here not only because the
                    // timeout passed without any IO operation, but also
                    // because the OS returned no events spuriously, so we
                    // should check ourselves if the timeout period has passed,
                    // and if not, retry the wait.
                    return Err(io::Error::from(io::ErrorKind::TimedOut));
                }
                for evt in &events {
                    if evt.key != 1 {
                        continue;
                    }
                    let write_buf = &buf[bytes_written..];
                    match self.stream.write(write_buf) {
                        Ok(n) => bytes_written = bytes_written + n,
                        Err(err) if err.kind() == io::ErrorKind::WouldBlock =>  continue,
                        Err(err) => {
                            self.stream.set_nonblocking(false)?;
                            return Err(err);
                        }
                    };
                    if bytes_written == buf_len {
                        break;
                    }
                }
                events.clear();
        }
        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{BufRead, BufReader, Write, Read};
    use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener};
    use std::thread;

    use super::*;

    #[test]
    fn read_reads_data() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let done = Arc::new(RwLock::new(false));
            let mut cstream = CancellableStream::new(&mut stream, done).unwrap();
            let mut reader = BufReader::new(&mut cstream);
            let mut content = Vec::new();
            reader.read_until(b' ', &mut content).unwrap();
            String::from_utf8_lossy(&content).to_string()
        });

        let mut client = TcpClient::connect(addr.to_string()).unwrap();
        thread::sleep(Duration::from_secs(1));
        client.send("test  ".as_bytes()).unwrap();
        let received = handle.join().unwrap();
        assert_eq!(received, "test ".to_string());
    }

    #[test]
    fn read_honors_timeout() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let read_timeout = Duration::from_secs(2);
        let expected_timeout = read_timeout.clone();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let done = Arc::new(RwLock::new(false));
            let mut cstream = CancellableStream::new(&mut stream, done).unwrap();
            cstream.set_read_timeout(Some(expected_timeout)).unwrap();
            let mut reader = BufReader::new(&mut cstream);
            let mut content = Vec::new();
            reader
                .read_until(b' ', &mut content)
                .expect_err("expected timeout")
        });

        let client = TcpClient::connect(addr.to_string()).unwrap();
        let received_err = handle.join().unwrap();
        drop(client);
        assert!(received_err.kind() == io::ErrorKind::TimedOut)
    }

    #[test]
    fn write_writes_data() {
        let data = "test ";
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let server_data = data.clone();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let done = Arc::new(RwLock::new(false));
            let mut cstream = CancellableStream::new(&mut stream, done).unwrap();
            let data = server_data.as_bytes();
            cstream.write(&data)
        });

        let mut client = TcpClient::connect(addr.to_string()).unwrap();
        let bytes_sent = handle.join().unwrap().expect("expected data to be written correctly");
        assert_eq!(bytes_sent, data.len());

        let mut reader = BufReader::new(&mut client);
        let mut content = Vec::new();
        reader.read_until(b' ', &mut content).expect("expect data to available");
        let content = String::from_utf8(content).expect("expect data to be valid");
        assert_eq!(content, "test ".to_string());
    }

    struct TcpClient {
        stream: Option<TcpStream>,
    }

    impl TcpClient {
        pub fn connect(addr: String) -> Result<Self, Box<dyn Error>> {
            let stream = TcpStream::connect(&addr)?;
            let stream = Some(stream);
            Ok(TcpClient { stream })
        }

        pub fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
            let stream = self.stream.as_mut().unwrap();
            stream.write(data)?;
            stream.flush()?;
            Ok(())
        }

        pub fn close(&mut self) -> io::Result<()> {
            let stream = self.stream.as_mut().unwrap();
            stream.shutdown(Shutdown::Both)?;
            Ok(())
        }

        pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let stream = self.stream.as_mut().unwrap();
            stream.read(buf)
        }
    }

    impl Drop for TcpClient {
        fn drop(&mut self) {
            let _ = self.close();
        }
    }

    impl io::Read for TcpClient {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.read(buf)
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
