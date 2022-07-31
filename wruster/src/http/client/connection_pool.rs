use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_IDLE_RESOURCE_TIMEOUT: Duration = Duration::from_secs(30);
const EXPIRE_RESOURCE_CYCLE_TIME: Duration = Duration::from_secs(15);
const MAX_RESOURCES: usize = 100;

pub struct PoolResource<T>
where
    T: Send + Sync + 'static,
{
    resource: T,
    last_used: Instant,
}

impl<T> PoolResource<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(resource: T) -> Self {
        PoolResource {
            resource,
            last_used: Instant::now(),
        }
    }

    pub fn resource(self) -> T {
        self.resource
    }
}

impl<T> Deref for PoolResource<T>
where
    T: Send + Sync + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

impl<T> DerefMut for PoolResource<T>
where
    T: Send + Sync + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.resource
    }
}

pub struct Pool<T>
where
    T: Send + Sync + 'static,
{
    resources: Arc<RwLock<HashMap<String, PoolResource<T>>>>,
    expire_worker_handle: Option<thread::JoinHandle<()>>,
    expire_worker_stop: Arc<AtomicBool>,
}

impl<T> Pool<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(idle_timeout: Option<Duration>) -> Self {
        let resources = Arc::new(RwLock::new(HashMap::new()));
        let expire_worker_resources = Arc::clone(&resources);
        let expire_worker_stop = Arc::new(AtomicBool::new(false));
        let idle_timeout = match idle_timeout {
            Some(timeout) => timeout,
            None => DEFAULT_IDLE_RESOURCE_TIMEOUT.clone(),
        };
        let expire_worker_stop2 = Arc::clone(&expire_worker_stop);
        let expire_worker_handle = thread::spawn(move || {
            Self::expire_connections(idle_timeout, expire_worker_resources, expire_worker_stop2);
        });
        let expire_worker_handle = Some(expire_worker_handle);
        Pool {
            resources,
            expire_worker_handle,
            expire_worker_stop,
        }
    }

    fn expire_connections(
        idle_timeout: Duration,
        resources: Arc<RwLock<HashMap<String, PoolResource<T>>>>,
        stop: Arc<AtomicBool>,
    ) {
        while !stop.load(Ordering::Acquire) {
            let mut resources = resources.write().unwrap();
            let now = Instant::now();
            let conns: Vec<(String, PoolResource<T>)> = resources.drain().collect();
            for (addr, conn) in conns {
                if now - conn.last_used < idle_timeout {
                    resources.insert(addr, conn);
                }
            }
            drop(resources);
            thread::park_timeout(EXPIRE_RESOURCE_CYCLE_TIME);
        }
    }

    pub fn get(&self, key: &str) -> Option<PoolResource<T>> {
        let mut resources = self.resources.write().unwrap();
        match resources.remove(key) {
            Some(conn) => Some(conn),
            _ => None,
        }
    }

    pub fn insert(&self, key: &str, connection: PoolResource<T>) {
        let mut connections = self.resources.write().unwrap();
        let connections = connections.borrow_mut();
        match connections.len() {
            MAX_RESOURCES => Self::remove_LRU(connections),
            _ => {
                connections.insert(key.to_string(), connection);
            }
        }
    }

    fn remove_LRU(connections: &mut HashMap<String, PoolResource<T>>) {
        // TODO: use a priority queue sorted by last_time to make this
        // operation O(1) instead of O(N).
        let conns: Vec<(String, PoolResource<T>)> = connections.drain().collect();
        let mut least_used_addr: String = "".into();
        let mut least_used_conn_time: Option<Instant> = None;
        for (addr, conn) in conns {
            match least_used_conn_time {
                Some(last_used) => {
                    if conn.last_used < last_used {
                        least_used_addr = addr.to_string();
                        least_used_conn_time = Some(conn.last_used);
                    }
                }
                None => {
                    least_used_addr = addr.to_string();
                    least_used_conn_time = Some(conn.last_used);
                }
            };
            connections.insert(addr, conn);
        }
        match connections.remove(&least_used_addr) {
            Some(_) => (),
            None => unreachable!(),
        }
    }
}

impl<T> Drop for Pool<T>
where
    T: Send + Sync + 'static,
{
    fn drop(&mut self) {
        let stop_worker = &self.expire_worker_stop;
        stop_worker.store(true, Ordering::Release);
        let handle = &mut self.expire_worker_handle;
        let handle = handle.take().unwrap();
        handle.thread().unpark();
        handle.join().unwrap();
    }
}

trait EnsureThreadShareable: Send + Sync {}

impl EnsureThreadShareable for Pool<String> {}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn returns_resource() {
        let pool: Pool<&str> = Pool::new(Some(Duration::from_secs(2)));
        pool.insert("addr1", PoolResource::new("resource1"));
        let pool_resource = pool.get("addr1").unwrap();
        let resource = &*pool_resource.resource();
        assert_eq!(resource, "resource1")
    }

    #[test]
    fn stops_the_worker_when_dropped() {
        let pool: Pool<&str> = Pool::new(Some(Duration::from_secs(2)));
        let now = Instant::now();
        // Give time for the expire worker thread to park itself.
        thread::sleep(Duration::from_secs(3));
        drop(pool);
        assert!(now.elapsed() < DEFAULT_IDLE_RESOURCE_TIMEOUT)
    }
}
