use super::{Action, PoolError, Worker};
use std::borrow::BorrowMut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

type DynamicWorkedFinished = Box<dyn FnOnce() + Send>;

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
                TrySendError::Disconnected(_) => unreachable!(),
            },
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

type DynamicWorkerElem = Mutex<Option<DynamicWorker>>;

pub struct Dynamic {
    workers: Vec<Arc<DynamicWorkerElem>>,
    timeout: Duration,
}

impl Dynamic {
    pub fn new(max: usize, timeout: Duration) -> Dynamic {
        let mut workers: Vec<Arc<DynamicWorkerElem>> = Vec::with_capacity(max);
        for i in 0..max {
            let elem: Option<DynamicWorker> = None;
            let elem = Arc::new(Mutex::new(elem));
            workers.push(elem)
        }
        Dynamic { workers, timeout }
    }

    pub fn add_worker(&mut self, pos: usize) {
        let cell = Arc::downgrade(&self.workers[pos]);
        let finished = move || {
            if let Some(cell) = cell.upgrade() {
                let cell = cell.lock().unwrap();
                *cell = None;
            }
        };
        let worker = DynamicWorker::new(pos, self.timeout, Box::new(finished));
        let cell = &self.workers[pos].lock().unwrap();
        *cell.borrow_mut().insert(worker);
    }
    
    

    // pub fn run(&mut self, action: Action) -> Result<(), PoolError> {
    //     let mut action = match self.DynamicWorkers[self.next].exec(action) {
    //         Ok(_) => {
    //             debug!(
    //                 "run: current DynamicWorker: {} not busy",
    //                 self.DynamicWorkers[self.next].id.to_string()
    //             );
    //             self.next = (self.next + 1) % self.size;
    //             return Ok(());
    //         }
    //         Err(action) => action,
    //     };
    //     let mut from = (self.next + 1) % self.size;
    //     loop {
    //         action = match self.DynamicWorkers[from].exec(action) {
    //             Ok(_) => break,
    //             Err(action) => action,
    //         };
    //         from = (from + 1) % self.size;
    //         if from == self.next {
    //            return Err(PoolError::Busy(action));
    //         }
    //     }
    //     self.next = from;
    //     debug!(
    //         "run: found the DynamicWorker: {}, that is not busy",
    //         self.DynamicWorkers[self.next].id.to_string()
    //     );
    //     self.next = (self.next + 1) % self.size;
    //     Ok(())
    // }
}

impl Drop for Dynamic {
    fn drop(&mut self) {
        let DynamicWorkers = &self.DynamicWorkers;
        for DynamicWorker in &*DynamicWorkers {
            #[allow(clippy::drop_ref)]
            drop(DynamicWorker);
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
        let mut Dynamic = Dynamic::new(1);
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
        Dynamic.run(action).unwrap();
        started_rcv.recv().unwrap();
        // Try to run another action.
        Dynamic.run(Box::new(action2)).expect_err("expected error");

        // Sginal the first thread to finish.
        sender.send(()).unwrap();
    }

    #[test]
    fn runs_an_action() {
        let mut Dynamic = Dynamic::new(1);
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        let action = move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        };
        Dynamic.run(Box::new(action)).unwrap();
        // Droping the Dynamic ensures the action is finished.
        drop(Dynamic);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "done");
    }

    #[test]
    fn runs_multiple_actions() {
        let mut Dynamic = Dynamic::new(2);
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

        Dynamic.run(Box::new(action)).unwrap();
        Dynamic.run(Box::new(action2)).unwrap();
        // Droping the Dynamic forces ensdures the actions are executed.
        drop(Dynamic);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "first done");

        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }
}
