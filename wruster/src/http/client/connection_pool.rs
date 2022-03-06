
use std::hash::Hash;
use std::io::{Read, Write};
use std::collections::{HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;


pub enum Error {
    RoodtripError(String)
}

type Roundtrip<T: Read + Write + Send> = Box<dyn Fn(T) -> T>;
type Sourcer<T: Read + Write + Send> = Box<dyn Fn(String) -> T>;

struct Pool<T: Read + Write + Send> {
    connections: RwLock<HashMap<String,T>>,
    roundtriper: Roundtrip<T>,
    sourcer: Sourcer<T>
}

impl<T: Read + Write + Send> Pool<T> {
    pub fn new(roundtriper: Roundtrip<T>, sourcer: Sourcer<T> ) -> Self {
        let map: HashMap<String, T> =  HashMap::new();
        Pool{
            connections: RwLock::new(map),
            roundtriper: roundtriper,
            sourcer: sourcer,
        }
    }

    pub fn roundtrip(mut self, to: String) -> Result<(),Error> {
        let connections = self.connections.get_mut().unwrap();
        let connection = match connections.remove(&to) {
            Some(connection) => connection,
            None => (self.sourcer)(to.clone())
        };
        drop(connections);
        let connection = (self.roundtriper)(connection);
        let connections = self.connections.get_mut().unwrap();
        connections.insert(to, connection);
        Ok(())
    }
}

#[cfg(test)] 
mod test {
    use super::*;
   
    #[test]
    fn execs_roundtrip
}
