use super::{Action, PoolError};
use atomic_refcell::AtomicRefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

type DynamicWorkerFinished = Box<dyn FnOnce() + Send>;

struct DynamicWorker {
    id: usize,
    handle: Option<thread::JoinHandle<()>>,
    sender: Option<SyncSender<Action>>,
}

impl DynamicWorker {
    fn new(
        id: usize,
        timeout: Duration,
        first_action: Action,
        finished: DynamicWorkerFinished,
    ) -> DynamicWorker {
        let (sender, receiver) = sync_channel::<Action>(0);
        let initialized = Arc::new(AtomicBool::new(false));
        let t_initialized = Arc::clone(&initialized);
        let handle = std::thread::spawn(move || {
            // When the worker is created it will execute, at least, one action
            // so we don't want to timeout waiting for it.
            t_initialized.store(true, Ordering::SeqCst);
            first_action();
            loop {
                let res = receiver.recv_timeout(timeout);
                match res {
                    Ok(action) => {
                        action();
                        debug!("action executed");
                        continue;
                    }
                    Err(err) => match err {
                        RecvTimeoutError::Timeout => debug!("worker {} timeout", id),
                        RecvTimeoutError::Disconnected => debug!("worker {} disconnected", id),
                    },
                }
                finished();
                debug!("worker {} stopped", id.to_string());
                break;
            }
        });
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
}

impl Drop for DynamicWorker {
    fn drop(&mut self) {
        drop(self.sender.take());
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
        debug!("worker {} dropped", self.id);
    }
}

type DynamicWorkerElem = Option<DynamicWorker>;

pub struct Dynamic {
    workers: Vec<Arc<AtomicRefCell<DynamicWorkerElem>>>,
    timeout: Duration,
    free_cells: Arc<RwLock<VecDeque<usize>>>,
    max: usize,
}

impl Dynamic {
    pub fn new(max: usize, timeout: Duration) -> Dynamic {
        let mut workers: Vec<Arc<AtomicRefCell<DynamicWorkerElem>>> = Vec::with_capacity(max);
        let mut free_cells = VecDeque::new();
        for i in 0..max {
            let elem: Option<DynamicWorker> = None;
            let elem = Arc::new(AtomicRefCell::new(elem));
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

    fn try_add_worker(&mut self, action: Action) -> Result<usize, Action> {
        let mut free_cells = self.free_cells.write().unwrap();
        let index = match free_cells.pop_front() {
            Some(index) => index,
            None => return Err(action),
        };

        let free_cells = Arc::downgrade(&self.free_cells);
        let finished = move || {
            if let Some(free_cells) = free_cells.upgrade() {
                let mut free_cells = free_cells.write().unwrap();
                free_cells.push_back(index);
            }
        };
        let worker = DynamicWorker::new(index, self.timeout, action, Box::new(finished));
        self.workers[index] = Arc::new(AtomicRefCell::new(Some(worker)));
        Ok(index)
    }

    pub fn run(&mut self, action: Action) -> Result<(), PoolError> {
        // Try to add a new thread and run the Action.
        let mut action = match self.try_add_worker(action) {
            Ok(_) => return Ok(()),
            Err(action) => action,
        };
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
        Err(PoolError::Busy(action))
    }

    #[allow(dead_code)]
    pub fn number_of_workers(&self) -> usize {
        let free_cells = self.free_cells.read().unwrap().len();
        self.max - free_cells
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

        // Signal the first thread to finish.
        sender.send(()).unwrap();
        // Give time for worker to finish the task and be ready to accept
        // another action.
        // TODO: This test could be flaky.
        thread::sleep(Duration::from_secs(1));

        // Try run the second action.
        let result2 = Arc::new(Mutex::new(String::new()));
        let action_result2 = Arc::clone(&result2);
        let action2 = move || {
            let mut str_result = action_result2.lock().unwrap();
            *str_result = String::from("second done");
        };
        pool.run(Box::new(action2)).unwrap();
        // Ensure the second action is finished by dropping the pool.
        drop(pool);
        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }

    #[test]
    fn workers_are_dropped_after_timeout() {
        let mut pool = Dynamic::new(1, Duration::from_millis(1));
        let (sender, receiver) = channel::<()>();
        let (started_sender, started_rcv) = channel::<()>();
        let action: Action = Box::new(move || {
            started_sender.send(()).unwrap();
            receiver.recv().unwrap();
        });
        pool.run(action).unwrap();
        started_rcv.recv().unwrap();
        // Signal the first action to finish.
        sender.send(()).unwrap();
        // Wait the timeout and check the worker has finished.
        thread::sleep(Duration::from_millis(200));
        assert_eq!(pool.number_of_workers(), 0);
    }
}
