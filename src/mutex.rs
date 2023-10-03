use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LockResult, PoisonError, TryLockError, TryLockResult};
use std::time::Instant;

/// An instrumented version of `std::sync::Mutex`
pub struct Mutex<T: ?Sized> {
    key: usize,
    poisoned: AtomicBool,
    manager: std::sync::Arc<crate::lock_manager::LockManager>,
    inner: UnsafeCell<T>,
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> Mutex<T> {
    pub fn new(inner: T) -> Self {
        let manager = crate::lock_manager::LockManager::get_global_manager();
        let key = manager.create_lock();
        Mutex {
            inner: UnsafeCell::new(inner),
            poisoned: AtomicBool::new(false),
            manager,
            key,
        }
    }

    pub fn with_manager(
        manager: std::sync::Arc<crate::lock_manager::LockManager>,
        inner: T,
    ) -> Self {
        let key = manager.create_lock();
        Mutex {
            inner: UnsafeCell::new(inner),
            poisoned: AtomicBool::new(false),
            manager,
            key,
        }
    }

    pub fn into_inner(self) -> LockResult<T> {
        let key = self.key;
        let poisonned = self.poisoned.load(Ordering::Relaxed);
        let manager = unsafe { core::ptr::read(&self.manager) };
        let value = unsafe { core::ptr::read(&self.inner) }.into_inner();
        core::mem::forget(self);
        manager.remove_lock(&key);
        if poisonned {
            Err(PoisonError::new(value))
        } else {
            Ok(value)
        }
    }
}

impl<T: ?Sized> Drop for Mutex<T> {
    fn drop(&mut self) {
        self.manager.remove_lock(&self.key)
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let reference = unsafe { &mut *self.inner.get() };
        if self.poisoned.load(Ordering::Relaxed) {
            Err(PoisonError::new(reference))
        } else {
            Ok(reference)
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed)
    }

    pub fn try_lock(&self) -> TryLockResult<MutexGuard<T>> {
        let mut guard = self.manager.write_lock();
        let representation = guard.locks.get_mut(&self.key).unwrap();
        if representation.try_write_lock() {
            let returned_guard = MutexGuard { inner: self };
            if self.is_poisoned() {
                Err(TryLockError::Poisoned(PoisonError::new(returned_guard)))
            } else {
                Ok(returned_guard)
            }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    pub fn lock(&self) -> LockResult<MutexGuard<T>> {
        let timeout = self.manager.analysis_timeout();
        let start = Instant::now();

        loop {
            let mut guard = self.manager.write_lock();
            let representation = guard.locks.get_mut(&self.key).unwrap();

            if representation.try_write_lock() {
                let returned_guard = MutexGuard { inner: self };
                if self.is_poisoned() {
                    return Err(PoisonError::new(returned_guard));
                } else {
                    return Ok(returned_guard);
                }
            } else if Instant::now().duration_since(start) > timeout {
                representation.subscribe_write();
                guard.analyse();
            }

            std::thread::yield_now();
        }
    }
}

pub struct MutexGuard<'l, T: ?Sized> {
    inner: &'l Mutex<T>,
}
impl<'l, T> std::ops::Deref for MutexGuard<'l, T> {
    type Target = T;
    fn deref(&self) -> &<Self as std::ops::Deref>::Target {
        unsafe { &*self.inner.inner.get() }
    }
}
impl<'l, T> std::ops::DerefMut for MutexGuard<'l, T> {
    fn deref_mut(&mut self) -> &mut <Self as std::ops::Deref>::Target {
        unsafe { &mut *self.inner.inner.get() }
    }
}
impl<'l, T: ?Sized> MutexGuard<'l, T> {
    pub(crate) fn unlock(self) -> &'l Mutex<T> {
        self.inner
    }
}
impl<'l, T: ?Sized> Drop for MutexGuard<'l, T> {
    fn drop(&mut self) {
        let mut guard = self.inner.manager.write_lock();
        guard.locks.get_mut(&self.inner.key).unwrap().unlock();
        if std::thread::panicking() {
            self.inner.poisoned.store(true, Ordering::Relaxed);
        }
    }
}
unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}
