use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicI32 as AtomicCount;

use backtrace::Backtrace;

use crate::Map;

static mut GLOBAL_MANAGER: Option<Arc<LockManager>> = None;

pub struct LockManagerReadGuard<'l> {
    inner: &'l LockManager
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
    pub(crate) locks: Map<usize, LockRepresentation>
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
        guard.next_key += 1;guard.locks.insert(key, LockRepresentation::new());
        key
    }

    #[allow(dead_code)]
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

    pub fn analyse(&mut self) {
        let mut graph = crate::graphs::Graph::new();
        for (id, representation) in self.locks.iter() {
            let lock_node = DependencyNode::Lock(*id);
            for (reader, _trace) in representation.readers.iter() {
                graph.add_edge_and_nodes(lock_node, DependencyNode::Thread(*reader));
            }
            for (requester, (request, _trace)) in representation.requests.iter() {
                if representation.write_locked || *request == RequestType::Write {
                    graph.add_edge_and_nodes(DependencyNode::Thread(*requester), lock_node);
                }
            }
        }
        if let Some(result) = graph.find_loop() {
            self.handle_deadlock(&result)
        }
    }

    fn handle_deadlock(&mut self, dependence_cycle: &Vec<&DependencyNode>) -> ! {
        for lock_id in dependence_cycle.iter().filter_map(|val| match *val {
            DependencyNode::Lock(id) => Some(id),
            _ => None
        }) {
            let representation = self.locks.get(lock_id).unwrap();
            let mut trace = representation.trace.clone();
            trace.resolve();
            eprintln!("Lock {}, taken at: {:?}", lock_id, trace);
            eprintln!("\t{}-locked by threads {:?}", if representation.write_locked {"write"} else {"read"}, representation.readers.iter().map(|(id, _trace)| id).collect::<Vec<_>>());
        }
        panic!("Deadlock detected! Cycle: {:?}", dependence_cycle)
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
enum DependencyNode {
    Thread(ThreadId),
    Lock(usize),
}

use std::thread::ThreadId;
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub(crate) enum RequestType {
    Read,
    Write,
}

pub struct LockRepresentation {
    write_locked: bool,
    pub(crate) readers: Vec<(ThreadId, Backtrace)>,
    pub(crate) requests: Map<ThreadId, (RequestType, Backtrace)>,
    pub(crate) trace: Backtrace,
}

impl LockRepresentation {
    pub fn new() -> Self {
        LockRepresentation {write_locked: false, readers: Vec::new(), requests: Map::new(), trace: Backtrace::new_unresolved()}
    }

    /// Returns `true` if write_lock succeeded
    pub fn try_write_lock(&mut self) -> bool {
        if self.readers.len() == 0 {
            self.write_locked = true;
            self.readers.push((std::thread::current().id(), Backtrace::new_unresolved()));
            true
        } else {
            false
        }
    }

    pub fn subscribe_write(&mut self) {
        self.requests.insert(std::thread::current().id(), (RequestType::Write, Backtrace::new_unresolved()));
    }

    /// Returns `true` if read_lock succeeded
    pub fn try_read_lock(&mut self) -> bool {
        if self.write_locked{
            false
        } else {
            self.readers.push((std::thread::current().id(), Backtrace::new_unresolved()));
            true
        }
    }

    pub fn subscribe_read(&mut self) {
        self.requests.insert(std::thread::current().id(), (RequestType::Read, Backtrace::new_unresolved()));
    }

    pub fn unlock(&mut self) {
        self.write_locked = false;
        let id = std::thread::current().id();
        if let Some(index) = self.readers.iter().position(|(i, _trace)| i == &id) {
            self.readers.swap_remove(index);
        }
    }
}

#[test]
#[should_panic]
fn deadlock_detection() {
    use std::sync::Arc;
    use crate::Mutex;
    let mut1 = Arc::new(Mutex::new(0));
    let mut2 = Arc::new(Mutex::new(0));
    let _guard1 = mut1.lock();
    std::thread::spawn({
        let mut1 = mut1.clone();
        let mut2 = mut2.clone();
        move ||{
            let _guard2 = mut2.lock();
            let _guard1 = mut1.lock();
            std::thread::sleep(std::time::Duration::from_millis(200));
    }});
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _guard2 = mut2.lock();
}

#[test]
fn no_deadlock_detection() {
    use std::sync::Arc;
    use crate::Mutex;
    let mut1 = Arc::new(Mutex::new(0));
    let mut2 = Arc::new(Mutex::new(0));
    let _guard1 = mut1.lock();
    std::thread::spawn({
        let mut1 = mut1.clone();
        let mut2 = mut2.clone();
        move ||{
            let _guard1 = mut1.lock();
            let _guard2 = mut2.lock();
            std::thread::sleep(std::time::Duration::from_millis(200));
    }});
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _guard2 = mut2.lock();
}