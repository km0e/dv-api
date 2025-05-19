use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone)]
pub struct Oneshot<T>(Arc<(Mutex<Option<T>>, Condvar)>);

impl<T> Oneshot<T> {
    pub fn new() -> Self {
        Self(Arc::new((Mutex::new(None), Condvar::new())))
    }
    pub fn try_wait(&self) -> Option<T> {
        let (lock, _) = &*self.0;
        let mut started = lock.lock().unwrap();
        (*started).as_ref()?;
        started.take()
    }
    pub fn wait(self) -> T {
        let (lock, cvar) = &*self.0;
        let mut started = lock.lock().unwrap();
        while (*started).is_none() {
            started = cvar.wait(started).unwrap();
        }
        started.take().unwrap()
    }
    pub fn send(self, v: T) {
        let (lock, cvar) = &*self.0;
        let mut started = lock.lock().unwrap();
        *started = Some(v);
        cvar.notify_one();
    }
}
