use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::{rc, thread};

type Action = Box<dyn FnOnce() + Send>;

#[derive(Debug)]
pub enum PoolError {
    Busy,
}

struct Worker {
    id: usize,
    handle: Option<thread::JoinHandle<()>>,
    sender: Option<SyncSender<Action>>,
}

impl Worker {
    fn new(id: usize) -> Worker {
        let (sender, receiver) = sync_channel::<Action>(0);
        let handle = std::thread::spawn(move || loop {
            let res = receiver.recv();
            if let Ok(action) = res {
                action();
                debug!("action executed");
                continue;
            }
            debug!("woker: {} stopped", id.to_string());
            break;
        });
        Worker {
            id,
            handle: Some(handle),
            sender: Some(sender),
        }
    }

    fn exec(&self, action: Action) -> Result<(), Action> {
        let sender = self.sender.as_ref().unwrap();
        match sender.try_send(action) {
            Ok(()) => Ok(()),
            Err(err) => match err {
                TrySendError::Full(action) => Err(action),
                TrySendError::Disconnected(_) => unreachable!(),
            },
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        drop(self.sender.take());
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
    }
}

pub struct Pool {
    size: usize,
    next: usize,
    workers: Vec<Worker>,
}

impl Pool {
    pub fn new(size: usize) -> Pool {
        let mut workers = Vec::with_capacity(size);
        for i in 0..size {
            workers.push(Worker::new(i));
        }
        Pool {
            size,
            next: 0,
            workers,
        }
    }

    pub fn run(&mut self, action: Action) -> Result<(), PoolError> {
        let mut action = match self.workers[self.next].exec(action) {
            Ok(_) => {
                debug!(
                    "run: current worker: {} not busy",
                    self.workers[self.next].id.to_string()
                );
                self.next = (self.next + 1) % self.size;
                return Ok(());
            }
            Err(action) => action,
        };
        let mut from = (self.next + 1) % self.size;
        while from != self.next {
            action = match self.workers[from].exec(action) {
                Ok(_) => break,
                Err(action) => action,
            };
            from = (from + 1) % self.size;
        }
        if from == self.next {
            return Err(PoolError::Busy);
        }
        self.next = from;
        debug!(
            "run: found the worker: {}, that is not busy",
            self.workers[self.next].id.to_string()
        );
        self.next = (self.next + 1) % self.size;
        Ok(())
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        let workers = &self.workers;
        for worker in &*workers {
            #[allow(clippy::drop_ref)]
            drop(worker);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn returns_busy_error() {
        let mut pool = Pool::new(1);
        let (sender, receiver) = channel::<()>();
        let (worker_started_sender, worker_started_rcv) = channel::<()>();
        let action = move || {
            println!("runing long task");
            worker_started_sender.send(()).unwrap();
            receiver.recv().unwrap();
        };
        let action2 = move || {
            unimplemented!();
        };

        pool.run(Box::new(action)).unwrap();
        // Ensure the worker already started.
        worker_started_rcv.recv().unwrap();
        // Try to run another action.
        pool.run(Box::new(action2)).expect_err("expected error");

        // Sginal the first thread to finish.
        sender.send(()).unwrap();
    }

    #[test]
    fn runs_an_action() {
        let mut pool = Pool::new(1);
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        let action = move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        };
        pool.run(Box::new(action)).unwrap();
        // Droping the pool ensures the action is finished.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "done");
    }

    #[test]
    fn runs_multiple_actions() {
        let mut pool = Pool::new(2);
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        let action = move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("first done");
        };

        let result2 = Arc::new(Mutex::new(String::new()));
        let action_result2 = Arc::clone(&result2);
        let action2 = move || {
            let mut str_result = action_result2.lock().unwrap();
            *str_result = String::from("second done");
        };

        pool.run(Box::new(action)).unwrap();
        pool.run(Box::new(action2)).unwrap();
        // Droping the pool forces to be sure the action send to the pool is
        // already done.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "first done");

        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }
}
