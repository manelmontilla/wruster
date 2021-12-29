use std::sync::atomic::{AtomicBool, Ordering, AtomicUsize};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use super::Worker;

type Action = Box<dyn FnOnce() + Send>;

#[derive(Debug)]
pub enum DynamicError {
    MaxReached,
}



struct DynamicWorker {
    id: usize,
    handle: Option<thread::JoinHandle<()>>,
}

impl DynamicWorker {
    fn new(id: usize, action: Action) -> DynamicWorker {
        let handle = std::thread::spawn(move || {
            action();
            debug!("action executed");
            println!("woker: {} stopped", id.to_string());
        });
        DynamicWorker {
            id,
            handle: Some(handle),
        }
    }
}

impl Drop for DynamicWorker {
    fn drop(&mut self) {
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
    }
}

pub struct Dynamic {
    max: usize,
    size: Arc<AtomicUsize>,
    workers: Arc<Mutex<Vec<DynamicWorker>>>
}

impl Dynamic {
    pub fn new(max: usize) -> Dynamic {
        let workers = Arc::new(Mutex::new(Vec::new()));
        Dynamic{
            max,
            size:Arc::new(AtomicUsize::new(0)),
            workers,
        }
    }

    pub fn run(&mut self, action: Action) -> Result<(), DynamicError> {
          
    }
}

impl Drop for Dynamic {
    fn drop(&mut self) {
    //     let workers = &self.workers;
    //     for worker in &*workers {
    //         #[allow(clippy::drop_ref)]
    //         drop(worker);
    //     }
    // }
}
