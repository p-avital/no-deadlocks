use std::sync::Arc;
use std::sync::atomic::Ordering;

use std::sync::atomic::AtomicI32 as AtomicCount;

static mut GLOBAL_MANAGER: Option<Arc<LockManager>> = None;

pub struct LockManagerReadGuard<'l> {
    inner: &'l LockManager
}

impl<'l> LockManagerReadGuard<'l> {
    pub fn try_upgrade<'u>(self) -> Result<LockManagerWriteGuard<'u>, Self> {
        if self.inner.lock.compare_and_swap(1, -1, Ordering::Relaxed) == 1 {
            Ok(LockManagerWriteGuard {inner: unsafe {&mut *(self.inner as *const _ as *mut _)}})
        } else {
            Err(self)
        }
    }

    pub fn upgrade<'u>(self) -> LockManagerWriteGuard<'u> {
        while self.inner.lock.compare_and_swap(1, -1, Ordering::Relaxed) != 1 {}
        LockManagerWriteGuard {inner: unsafe {&mut *(self.inner as *const _ as *mut _)}}
    }
}

impl<'l> Drop for LockManagerReadGuard<'l> {
    fn drop(&mut self) {
        self.inner.lock.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<'l> std::ops::Deref for LockManagerReadGuard<'l> {
    type Target = LockManager;
    fn deref(&self) -> &<Self as std::ops::Deref>::Target { 
        self.inner
     }
}

pub struct LockManagerWriteGuard<'l> {
    inner: &'l mut LockManager,
}

impl<'l> Drop for LockManagerWriteGuard<'l> {
    fn drop(&mut self) {
        self.inner.lock.store(0, Ordering::Relaxed);
    }
}

impl<'l> std::ops::Deref for LockManagerWriteGuard<'l> {
    type Target = LockManager;
    fn deref(&self) -> &<Self as std::ops::Deref>::Target { 
        self.inner
     }
}

impl<'l> std::ops::DerefMut for LockManagerWriteGuard<'l> {
    fn deref_mut(&mut self) -> &mut <Self as std::ops::Deref>::Target { 
        self.inner
     }
}

pub struct LockManager {
    lock: AtomicCount,
    next_key: usize,
    pub(crate) locks: vector_map::VecMap<usize, LockRepresentation>
}

impl LockManager {
    fn new() -> Self {
        LockManager {lock: AtomicCount::new(0), next_key: 0, locks: Default::default()}
    }

    pub fn get_global_manager() -> Arc<Self> {
        if let Some(manager) = unsafe {&GLOBAL_MANAGER} {
            manager.clone()
        } else {
            let manager = Arc::new(Self::new());
            unsafe {
                GLOBAL_MANAGER = Some(manager.clone())
            };
            manager
        }
    }

    pub fn create_lock(&self) -> usize {
        let mut guard = self.write_lock();
        let key = guard.next_key;
        guard.next_key += 1;
        unsafe {guard.locks.inner_mut().push((key, LockRepresentation::new()))};
        key
    }

    pub(crate) fn read_lock(&self) -> LockManagerReadGuard {
        let mut state = self.lock.load(Ordering::Relaxed);
        loop {
            if state == 0 {
                let new_state = self.lock.compare_and_swap(state, state + 1, Ordering::Relaxed);
                if new_state == state {break;} else {state = new_state}
            } else {
                state = self.lock.load(Ordering::Relaxed);
            }
        }
        LockManagerReadGuard {inner: self}
    }

    pub(crate) fn write_lock(&self) -> LockManagerWriteGuard {
        while self.lock.compare_and_swap(0, -1, Ordering::Relaxed) != 0 {}
        LockManagerWriteGuard {inner: unsafe {&mut *(self as *const _ as  *mut _)}}
    }
}

pub struct LockRepresentation {
    pub state: isize,
}

impl LockRepresentation {
    pub fn new() -> Self {
        LockRepresentation {state: 0}
    }

    /// Returns `true` if write_lock succeeded
    pub fn try_write_lock(&mut self) -> bool {
        if self.state == 0 {
            self.state = -1;
            true
        } else {
            false
        }
    }

    /// Returns `true` if read_lock succeeded
    pub fn try_read_lock(&mut self) -> bool {
        if self.state >= 0 {
            self.state += 1;
            true
        } else {
            false
        }
    }

    pub fn unlock(&mut self) {
        if self.state > 0 {
            self.state -= 1
        } else {
            self.state = 0
        }
    }
}