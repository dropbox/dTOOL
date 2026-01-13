//! Synchronization types.
//!
//! Provides a fair mutex implementation to ensure neither the PTY reader
//! nor the render thread starves when accessing the terminal.

use parking_lot::{Mutex, MutexGuard};

/// A fair mutex.
///
/// Uses an extra lock to ensure that if one thread is waiting, it will get
/// the lock before a single thread can re-lock it. This prevents starvation
/// when the PTY reader and render thread compete for terminal access.
///
/// # Example
///
/// ```
/// use dterm_alacritty_bridge::sync::FairMutex;
///
/// let mutex = FairMutex::new(42);
///
/// // Fair lock - waits for any pending lease
/// {
///     let guard = mutex.lock();
///     assert_eq!(*guard, 42);
/// }
///
/// // Unfair lock - doesn't wait for lease (use for high-priority paths)
/// {
///     let guard = mutex.lock_unfair();
///     assert_eq!(*guard, 42);
/// }
/// ```
pub struct FairMutex<T> {
    /// The actual data.
    data: Mutex<T>,
    /// Next-to-access lock for fairness.
    next: Mutex<()>,
}

impl<T> FairMutex<T> {
    /// Create a new fair mutex.
    pub fn new(data: T) -> FairMutex<T> {
        FairMutex {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Acquire a lease to reserve the mutex lock.
    ///
    /// This will prevent others from acquiring a fair lock, but block if anyone
    /// else is already holding a lease. Used by the PTY reader to signal that
    /// it intends to lock soon.
    ///
    /// Dropping the returned guard releases the lease.
    pub fn lease(&self) -> MutexGuard<'_, ()> {
        self.next.lock()
    }

    /// Lock the mutex fairly.
    ///
    /// This acquires the next-to-access lock first, ensuring that threads
    /// waiting with a lease get priority over rapid re-locking.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Must bind to a temporary or the lock will be freed before going
        // into data.lock().
        let _next = self.next.lock();
        self.data.lock()
    }

    /// Unfairly lock the mutex.
    ///
    /// Bypasses the fairness mechanism. Use this in high-priority paths where
    /// fairness would add unnecessary latency.
    pub fn lock_unfair(&self) -> MutexGuard<'_, T> {
        self.data.lock()
    }

    /// Unfairly try to lock the mutex without blocking.
    ///
    /// Returns `None` if the lock is currently held.
    pub fn try_lock_unfair(&self) -> Option<MutexGuard<'_, T>> {
        self.data.try_lock()
    }

    /// Get a mutable reference to the underlying data.
    ///
    /// This requires mutable access to the mutex, which guarantees exclusivity.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consume the mutex and return the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

// Safety: FairMutex is Send/Sync if the contained type is Send.
// This follows the same safety reasoning as parking_lot::Mutex.
unsafe impl<T: Send> Send for FairMutex<T> {}
unsafe impl<T: Send> Sync for FairMutex<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_basic_lock() {
        let mutex = FairMutex::new(42);
        {
            let guard = mutex.lock();
            assert_eq!(*guard, 42);
        }
    }

    #[test]
    fn test_unfair_lock() {
        let mutex = FairMutex::new(42);
        {
            let guard = mutex.lock_unfair();
            assert_eq!(*guard, 42);
        }
    }

    #[test]
    fn test_try_lock() {
        let mutex = FairMutex::new(42);
        let guard = mutex.try_lock_unfair();
        assert!(guard.is_some());
        assert_eq!(*guard.unwrap(), 42);
    }

    #[test]
    fn test_try_lock_fails_when_held() {
        let mutex = FairMutex::new(42);
        let _guard = mutex.lock();
        let try_guard = mutex.try_lock_unfair();
        assert!(try_guard.is_none());
    }

    #[test]
    fn test_get_mut() {
        let mut mutex = FairMutex::new(42);
        *mutex.get_mut() = 100;
        assert_eq!(*mutex.lock(), 100);
    }

    #[test]
    fn test_into_inner() {
        let mutex = FairMutex::new(42);
        assert_eq!(mutex.into_inner(), 42);
    }

    #[test]
    fn test_concurrent_access() {
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
    fn test_lease_blocks_fair_lock() {
        let mutex = Arc::new(FairMutex::new(0));
        let mutex2 = Arc::clone(&mutex);

        // Acquire a lease
        let _lease = mutex.lease();

        // Spawn a thread that tries to get a fair lock
        let handle = thread::spawn(move || {
            // This should block until the lease is dropped
            let _guard = mutex2.lock();
        });

        // Give the thread time to start
        thread::sleep(std::time::Duration::from_millis(10));

        // The thread should still be blocked
        assert!(!handle.is_finished());

        // Drop the lease - now the thread can proceed
        drop(_lease);

        // Thread should complete
        handle.join().unwrap();
    }
}
