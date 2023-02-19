use std::{fmt::Debug, time::Duration};

mod dynamic_pool;
mod static_pool;

use self::{dynamic_pool::Dynamic, static_pool::Static};

type Action = Box<dyn FnOnce() + Send + 'static>;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub enum PoolError {
    Busy(Action),
}

impl Debug for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy(_) => write!(f, "Debug"),
        }
    }
}

pub struct Pool {
    dynamic: Option<Dynamic>,
    stat: Option<Static>,
}

impl Pool {
    pub fn new(min: usize, max: usize) -> Pool {
        assert!(min > 0 || max > 0);
        let mut stat = None;
        if min > 0 {
            stat = Some(Static::new(min));
        }
        let mut dynamic: Option<Dynamic> = None;
        if min < max {
            dynamic = Some(Dynamic::new(max - min, DEFAULT_TIMEOUT));
        }
        Pool { dynamic, stat }
    }

    pub fn run(&mut self, action: Action) -> Result<(), PoolError> {
        let mut action = action;
        if let Some(stat) = self.stat.as_mut() {
            action = match stat.run(action) {
                Ok(_) => return Ok(()),
                Err(err) => match err {
                    PoolError::Busy(action) => action,
                },
            };
        };
        match self.dynamic.as_mut() {
            Some(dynamic) => dynamic.run(action),
            None => Err(PoolError::Busy(action)),
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
    fn accepts_max_less_than_min() {
        let pool = Pool::new(1, 0);
        assert!(pool.dynamic.is_none());
        assert!(pool.stat.is_some());
    }

    #[test]
    fn accepts_min_zero() {
        let pool = Pool::new(0, 1);
        assert!(pool.dynamic.is_some());
        assert!(pool.stat.is_none());
    }

    #[test]
    fn returns_busy_error() {
        let mut pool = Pool::new(1, 2);

        // Run and pause one action.
        let (sender, receiver) = channel::<()>();
        let (worker_started_sender, worker_started_rcv) = channel::<()>();
        let action: Action = Box::new(move || {
            worker_started_sender.send(()).unwrap();
            receiver.recv().unwrap();
        });
        pool.run(action).unwrap();
        worker_started_rcv.recv().unwrap();

        // Run and pause another action.
        let (sender1, receiver1) = channel::<()>();
        let (worker_started_sender1, worker_started_rcv1) = channel::<()>();
        let action: Action = Box::new(move || {
            worker_started_sender1.send(()).unwrap();
            receiver1.recv().unwrap();
        });
        pool.run(action).unwrap();
        worker_started_rcv1.recv().unwrap();

        // Try to run another action.
        let action3 = move || {
            unimplemented!();
        };
        pool.run(Box::new(action3)).expect_err("expected error");
        // Unblock the running actions.
        sender.send(()).unwrap();
        sender1.send(()).unwrap();
    }

    #[test]
    fn runs_an_action() {
        let mut pool = Pool::new(1, 1);
        let result = Arc::new(Mutex::new(String::new()));
        let action_result = Arc::clone(&result);
        let action = move || {
            let mut str_result = action_result.lock().unwrap();
            *str_result = String::from("done");
        };
        pool.run(Box::new(action)).unwrap();
        // Dropping the pool ensures the action is finished.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "done");
    }

    #[test]
    fn runs_multiple_actions() {
        let mut pool = Pool::new(1, 2);
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
        // Drop to pool to ensure the actions are finished.
        drop(pool);
        let result = &*result.lock().unwrap();
        assert_eq!(result, "first done");

        let result2 = &*result2.lock().unwrap();
        assert_eq!(result2, "second done");
    }
}
