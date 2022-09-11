use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::cancellable_stream::{BaseStream, CancellableStream};

pub trait Timeout {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
}

impl Timeout for &TcpStream {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let s = *self;
        s.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let s = *self;
        s.set_write_timeout(dur)
    }
}

impl Timeout for TcpStream {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(dur)
    }
}

impl<T> Timeout for &CancellableStream<T>
where
    T: BaseStream,
{
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let s = *self;
        s.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let s = *self;
        s.set_write_timeout(dur)
    }
}

impl<T> Timeout for CancellableStream<T>
where
    T: BaseStream,
{
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(dur)
    }
}

pub struct ArcStream<T>(Arc<CancellableStream<T>>)
where
    T: BaseStream;

impl<T> Timeout for &ArcStream<T>
where
    T: BaseStream,
{
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let s = &*self.0;
        s.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        let s = &*self.0;
        s.set_write_timeout(dur)
    }
}

impl<T> Read for &ArcStream<T>
where
    T: BaseStream,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = &*self.0;
        s.read(buf)
    }
}

impl<T> Write for &ArcStream<T>
where
    T: BaseStream,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut s = &*self.0;
        s.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut s = &*self.0;
        s.flush()
    }
}

pub struct TimeoutStream<T>
where
    T: Read + Write + Timeout,
{
    stream: T,
    read: Option<Duration>,
    write: Option<Duration>,
    ongoing_read: Option<Operation>,
    ongoing_write: Option<Operation>,
}

impl<T> TimeoutStream<T>
where
    T: Read + Write + Timeout,
{
    pub fn from(
        from: T,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> TimeoutStream<T> {
        TimeoutStream {
            stream: from,
            ongoing_read: None,
            ongoing_write: None,
            read: read_timeout,
            write: write_timeout,
        }
    }
}

impl<T> Read for TimeoutStream<T>
where
    T: Read + Write + Timeout,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If no timeout is defined for reading, we just pass through to the
        // underlaying reader.
        let stream = &mut self.stream;
        let timeout = match self.read {
            None => return stream.read(buf),
            Some(timeout) => timeout,
        };
        let read = match self.ongoing_read.as_mut() {
            None => {
                // if there is no current ongoing read operation create a new
                // one.
                self.ongoing_read = Some(Operation::from_timeout(timeout));
                self.ongoing_read.as_mut().unwrap()
            }
            Some(operation) => operation,
        };
        let next_timeout = read.next_timeout();
        if next_timeout.as_secs() == 0 {
            return Err(io::Error::from(io::ErrorKind::TimedOut));
        }
        let stream = &self.stream;
        if let Err(err) = stream.set_read_timeout(Some(next_timeout)) {
            return io::Result::Err(err);
        }
        read.start();
        let stream = &mut self.stream;
        let res = stream.read(buf).map_err(|err| {
            // REVIEW: In some cases a read operation over a stream returns
            // WouldBlock instead of the expected TimedOut.
            match err.kind() {
                ErrorKind::WouldBlock => io::Error::from(ErrorKind::TimedOut),
                _ => err,
            }
        });
        read.stop();
        res
    }
}

impl<T> io::Write for TimeoutStream<T>
where
    T: Read + Write + Timeout,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // If no timeout has set for write, we just pass through to the
        // underlaying writer.
        let stream = &mut self.stream;
        let timeout = match self.write {
            None => return stream.write(buf),
            Some(timeout) => timeout,
        };
        let write = match self.ongoing_write.as_mut() {
            None => {
                // If no timeout has set for write, we just pass through to the
                // underlaying writer.
                self.ongoing_write = Some(Operation::from_timeout(timeout));
                self.ongoing_write.as_mut().unwrap()
            }
            Some(operation) => operation,
        };
        let next_timeout = write.next_timeout();
        if next_timeout.as_secs() == 0 {
            return Err(io::Error::from(io::ErrorKind::TimedOut));
        }
        if let Err(err) = stream.set_write_timeout(Some(next_timeout)) {
            return io::Result::Err(err);
        }
        write.start();
        let res = stream.write(buf).map_err(|err| {
            // REVIEW: In some cases a read operation over a stream returns
            // WouldBlock instead of the expected TimedOut.
            match err.kind() {
                ErrorKind::WouldBlock => io::Error::from(ErrorKind::TimedOut),
                _ => err,
            }
        });
        write.stop();
        res
    }

    fn flush(&mut self) -> io::Result<()> {
        let stream = &mut self.stream;
        stream.flush()
    }
}

struct Operation {
    timeout: Duration,
    started: Option<Instant>,
    elapsed_secs: u64,
}

impl Operation {
    fn from_timeout(timeout: Duration) -> Operation {
        Operation {
            timeout,
            started: None,
            elapsed_secs: 0,
        }
    }

    fn next_timeout(&mut self) -> Duration {
        if self.elapsed_secs >= self.timeout.as_secs() {
            Duration::from_secs(0)
        } else {
            let remaining = self.timeout.as_secs() - self.elapsed_secs;
            Duration::from_secs(remaining)
        }
    }

    fn start(&mut self) {
        self.started = Some(Instant::now());
    }

    fn stop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_secs();
            self.elapsed_secs += elapsed;
        } else {
            println!("operation not initialized stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener};
    use std::thread;

    use super::*;

    #[test]
    fn enforces_read_timeouts() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let read_timeout = Duration::from_secs(3);
        let expected_timeout = read_timeout.clone();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut tstream = TimeoutStream::from(stream, Some(read_timeout), None);
            let mut reader = BufReader::new(&mut tstream);
            let mut content = Vec::new();
            reader.read_until(b' ', &mut content).unwrap();
            let first_read = String::from_utf8_lossy(&content);
            let mut content = Vec::new();
            let second_read_err = reader
                .read_until(b' ', &mut content)
                .expect_err("expected timeout error");
            (first_read.into_owned(), second_read_err)
        });

        let mut client = TcpClient::connect(addr.to_string()).unwrap();
        thread::sleep(Duration::from_secs(1));
        client.send("test ".as_bytes()).unwrap();
        thread::sleep(expected_timeout);
        client.send("this should not get read".as_bytes()).unwrap();
        let (received, err) = handle.join().unwrap();
        assert_eq!(received, "test ".to_string());
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    struct TcpClient {
        #[allow(dead_code)]
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
