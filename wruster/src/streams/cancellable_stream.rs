use super::BaseStream;
use polling::Event;
use std::{
    io,
    net::Shutdown,
    sync::{
        atomic::{self, AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

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
        Ok(CancellableStream {
            stream,
            done,
            poller,
            read_timeout,
            write_timeout,
        })
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

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.done.store(true, Ordering::SeqCst);
        self.stream.shutdown(how)
    }

    fn read_int(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.poller
            .modify(self.stream.as_raw(), Event::readable(1))?;
        let mut events = Vec::new();
        let timeout = &self.read_timeout.write().unwrap().clone();
        let mut bytes_read = 0;
        let buf_len = buf.len();
        if self.poller.wait(&mut events, *timeout)? == 0 {
            let stop = self.done.load(atomic::Ordering::SeqCst);
            if stop {
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
                    return Err(io::Error::from(io::ErrorKind::NotConnected));
                }
                Ok(n) => {
                    bytes_read += n;
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => {
                    return Err(err);
                }
            };

            // TODO: Actually this is not correct, we should read all the
            // events returned by wait, even if we end up reading more bytes
            // than the len of the buffer provide by the caller.
            if bytes_read == buf_len {
                break;
            }
        }
        // If we were unable to read anything we signal the reader that is safe to retry
        // the operation by returning and error of kind :Interrupted.
        // Reference: https://doc.rust-lang.org/std/io/trait.Read.html#errors.
        if bytes_read == 0 {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        Ok(bytes_read)
    }

    fn write_int(&self, buf: &[u8]) -> io::Result<usize> {
        let mut events = Vec::new();
        let timeout = &self.write_timeout.write().unwrap().clone();
        let mut bytes_written = 0;
        let buf_len = buf.len();
        while bytes_written < buf_len {
            events.clear();
            self.poller
                .modify(self.stream.as_raw(), Event::writable(1))?;
            if self.poller.wait(&mut events, *timeout)? == 0 {
                let stop = self.done.load(atomic::Ordering::SeqCst);
                if stop {
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
                if evt.key != 1 || !evt.writable {
                    continue;
                }
                let write_buf = &buf[bytes_written..];
                let s = &self.stream;
                match s.write_buf(write_buf) {
                    Ok(n) => {
                        bytes_written += n;
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(err) => {
                        self.stream.set_nonblocking(false)?;
                        return Err(err);
                    }
                };
                break;
            }
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
