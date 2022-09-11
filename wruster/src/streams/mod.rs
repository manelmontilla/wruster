use std::{
    collections::HashMap,
    io::{self, Read, Write},
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock, Weak,
    },
};

pub mod cancellable_stream;
pub mod timeout_stream;

mod test_utils;

use timeout_stream::Timeout;

use self::cancellable_stream::CancellableStream;

pub struct ObservedStream {
    observed: CancellableStream,
    parent: Option<(usize, Weak<TrackedStreamList>)>,
}

impl ObservedStream {
    pub fn new(observed: CancellableStream) -> ObservedStream {
        ObservedStream {
            observed,
            parent: None,
        }
    }
}

impl Drop for ObservedStream {
    fn drop(&mut self) {
        let parent = match &self.parent {
            Some(it) => it,
            _ => return,
        };
        let key = parent.0;
        if let Some(parent) = parent.1.upgrade() {
            parent.dropped(key);
        }
    }
}

impl Deref for ObservedStream {
    type Target = CancellableStream;

    fn deref(&self) -> &Self::Target {
        &self.observed
    }
}

impl DerefMut for ObservedStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.observed
    }
}

impl From<CancellableStream> for ObservedStream {
    fn from(it: CancellableStream) -> Self {
        ObservedStream::new(it)
    }
}

pub struct TrackedStream {
    stream: Arc<ObservedStream>,
}

impl Clone for TrackedStream {
    fn clone(&self) -> Self {
        let stream = Arc::clone(&self.stream);
        Self { stream }
    }
}

impl Deref for TrackedStream {
    type Target = ObservedStream;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl Read for TrackedStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = &self.stream.observed;
        s.read(buf)
    }
}

impl Write for TrackedStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut s = &self.stream.observed;
        s.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut s = &self.stream.observed;
        s.flush()
    }
}

impl Timeout for TrackedStream {
    fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.stream.set_write_timeout(dur)
    }
}

pub struct TrackedStreamList {
    items: RwLock<HashMap<usize, Weak<ObservedStream>>>,
    next_key: AtomicUsize,
}

impl TrackedStreamList {
    pub fn new() -> Arc<TrackedStreamList> {
        let items = HashMap::<usize, Weak<ObservedStream>>::new();
        let list = TrackedStreamList {
            items: RwLock::new(items),
            next_key: AtomicUsize::new(0),
        };
        Arc::new(list)
    }

    pub fn track(list: &Arc<TrackedStreamList>, stream: CancellableStream) -> TrackedStream {
        let mut stream = ObservedStream::new(stream);
        let parent = Arc::downgrade(list);
        let key = list.next_key.fetch_add(1, Ordering::SeqCst);
        stream.parent = Some((key, parent));
        let stream = Arc::new(stream);
        let mut items = list.items.write().unwrap();
        items.insert(key, Arc::downgrade(&stream));
        TrackedStream { stream }
    }

    pub fn len(&self) -> usize {
        self.items.read().unwrap().len()
    }

    fn dropped(&self, key: usize) {
        let mut items = self.items.write().unwrap();
        items.remove(&key);
    }

    pub fn drain(&self) -> Vec<Weak<ObservedStream>> {
        let mut items = self.items.write().unwrap();
        items.drain().map(|x| x.1).collect()
    }
}

#[cfg(test)]
mod test {
    use super::timeout_stream::TimeoutStream;
    use super::*;
    use std::io::Read;
    use std::net::Shutdown;
    use std::net::TcpListener;
    use std::str::FromStr;
    use std::thread;
    use std::time::Duration;
    use test_utils::{get_free_port, TcpClient};

    #[test]
    fn test_shutdown_list() {
        let port = get_free_port();
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let read_timeout = Duration::from_secs(3);
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let stream = Box::new(stream);
            let cstream = CancellableStream::new(stream).unwrap();
            let track_list = TrackedStreamList::new();
            let stream_tracked = TrackedStreamList::track(&track_list, cstream);
            let cstream2 = stream_tracked.clone();
            assert_eq!(1, track_list.len());
            let handle = thread::spawn(move || {
                let mut data = String::from_str("").unwrap();
                let mut tstream = TimeoutStream::from(stream_tracked, Some(read_timeout), None);
                tstream
                    .read_to_string(&mut data)
                    .expect_err("expected error reading data");
            });
            cstream2.shutdown(Shutdown::Read).unwrap();
            handle.join().unwrap();
            drop(cstream2);
            assert_eq!(0, track_list.len());
        });
        let client = TcpClient::connect(addr.to_string()).unwrap();
        handle.join().unwrap();
        drop(client)
    }
}
