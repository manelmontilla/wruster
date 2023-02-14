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
pub mod tls;

#[cfg(test)]
mod test;
mod test_utils;

use timeout_stream::Timeout;

use self::cancellable_stream::{BaseStream, CancellableStream};

pub struct ObservedStream<T>
where
    T: BaseStream,
{
    observed: CancellableStream<T>,
    parent: Option<(usize, Weak<TrackedStreamList<T>>)>,
}

impl<T> ObservedStream<T>
where
    T: BaseStream,
{
    pub fn new(observed: CancellableStream<T>) -> ObservedStream<T> {
        ObservedStream {
            observed,
            parent: None,
        }
    }
}

impl<T> Drop for ObservedStream<T>
where
    T: BaseStream,
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

impl<T> Deref for ObservedStream<T>
where
    T: BaseStream,
{
    type Target = CancellableStream<T>;

    fn deref(&self) -> &Self::Target {
        &self.observed
    }
}

impl<T> DerefMut for ObservedStream<T>
where
    T: BaseStream,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.observed
    }
}

impl<T> From<CancellableStream<T>> for ObservedStream<T>
where
    T: BaseStream,
{
    fn from(it: CancellableStream<T>) -> Self {
        ObservedStream::new(it)
    }
}

pub struct TrackedStream<T>
where
    T: BaseStream,
{
    stream: Arc<ObservedStream<T>>,
}

impl<T> Clone for TrackedStream<T>
where
    T: BaseStream,
{
    fn clone(&self) -> Self {
        let stream = Arc::clone(&self.stream);
        Self { stream }
    }
}

impl<T> Deref for TrackedStream<T>
where
    T: BaseStream,
{
    type Target = ObservedStream<T>;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<T> Read for TrackedStream<T>
where
    T: BaseStream,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = &self.stream.observed;
        s.read(buf)
    }
}

impl<T> Write for TrackedStream<T>
where
    T: BaseStream,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut s = &self.stream.observed;
        s.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut s = &self.stream.observed;
        s.flush()
    }
}

impl<T> Timeout for TrackedStream<T>
where
    T: BaseStream,
{
    fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.stream.set_write_timeout(dur)
    }
}

pub struct TrackedStreamList<T>
where
    T: BaseStream,
{
    items: RwLock<HashMap<usize, Weak<ObservedStream<T>>>>,
    next_key: AtomicUsize,
}

impl<T> TrackedStreamList<T>
where
    T: BaseStream,
{
    pub fn new() -> Arc<TrackedStreamList<T>> {
        let items = HashMap::<usize, Weak<ObservedStream<T>>>::new();
        let list = TrackedStreamList {
            items: RwLock::new(items),
            next_key: AtomicUsize::new(0),
        };
        Arc::new(list)
    }

    pub fn track(
        list: &Arc<TrackedStreamList<T>>,
        stream: CancellableStream<T>,
    ) -> TrackedStream<T> {
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

    pub fn drain(&self) -> Vec<Weak<ObservedStream<T>>> {
        let mut items = self.items.write().unwrap();
        items.drain().map(|x| x.1).collect()
    }
}
