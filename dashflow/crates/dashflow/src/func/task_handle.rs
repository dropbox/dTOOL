// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Task handle for `DashFlow` Functional API
//!
//! `TaskHandle<T>` wraps a tokio task and provides methods to retrieve the result.
//! It implements `Future` so it can be awaited directly, and also provides a `.result()`
//! method for compatibility with the Python API.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::task::JoinHandle;

/// A handle to a spawned task that will produce a value of type `T`.
///
/// `TaskHandle<T>` is similar to tokio's `JoinHandle<T>` but provides additional
/// methods for convenience and compatibility with the `DashFlow` Functional API.
///
/// # Examples
///
/// ```rust,ignore
/// use dashflow::func::TaskHandle;
///
/// // Spawn a task
/// let handle = TaskHandle::spawn(async { 42 });
///
/// // Await directly
/// let result = handle.await.unwrap();
/// assert_eq!(result, 42);
/// ```
///
/// ```rust,ignore
/// // Or use .result() method
/// let handle = TaskHandle::spawn(async { 42 });
/// let result = handle.result().await.unwrap();
/// assert_eq!(result, 42);
/// ```
pub struct TaskHandle<T> {
    inner: JoinHandle<T>,
}

impl<T> TaskHandle<T>
where
    T: Send + 'static,
{
    /// Spawn a new task and return a handle to it.
    ///
    /// The task is spawned on the current tokio runtime.
    ///
    /// # Panics
    ///
    /// Panics if called outside of a tokio runtime context.
    pub fn spawn<F>(future: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        Self {
            inner: tokio::spawn(future),
        }
    }

    /// Wait for the task to complete and return its result.
    ///
    /// This is equivalent to awaiting the `TaskHandle` directly, but provides
    /// an explicit method name for clarity and Python API compatibility.
    ///
    /// # Errors
    ///
    /// Returns an error if the task panicked or was cancelled.
    pub async fn result(self) -> Result<T, tokio::task::JoinError> {
        self.inner.await
    }

    /// Abort the task.
    ///
    /// This will cause the task to be cancelled. If the task has already completed,
    /// this method has no effect.
    pub fn abort(&self) {
        self.inner.abort();
    }

    /// Check if the task has finished.
    ///
    /// Returns `true` if the task has completed (either successfully or with an error),
    /// `false` if it's still running.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }
}

impl<T> Future for TaskHandle<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // JoinHandle<T> implements Unpin, so we can safely use Pin::new() without unsafe
        let inner = Pin::new(&mut self.inner);
        inner.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_handle_spawn_and_await() {
        let handle = TaskHandle::spawn(async { 42 });
        let result = handle.await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_task_handle_result_method() {
        let handle = TaskHandle::spawn(async { "hello" });
        let result = handle.result().await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_task_handle_with_error() {
        let handle: TaskHandle<Result<i32, String>> =
            TaskHandle::spawn(async { Err("task failed".to_string()) });
        let result = handle.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "task failed");
    }

    #[tokio::test]
    async fn test_task_handle_abort() {
        let handle = TaskHandle::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            42
        });
        handle.abort();
        let result = handle.await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_handle_is_finished() {
        let handle = TaskHandle::spawn(async { 42 });
        assert!(!handle.is_finished());
        let _result = handle.await;
        // Note: We can't check is_finished after awaiting because we consumed the handle
    }

    #[tokio::test]
    async fn test_task_handle_async_computation() {
        let handle = TaskHandle::spawn(async {
            let mut sum = 0;
            for i in 1..=10 {
                sum += i;
            }
            sum
        });
        let result = handle.await.unwrap();
        assert_eq!(result, 55);
    }
}
