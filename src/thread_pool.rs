use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

struct Pool {
    sender: SyncSender<usize>,
    receiver: Receiver<usize>,
    current: Arc<Mutex<usize>>,
}

impl Pool {
    fn new(n: usize) -> Pool {
        let (send, receive) = sync_channel::<usize>(n);
        for i in 0..n {
            send.send(i).unwrap();
        }
        Pool {
            sender: send,
            current: Arc::new(Mutex::new(0)),
            receiver: receive,
        }
    }

    fn run<F>(&mut self, action: F) -> thread::JoinHandle<()>
    where
        F: FnOnce(),
        F: Send + 'static,
    {
        let token = self.receiver.recv().unwrap();
        let sender = self.sender.clone();
        let current = Arc::clone(&self.current);
        self.incr_running();
        thread::spawn(move || {
            action();
            let mut d = current.lock().unwrap();
            *d = *d - 1;
            sender.send(token).unwrap();
        })
    }

    fn current(&self) -> usize {
        *self.current.lock().unwrap()
    }

    fn incr_running(&self) {
        let current = Arc::clone(&self.current);
        let mut d = current.lock().unwrap();
        *d = *d + 1;
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
        let handler = pool.run(move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        });
        handler.join().unwrap();
        let result = &*result.lock().unwrap();
        assert_eq!(result, "done");
    }

    #[test]
    fn runs_n_actions() {
        let mut pool = Pool::new(2);
        let result = Arc::new(Mutex::new(0));

        let action_result = Arc::clone(&result);
        let hold_result = result.lock().unwrap();

        let handler1 = pool.run(move || {
            let mut d = action_result.lock().unwrap();
            *d = *d + 1;
        });

        let action_result2 = Arc::clone(&result);
        let handler2 = pool.run(move || {
            let mut d = action_result2.lock().unwrap();
            *d = *d + 1;
        });

        let n = pool.current();
        assert_eq!(n, 2);
        drop(hold_result);
        handler2.join().unwrap();
        handler1.join().unwrap();
        let d = *result.lock().unwrap();
        assert_eq!(d, 2);
    }
}
