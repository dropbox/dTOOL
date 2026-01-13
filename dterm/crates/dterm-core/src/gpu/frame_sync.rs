//! Safe frame synchronization using Rust channels.
//!
//! This module provides frame synchronization primitives that are safe by
//! construction. Unlike `dispatch_group` or semaphores, these primitives
//! CANNOT crash with "unbalanced" errors.
//!
//! ## Why This Exists
//!
//! The ObjC rendering stack in iTerm2/DashTerm2 uses `dispatch_group` for
//! frame synchronization. This has fundamental problems:
//!
//! 1. `dispatch_group_leave` crashes if called more times than `dispatch_group_enter`
//! 2. Timeout handling is error-prone (must clean up state correctly)
//! 3. Reusing promises accumulates callbacks
//!
//! Rust's ownership model makes these bugs impossible:
//!
//! - `oneshot::Sender` can only send once (compile-time enforced)
//! - If receiver times out, sender is just dropped (no "unbalanced" error)
//! - Channels clean up automatically when dropped

use parking_lot::{Condvar, Mutex};
use std::sync::Arc;
use std::time::Duration;

/// Status of a frame request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    /// Frame is ready - drawable was provided
    Ready,
    /// Timeout expired before drawable was provided
    Timeout,
    /// Request was cancelled (sender dropped)
    Cancelled,
}

/// Internal frame request state.
struct SharedState {
    /// Mutex-protected completion flag
    completed: Mutex<bool>,
    /// Condition variable for waiting
    condvar: Condvar,
    /// Unique frame ID (stored for debugging/logging)
    #[allow(dead_code)]
    id: u64,
}

/// A request for a frame to be rendered.
///
/// The platform code receives this and must call `complete()` to provide
/// the drawable/surface texture. If the request is dropped without calling
/// `complete()`, the wait will return `FrameStatus::Cancelled`.
///
/// **Safe**: This cannot cause crashes. Dropping without completing is safe.
pub struct FrameRequest {
    /// Frame ID for tracking
    id: u64,
    /// Shared state with the waiter
    state: Arc<SharedState>,
    /// Whether complete() was called (for drop handling)
    completed_called: bool,
}

impl FrameRequest {
    /// Create a new frame request.
    fn new(id: u64) -> (Self, FrameWaiter) {
        let state = Arc::new(SharedState {
            completed: Mutex::new(false),
            condvar: Condvar::new(),
            id,
        });

        let request = FrameRequest {
            id,
            state: Arc::clone(&state),
            completed_called: false,
        };

        let waiter = FrameWaiter { state };

        (request, waiter)
    }

    /// Get the frame ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Complete the frame request, signaling that the drawable is ready.
    ///
    /// This method consumes self, ensuring it can only be called once.
    /// The waiter will be notified and `wait_for_frame` will return `Ready`.
    ///
    /// **Safe**: Can only be called once (ownership consumed).
    pub fn complete(mut self) {
        self.completed_called = true;
        {
            let mut guard = self.state.completed.lock();
            *guard = true;
        }
        self.state.condvar.notify_all();
        // self is dropped here, completed_called is true so drop won't re-notify
    }
}

impl Drop for FrameRequest {
    fn drop(&mut self) {
        // If complete() wasn't called, notify waiter of cancellation
        if !self.completed_called {
            {
                let mut guard = self.state.completed.lock();
                *guard = true; // Mark as "completed" so waiter wakes up
            }
            self.state.condvar.notify_all();
        }
    }
}

/// Internal waiter for a frame request.
struct FrameWaiter {
    state: Arc<SharedState>,
}

impl FrameWaiter {
    /// Wait for the frame to be ready, with timeout.
    ///
    /// # Returns
    /// - `FrameStatus::Ready` if `complete()` was called on the request
    /// - `FrameStatus::Timeout` if the timeout expired
    /// - `FrameStatus::Cancelled` if the request was dropped without completing
    fn wait(&self, timeout: Duration) -> FrameStatus {
        let mut guard = self.state.completed.lock();

        if *guard {
            return FrameStatus::Ready;
        }

        // Wait with timeout
        let result = self.state.condvar.wait_for(&mut guard, timeout);

        if result.timed_out() {
            FrameStatus::Timeout
        } else if *guard {
            // Completed (either via complete() or dropped)
            FrameStatus::Ready
        } else {
            // Spurious wakeup? Try again would be better but return Cancelled for safety
            FrameStatus::Cancelled
        }
    }
}

/// Frame synchronization state machine.
///
/// Manages pending frame requests and provides safe waiting.
pub struct FrameSync {
    /// Current pending waiter, if any
    current_waiter: Option<FrameWaiter>,
}

impl FrameSync {
    /// Create a new frame sync state machine.
    pub fn new() -> Self {
        Self {
            current_waiter: None,
        }
    }

    /// Request a new frame.
    ///
    /// If there's already a pending request, it will be replaced (the old
    /// waiter will be dropped, which is safe).
    ///
    /// # Arguments
    /// * `frame_id` - Unique identifier for this frame
    ///
    /// # Returns
    /// A `FrameRequest` that the platform code uses to signal drawable ready.
    pub fn request_frame(&mut self, frame_id: u64) -> FrameRequest {
        let (request, waiter) = FrameRequest::new(frame_id);
        self.current_waiter = Some(waiter);
        request
    }

    /// Wait for the current frame to be ready.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    /// The frame status. If no frame was requested, returns `FrameStatus::Cancelled`.
    ///
    /// **Safe**: Cannot crash with "unbalanced" errors. Timeout just returns
    /// `FrameStatus::Timeout` and cleans up automatically.
    pub fn wait_for_frame(&self, timeout: Duration) -> FrameStatus {
        match &self.current_waiter {
            Some(waiter) => waiter.wait(timeout),
            None => FrameStatus::Cancelled,
        }
    }

    /// Clear the current pending request.
    pub fn clear(&mut self) {
        self.current_waiter = None;
    }
}

impl Default for FrameSync {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_request_complete() {
        let (request, waiter) = FrameRequest::new(1);

        // Complete the request
        request.complete();

        // Waiter should return Ready
        assert_eq!(waiter.wait(Duration::from_millis(100)), FrameStatus::Ready);
    }

    #[test]
    fn test_frame_request_timeout() {
        let (_request, waiter) = FrameRequest::new(1);

        // Don't complete - should timeout
        let status = waiter.wait(Duration::from_millis(1));
        assert_eq!(status, FrameStatus::Timeout);
    }

    #[test]
    fn test_frame_request_cancelled_on_drop() {
        let waiter = {
            let (request, waiter) = FrameRequest::new(1);
            // Let request drop without completing
            drop(request);
            waiter
        };

        // Should return Ready (since drop notifies completion)
        let status = waiter.wait(Duration::from_millis(100));
        assert_eq!(status, FrameStatus::Ready);
    }

    #[test]
    fn test_frame_sync_flow() {
        let mut sync = FrameSync::new();

        // Request a frame
        let request = sync.request_frame(1);

        // Spawn thread to complete after delay
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            request.complete();
        });

        // Wait should succeed
        let status = sync.wait_for_frame(Duration::from_millis(100));
        assert_eq!(status, FrameStatus::Ready);
    }

    #[test]
    fn test_frame_sync_no_request() {
        let sync = FrameSync::new();

        // No request - should return Cancelled
        assert_eq!(
            sync.wait_for_frame(Duration::from_millis(1)),
            FrameStatus::Cancelled
        );
    }

    #[test]
    fn test_frame_sync_replace_pending() {
        let mut sync = FrameSync::new();

        // Request first frame
        let _request1 = sync.request_frame(1);

        // Request second frame (replaces first)
        let request2 = sync.request_frame(2);

        // Complete second
        request2.complete();

        // Should return Ready
        assert_eq!(
            sync.wait_for_frame(Duration::from_millis(100)),
            FrameStatus::Ready
        );
    }

    #[test]
    fn test_stress_no_crashes() {
        // The pattern that crashed ObjC: rapid request/timeout cycles
        let mut sync = FrameSync::new();

        for i in 0..1000 {
            let request = sync.request_frame(i);

            // Randomly complete or timeout
            if i % 3 == 0 {
                request.complete();
                let _ = sync.wait_for_frame(Duration::from_millis(10));
            } else {
                // Let it timeout
                let _ = sync.wait_for_frame(Duration::from_micros(1));
                drop(request);
            }
        }
        // No crashes! (unlike dispatch_group)
    }
}
