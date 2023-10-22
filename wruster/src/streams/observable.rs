use super::{cancellable_stream::CancellableStream, timeout_stream::Timeout, Stream};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock, Weak,
    },
};

/**
Wraps a [CancellableStream] so it can be included in a [ObservedStreamList].
See the [ObservedStreamList] documentation for more info.
*/
pub struct ObservableStream<T>
where
    T: Stream,
{
    observed: CancellableStream<T>,
    parent: Option<(usize, Weak<ObservedStreamList<T>>)>,
}

impl<T> ObservableStream<T>
where
    T: Stream,
{
    pub fn new(observed: CancellableStream<T>) -> ObservableStream<T> {
        ObservableStream {
            observed,
            parent: None,
        }
    }
}

impl<T> Drop for ObservableStream<T>
where
    T: Stream,
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

impl<T> Deref for ObservableStream<T>
where
    T: Stream,
{
    type Target = CancellableStream<T>;

    fn deref(&self) -> &Self::Target {
        &self.observed
    }
}

impl<T> DerefMut for ObservableStream<T>
where
    T: Stream,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.observed
    }
}

impl<T> From<CancellableStream<T>> for ObservableStream<T>
where
    T: Stream,
{
    fn from(it: CancellableStream<T>) -> Self {
        ObservableStream::new(it)
    }
}

/**
 Represents an [ObservableStream] that was added to an [ObservedStreamList],
 so it is returned by the method [ObservedStreamList.track].
* A [ObservedStream] is dereferenced to the [ObservableStream] being Observed.
* It can be cloned and the new ObservedStream will be also observed.
*/
pub struct ObservedStream<T>
where
    T: Stream,
{
    stream: Arc<ObservableStream<T>>,
}

impl<T> Clone for ObservedStream<T>
where
    T: Stream,
{
    fn clone(&self) -> Self {
        let stream = Arc::clone(&self.stream);
        Self { stream }
    }
}

impl<T> Deref for ObservedStream<T>
where
    T: Stream,
{
    type Target = ObservableStream<T>;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<T> Read for ObservedStream<T>
where
    T: Stream,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = &self.stream.observed;
        s.read(buf)
    }
}

impl<T> Write for ObservedStream<T>
where
    T: Stream,
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

impl<T> Timeout for ObservedStream<T>
where
    T: Stream,
{
    fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        self.stream.set_write_timeout(dur)
    }
}

/**
Allows to track a list of [ObservableStream], so whenever one of the Streams in the list, and all of its clones,
is dropped, it's automatically removed from the list. An [ObservableStream] is included in an [ObservedStreamList]
by calling the method: [ObservedStreamList.track].
*/
pub struct ObservedStreamList<T>
where
    T: Stream,
{
    items: RwLock<HashMap<usize, Weak<ObservableStream<T>>>>,
    next_key: AtomicUsize,
}

impl<T> ObservedStreamList<T>
where
    T: Stream,
{
    pub fn new() -> Arc<ObservedStreamList<T>> {
        let items = HashMap::<usize, Weak<ObservableStream<T>>>::new();
        let list = ObservedStreamList {
            items: RwLock::new(items),
            next_key: AtomicUsize::new(0),
        };
        Arc::new(list)
    }

    pub fn track(
        list: &Arc<ObservedStreamList<T>>,
        stream: CancellableStream<T>,
    ) -> ObservedStream<T> {
        let mut stream = ObservableStream::new(stream);
        let parent = Arc::downgrade(list);
        let key = list.next_key.fetch_add(1, Ordering::SeqCst);
        stream.parent = Some((key, parent));
        let stream = Arc::new(stream);
        let mut items = list.items.write().unwrap();
        items.insert(key, Arc::downgrade(&stream));
        ObservedStream { stream }
    }

    pub fn len(&self) -> usize {
        self.items.read().unwrap().len()
    }

    fn dropped(&self, key: usize) {
        let mut items = self.items.write().unwrap();
        items.remove(&key);
    }

    pub fn drain(&self) -> Vec<Weak<ObservableStream<T>>> {
        let mut items = self.items.write().unwrap();
        items.drain().map(|x| x.1).collect()
    }
}
