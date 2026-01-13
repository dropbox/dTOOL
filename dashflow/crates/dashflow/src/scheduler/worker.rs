// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Remote worker management for work-stealing scheduler

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

use crate::constants::{DEFAULT_HEALTH_CHECK_INTERVAL, DEFAULT_HTTP_CONNECT_TIMEOUT, SHORT_TIMEOUT};
use crate::error::{Error, Result};
use crate::state::GraphState;

/// Default health check path appended to worker endpoint
const DEFAULT_HEALTH_PATH: &str = "/health";

use super::task::Task;

/// Health status of a remote worker
#[derive(Debug, Clone)]
pub struct WorkerHealth {
    /// Whether worker is currently healthy
    pub is_healthy: bool,
    /// Last successful health check timestamp
    pub last_health_check: SystemTime,
    /// Number of consecutive health check failures
    pub consecutive_failures: u32,
}

impl WorkerHealth {
    /// Create a new healthy worker status
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            is_healthy: true,
            last_health_check: SystemTime::now(),
            consecutive_failures: 0,
        }
    }

    /// Mark worker as unhealthy
    pub fn mark_unhealthy(&mut self) {
        self.is_healthy = false;
        self.consecutive_failures += 1;
    }

    /// Mark worker as healthy
    pub fn mark_healthy(&mut self) {
        self.is_healthy = true;
        self.last_health_check = SystemTime::now();
        self.consecutive_failures = 0;
    }

    /// Check if health check is stale (older than threshold)
    #[must_use]
    pub fn is_stale(&self, threshold: Duration) -> bool {
        self.last_health_check
            .elapsed()
            .map(|elapsed| elapsed > threshold)
            .unwrap_or(true)
    }
}

/// A remote worker that can execute tasks via gRPC
///
/// `RemoteWorker` wraps a dashflow-remote-node client and provides
/// load tracking for work-stealing decisions.
pub struct RemoteWorker {
    /// Unique worker ID
    pub id: String,
    /// gRPC endpoint
    pub endpoint: String,
    /// Estimated current load (number of in-flight tasks)
    load: AtomicU32,
    /// Health status
    health: Arc<RwLock<WorkerHealth>>,
}

impl RemoteWorker {
    /// Create a new remote worker
    ///
    /// # Arguments
    ///
    /// * `id` - Unique worker identifier
    /// * `endpoint` - gRPC endpoint (e.g., "<http://worker1:50051>")
    #[must_use]
    pub fn new(id: String, endpoint: String) -> Self {
        Self {
            id,
            endpoint,
            load: AtomicU32::new(0),
            health: Arc::new(RwLock::new(WorkerHealth::healthy())),
        }
    }

    /// Get current load estimate
    pub fn load(&self) -> u32 {
        self.load.load(Ordering::Relaxed)
    }

    /// Check if worker is healthy
    pub async fn is_healthy(&self) -> bool {
        self.health.read().await.is_healthy
    }

    /// Increment load counter
    fn increment_load(&self) {
        self.load.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement load counter
    fn decrement_load(&self) {
        self.load.fetch_sub(1, Ordering::Relaxed);
    }

    /// Mark worker as healthy
    pub async fn mark_healthy(&self) {
        self.health.write().await.mark_healthy();
    }

    /// Mark worker as unhealthy
    pub async fn mark_unhealthy(&self) {
        self.health.write().await.mark_unhealthy();
    }

    /// Execute a batch of tasks on this worker
    ///
    /// This will be implemented to use dashflow-remote-node's `RemoteNode`
    /// to execute each task via gRPC.
    ///
    /// # Arguments
    ///
    /// * `tasks` - Tasks to execute
    ///
    /// # Returns
    ///
    /// Vector of results in the same order as tasks
    pub async fn execute_batch<S>(&self, _tasks: Vec<Task<S>>) -> Result<Vec<S>>
    where
        S: GraphState,
    {
        // Increment load before execution
        self.increment_load();

        // Remote execution placeholder.
        // Full implementation requires dashflow-remote-node crate integration:
        // 1. RemoteNode gRPC client for task serialization
        // 2. Distributed executor with load balancing
        // 3. Result deserialization and error propagation
        //
        // Current behavior: Returns error indicating remote execution unavailable
        let result: Result<Vec<S>> = Err(Error::Validation(format!(
            "Remote execution not yet integrated with dashflow-remote-node (worker: {})",
            self.id
        )));

        // Decrement load after execution
        self.decrement_load();

        // Update health based on result
        if result.is_ok() {
            self.mark_healthy().await;
        } else {
            self.mark_unhealthy().await;
        }

        result
    }
}

impl std::fmt::Debug for RemoteWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteWorker")
            .field("id", &self.id)
            .field("endpoint", &self.endpoint)
            .field("load", &self.load())
            .finish()
    }
}

/// Pool of remote workers
///
/// Manages a collection of remote workers and tracks their health status.
pub struct WorkerPool {
    /// Workers in the pool
    workers: Vec<Arc<RemoteWorker>>,
    /// Health check interval
    health_check_interval: Duration,
    /// Health check path to append to worker endpoint (default: "/health")
    health_check_path: String,
    /// HTTP timeout for health checks (default: 5 seconds)
    health_check_timeout: Duration,
}

impl WorkerPool {
    /// Create an empty worker pool
    #[must_use]
    pub fn new() -> Self {
        Self {
            workers: Vec::new(),
            health_check_interval: DEFAULT_HEALTH_CHECK_INTERVAL,
            health_check_path: DEFAULT_HEALTH_PATH.to_string(),
            health_check_timeout: SHORT_TIMEOUT,
        }
    }

    /// Create a worker pool with workers
    #[must_use]
    pub fn with_workers(workers: Vec<RemoteWorker>) -> Self {
        Self {
            workers: workers.into_iter().map(Arc::new).collect(),
            health_check_interval: DEFAULT_HEALTH_CHECK_INTERVAL,
            health_check_path: DEFAULT_HEALTH_PATH.to_string(),
            health_check_timeout: SHORT_TIMEOUT,
        }
    }

    /// Set custom health check path
    ///
    /// # Arguments
    ///
    /// * `path` - Path to append to worker endpoint (e.g., "/health", "/api/status")
    #[must_use]
    pub fn with_health_check_path(mut self, path: impl Into<String>) -> Self {
        self.health_check_path = path.into();
        self
    }

    /// Set custom health check timeout
    #[must_use]
    pub fn with_health_check_timeout(mut self, timeout: Duration) -> Self {
        self.health_check_timeout = timeout;
        self
    }

    /// Check if pool has any workers
    #[must_use]
    pub fn has_workers(&self) -> bool {
        !self.workers.is_empty()
    }

    /// Get all workers
    #[must_use]
    pub fn workers(&self) -> &[Arc<RemoteWorker>] {
        &self.workers
    }

    /// Get available (healthy) workers
    pub async fn available_workers(&self) -> Vec<Arc<RemoteWorker>> {
        let mut available = Vec::new();
        for worker in &self.workers {
            if worker.is_healthy().await {
                available.push(worker.clone());
            }
        }
        available
    }

    /// Perform health checks on all workers
    ///
    /// Sends HTTP GET requests to each worker's health endpoint and updates
    /// their health status based on the response. Workers that respond with
    /// 2xx status codes are marked healthy; all others are marked unhealthy.
    ///
    /// # Health Check Behavior
    ///
    /// - Constructs URL as `{worker.endpoint}{health_check_path}`
    /// - Uses configured timeout (default: 5 seconds)
    /// - 2xx responses → worker marked healthy
    /// - Any error or non-2xx → worker marked unhealthy
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dashflow::scheduler::worker::{WorkerPool, RemoteWorker};
    ///
    /// let workers = vec![
    ///     RemoteWorker::new("worker1".to_string(), "http://worker1:8080".to_string()),
    ///     RemoteWorker::new("worker2".to_string(), "http://worker2:8080".to_string()),
    /// ];
    ///
    /// let pool = WorkerPool::with_workers(workers)
    ///     .with_health_check_path("/api/health")
    ///     .with_health_check_timeout(Duration::from_secs(3));
    ///
    /// // Check all workers
    /// pool.health_check_all().await?;
    ///
    /// // Get healthy workers only
    /// let available = pool.available_workers().await;
    /// ```
    pub async fn health_check_all(&self) -> Result<()> {
        if self.workers.is_empty() {
            return Ok(());
        }

        let client = reqwest::Client::builder()
            .timeout(self.health_check_timeout)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .map_err(|e| {
                Error::InternalExecutionError(format!("Failed to create HTTP client: {}", e))
            })?;

        // Check all workers concurrently
        let futures: Vec<_> = self
            .workers
            .iter()
            .map(|worker| {
                let client = client.clone();
                let health_url = format!(
                    "{}{}",
                    worker.endpoint.trim_end_matches('/'),
                    self.health_check_path
                );
                let worker = worker.clone();

                async move {
                    match client.get(&health_url).send().await {
                        Ok(response) if response.status().is_success() => {
                            worker.mark_healthy().await;
                            tracing::debug!(
                                worker_id = %worker.id,
                                endpoint = %health_url,
                                "Worker health check passed"
                            );
                        }
                        Ok(response) => {
                            worker.mark_unhealthy().await;
                            tracing::warn!(
                                worker_id = %worker.id,
                                endpoint = %health_url,
                                status = %response.status(),
                                "Worker health check failed: non-success status"
                            );
                        }
                        Err(e) => {
                            worker.mark_unhealthy().await;
                            tracing::warn!(
                                worker_id = %worker.id,
                                endpoint = %health_url,
                                error = %e,
                                "Worker health check failed: request error"
                            );
                        }
                    }
                }
            })
            .collect();

        // Execute all health checks concurrently
        futures::future::join_all(futures).await;

        Ok(())
    }

    /// Perform health check on a single worker
    ///
    /// # Arguments
    ///
    /// * `worker_id` - ID of the worker to check
    ///
    /// # Returns
    ///
    /// `true` if worker is healthy after check, `false` otherwise
    pub async fn health_check_worker(&self, worker_id: &str) -> Result<bool> {
        let worker = self
            .workers
            .iter()
            .find(|w| w.id == worker_id)
            .ok_or_else(|| Error::Validation(format!("Worker not found: {}", worker_id)))?;

        let client = reqwest::Client::builder()
            .timeout(self.health_check_timeout)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .map_err(|e| {
                Error::InternalExecutionError(format!("Failed to create HTTP client: {}", e))
            })?;

        let health_url = format!(
            "{}{}",
            worker.endpoint.trim_end_matches('/'),
            self.health_check_path
        );

        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => {
                worker.mark_healthy().await;
                Ok(true)
            }
            Ok(_) | Err(_) => {
                worker.mark_unhealthy().await;
                Ok(false)
            }
        }
    }
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WorkerPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerPool")
            .field("worker_count", &self.workers.len())
            .field("health_check_interval", &self.health_check_interval)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    /// Test state for worker tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }

    #[test]
    fn test_worker_health_healthy() {
        let health = WorkerHealth::healthy();

        assert!(health.is_healthy);
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.last_health_check.elapsed().unwrap() < Duration::from_secs(1));
    }

    #[test]
    fn test_worker_health_mark_unhealthy() {
        let mut health = WorkerHealth::healthy();

        health.mark_unhealthy();

        assert!(!health.is_healthy);
        assert_eq!(health.consecutive_failures, 1);
    }

    #[test]
    fn test_worker_health_mark_unhealthy_increments() {
        let mut health = WorkerHealth::healthy();

        health.mark_unhealthy();
        health.mark_unhealthy();
        health.mark_unhealthy();

        assert!(!health.is_healthy);
        assert_eq!(health.consecutive_failures, 3);
    }

    #[test]
    fn test_worker_health_mark_healthy_resets() {
        let mut health = WorkerHealth::healthy();

        // Mark unhealthy multiple times
        health.mark_unhealthy();
        health.mark_unhealthy();
        health.mark_unhealthy();

        assert_eq!(health.consecutive_failures, 3);

        // Mark healthy should reset
        health.mark_healthy();

        assert!(health.is_healthy);
        assert_eq!(health.consecutive_failures, 0);
    }

    #[test]
    fn test_worker_health_is_stale() {
        let mut health = WorkerHealth::healthy();

        // Fresh health check is not stale
        assert!(!health.is_stale(Duration::from_secs(10)));

        // Simulate old health check (by manually setting timestamp to past)
        health.last_health_check = SystemTime::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap();

        // Should be stale with 30 second threshold
        assert!(health.is_stale(Duration::from_secs(30)));

        // Should not be stale with 120 second threshold
        assert!(!health.is_stale(Duration::from_secs(120)));
    }

    #[test]
    fn test_worker_health_clone() {
        let health = WorkerHealth::healthy();
        let cloned = health.clone();

        assert_eq!(cloned.is_healthy, health.is_healthy);
        assert_eq!(cloned.consecutive_failures, health.consecutive_failures);
    }

    #[test]
    fn test_remote_worker_new() {
        let worker =
            RemoteWorker::new("worker-1".to_string(), "http://localhost:50051".to_string());

        assert_eq!(worker.id, "worker-1");
        assert_eq!(worker.endpoint, "http://localhost:50051");
        assert_eq!(worker.load(), 0);
    }

    #[tokio::test]
    async fn test_remote_worker_is_healthy() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());

        // New worker should be healthy
        assert!(worker.is_healthy().await);
    }

    #[tokio::test]
    async fn test_remote_worker_mark_unhealthy() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());

        worker.mark_unhealthy().await;

        assert!(!worker.is_healthy().await);
    }

    #[tokio::test]
    async fn test_remote_worker_mark_healthy() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());

        // Mark unhealthy first
        worker.mark_unhealthy().await;
        assert!(!worker.is_healthy().await);

        // Mark healthy should restore
        worker.mark_healthy().await;
        assert!(worker.is_healthy().await);
    }

    #[test]
    fn test_remote_worker_load() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());

        // Initial load should be 0
        assert_eq!(worker.load(), 0);

        // Increment load
        worker.increment_load();
        assert_eq!(worker.load(), 1);

        worker.increment_load();
        assert_eq!(worker.load(), 2);

        // Decrement load
        worker.decrement_load();
        assert_eq!(worker.load(), 1);
    }

    #[tokio::test]
    async fn test_remote_worker_execute_batch_returns_error() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());

        let tasks = vec![Task::new("test_node".to_string(), TestState { value: 42 })];

        // Execute batch should return error (remote execution not yet implemented)
        let result = worker.execute_batch::<TestState>(tasks).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Remote execution not yet integrated"));
    }

    #[tokio::test]
    async fn test_remote_worker_execute_batch_marks_unhealthy_on_error() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());

        // Worker starts healthy
        assert!(worker.is_healthy().await);

        let tasks = vec![Task::new("test_node".to_string(), TestState { value: 42 })];

        // Execute batch returns error
        let _ = worker.execute_batch::<TestState>(tasks).await;

        // Worker should be marked unhealthy
        assert!(!worker.is_healthy().await);
    }

    #[test]
    fn test_remote_worker_debug_format() {
        let worker = RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string());
        let debug_str = format!("{:?}", worker);

        assert!(debug_str.contains("RemoteWorker"));
        assert!(debug_str.contains("worker-1"));
        assert!(debug_str.contains("localhost:50051"));
    }

    #[test]
    fn test_worker_pool_new() {
        let pool = WorkerPool::new();

        assert!(!pool.has_workers());
        assert_eq!(pool.workers().len(), 0);
    }

    #[test]
    fn test_worker_pool_with_workers() {
        let workers = vec![
            RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string()),
            RemoteWorker::new("worker-2".to_string(), "localhost:50052".to_string()),
        ];

        let pool = WorkerPool::with_workers(workers);

        assert!(pool.has_workers());
        assert_eq!(pool.workers().len(), 2);
    }

    #[test]
    fn test_worker_pool_has_workers() {
        let mut pool = WorkerPool::new();
        assert!(!pool.has_workers());

        pool.workers.push(Arc::new(RemoteWorker::new(
            "worker-1".to_string(),
            "localhost:50051".to_string(),
        )));

        assert!(pool.has_workers());
    }

    #[tokio::test]
    async fn test_worker_pool_available_workers() {
        let workers = vec![
            RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string()),
            RemoteWorker::new("worker-2".to_string(), "localhost:50052".to_string()),
            RemoteWorker::new("worker-3".to_string(), "localhost:50053".to_string()),
        ];

        let pool = WorkerPool::with_workers(workers);

        // All workers start healthy
        let available = pool.available_workers().await;
        assert_eq!(available.len(), 3);

        // Mark one worker unhealthy
        pool.workers[1].mark_unhealthy().await;

        // Should now have 2 available workers
        let available = pool.available_workers().await;
        assert_eq!(available.len(), 2);
    }

    #[tokio::test]
    async fn test_worker_pool_available_workers_all_unhealthy() {
        let workers = vec![
            RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string()),
            RemoteWorker::new("worker-2".to_string(), "localhost:50052".to_string()),
        ];

        let pool = WorkerPool::with_workers(workers);

        // Mark all workers unhealthy
        for worker in pool.workers() {
            worker.mark_unhealthy().await;
        }

        // Should have no available workers
        let available = pool.available_workers().await;
        assert_eq!(available.len(), 0);
    }

    #[tokio::test]
    async fn test_worker_pool_health_check_all() {
        let workers = vec![RemoteWorker::new(
            "worker-1".to_string(),
            "localhost:50051".to_string(),
        )];

        let pool = WorkerPool::with_workers(workers);

        // Health check is currently a no-op placeholder
        // Users should implement their own health checks using RemoteNodeClient::check_health()
        let result = pool.health_check_all().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_worker_pool_default() {
        let pool = WorkerPool::default();

        assert!(!pool.has_workers());
        assert_eq!(pool.workers().len(), 0);
    }

    #[test]
    fn test_worker_pool_debug_format() {
        let workers = vec![
            RemoteWorker::new("worker-1".to_string(), "localhost:50051".to_string()),
            RemoteWorker::new("worker-2".to_string(), "localhost:50052".to_string()),
        ];

        let pool = WorkerPool::with_workers(workers);
        let debug_str = format!("{:?}", pool);

        assert!(debug_str.contains("WorkerPool"));
        assert!(debug_str.contains("worker_count"));
        assert!(debug_str.contains("2")); // Should show 2 workers
    }
}
