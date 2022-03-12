use atomic_refcell::AtomicRefCell;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    RoodtripError(String),
}

/*trait Roundtrip<T> where T: Read + Write + Send {
}*/

//type Roundtrip<T: Read + Write + Send> = Box<dyn Fn(T) -> T>;
// type Sourcer<T: Read + Write + Send> = Box<dyn Fn(String) -> T>;

pub struct Pool<T, F>
where
    T: Read + Write + Send,
    F: Fn(String) -> T,
{
    connections: RwLock<HashMap<String, T>>,
    sourcer: F,
}

impl<T, F> Pool<T, F>
where
    T: Read + Write + Send,
    F: Fn(String) -> T,
{
    pub fn new(sourcer: F) -> Self {
        let map: HashMap<String, T> = HashMap::new();
        Pool {
            connections: RwLock::new(map),
            sourcer: sourcer,
        }
    }

    pub fn roundtrip<G>(mut self, to: String, roundtriper: G) -> Result<(), Error>
    where
        G: FnOnce(T) -> T + Send + 'static,
    {
        let connections = self.connections.borrow_mut();
        let connections = connections.get_mut().unwrap();
        let connection = match connections.remove(&to) {
            Some(connection) => connection,
            None => (self.sourcer)(to.clone()),
        };
        // We release the lock here so we don't block all the connections while
        // executing the roundtrip.
        drop(connections);
        let connection = (roundtriper)(connection);
        // Return the connection to the pool.
        let connections = self.connections.borrow_mut();
        let connections = connections.get_mut().unwrap();
        connections.insert(to, connection);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    // use std::net::TcpStream;

    use super::*;

    fn copy_processor(src: &[u8], dst: &mut Vec<u8>) -> std::io::Result<usize> {
        for i in 0..src.len() {
            dst.push(src[i])
        }
        Ok(src.len())
    }

    #[test]
    fn test() {
        let sourcer =
            |_: String| -> Processor<fn(&[u8], &mut Vec<u8>) -> Result<usize, std::io::Error>> {
                Processor::new(copy_processor)
            };
        let pool = Pool::new(sourcer);
        // type Roundtrip<T: Read + Write + Send> = Box<dyn Fn(T) -> T>;
        let roundtrip =
            |mut p: Processor<fn(&[u8], &mut Vec<u8>) -> Result<usize, std::io::Error>>| {
                let w = &mut p;
                w.write("never gonna give you up".as_bytes()).unwrap();
                p
            };
        pool.roundtrip("a".to_string(), roundtrip)
            .unwrap();
    }

    use std::{
        collections::VecDeque,
        io::{Read, Write},
    };

    struct Processor<F>
    where
        F: Fn(&[u8], &mut Vec<u8>) -> std::io::Result<usize>,
    {
        out: VecDeque<u8>,
        process: F,
    }

    impl<F> Processor<F>
    where
        F: Fn(&[u8], &mut Vec<u8>) -> std::io::Result<usize>,
    {
        fn new(processor: F) -> Self {
            Processor {
                out: VecDeque::new(),
                process: processor,
            }
        }
    }

    impl<F> Write for Processor<F>
    where
        F: Fn(&[u8], &mut Vec<u8>) -> std::io::Result<usize>,
    {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut out = Vec::new();
            (self.process)(buf, &mut out)?;
            for i in 0..out.len() {
                self.out.push_back(out[i]);
            }
            Ok(out.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<F> Read for Processor<F>
    where
        F: Fn(&[u8], &mut Vec<u8>) -> std::io::Result<usize>,
    {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut i = 0;
            while i < buf.len() {
                match self.out.pop_front() {
                    Some(data) => buf[i] = data,
                    None => break,
                };
                i += 1
            }
            Ok(i)
        }
    }
}
