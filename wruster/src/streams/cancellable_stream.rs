use polling::{Event, Source};
use std::io::Read;
use std::io::{self, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{atomic, Arc, RwLock};
use std::time::Duration;

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

pub trait Stream: Send + Sync + BaseStream {}

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

impl Stream for TcpStream {}

pub struct CancellableStream<T: BaseStream> {
    stream: T,
    poller: Arc<polling::Poller>,
    done: AtomicBool,
    read_timeout: RwLock<Option<Duration>>,
    write_timeout: RwLock<Option<Duration>>,
}

impl<T> CancellableStream<T>
where
    T: BaseStream,
{
    pub fn new(stream: T) -> io::Result<CancellableStream<T>> {
        let poller = Arc::new(polling::Poller::new()?);
        stream.set_nonblocking(true)?;
        let read_timeout = RwLock::new(None);
        let write_timeout = RwLock::new(None);
        let done = atomic::AtomicBool::new(false);
        poller.add(&stream.as_raw(), Event::all(1))?;
        return Ok(CancellableStream {
            stream,
            done,
            poller,
            read_timeout,
            write_timeout,
        });
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let mut read_timeout = self.read_timeout.write().unwrap();
        *read_timeout = dur;
        Ok(())
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let mut write_timeout = self.write_timeout.write().unwrap();
        *write_timeout = dur;
        Ok(())
    }

    pub fn cancel(&self) -> io::Result<()> {
        self.poller.notify()
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.done.store(true, Ordering::SeqCst);
        self.stream.shutdown(how)
    }

    fn read_int(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.poller
            .modify(&self.stream.as_raw(), Event::readable(1))?;
        let mut events = Vec::new();
        let timeout = &self.read_timeout.write().unwrap().clone();
        let mut bytes_read = 0;
        let buf_len = buf.len();

        if self.poller.wait(&mut events, *timeout)? == 0 {
            let stop = self.done.load(atomic::Ordering::SeqCst);
            if stop == true {
                return Err(io::Error::from(io::ErrorKind::NotConnected));
            };
            // TODO: Actually we could be here not only because the timeout
            // passed without read operations available, but also because the
            // OS returned no events spuriously, so we should check ourselves
            // if the timeout period has passed, and if not, retry the wait.
            return Err(io::Error::from(io::ErrorKind::TimedOut));
        }
        for evt in &events {
            if evt.key != 1 {
                continue;
            }
            let read_buf = &mut buf[bytes_read..];
            let s = &self.stream;
            match s.read_buf(read_buf) {
                Ok(0) if self.done.load(Ordering::SeqCst) => {
                    return Err(io::Error::from(io::ErrorKind::NotConnected))
                }
                Ok(n) => bytes_read = bytes_read + n,
                Err(err) => {
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

    fn write_int(&self, buf: &[u8]) -> io::Result<usize> {
        self.poller
            .modify(&self.stream.as_raw(), Event::writable(1))?;
        let mut events = Vec::new();
        let timeout = &self.write_timeout.write().unwrap().clone();
        let mut bytes_written = 0;
        let buf_len = buf.len();
        while bytes_written < buf_len {
            if self.poller.wait(&mut events, *timeout)? == 0 {
                let stop = self.done.load(atomic::Ordering::SeqCst);
                if stop == true {
                    return Err(io::Error::from(io::ErrorKind::NotConnected));
                };
                // TODO: Actually we could be here not only because the timeout
                // passed without the stream being ready to accept writes, but
                // also because the OS returned no events spuriously, so we
                // should check ourselves if the timeout period has passed, and
                // if not, retry the wait.
                return Err(io::Error::from(io::ErrorKind::TimedOut));
            }
            for evt in &events {
                if evt.key != 1 {
                    continue;
                }
                let write_buf = &buf[bytes_written..];
                let s = &self.stream;
                match s.write_buf(write_buf) {
                    Ok(n) => bytes_written = bytes_written + n,
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => continue,
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
}

impl<T> io::Read for &CancellableStream<T>
where
    T: BaseStream,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_int(buf)
    }
}

impl<T> io::Write for &CancellableStream<T>
where
    T: BaseStream,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_int(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush_data()
    }
}

impl<T> io::Read for CancellableStream<T>
where
    T: BaseStream,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stream = &self;
        stream.read_int(buf)
    }
}

impl<T> io::Write for CancellableStream<T>
where
    T: BaseStream,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stream = &self;
        stream.write_int(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush_data()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
    use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener};
    use std::thread;

    use super::*;

    #[test]
    fn shutdown_stops_reading() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let cstream = Arc::new(CancellableStream::new(stream).unwrap());
            let cstream2 = Arc::clone(&cstream);
            let m = AtomicBool::new(true);
            let sm = &m;
            let result = thread::scope(|s| {
                let worker_handle = s.spawn(move || {
                    let mut content = Vec::new();
                    let s = cstream.as_ref();
                    let mut reader = BufReader::new(s);
                    reader.read_until(b't', &mut content).unwrap();
                    sm.store(false, Ordering::SeqCst);

                    let err = reader
                        .read_until(b't', &mut content)
                        .expect_err("expected error");
                    assert_eq!(err.kind(), ErrorKind::NotConnected)
                });
                while m.load(Ordering::SeqCst) {
                    thread::yield_now();
                }
                let s = cstream2.as_ref();
                s.shutdown(Shutdown::Both).unwrap();
                worker_handle.join().unwrap()
            });
            result
        });

        let mut client = TcpClient::connect(addr.to_string()).unwrap();
        thread::sleep(Duration::from_secs(1));
        client.send("t".as_bytes()).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn read_reads_data() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut cstream = CancellableStream::new(stream).unwrap();
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
            let (stream, _) = listener.accept().unwrap();
            let mut cstream = CancellableStream::new(stream).unwrap();
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
        assert_eq!(received_err.kind(), io::ErrorKind::TimedOut)
    }

    #[test]
    fn write_writes_data() {
        let data = "test ";
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let server_data = data.clone();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut cstream = CancellableStream::new(stream).unwrap();
            let data = server_data.as_bytes();
            cstream.write(&data)
        });

        let mut client = TcpClient::connect(addr.to_string()).unwrap();
        let bytes_sent = handle
            .join()
            .unwrap()
            .expect("expected data to be written correctly");
        assert_eq!(bytes_sent, data.len());

        let mut reader = BufReader::new(&mut client);
        let mut content = Vec::new();
        reader
            .read_until(b' ', &mut content)
            .expect("expect data to available");
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
