use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

struct Worker<F>
where
    F: FnOnce(),
    F: Send + 'static,
{
    handle: Option<thread::JoinHandle<()>>,
    sender: Option<Sender<F>>,
}

impl<F> Worker<F>
where
    F: FnOnce(),
    F: Send + 'static,
{
    fn new() -> Worker<F> {
        let (sender, receiver) = channel::<F>();
        let handle = std::thread::spawn(move || loop {
            let res = receiver.recv();
            if let Ok(action) = res {
                action();
                continue;
            }
            break;
        });
        Worker {
            handle: Some(handle),
            sender: Some(sender),
        }
    }

    fn exec(&self, action: F) {
        if let Some(sender) = &self.sender {
            sender.send(action).unwrap();
        }
    }
}

impl<F: FnOnce() -> () + Send + 'static> Drop for Worker<F> {
    fn drop(&mut self) {
        drop(self.sender.take());
        println!("sender dropped");
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
    }
}

pub struct Pool<F>
where
    F: FnOnce(),
    F: Send + 'static,
{
    next: usize,
    workers: Arc<Mutex<Vec<Worker<F>>>>,
}

impl<F> Pool<F>
where
    F: FnOnce(),
    F: Send + 'static,
{
    pub fn new(n: usize) -> Pool<F> {
        let mut workers = Vec::with_capacity(n);
        for _ in 0..n {
            workers.push(Worker::new());
        }
        Pool {
            next: 0,
            workers: Arc::new(Mutex::new(workers)),
        }
    }

    pub fn run(&mut self, action: F) {
        self.workers.lock().unwrap()[0].exec(action);
    }
}

impl<F: FnOnce() -> () + Send + 'static> Drop for Pool<F> {
    fn drop(&mut self) {
        let workers = self.workers.lock().unwrap();
        for worker in &*workers {
            println!("dropping worker");
            drop(worker);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_an_action() {
        let mut pool = Pool::new(1);
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        pool.run(move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        });
        // Droping the pool forces to be sure the action send to the pool is
        // already done.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "done");
    }
}
