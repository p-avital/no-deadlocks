use std::cell::UnsafeCell;
use std::ops::Deref;
use std::sync::atomic::AtomicI32 as AtomicCount;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::ThreadId;

use backtrace::Backtrace;

use crate::Map;

static GLOBAL_MANAGER: AtomicPtr<Arc<LockManager>> =
    AtomicPtr::new(std::ptr::null_mut() as *mut Arc<LockManager>);

pub struct LockManagerReadGuard<'l> {
    inner: &'l LockManagerInner,
}

impl<'l> Drop for LockManagerReadGuard<'l> {
    fn drop(&mut self) {
        self.inner.lock.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<'l> std::ops::Deref for LockManagerReadGuard<'l> {
    type Target = LockManagerInner;
    fn deref(&self) -> &<Self as std::ops::Deref>::Target {
        self.inner
    }
}

pub struct LockManagerWriteGuard<'l> {
    inner: &'l UnsafeCell<LockManagerInner>,
}

impl<'l> Drop for LockManagerWriteGuard<'l> {
    fn drop(&mut self) {
        unsafe {
            (*self.inner.get()).lock.store(0, Ordering::Relaxed);
        }
    }
}

impl<'l> std::ops::Deref for LockManagerWriteGuard<'l> {
    type Target = LockManagerInner;
    fn deref(&self) -> &<Self as std::ops::Deref>::Target {
        unsafe { &*self.inner.get() }
    }
}

impl<'l> std::ops::DerefMut for LockManagerWriteGuard<'l> {
    fn deref_mut(&mut self) -> &mut <Self as std::ops::Deref>::Target {
        unsafe { &mut *self.inner.get() }
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash)]
enum DependencyNode {
    Thread(ThreadId),
    Lock(usize),
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub(crate) enum RequestType {
    Read,
    Write,
}

pub struct LockRepresentation {
    write_locked: bool,
    pub(crate) readers: Vec<(ThreadId, Backtrace)>,
    pub(crate) requests: Map<ThreadId, (RequestType, Backtrace)>,
}

impl LockRepresentation {
    pub fn new() -> Self {
        LockRepresentation {
            write_locked: false,
            readers: Vec::new(),
            requests: Map::new(),
        }
    }

    /// Returns `true` if write_lock succeeded
    pub fn try_write_lock(&mut self) -> bool {
        if self.readers.is_empty() {
            self.write_locked = true;
            self.readers
                .push((std::thread::current().id(), Backtrace::new_unresolved()));
            self.unsubscribe();
            true
        } else {
            false
        }
    }

    pub fn unsubscribe(&mut self) {
        self.requests.remove(&std::thread::current().id());
    }

    pub fn subscribe_write(&mut self) {
        let id = std::thread::current().id();
        if let Some((RequestType::Write, _)) = self.requests.get(&id) {
            return;
        }
        self.requests
            .insert(id, (RequestType::Write, Backtrace::new_unresolved()));
    }

    /// Returns `true` if read_lock succeeded
    pub fn try_read_lock(&mut self) -> bool {
        if self.write_locked {
            false
        } else {
            self.readers
                .push((std::thread::current().id(), Backtrace::new_unresolved()));
            self.unsubscribe();
            true
        }
    }

    pub fn subscribe_read(&mut self) {
        let id = std::thread::current().id();
        if let Some((RequestType::Read, _)) = self.requests.get(&id) {
            return;
        }
        self.requests
            .insert(id, (RequestType::Read, Backtrace::new_unresolved()));
    }

    pub fn unlock(&mut self) {
        self.write_locked = false;
        let id = std::thread::current().id();
        if let Some(index) = self.readers.iter().position(|(i, _trace)| i == &id) {
            self.readers.swap_remove(index);
        }
    }
}

impl Default for LockRepresentation {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LockManagerInner {
    lock: AtomicCount,
    next_key: usize,
    analysis_timeout: std::time::Duration,
    pub(crate) locks: Map<usize, LockRepresentation>,
}

pub struct LockManager(UnsafeCell<LockManagerInner>);

impl LockManagerInner {
    fn new() -> Self {
        LockManagerInner {
            lock: AtomicCount::new(0),
            next_key: 0,
            locks: Map::new(),
            analysis_timeout: std::time::Duration::from_secs(1),
        }
    }

    fn with_analysis_timeout(analysis_timeout: std::time::Duration) -> Self {
        LockManagerInner {
            lock: AtomicCount::new(0),
            next_key: 0,
            locks: Map::new(),
            analysis_timeout,
        }
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
            self.handle_deadlock(&result);
        }
    }

    #[allow(unused_must_use)]
    fn handle_deadlock(&mut self, dependence_cycle: &[&DependencyNode]) {
        let this_thread = DependencyNode::Thread(std::thread::current().id());
        if !dependence_cycle.iter().any(|node| *node == &this_thread) {
            return;
        }
        let (mut output, path): (Box<dyn std::io::Write>, _) =
            if let Some(path) = std::env::var_os("NO_DEADLOCKS") {
                match std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&path)
                {
                    Ok(file) => (Box::new(file), path.to_str().unwrap().to_owned()),
                    Err(_) => (Box::new(std::io::stderr()), "stderr".to_owned()),
                }
            } else {
                (Box::new(std::io::stderr()), "stderr".to_owned())
            };
        writeln!(output, "=========== REPORT START ===========");
        if dependence_cycle.len() == 2 {
            writeln!(output, "A reentrance has been attempted, but `std::sync`'s locks are not reentrant. This results in a deadlock. dependence cycle: {:?}", dependence_cycle);
            let lock_id = match dependence_cycle[0] {
                DependencyNode::Lock(id) => id,
                _ => {
                    if let DependencyNode::Lock(id) = dependence_cycle[1] {
                        id
                    } else {
                        unreachable!()
                    }
                }
            };
            let lock = self.locks.get(lock_id).unwrap();
            let locked_trace = resolve_and_trim(&lock.readers[0].1);
            let reentrance_trace =
                resolve_and_trim(&lock.requests.get(&std::thread::current().id()).unwrap().1);
            writeln!(
                output,
                "Lock taken at:\r\n{:?}\r\nReentrace at:\r\n{:?}",
                locked_trace, reentrance_trace
            );
        } else {
            writeln!(
                output,
                "A deadlock has been detected, here's the dependence cycle: {:?}",
                dependence_cycle
            );
            for lock_id in dependence_cycle.iter().filter_map(|val| match *val {
                DependencyNode::Lock(id) => Some(id),
                _ => None,
            }) {
                let representation = self.locks.get(lock_id).unwrap();
                writeln!(output, "LOCK {}:", lock_id);
                writeln!(output, "BLOCKING:");
                for (thread_id, (request, trace)) in representation.requests.iter() {
                    writeln!(
                        output,
                        " THREAD {:?} requesting {} rights at:",
                        thread_id,
                        match request {
                            RequestType::Read => "read",
                            RequestType::Write => "write",
                        }
                    );
                    writeln!(output, "{:?}", resolve_and_trim(trace));
                }
                writeln!(output, "BLOCKED BY:");
                for (thread_id, trace) in representation.readers.iter() {
                    writeln!(output, " THREAD {:?} blocked at:", thread_id);
                    writeln!(output, "{:?}", resolve_and_trim(trace));
                }
            }
        }
        writeln!(output, "=========== REPORT END ===========");
        writeln!(output);
        panic!("DEADLOCK DETECTED! See {} for details", path);
    }
}

impl Deref for LockManager {
    type Target = LockManagerInner;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}
impl LockManager {
    pub fn new() -> Self {
        LockManager(UnsafeCell::new(LockManagerInner::new()))
    }

    pub fn with_analysis_timeout(analysis_timeout: std::time::Duration) -> Self {
        LockManager(UnsafeCell::new(LockManagerInner::with_analysis_timeout(
            analysis_timeout,
        )))
    }

    pub fn analysis_timeout(&self) -> std::time::Duration {
        unsafe { (*self.0.get()).analysis_timeout }
    }

    pub fn get_global_manager() -> Arc<Self> {
        let manager = GLOBAL_MANAGER.load(Ordering::Relaxed);
        if !manager.is_null() {
            return unsafe { (*manager).clone() };
        }
        let new_manager = Box::into_raw(Box::new(Arc::new(LockManager::new())));
        match GLOBAL_MANAGER.compare_exchange(
            manager,
            new_manager,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Err(manager) => unsafe {
                Box::from_raw(new_manager);
                (*manager).clone()
            },
            Ok(_) => unsafe { (*new_manager).clone() },
        }
    }

    pub fn create_lock(&self) -> usize {
        let mut guard = self.write_lock();
        let key = guard.next_key;
        guard.next_key += 1;
        guard.locks.insert(key, LockRepresentation::new());
        key
    }

    pub fn remove_lock(&self, key: &usize) {
        let mut guard = self.write_lock();
        guard.locks.remove(key);
    }

    #[allow(dead_code)]
    pub(crate) fn read_lock(&self) -> LockManagerReadGuard {
        let mut state = self.lock.load(Ordering::Relaxed);
        loop {
            if state >= 0 {
                match self.lock.compare_exchange(
                    state,
                    state + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(new_state) => state = new_state,
                }
            } else {
                std::hint::spin_loop();
                state = self.lock.load(Ordering::Relaxed);
            }
        }
        LockManagerReadGuard { inner: self }
    }

    pub(crate) fn write_lock(&self) -> LockManagerWriteGuard {
        while self
            .lock
            .compare_exchange_weak(0, -1, Ordering::Relaxed, Ordering::Relaxed)
            != Ok(0)
        {}
        LockManagerWriteGuard { inner: &self.0 }
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_and_trim(trace: &Backtrace) -> Backtrace {
    let mut resolved: Backtrace = trace
        .frames()
        .iter()
        .skip(6)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    resolved.resolve();
    resolved
}

#[test]
#[should_panic]
fn with_deadlock() {
    use crate::Mutex;
    use std::sync::Arc;
    let mut1 = Arc::new(Mutex::new(0));
    let mut2 = Arc::new(Mutex::new(0));
    let _guard1 = mut1.lock();
    let th = std::thread::spawn({
        let mut1 = mut1.clone();
        let mut2 = mut2.clone();
        move || {
            let _guard2 = mut2.lock();
            let _guard1 = mut1.lock();
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _guard2 = mut2.lock();
    th.join().unwrap();
}

#[test]
fn without_deadlock() {
    use crate::Mutex;
    use std::sync::Arc;
    let mut1 = Arc::new(Mutex::new(0));
    let mut2 = Arc::new(Mutex::new(0));
    let guard1 = mut1.lock();
    let th = std::thread::spawn({
        let mut1 = mut1.clone();
        let mut2 = mut2.clone();
        move || {
            let _guard1 = mut1.lock();
            let _guard2 = mut2.lock();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    let guard2 = mut2.lock();
    std::mem::drop((guard1, guard2));
    th.join().unwrap();
}

#[test]
#[should_panic]
fn reentrance_detection() {
    use crate::Mutex;
    let mutex = Mutex::new(0);
    let _guard1 = mutex.lock();
    let _guard2 = mutex.lock();
}
