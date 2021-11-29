use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;

type Action = Box<dyn FnOnce() + Send>;

struct Worker {
    handle: Option<thread::JoinHandle<()>>,
    sender: Option<Sender<Action>>,
    busy: Arc<AtomicBool>,
}

impl Worker {
    fn new() -> Worker {
        let busy = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = channel::<Action>();
        let hbusy = Arc::clone(&busy);
        let handle = std::thread::spawn(move || loop {
            let res = receiver.recv();
            hbusy.store(true, Ordering::SeqCst);
            if let Ok(action) = res {
                action();
                hbusy.store(false, Ordering::SeqCst);
                debug!("action executed");
                continue;
            }
            debug!("worker stopped");
            break;
        });
        Worker {
            handle: Some(handle),
            sender: Some(sender),
            busy,
        }
    }

    fn exec(&self, action: Action) {
        let sender = self.sender.as_ref().unwrap();
        sender.send(action).unwrap();
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
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
    pub fn new(n: usize) -> Pool {
        let mut workers = Vec::with_capacity(n);
        for _ in 0..n {
            workers.push(Worker::new());
        }
        Pool {
            size: n,
            next: 0,
            workers,
        }
    }

    pub fn run(&mut self, action: Action) {
        if !self.workers[self.next].is_busy() {
            self.workers[self.next].exec(action);
            self.next = (self.next + 1) % self.size;
            return;
        }
        let mut from = (self.next + 1) % self.size;
        while self.workers[from].is_busy() && from != self.next {
            from = (from + 1) % self.size;
        }
        self.next = from;
        self.workers[self.next].exec(action);
        self.next = (self.next + 1) % self.size;
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
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn runs_an_action() {
        let mut pool = Pool::new(1);
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        let action = move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        };
        pool.run(Box::new(action));
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

        pool.run(Box::new(action));
        pool.run(Box::new(action2));
        // Droping the pool forces to be sure the action send to the pool is
        // already done.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "first done");

        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }
}
