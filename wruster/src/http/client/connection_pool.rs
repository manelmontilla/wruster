use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::{thread, fmt};
use std::time::{Duration, Instant};

const DEFAULT_IDLE_RESOURCE_TIMEOUT: Duration = Duration::from_secs(30);
const EXPIRE_RESOURCE_CYCLE_TIME: Duration = Duration::from_secs(30);
const MAX_RESOURCES: usize = 100;



pub struct PoolResource<T> where T: Send {
    resource: T,
    lastUsed: Instant,
}

impl<T> PoolResource<T>  where T: Send {
    pub fn new(resource: T) -> Self {
        PoolResource {
            resource,
            lastUsed: Instant::now(),
        }
    }

    pub fn resource(&mut self) -> &mut T {
        &mut self.resource
    }
}

pub struct Pool<T> where T: Send {
    resources: Arc<RwLock<HashMap<String, PoolResource<T>>>>,
    expire_worker_handle: thread::JoinHandle<()>,
    expire_worker_finish: mpsc::Sender<()>,
}

impl<T> Pool<T> where T: Send + Sync + 'static {
    pub fn new(idle_timeout: Option<Duration>) -> Self {
        let resources = Arc::new(RwLock::new(HashMap::new()));
        let expire_worker_resources = Arc::clone(&resources);
        let (expire_worker_finish, recv) = mpsc::channel();
        let idle_timeout = match idle_timeout {
            Some(timeout) => timeout,
            None => DEFAULT_IDLE_RESOURCE_TIMEOUT.clone(),
        };
        let expire_worker_handle = thread::spawn(move || {
            Self::expire_connections(idle_timeout, expire_worker_resources, recv);
        });
        Pool {
            resources,
            expire_worker_handle,
            expire_worker_finish,
        }
    }

    pub fn expire_connections(
        idle_timeout: Duration,
        resources: Arc<RwLock<HashMap<String, PoolResource<T>>>>,
        finish: mpsc::Receiver<()>,
    ) {
        while let Err(err) = finish.try_recv() {
            if let mpsc::TryRecvError::Disconnected = err {
                break;
            }
            let mut resources = resources.write().unwrap();
            let now = Instant::now();
            let conns: Vec<(String, PoolResource<T>)> = resources.drain().collect();
            for (addr, conn) in conns {
                if now - conn.lastUsed < idle_timeout {
                    resources.insert(addr, conn);
                }
            }
            thread::sleep(EXPIRE_RESOURCE_CYCLE_TIME);
        }
    }

    pub fn get(&self, addr: &str) -> Option<PoolResource<T>> {
        let mut resources = self.resources.write().unwrap();
        match resources.remove(addr) {
            Some(conn) => Some(conn),
            _ => None,
        }
    }

    pub fn insert(
        &self,
        addr: &str,
        connection: PoolResource<T>,
    ) -> Option<()> {
        let mut connections = self.resources.write().unwrap();
        match connections.len() {
            MAX_RESOURCES => None,
            _ => {
                connections.insert(addr.to_string(), connection);
                Some(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn return_resource_if_exists() {
       let pool: Pool<&str> = Pool::new(Some(Duration::from_secs(2)));
       pool.insert("addr1", PoolResource::new("resource1"));
       let a = pool.get(addr).unwrap();
       let a = a.resource();

    }
}
