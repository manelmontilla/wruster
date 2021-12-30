use super::{Action, PoolError};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::os::unix::prelude::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

type DynamicWorkedFinished = Box<dyn FnOnce() + Send>;

pub enum DynamicPoolError {
    Busy(Action),
}

struct DynamicWorker {
    id: usize,
    handle: Option<thread::JoinHandle<()>>,
    sender: Option<SyncSender<Action>>,
}

impl DynamicWorker {
    fn new(id: usize, timeout: Duration, finished: DynamicWorkedFinished) -> DynamicWorker {
        let (sender, receiver) = sync_channel::<Action>(0);
        let initialized = Arc::new(AtomicBool::new(false));
        let t_initialized = Arc::clone(&initialized);
        let handle = std::thread::spawn(move || loop {
            t_initialized.store(true, Ordering::SeqCst);
            let res = receiver.recv_timeout(timeout);
            match res {
                Ok(action) => {
                    action();
                    debug!("action executed");
                    continue;
                }
                Err(err) => match err {
                    RecvTimeoutError::Timeout => debug!("worked timeout"),
                    RecvTimeoutError::Disconnected => debug!("worked disconnected"),
                },
            }
            finished();
            debug!("woker: {} stopped", id.to_string());
            break;
        });
        // Wait for the thread to be initialized.
        while !initialized.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        DynamicWorker {
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
                TrySendError::Disconnected(action) => Err(action),
            },
        }
    }

    fn retry_exec(&self, action: Action) {
        let sender = self.sender.as_ref().unwrap();
        let mut action = action;
        loop {
            action = match sender.try_send(action) {
                Ok(()) => return,
                Err(err) => match err {
                    TrySendError::Full(action) => action,
                    TrySendError::Disconnected(_) => unreachable!(),
                },
            };
        }
    }
}

impl Drop for DynamicWorker {
    fn drop(&mut self) {
        drop(self.sender.take());
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
    }
}

type DynamicWorkerElem = Option<DynamicWorker>;

pub struct Dynamic {
    workers: Vec<Arc<RefCell<DynamicWorkerElem>>>,
    timeout: Duration,
    free_cells: Arc<RwLock<VecDeque<usize>>>,
    max: usize,
}

impl Dynamic {
    pub fn new(max: usize, timeout: Duration) -> Dynamic {
        let mut workers: Vec<Arc<RefCell<DynamicWorkerElem>>> = Vec::with_capacity(max);
        let mut free_cells = VecDeque::new();
        for i in 0..max {
            let elem: Option<DynamicWorker> = None;
            let elem = Arc::new(RefCell::new(elem));
            workers.push(elem);
            free_cells.push_back(i);
        }
        let free_cells = Arc::new(RwLock::new(free_cells));
        Dynamic {
            workers,
            timeout,
            free_cells,
            max,
        }
    }

    pub fn try_add_worker(&mut self) -> Option<usize> {
        let mut free_cells = self.free_cells.write().unwrap();
        let index = match free_cells.pop_front() {
            Some(index) => index,
            None => return None,
        };

        let free_cells = Arc::downgrade(&self.free_cells);
        let finished = move || {
            if let Some(free_cells) = free_cells.upgrade() {
                let mut free_cells = free_cells.write().unwrap();
                free_cells.push_back(index);
            }
        };
        let worker = DynamicWorker::new(index, self.timeout, Box::new(finished));
        self.workers[index] = Arc::new(RefCell::new(Some(worker)));
        Some(index)
    }

    pub fn run(&mut self, action: Action) -> Result<(), PoolError> {
        // Try to add a new thread and run the Action.
        if let Some(index) = self.try_add_worker() {
            // We use retry_exec here, even though there is a low chance for
            // the thread in the new Worker not to being yet ready.
            let mut worker = self.workers[index].as_ref().borrow_mut();
            worker.as_mut().unwrap().retry_exec(action);
            return Ok(());
        };
        let mut action = action;
        // There is no room for adding more workers, try to see if any of the
        // current ones is not busy.
        for i in 0..self.max {
            let mut worker = self.workers[i].as_ref().borrow_mut();
            let worker = worker.as_mut();
            action = match worker {
                Some(worker) => match worker.exec(action) {
                    Ok(_) => return Ok(()),
                    Err(action) => action,
                },
                None => action,
            };
        }
        return Err(PoolError::Busy(action));
    }
}

impl Drop for Dynamic {
    fn drop(&mut self) {
        for worker in &*self.workers {
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
        let mut pool = Dynamic::new(1, Duration::from_secs(10));
        let (sender, receiver) = channel::<()>();
        let (started_sender, started_rcv) = channel::<()>();
        let action: Action = Box::new(move || {
            println!("runing long task");
            started_sender.send(()).unwrap();
            receiver.recv().unwrap();
        });
        let action2 = move || {
            unimplemented!();
        };
        pool.run(action).unwrap();
        started_rcv.recv().unwrap();
        // Try to run another action.
        pool.run(Box::new(action2)).expect_err("expected error");
        // Sginal the first thread to finish.
        sender.send(()).unwrap();
    }

    #[test]
    fn runs_an_action() {
        let mut pool = Dynamic::new(2, Duration::from_secs(10));
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        let action = move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        };
        pool.run(Box::new(action)).unwrap();
        // Droping the Dynamic ensures the action is finished.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "done");
    }

    #[test]
    fn runs_multiple_actions() {
        let mut pool = Dynamic::new(2, Duration::from_secs(10));
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
        // Droping the Dynamic forces ensdures the actions are executed.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "first done");

        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }

    #[test]
    fn runs_multiple_actions_in_one_worker() {
        let mut pool = Dynamic::new(1, Duration::from_secs(10));
        let (sender, receiver) = channel::<()>();
        let (started_sender, started_rcv) = channel::<()>();
        let action: Action = Box::new(move || {
            println!("runing long task");
            started_sender.send(()).unwrap();
            receiver.recv().unwrap();
        });
        pool.run(action).unwrap();
        started_rcv.recv().unwrap();
        // Sginal the first thread to finish.
        sender.send(()).unwrap();
        // Try run the second action.
        let result2 = Arc::new(Mutex::new(String::new()));
        let action_result2 = Arc::clone(&result2);
        let action2 = move || {
            let mut str_result = action_result2.lock().unwrap();
            *str_result = String::from("second done");
        };
        pool.run(Box::new(action2)).unwrap();
        // Droping the worker ensure the second action is finished.
        drop(pool);
        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }
}
