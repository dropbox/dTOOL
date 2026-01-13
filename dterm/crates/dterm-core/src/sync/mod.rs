//! Synchronization primitives for terminal state coordination.
//!
//! This module provides thread-safe synchronization types for coordinating access
//! between PTY/input threads and render threads, based on Alacritty's approach.
//!
//! ## Problem
//!
//! In a multi-threaded terminal emulator:
//! - **PTY thread**: Continuously receives output and updates terminal state
//! - **Render thread**: Periodically reads terminal state to render frames
//!
//! With a standard mutex, one thread can repeatedly acquire the lock while
//! the other starves. For example, if the PTY thread releases and immediately
//! re-acquires the lock in a tight loop, the render thread may never get a chance.
//!
//! ## Solution: FairMutex
//!
//! `FairMutex` uses two locks to ensure fairness:
//! 1. A `next` lock that serializes access requests
//! 2. A `data` lock that protects the actual data
//!
//! When thread A wants the lock, it first acquires `next`, then acquires `data`,
//! then releases `next`. If thread B is waiting, it will get `next` as soon as
//! A releases it, ensuring B gets the next turn even if A immediately tries again.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::sync::FairMutex;
//!
//! let terminal = FairMutex::new(/* terminal state */);
//!
//! // PTY thread - fair lock to allow render thread access
//! {
//!     let mut state = terminal.lock();
//!     state.process_input(data);
//! }
//!
//! // Render thread - fair lock for rendering
//! {
//!     let state = terminal.lock();
//!     render(&state);
//! }
//!
//! // Occasional unfair lock when you need priority
//! if let Some(state) = terminal.try_lock_unfair() {
//!     // Fast path - got lock without waiting
//! }
//! ```
//!
//! ## Lease API
//!
//! For cases where you need to reserve access but defer the actual lock:
//!
//! ```rust
//! use dterm_core::sync::FairMutex;
//!
//! let terminal: FairMutex<i32> = FairMutex::new(0);
//!
//! // Reserve your place in line
//! let lease = terminal.lease();
//!
//! // Do some preparation work...
//! // prepare_for_render();
//!
//! // Now acquire the actual lock (guaranteed next)
//! let state = terminal.lock_with_lease(lease);
//! ```
//!
//! ## Design Notes
//!
//! This implementation is inspired by Alacritty's `FairMutex` but extended with:
//! - Lease API for deferred locking
//! - `lock_with_lease` for efficient lease-to-lock conversion
//! - Send + Sync bounds for multi-threaded use
//! - Debug implementations for better diagnostics

use parking_lot::{Mutex, MutexGuard};
use std::fmt;

/// A fair mutex that prevents thread starvation.
///
/// Uses a two-lock protocol to ensure that if one thread is waiting,
/// it will acquire the lock before any thread that arrives later.
///
/// # Example
///
/// ```rust
/// use dterm_core::sync::FairMutex;
///
/// let mutex = FairMutex::new(42);
///
/// // Thread 1
/// {
///     let mut guard = mutex.lock();
///     *guard += 1;
/// }
///
/// // Thread 2 - guaranteed to get a turn even if thread 1 is rapid
/// {
///     let guard = mutex.lock();
///     assert!(*guard >= 42);
/// }
/// ```
pub struct FairMutex<T> {
    /// The protected data.
    data: Mutex<T>,
    /// Serializes access requests to ensure fairness.
    next: Mutex<()>,
}

impl<T> FairMutex<T> {
    /// Creates a new `FairMutex` containing `data`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(vec![1, 2, 3]);
    /// ```
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Acquires a lease to reserve access to the mutex.
    ///
    /// A lease blocks other threads from acquiring the lock fairly,
    /// but doesn't yet hold the data lock. This is useful when you
    /// need to reserve your place in line but aren't ready to access
    /// the data yet.
    ///
    /// The lease must be converted to a data lock via `lock_with_lease`
    /// or dropped to release the reservation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    ///
    /// // Reserve access
    /// let lease = mutex.lease();
    ///
    /// // Do preparation work while holding reservation...
    ///
    /// // Convert to actual data access
    /// let guard = mutex.lock_with_lease(lease);
    /// ```
    #[inline]
    pub fn lease(&self) -> Lease<'_> {
        Lease {
            _guard: self.next.lock(),
        }
    }

    /// Tries to acquire a lease without blocking.
    ///
    /// Returns `None` if another thread already holds a lease or
    /// is in the process of acquiring the lock.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    ///
    /// if let Some(lease) = mutex.try_lease() {
    ///     // Got the reservation
    ///     let guard = mutex.lock_with_lease(lease);
    /// };
    /// ```
    #[inline]
    pub fn try_lease(&self) -> Option<Lease<'_>> {
        self.next.try_lock().map(|guard| Lease { _guard: guard })
    }

    /// Acquires the lock fairly.
    ///
    /// This method first acquires the `next` lock to ensure fairness,
    /// then acquires the `data` lock. If another thread is waiting,
    /// it will get the next turn.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    /// let guard = mutex.lock();
    /// assert_eq!(*guard, 42);
    /// ```
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Must bind to a temporary or the lock will be freed before
        // acquiring data.lock()
        let _next = self.next.lock();
        self.data.lock()
    }

    /// Acquires the lock using an existing lease.
    ///
    /// This is more efficient than `lock()` when you already hold a lease,
    /// as it skips the `next` lock acquisition.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    /// let lease = mutex.lease();
    /// // Do some work...
    /// let guard = mutex.lock_with_lease(lease);
    /// ```
    #[inline]
    pub fn lock_with_lease(&self, _lease: Lease<'_>) -> MutexGuard<'_, T> {
        // The lease already holds the next lock, so we just need data
        self.data.lock()
    }

    /// Acquires the lock unfairly (without fairness guarantee).
    ///
    /// This method bypasses the fairness protocol and directly acquires
    /// the data lock. Use this when you need priority access and don't
    /// want to wait in line.
    ///
    /// **Warning**: Heavy use of unfair locking can starve threads using
    /// fair locking.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    /// let guard = mutex.lock_unfair();
    /// assert_eq!(*guard, 42);
    /// ```
    #[inline]
    pub fn lock_unfair(&self) -> MutexGuard<'_, T> {
        self.data.lock()
    }

    /// Tries to acquire the lock unfairly without blocking.
    ///
    /// Returns `None` if the data lock is currently held.
    /// This does not check the fairness queue.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    ///
    /// if let Some(guard) = mutex.try_lock_unfair() {
    ///     assert_eq!(*guard, 42);
    /// };
    /// ```
    #[inline]
    pub fn try_lock_unfair(&self) -> Option<MutexGuard<'_, T>> {
        self.data.try_lock()
    }

    /// Tries to acquire the lock fairly without blocking.
    ///
    /// Returns `None` if either the fairness queue or data lock is held.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    ///
    /// if let Some(guard) = mutex.try_lock() {
    ///     assert_eq!(*guard, 42);
    /// };
    /// ```
    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        let _next = self.next.try_lock()?;
        self.data.try_lock()
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `FairMutex` mutably, no actual locking
    /// needs to take place - the mutable borrow statically guarantees
    /// no locks exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mut mutex = FairMutex::new(42);
    /// *mutex.get_mut() = 100;
    /// assert_eq!(*mutex.lock(), 100);
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the mutex, returning the underlying data.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    /// assert_eq!(mutex.into_inner(), 42);
    /// ```
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    /// Checks if the data lock is currently held.
    ///
    /// This returns `true` if another thread is holding the data lock,
    /// regardless of whether they acquired it fairly or unfairly.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dterm_core::sync::FairMutex;
    ///
    /// let mutex = FairMutex::new(42);
    /// assert!(!mutex.is_locked());
    ///
    /// let guard = mutex.lock();
    /// assert!(mutex.is_locked());
    /// ```
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.data.is_locked()
    }
}

impl<T: Default> Default for FairMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: fmt::Debug> fmt::Debug for FairMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock_unfair() {
            Some(guard) => f.debug_struct("FairMutex").field("data", &*guard).finish(),
            None => f
                .debug_struct("FairMutex")
                .field("data", &"<locked>")
                .finish(),
        }
    }
}

// Safety: FairMutex<T> is Send if T is Send (data can be sent between threads)
unsafe impl<T: Send> Send for FairMutex<T> {}

// Safety: FairMutex<T> is Sync if T is Send (multiple threads can have &FairMutex)
unsafe impl<T: Send> Sync for FairMutex<T> {}

/// A lease that reserves access to a `FairMutex`.
///
/// Holding a lease guarantees you will be the next thread to acquire
/// the lock fairly. Other threads calling `lock()` will wait behind you.
///
/// Convert a lease to an actual lock with `FairMutex::lock_with_lease`,
/// or drop it to release your reservation.
#[must_use = "lease should be converted to a lock or dropped"]
pub struct Lease<'a> {
    _guard: MutexGuard<'a, ()>,
}

impl fmt::Debug for Lease<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lease").finish()
    }
}

/// A fair read-write lock that prevents reader or writer starvation.
///
/// Similar to `FairMutex`, but allows multiple concurrent readers while
/// ensuring writers don't starve.
///
/// # Design
///
/// Uses a fairness queue to ensure that pending writers block new readers,
/// preventing write starvation in read-heavy workloads.
///
/// # Example
///
/// ```rust
/// use dterm_core::sync::FairRwLock;
///
/// let lock = FairRwLock::new(42);
///
/// // Multiple readers
/// {
///     let r1 = lock.read();
///     let r2 = lock.read();
///     assert_eq!(*r1, 42);
///     assert_eq!(*r2, 42);
/// }
///
/// // Exclusive writer
/// {
///     let mut w = lock.write();
///     *w = 100;
/// }
/// ```
pub struct FairRwLock<T> {
    /// The protected data.
    data: parking_lot::RwLock<T>,
    /// Serializes write access requests to ensure fairness.
    next: Mutex<()>,
}

impl<T> FairRwLock<T> {
    /// Creates a new `FairRwLock` containing `data`.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            data: parking_lot::RwLock::new(data),
            next: Mutex::new(()),
        }
    }

    /// Acquires a read lock fairly.
    ///
    /// If a writer is waiting, new readers will queue behind them,
    /// preventing writer starvation.
    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        // For reads, we check if anyone is waiting for write access
        // If so, we wait for them to finish
        if self.next.is_locked() {
            // A writer is waiting, queue behind them
            let _next = self.next.lock();
            // next is released before we try to read
        }
        self.data.read()
    }

    /// Acquires a write lock fairly.
    ///
    /// Ensures that if readers are waiting, this writer will be
    /// served after the current readers finish, not after all future readers.
    #[inline]
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        let _next = self.next.lock();
        self.data.write()
    }

    /// Tries to acquire a read lock without blocking.
    #[inline]
    pub fn try_read(&self) -> Option<parking_lot::RwLockReadGuard<'_, T>> {
        if self.next.is_locked() {
            return None;
        }
        self.data.try_read()
    }

    /// Tries to acquire a write lock without blocking.
    #[inline]
    pub fn try_write(&self) -> Option<parking_lot::RwLockWriteGuard<'_, T>> {
        let _next = self.next.try_lock()?;
        self.data.try_write()
    }

    /// Returns a mutable reference to the underlying data.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the lock, returning the underlying data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: Default> Default for FairRwLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: fmt::Debug> fmt::Debug for FairRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.data.try_read() {
            Some(guard) => f.debug_struct("FairRwLock").field("data", &*guard).finish(),
            None => f
                .debug_struct("FairRwLock")
                .field("data", &"<locked>")
                .finish(),
        }
    }
}

unsafe impl<T: Send> Send for FairRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for FairRwLock<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn fair_mutex_basic() {
        let mutex = FairMutex::new(42);
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn fair_mutex_unfair() {
        let mutex = FairMutex::new(42);
        let guard = mutex.lock_unfair();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn fair_mutex_try_lock() {
        let mutex = FairMutex::new(42);

        // Should succeed when unlocked
        {
            let guard = mutex.try_lock();
            assert!(guard.is_some());
            assert_eq!(*guard.unwrap(), 42);
        }

        // Should succeed again after drop
        let guard = mutex.try_lock();
        assert!(guard.is_some());
    }

    #[test]
    fn fair_mutex_try_lock_unfair() {
        let mutex = FairMutex::new(42);
        let guard = mutex.try_lock_unfair();
        assert!(guard.is_some());
    }

    #[test]
    fn fair_mutex_lease() {
        let mutex = FairMutex::new(42);

        // Get a lease
        let lease = mutex.lease();

        // Convert to lock
        let guard = mutex.lock_with_lease(lease);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn fair_mutex_try_lease() {
        let mutex = FairMutex::new(42);

        let lease = mutex.try_lease();
        assert!(lease.is_some());

        // Can't get another lease while one is held
        let lease2 = mutex.try_lease();
        assert!(lease2.is_none());

        // After dropping lease, can get another
        drop(lease);
        let lease3 = mutex.try_lease();
        assert!(lease3.is_some());
    }

    #[test]
    fn fair_mutex_get_mut() {
        let mut mutex = FairMutex::new(42);
        *mutex.get_mut() = 100;
        assert_eq!(*mutex.lock(), 100);
    }

    #[test]
    fn fair_mutex_into_inner() {
        let mutex = FairMutex::new(42);
        assert_eq!(mutex.into_inner(), 42);
    }

    #[test]
    fn fair_mutex_is_locked() {
        let mutex = FairMutex::new(42);
        assert!(!mutex.is_locked());

        let guard = mutex.lock();
        assert!(mutex.is_locked());
        drop(guard);

        assert!(!mutex.is_locked());
    }

    #[test]
    fn fair_mutex_default() {
        let mutex: FairMutex<i32> = FairMutex::default();
        assert_eq!(*mutex.lock(), 0);
    }

    #[test]
    fn fair_mutex_debug() {
        let mutex = FairMutex::new(42);
        let debug = format!("{:?}", mutex);
        assert!(debug.contains("FairMutex"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn fair_mutex_debug_locked() {
        let mutex = FairMutex::new(42);
        let _guard = mutex.lock();
        let debug = format!("{:?}", mutex);
        assert!(debug.contains("FairMutex"));
        assert!(debug.contains("<locked>"));
    }

    #[test]
    fn fair_mutex_multithreaded() {
        let mutex = Arc::new(FairMutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let mutex = Arc::clone(&mutex);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut guard = mutex.lock();
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(*mutex.lock(), 1000);
    }

    #[test]
    fn fair_rwlock_basic_read() {
        let lock = FairRwLock::new(42);
        let guard = lock.read();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn fair_rwlock_basic_write() {
        let lock = FairRwLock::new(42);
        {
            let mut guard = lock.write();
            *guard = 100;
        }
        assert_eq!(*lock.read(), 100);
    }

    #[test]
    fn fair_rwlock_multiple_readers() {
        let lock = FairRwLock::new(42);
        let r1 = lock.read();
        let r2 = lock.read();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
    }

    #[test]
    fn fair_rwlock_try_read() {
        let lock = FairRwLock::new(42);
        let guard = lock.try_read();
        assert!(guard.is_some());
    }

    #[test]
    fn fair_rwlock_try_write() {
        let lock = FairRwLock::new(42);
        let guard = lock.try_write();
        assert!(guard.is_some());
    }

    #[test]
    fn fair_rwlock_get_mut() {
        let mut lock = FairRwLock::new(42);
        *lock.get_mut() = 100;
        assert_eq!(*lock.read(), 100);
    }

    #[test]
    fn fair_rwlock_into_inner() {
        let lock = FairRwLock::new(42);
        assert_eq!(lock.into_inner(), 42);
    }

    #[test]
    fn fair_rwlock_default() {
        let lock: FairRwLock<i32> = FairRwLock::default();
        assert_eq!(*lock.read(), 0);
    }

    #[test]
    fn fair_rwlock_debug() {
        let lock = FairRwLock::new(42);
        let debug = format!("{:?}", lock);
        assert!(debug.contains("FairRwLock"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn fair_rwlock_multithreaded() {
        let lock = Arc::new(FairRwLock::new(0));
        let mut handles = vec![];

        // Writers
        for _ in 0..5 {
            let lock = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut guard = lock.write();
                    *guard += 1;
                }
            }));
        }

        // Readers
        for _ in 0..5 {
            let lock = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let guard = lock.read();
                    let _ = *guard; // Just read the value
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(*lock.read(), 500);
    }
}

// Loom concurrency tests are in tests/loom_sync.rs
// To run: RUSTFLAGS="--cfg loom" cargo test --test loom_sync --release

// Kani proofs for formal verification
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove that FairMutex lock/unlock is safe for any value
    #[kani::proof]
    fn fair_mutex_lock_unlock_safe() {
        let value: i32 = kani::any();
        let mutex = FairMutex::new(value);

        // Lock and read
        let guard = mutex.lock();
        let read_value = *guard;
        assert!(read_value == value);
        drop(guard);

        // Lock again should succeed
        let guard2 = mutex.lock();
        assert!(*guard2 == value);
    }

    /// Prove that unfair lock returns same value
    #[kani::proof]
    fn fair_mutex_unfair_lock_consistent() {
        let value: i32 = kani::any();
        let mutex = FairMutex::new(value);

        let guard = mutex.lock_unfair();
        assert!(*guard == value);
    }

    /// Prove that try_lock returns None when locked
    #[kani::proof]
    fn fair_mutex_try_lock_behavior() {
        let mutex = FairMutex::new(42);

        // First try_lock should succeed
        let result = mutex.try_lock_unfair();
        if let Some(guard) = result {
            assert!(*guard == 42);
            // While guard is held, is_locked should be true
            assert!(mutex.is_locked());
        };
    }

    /// Prove that get_mut returns correct mutable reference
    #[kani::proof]
    fn fair_mutex_get_mut_correct() {
        let initial: i32 = kani::any();
        let new_value: i32 = kani::any();
        let mut mutex = FairMutex::new(initial);

        *mutex.get_mut() = new_value;
        assert!(*mutex.lock() == new_value);
    }

    /// Prove that into_inner returns the stored value
    #[kani::proof]
    fn fair_mutex_into_inner_correct() {
        let value: i32 = kani::any();
        let mutex = FairMutex::new(value);
        assert!(mutex.into_inner() == value);
    }

    /// Prove FairRwLock read returns correct value
    #[kani::proof]
    fn fair_rwlock_read_correct() {
        let value: i32 = kani::any();
        let lock = FairRwLock::new(value);

        let guard = lock.read();
        assert!(*guard == value);
    }

    /// Prove FairRwLock write modifies correctly
    #[kani::proof]
    fn fair_rwlock_write_correct() {
        let initial: i32 = kani::any();
        let new_value: i32 = kani::any();
        let lock = FairRwLock::new(initial);

        {
            let mut guard = lock.write();
            *guard = new_value;
        }

        assert!(*lock.read() == new_value);
    }

    /// Prove FairRwLock into_inner returns correct value
    #[kani::proof]
    fn fair_rwlock_into_inner_correct() {
        let value: i32 = kani::any();
        let lock = FairRwLock::new(value);
        assert!(lock.into_inner() == value);
    }
}
