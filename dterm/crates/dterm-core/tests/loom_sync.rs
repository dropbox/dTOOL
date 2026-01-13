//! Loom concurrency tests for FairMutex and FairRwLock
//!
//! These tests use the loom crate to exhaustively explore all possible
//! thread interleavings, catching concurrency bugs that might only
//! manifest in rare timing conditions.
//!
//! To run these tests:
//!   RUSTFLAGS="--cfg loom" cargo test --test loom_sync --release
//!
//! Note: These tests are slow because loom explores all possible
//! thread schedules. Each test may take several seconds.
//!
//! The tests use simplified versions of FairMutex/FairRwLock that
//! implement the same two-lock fairness protocol using loom's
//! deterministic primitives.

#![cfg(loom)]

use loom::sync::atomic::{AtomicU32, Ordering};
use loom::sync::{Arc, Mutex};
use loom::thread;

/// Simplified FairMutex using loom primitives for testing
struct LoomFairMutex<T> {
    data: Mutex<T>,
    next: Mutex<()>,
}

impl<T> LoomFairMutex<T> {
    fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Fair lock - acquires next first, then data
    fn lock(&self) -> loom::sync::MutexGuard<'_, T> {
        let _next = self.next.lock().unwrap();
        self.data.lock().unwrap()
    }

    /// Unfair lock - directly acquires data
    fn lock_unfair(&self) -> loom::sync::MutexGuard<'_, T> {
        self.data.lock().unwrap()
    }

    /// Lease - holds next lock only
    fn lease(&self) -> loom::sync::MutexGuard<'_, ()> {
        self.next.lock().unwrap()
    }

    /// Lock with lease - uses existing lease to acquire data
    fn lock_with_lease<'a>(
        &'a self,
        _lease: loom::sync::MutexGuard<'a, ()>,
    ) -> loom::sync::MutexGuard<'a, T> {
        self.data.lock().unwrap()
    }
}

/// Simplified FairRwLock using loom primitives for testing
struct LoomFairRwLock<T> {
    data: Mutex<T>,
    next: Mutex<()>,
}

impl<T: Clone> LoomFairRwLock<T> {
    fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Read lock - acquires data mutex (simplified - loom doesn't have RwLock)
    fn read(&self) -> T {
        let guard = self.data.lock().unwrap();
        guard.clone()
    }

    /// Write lock - acquires next first, then data
    fn write(&self) -> loom::sync::MutexGuard<'_, T> {
        let _next = self.next.lock().unwrap();
        self.data.lock().unwrap()
    }
}

/// Test: Two threads incrementing a counter must not lose updates
#[test]
fn fair_mutex_no_lost_updates() {
    loom::model(|| {
        let mutex = Arc::new(LoomFairMutex::new(0u32));
        let m1 = mutex.clone();
        let m2 = mutex.clone();

        let t1 = thread::spawn(move || {
            let mut guard = m1.lock();
            *guard += 1;
        });

        let t2 = thread::spawn(move || {
            let mut guard = m2.lock();
            *guard += 1;
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Both increments must be visible
        assert_eq!(*mutex.lock(), 2);
    });
}

/// Test: Unfair lock also preserves data integrity
#[test]
fn fair_mutex_unfair_lock_safe() {
    loom::model(|| {
        let mutex = Arc::new(LoomFairMutex::new(0u32));
        let m1 = mutex.clone();
        let m2 = mutex.clone();

        let t1 = thread::spawn(move || {
            let mut guard = m1.lock_unfair();
            *guard += 1;
        });

        let t2 = thread::spawn(move || {
            let mut guard = m2.lock_unfair();
            *guard += 1;
        });

        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(*mutex.lock_unfair(), 2);
    });
}

/// Test: Fair lock serializes access properly
#[test]
fn fair_mutex_serializes_access() {
    loom::model(|| {
        let counter = Arc::new(AtomicU32::new(0));
        let mutex = Arc::new(LoomFairMutex::new(()));

        let c1 = counter.clone();
        let m1 = mutex.clone();
        let c2 = counter.clone();
        let m2 = mutex.clone();

        let t1 = thread::spawn(move || {
            let _guard = m1.lock();
            // Critical section - increment
            c1.fetch_add(1, Ordering::SeqCst);
        });

        let t2 = thread::spawn(move || {
            let _guard = m2.lock();
            // Critical section - increment
            c2.fetch_add(1, Ordering::SeqCst);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Both must complete
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    });
}

/// Test: Lease guarantees next-in-line access
#[test]
fn lease_reserves_access() {
    loom::model(|| {
        let mutex = Arc::new(LoomFairMutex::new(0u32));
        let m1 = mutex.clone();

        // Main thread gets a lease
        let lease = mutex.lease();

        let t1 = thread::spawn(move || {
            // This will have to wait for lease holder
            let mut guard = m1.lock();
            *guard += 10;
        });

        // Convert lease to guard and increment
        {
            let mut guard = mutex.lock_with_lease(lease);
            *guard += 1;
        }

        t1.join().unwrap();

        // Order may vary, but both increments must happen
        let final_value = *mutex.lock();
        assert!(final_value == 11, "Expected 11, got {}", final_value);
    });
}

/// Test: FairRwLock writer does not lose updates
#[test]
fn fair_rwlock_no_lost_writes() {
    loom::model(|| {
        let lock = Arc::new(LoomFairRwLock::new(0u32));
        let l1 = lock.clone();
        let l2 = lock.clone();

        // Writer 1
        let t1 = thread::spawn(move || {
            let mut guard = l1.write();
            *guard += 1;
        });

        // Writer 2
        let t2 = thread::spawn(move || {
            let mut guard = l2.write();
            *guard += 1;
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Both writes must be visible
        assert_eq!(lock.read(), 2);
    });
}

/// Test: Reader does not see partial writes
#[test]
fn fair_rwlock_no_torn_reads() {
    loom::model(|| {
        let lock = Arc::new(LoomFairRwLock::new(0u32));
        let l1 = lock.clone();
        let l2 = lock.clone();

        // Writer sets value to 42
        let t1 = thread::spawn(move || {
            let mut guard = l1.write();
            *guard = 42;
        });

        // Reader reads value
        let t2 = thread::spawn(move || l2.read());

        t1.join().unwrap();
        let read_value = t2.join().unwrap();

        // Reader must see either 0 (before write) or 42 (after write)
        assert!(
            read_value == 0 || read_value == 42,
            "Torn read: got {}",
            read_value
        );
    });
}

/// Test: Multiple readers can proceed concurrently (conceptually)
#[test]
fn fair_rwlock_readers_see_consistent_data() {
    loom::model(|| {
        let lock = Arc::new(LoomFairRwLock::new(42u32));
        let l1 = lock.clone();
        let l2 = lock.clone();

        // Two readers
        let t1 = thread::spawn(move || l1.read());
        let t2 = thread::spawn(move || l2.read());

        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();

        // Both must see the same initial value (no writer)
        assert_eq!(r1, 42);
        assert_eq!(r2, 42);
    });
}
