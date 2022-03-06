use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

pub enum Error {
    RoodtripError(String),
}

type Roundtrip<T: Read + Write + Send> = Box<dyn Fn(T) -> T>;
type Sourcer<T: Read + Write + Send> = Box<dyn Fn(String) -> T>;

struct Pool<T: Read + Write + Send> {
    connections: RwLock<HashMap<String, T>>,
    sourcer: Sourcer<T>,
}

impl<T: Read + Write + Send> Pool<T> {
    pub fn new(sourcer: Sourcer<T>) -> Self {
        let map: HashMap<String, T> = HashMap::new();
        Pool {
            connections: RwLock::new(map),
            sourcer: sourcer,
        }
    }

    pub fn roundtrip(mut self, to: String, roundtriper: Roundtrip<T>) -> Result<(), Error> {
        let connections = self.connections.get_mut().unwrap();
        let connection = match connections.remove(&to) {
            Some(connection) => connection,
            None => (self.sourcer)(to.clone()),
        };
        drop(connections);
        let connection = (roundtriper)(connection);
        let connections = self.connections.get_mut().unwrap();
        connections.insert(to, connection);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    // use std::net::TcpStream;

    // use super::*;

    fn copy_processor(src: &[u8], dst: &mut Vec<u8>) -> std::io::Result<usize> {
        todo!()
    }

    #[test]
    fn test() {
        let processor = Processor::new(copy_processor);
        // let sourcer = |addr: String| -> Vec<u8> {
        //     Vec::<u8>::new()
        // };
        // let pool = Pool::new(Box::new(sourcer));
        // pool.roundtrip("a", roundtriper)
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
