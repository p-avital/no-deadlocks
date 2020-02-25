mod lock_manager;
mod mutex;
mod rwlock;
mod graphs;
pub use mutex::{Mutex, MutexGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
pub mod prelude {
    pub use crate::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
}
/// Replaces the locks defined in this crate by their `std` counterparts.
/// Convenient to switch to the less costly `std` implementations.
pub mod std_override {
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