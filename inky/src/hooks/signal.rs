//! Reactive signal implementation.
//!
//! Signals are the core primitive for reactive state management in inky.
//! When a signal's value changes, it notifies all subscribers, which can
//! trigger re-renders or other side effects.
//!
//! # Example
//!
//! ```
//! use inky::hooks::Signal;
//!
//! let count = Signal::new(0);
//! assert_eq!(count.get(), 0);
//!
//! count.set(5);
//! assert_eq!(count.get(), 5);
//!
//! count.update(|c| *c += 1);
//! assert_eq!(count.get(), 6);
//! ```

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock, Weak};

// Type alias for subscriber list to satisfy clippy type_complexity
type SubscriberList = Arc<RwLock<Vec<Weak<dyn Fn() + Send + Sync>>>>;

/// Global flag to request re-render when signals change.
/// This is checked by the App event loop.
static RENDER_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Check if a re-render has been requested and clear the flag.
pub fn take_render_request() -> bool {
    RENDER_REQUESTED.swap(false, Ordering::SeqCst)
}

/// Request a re-render from signal changes.
pub fn request_render() {
    RENDER_REQUESTED.store(true, Ordering::SeqCst);
}

/// A reactive signal that holds a value and notifies subscribers on change.
///
/// Signals are thread-safe (`Send + Sync`) and can be cloned cheaply since
/// they use reference counting internally. Multiple clones share the same
/// underlying value.
pub struct Signal<T> {
    value: Arc<RwLock<T>>,
    subscribers: SubscriberList,
}

impl<T> Signal<T> {
    /// Create a new signal with an initial value.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::hooks::Signal;
    ///
    /// let name = Signal::new(String::from("Alice"));
    /// let count = Signal::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to changes. The callback is called whenever the value changes.
    ///
    /// The callback is stored as a weak reference, so it will be cleaned up
    /// if the Arc is dropped elsewhere.
    pub fn subscribe<F: Fn() + Send + Sync + 'static>(&self, callback: Arc<F>) {
        self.subscribers
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(Arc::downgrade(&callback) as Weak<dyn Fn() + Send + Sync>);
    }

    /// Notify all subscribers and request a re-render.
    /// Dead weak references are cleaned up during notification to prevent unbounded growth.
    fn notify(&self) {
        // Request re-render from the App
        request_render();

        // Collect live subscribers while retaining only alive ones
        // This combines notification with cleanup in a single pass
        let callbacks: Vec<_> = {
            let mut subscribers = self
                .subscribers
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut live_callbacks = Vec::new();
            subscribers.retain(|weak| {
                if let Some(callback) = weak.upgrade() {
                    live_callbacks.push(callback);
                    true // Keep alive reference
                } else {
                    false // Remove dead reference
                }
            });
            live_callbacks
        }; // Lock released here

        // Call subscribers without holding lock
        for callback in callbacks {
            callback();
        }
    }

    /// Clean up expired weak references.
    pub fn cleanup_subscribers(&self) {
        let mut subscribers = self
            .subscribers
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        subscribers.retain(|weak| weak.strong_count() > 0);
    }
}

impl<T: Clone> Signal<T> {
    /// Get the current value.
    pub fn get(&self) -> T {
        self.value
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Set a new value and notify subscribers.
    pub fn set(&self, value: T) {
        *self
            .value
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
        self.notify();
    }
}

impl<T> Signal<T> {
    /// Update the value with a function and notify subscribers.
    pub fn update<F: FnOnce(&mut T)>(&self, f: F) {
        f(&mut *self
            .value
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()));
        self.notify();
    }

    /// Get a read reference (borrows the lock).
    pub fn with<R, F: FnOnce(&T) -> R>(&self, f: F) -> R {
        f(&*self
            .value
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    /// Get a write reference (borrows the lock).
    pub fn with_mut<R, F: FnOnce(&mut T) -> R>(&self, f: F) -> R {
        let result = f(&mut *self
            .value
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()));
        self.notify();
        result
    }
}

impl<T: Clone> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            subscribers: self.subscribers.clone(),
        }
    }
}

impl<T: Default> Default for Signal<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: fmt::Debug> fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value.try_read() {
            Ok(value) => f.debug_struct("Signal").field("value", &*value).finish(),
            Err(_) => f
                .debug_struct("Signal")
                .field("value", &"<locked>")
                .finish(),
        }
    }
}

impl<T: PartialEq + Clone> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

// SAFETY: `Signal<T>` is safe to send across threads when `T: Send` because:
// 1. The inner value is protected by `Arc<RwLock<T>>`, which provides thread-safe
//    interior mutability via synchronized read/write locking.
// 2. The subscribers list is also protected by `Arc<RwLock<...>>`.
// 3. When `T: Send`, the value can be safely transferred to another thread.
// 4. All access to the inner value goes through the RwLock, preventing data races.
unsafe impl<T: Send> Send for Signal<T> {}

// SAFETY: `Signal<T>` can be safely shared between threads when `T: Send + Sync` because:
// 1. Multiple threads can hold `&Signal<T>` simultaneously.
// 2. All reads go through `RwLock::read()` which allows concurrent readers.
// 3. All writes go through `RwLock::write()` which ensures exclusive access.
// 4. `T: Sync` ensures that shared references to the value are safe across threads.
// 5. The `Arc` wrapper provides thread-safe reference counting.
unsafe impl<T: Send + Sync> Sync for Signal<T> {}

/// Create a new signal.
///
/// This is a convenience function that simply calls `Signal::new`.
/// Use it when you want hook-like semantics in your code.
///
/// # Example
///
/// ```
/// use inky::hooks::use_signal;
///
/// let counter = use_signal(0);
/// counter.update(|c| *c += 1);
/// assert_eq!(counter.get(), 1);
/// ```
pub fn use_signal<T>(value: T) -> Signal<T> {
    Signal::new(value)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_new() {
        let signal = Signal::new(42);
        assert_eq!(signal.get(), 42);
    }

    #[test]
    fn test_signal_set() {
        let signal = Signal::new(0);
        signal.set(100);
        assert_eq!(signal.get(), 100);
    }

    #[test]
    fn test_signal_update() {
        let signal = Signal::new(10);
        signal.update(|v| *v += 5);
        assert_eq!(signal.get(), 15);
    }

    #[test]
    fn test_signal_with() {
        let signal = Signal::new(vec![1, 2, 3]);
        let sum = signal.with(|v| v.iter().sum::<i32>());
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_signal_with_mut() {
        let signal = Signal::new(vec![1, 2, 3]);
        let popped = signal.with_mut(|v| v.pop());
        assert_eq!(popped, Some(3));
        assert_eq!(signal.get(), vec![1, 2]);
    }

    #[test]
    fn test_signal_clone_shares_value() {
        let signal1 = Signal::new(42);
        let signal2 = signal1.clone();

        signal1.set(100);
        assert_eq!(signal2.get(), 100);
    }

    #[test]
    fn test_signal_default() {
        let signal: Signal<i32> = Signal::default();
        assert_eq!(signal.get(), 0);

        let signal: Signal<String> = Signal::default();
        assert_eq!(signal.get(), "");
    }

    #[test]
    fn test_signal_debug() {
        let signal = Signal::new(42);
        let debug_str = format!("{:?}", signal);
        assert!(debug_str.contains("Signal"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_signal_partial_eq() {
        let signal1 = Signal::new(42);
        let signal2 = Signal::new(42);
        let signal3 = Signal::new(100);

        assert_eq!(signal1, signal2);
        assert_ne!(signal1, signal3);
    }

    #[test]
    fn test_use_signal_function() {
        let signal = use_signal("hello");
        assert_eq!(signal.get(), "hello");
    }

    #[test]
    fn test_signal_requests_render() {
        // Clear any pending render requests from other tests.
        // Since RENDER_REQUESTED is a global static and tests run in parallel,
        // we need to clear repeatedly until we get a clean state.
        while take_render_request() {}

        let signal = Signal::new(0);

        // Signal::new should NOT request a render - only set/update do.
        // However, other tests may set the flag concurrently, so we just
        // clear again to ensure we have a clean baseline.
        while take_render_request() {}

        signal.set(1);
        // After set(), a render must have been requested
        assert!(take_render_request(), "set() should request a render");

        // After taking, the flag should be cleared (modulo concurrent tests,
        // but we just need to verify our set() worked, which we did above)
    }

    #[test]
    fn test_signal_subscriber_notification() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let signal = Signal::new(0);
        let call_count = Arc::new(AtomicUsize::new(0));

        let count_clone = call_count.clone();
        let callback = Arc::new(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        signal.subscribe(callback.clone());

        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        signal.set(1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        signal.update(|v| *v += 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        // Keep callback alive to prevent weak ref cleanup
        drop(callback);
    }

    #[test]
    fn test_signal_thread_safety() {
        use std::thread;

        let signal = Signal::new(0);
        let signal_clone = signal.clone();

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                signal_clone.update(|v| *v += 1);
            }
        });

        for _ in 0..100 {
            signal.update(|v| *v += 1);
        }

        handle.join().unwrap();
        assert_eq!(signal.get(), 200);
    }

    #[test]
    fn test_signal_cleanup_subscribers() {
        let signal = Signal::new(0);

        // Create a callback that will be dropped
        {
            let callback = Arc::new(|| {});
            signal.subscribe(callback);
            // callback is dropped here
        }

        // Cleanup should remove the expired weak reference
        signal.cleanup_subscribers();

        // Verify no panic when notifying with empty/cleaned subscribers
        signal.set(1);
    }
}
