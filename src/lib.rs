mod lock_manager;
mod mutex;
mod rwlock;
mod graphs;
pub use mutex::{Mutex, MutexGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// A convenience import: imports all lock and guard types from `no_deadlock`.
/// Replace `prelude` by `prelude_std` to import their equivalent types from `std::sync` instead.
pub mod prelude {
    pub use crate::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
}

/// A convenience import: imports all lock and guard types from `std::sync`.
/// Replace `prelude_std` by `prelude` to import their equivalent types from `no_deadlocks` instead.
pub mod prelude_std {
    pub use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
}

#[cfg(feature="use_vecmap")]
pub(crate) type Set<T> = vector_map::VecMap<T, ()>;
#[cfg(not(feature="use_vecmap"))]
pub(crate) type Set<T> = std::collections::HashMap<T, ()>;

#[cfg(feature = "use_vecmap")]
pub(crate) type Map<K, V> = vector_map::VecMap<K, V>;
#[cfg(not(feature = "use_vecmap"))]
pub(crate) type Map<K, V> = std::collections::HashMap<K, V>;