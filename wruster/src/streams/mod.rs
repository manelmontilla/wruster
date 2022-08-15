use std::{
    collections::{hash_map, HashMap},
    io::{self, Read, Write},
    ops::Deref,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock, Weak,
    },
};

use std::collections::hash_map::Iter;

pub mod cancellable_stream;
pub mod timeout_stream;

use timeout_stream::Timeout;

pub struct SyncStream<T: io::Read + io::Write + Timeout> {
    read_writer: T,
    parent: Option<(usize, Weak<TrackedStreamList<T>>)>,
}

impl<T> SyncStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    pub fn new(read_writer: T) -> SyncStream<T> {
        SyncStream {
            read_writer,
            parent: None,
        }
    }
}

impl<T> io::Read for SyncStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_writer.read(buf)
    }
}

impl<T> io::Write for SyncStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.read_writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.read_writer.flush()
    }
}

impl<T> Timeout for SyncStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.read_writer.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.read_writer.set_write_timeout(dur)
    }
}

impl<T> From<T> for SyncStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn from(it: T) -> Self {
        SyncStream::new(it)
    }
}

pub trait Dropped {
    fn dropped(&self, key: usize);
}

pub struct TrackedStream<T>
where
    T: Read + Write + Timeout,
{
    stream: Arc<RwLock<SyncStream<T>>>,
}

impl<T> Clone for TrackedStream<T>
where
    T: Read + Write + Timeout,
{
    fn clone(&self) -> Self {
        let stream = Arc::clone(&self.stream);
        Self { stream }
    }
}

impl<T> Drop for SyncStream<T>
where
    T: Read + Write + Timeout,
{
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

impl<T> io::Read for TrackedStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut reader = self.stream.write().unwrap();
        reader.read(buf)
    }
}

impl<T> io::Write for TrackedStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut writer = self.stream.write().unwrap();
        writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut writer = self.stream.write().unwrap();
        writer.flush()
    }
}

impl<T> Timeout for TrackedStream<T>
where
    T: io::Read + io::Write + Timeout,
{
    fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        let reader = self.stream.read().unwrap();
        reader.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        let reader = self.stream.read().unwrap();
        reader.set_write_timeout(dur)
    }
}

pub struct TrackedStreamList<T>
where
    T: Read + Write + Timeout,
{
    items: RwLock<HashMap<usize, Weak<RwLock<SyncStream<T>>>>>,
    next_key: AtomicUsize,
}

impl<T> TrackedStreamList<T>
where
    T: Read + Write + Timeout,
{
    pub fn new() -> Arc<TrackedStreamList<T>> {
        let items = HashMap::<usize, Weak<RwLock<SyncStream<T>>>>::new();
        let list = TrackedStreamList {
            items: RwLock::new(items),
            next_key: AtomicUsize::new(0),
        };
        Arc::new(list)
    }

    pub fn track(list: &Arc<TrackedStreamList<T>>, stream: T) -> TrackedStream<T> {
        let mut stream = SyncStream::new(stream);
        let parent = Arc::downgrade(list);
        let key = list.next_key.fetch_add(1, Ordering::SeqCst);
        stream.parent = Some((key, parent));
        let stream = Arc::new(RwLock::new(stream));
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
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{Read, Write};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Weak};
    use std::thread;

    use crate::art::*;
    use crate::http::{Body, Request, Response};
    use crate::streams::SyncStream;

    use super::timeout_stream::Timeout;

    struct Dummy {
        data: RwLock<Vec<u8>>,
        finished: AtomicBool,
    }

    impl Dummy {
        pub fn signal(&self) {
            self.finished.store(true, Ordering::SeqCst)
        }
    }

    impl Read for Dummy {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let d = self.data.write().unwrap();
            let mut d = &d[..];
            d.read(buf)
        }
    }

    impl Write for Dummy {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut d = self.data.write().unwrap();
            for v in buf {
                d.push(*v);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Timeout for Dummy {
        fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
            Ok(())
        }

        fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_resource_list() {
        let dummy = Dummy {
            data: RwLock::new(Vec::new()),
            finished: AtomicBool::new(false),
        };
        let track_list = TrackedStreamList::<Dummy>::new();

        let mut dummy_tracked = TrackedStreamList::track(&track_list, dummy);
        let dummy_tracked1 = dummy_tracked.clone();
        println!("data before {:}", &track_list.len());
        let handle = thread::spawn(move || {
            let dummy_tracked2 = dummy_tracked1.clone();
            let request = Request::read_from(dummy_tracked2).unwrap();
            let mut body = request.body.unwrap();
            let mut request_data = String::new();
            body.content.read_to_string(&mut request_data).unwrap();
            assert_eq!(request_data, "request data");
        });
        let body = Body::from("data", mime::TEXT_PLAIN);
        let request = Request::from_body(body, crate::http::HttpMethod::POST, "/");
        request.write(&mut dummy_tracked).unwrap();

        handle.join().unwrap();
        assert_eq!(track_list.len(), 1);
        let _ = &dummy_tracked.stream;
        drop(dummy_tracked);
        assert_eq!(track_list.len(), 0);
    }
}
