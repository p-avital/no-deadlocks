use std::{
    sync::{LockResult, PoisonError, WaitTimeoutResult},
    time::Duration,
};

use crate::MutexGuard;

#[derive(Default)]
pub struct Condvar {
    condvar: std::sync::Condvar,
    mutex: std::sync::Mutex<()>,
}
impl Condvar {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn wait<'l, T>(&self, guard: MutexGuard<'l, T>) -> LockResult<MutexGuard<'l, T>> {
        let mutex = guard.unlock();
        #[allow(unused_must_use)]
        {
            self.condvar.wait(self.mutex.lock().unwrap());
        }
        mutex.lock()
    }
    pub fn wait_timeout<'l, T>(
        &self,
        guard: MutexGuard<'l, T>,
        dur: Duration,
    ) -> LockResult<(MutexGuard<'l, T>, WaitTimeoutResult)> {
        let mutex = guard.unlock();
        #[allow(unused_must_use)]
        let result = self
            .condvar
            .wait_timeout(self.mutex.lock().unwrap(), dur)
            .unwrap()
            .1;
        match mutex.lock() {
            Ok(guard) => Ok((guard, result)),
            Err(e) => Err(PoisonError::new((e.into_inner(), result))),
        }
    }
    pub fn wait_timeout_while<'l, T, F: FnMut(&mut T) -> bool>(
        &self,
        guard: MutexGuard<'l, T>,
        dur: Duration,
        mut condition: F,
    ) -> LockResult<(MutexGuard<'l, T>, WaitTimeoutResult)> {
        use std::time::Instant;
        let mut mutex = guard.unlock();
        let expiry = Instant::now() + dur;
        loop {
            let timedout = self
                .condvar
                .wait_timeout(self.mutex.lock().unwrap(), expiry - Instant::now())
                .unwrap()
                .1;
            let guard = mutex.lock();
            match guard {
                Ok(mut guard) => {
                    if condition(&mut *guard) {
                        return Ok((guard, timedout));
                    } else {
                        mutex = guard.unlock();
                    }
                }
                Err(e) => return Err(PoisonError::new((e.into_inner(), timedout))),
            }
        }
    }
    pub fn wait_while<'l, T, F: FnMut(&mut T) -> bool>(
        &self,
        guard: MutexGuard<'l, T>,
        mut condition: F,
    ) -> LockResult<MutexGuard<'l, T>> {
        let mut mutex = guard.unlock();
        loop {
            #[allow(unused_must_use)]
            {
                self.condvar.wait(self.mutex.lock().unwrap());
            }
            let guard = mutex.lock();
            match guard {
                Ok(mut guard) => {
                    if condition(&mut *guard) {
                        return Ok(guard);
                    } else {
                        mutex = guard.unlock();
                    }
                }
                Err(_) => return guard,
            }
        }
    }
    pub fn notify_one(&self) {
        self.condvar.notify_one()
    }
    pub fn notify_all(&self) {
        self.condvar.notify_all()
    }
}
