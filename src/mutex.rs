use std::sync::{LockResult, PoisonError, TryLockError, TryLockResult};

pub struct Mutex<T> {
    manager: std::sync::Arc<crate::lock_manager::LockManager>,
    key: usize,
    poisoned: bool,
    inner: T,
}

impl<T> Mutex<T> {
    pub fn new(inner: T) -> Self {
        let manager = crate::lock_manager::LockManager::get_global_manager();
        let key = manager.create_lock();
        Mutex {
            inner,
            poisoned: false,
            manager,
            key,
        }
    }

    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        if self.poisoned {
            Err(PoisonError::new(&mut self.inner))
        } else {
            Ok(&mut self.inner)
        }
    }

    pub fn into_inner(self) -> LockResult<T> {
        if self.poisoned {
            Err(PoisonError::new(self.inner))
        } else {
            Ok(self.inner)
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn try_lock(&self) -> TryLockResult<MutexGuard<T>> {
        let mut guard = self.manager.write_lock();
        let representation = guard.locks.get_mut(&self.key).unwrap();
        if representation.try_write_lock() {
            let returned_guard = MutexGuard {
                inner: unsafe { &mut *(self as *const _ as *mut _) },
            };
            if self.is_poisoned() {
                Err(TryLockError::Poisoned(
                    std::sync::PoisonError::new(returned_guard),
                ))
            } else {
                Ok(returned_guard)
            }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    pub fn lock(&self) -> LockResult<MutexGuard<T>> {
        unimplemented!()
    }
}

pub struct MutexGuard<'l, T> {
    inner: &'l mut Mutex<T>,
}
impl<'l, T> std::ops::Deref for MutexGuard<'l, T> {
    type Target = T;
    fn deref(&self) -> &<Self as std::ops::Deref>::Target {
        &self.inner.inner
    }
}
impl<'l, T> std::ops::DerefMut for MutexGuard<'l, T> {
    fn deref_mut(&mut self) -> &mut <Self as std::ops::Deref>::Target {
        &mut self.inner.inner
    }
}
impl<'l, T> Drop for MutexGuard<'l, T> {
    fn drop(&mut self) {
        let mut guard = self.inner.manager.write_lock();
        guard.locks.get_mut(&self.inner.key).unwrap().unlock();
        if std::thread::panicking() {
            self.inner.poisoned = true;
        }
    }
}