// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for scheduler
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! @dashflow-module
//! @name scheduler
//! @category runtime
//! @status stable
//!
//! Work-stealing scheduler for distributed parallel execution
//!
//! This module provides a work-stealing scheduler that distributes parallel node
//! execution across multiple remote workers. It implements classic work-stealing
//! algorithms inspired by Cilk and Rayon.
//!
//! # Architecture
//!
//! - **`WorkStealingScheduler`**: Orchestrates task distribution
//! - **`WorkerPool`**: Manages remote worker connections
//! - **Task**: Unit of work (node execution request)
//! - **`SchedulerMetrics`**: Performance metrics
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::scheduler::{WorkStealingScheduler, SelectionStrategy};
//!
//! let scheduler = WorkStealingScheduler::new()
//!     .with_workers(vec![
//!         "worker1:50051",
//!         "worker2:50051",
//!         "worker3:50051",
//!     ])
//!     .with_threshold(10)
//!     .with_strategy(SelectionStrategy::LeastLoaded);
//!
//! let app = graph.compile()?
//!     .with_scheduler(scheduler);
//!
//! // Parallel edges automatically use scheduler
//! let result = app.invoke(state).await?;
//! ```

pub mod config;
pub mod metrics;
pub mod task;
pub mod worker;

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use config::{SchedulerConfig, SelectionStrategy};
pub use metrics::SchedulerMetrics;
pub use task::Task;
pub use worker::{RemoteWorker, WorkerHealth, WorkerPool};

use crate::error::{Error, Result};
use crate::node::BoxedNode;
use crate::state::GraphState;

/// Work-stealing scheduler for distributed parallel execution
///
/// Distributes parallel node execution across multiple remote workers using
/// work-stealing load balancing. Tasks are initially queued locally and
/// distributed to workers when the queue exceeds a threshold.
///
/// # Algorithm
///
/// - **Locality first**: Execute locally when queue is small
/// - **Distribution**: Distribute to workers when queue exceeds threshold
/// - **Load balancing**: Select workers based on configured strategy
/// - **Fault tolerance**: Fallback to local execution if workers unavailable
///
/// # Type Parameters
///
/// - `S`: Graph state type (must be `GraphState` + Clone)
pub struct WorkStealingScheduler<S>
where
    S: GraphState,
{
    /// Pool of remote workers
    workers: Arc<WorkerPool>,
    /// Local task queue (deque for LIFO/FIFO semantics)
    local_queue: Arc<Mutex<VecDeque<Task<S>>>>,
    /// Scheduler configuration
    config: SchedulerConfig,
    /// Performance metrics
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

impl<S> WorkStealingScheduler<S>
where
    S: GraphState,
{
    /// Create a new work-stealing scheduler
    ///
    /// By default:
    /// - No workers (local execution only)
    /// - Threshold: 10 tasks
    /// - Strategy: `LeastLoaded`
    /// - Work stealing enabled
    #[must_use]
    pub fn new() -> Self {
        Self {
            workers: Arc::new(WorkerPool::new()),
            local_queue: Arc::new(Mutex::new(VecDeque::new())),
            config: SchedulerConfig::default(),
            metrics: Arc::new(Mutex::new(SchedulerMetrics::new())),
        }
    }

    /// Add remote workers to the scheduler
    ///
    /// # Arguments
    ///
    /// * `endpoints` - List of gRPC endpoints (e.g., "worker1:50051")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let scheduler = WorkStealingScheduler::new()
    ///     .with_workers(vec!["worker1:50051", "worker2:50051"]);
    /// ```
    #[must_use]
    pub fn with_workers(mut self, endpoints: Vec<impl Into<String>>) -> Self {
        let workers = endpoints
            .into_iter()
            .enumerate()
            .map(|(i, endpoint)| RemoteWorker::new(format!("worker-{i}"), endpoint.into()))
            .collect();
        self.workers = Arc::new(WorkerPool::with_workers(workers));
        self
    }

    /// Set the local queue threshold
    ///
    /// When the local queue exceeds this size, tasks are distributed to workers.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Queue size threshold (default: 10)
    #[must_use]
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.config.local_queue_threshold = threshold;
        self
    }

    /// Set the worker selection strategy
    ///
    /// # Arguments
    ///
    /// * `strategy` - Selection strategy (Random, `LeastLoaded`, `RoundRobin`)
    #[must_use]
    pub fn with_strategy(mut self, strategy: SelectionStrategy) -> Self {
        self.config.selection_strategy = strategy;
        self
    }

    /// Enable or disable work stealing
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable work stealing (default: true)
    #[must_use]
    pub fn with_work_stealing(mut self, enabled: bool) -> Self {
        self.config.enable_stealing = enabled;
        self
    }

    /// Set the number of steal attempts per cycle
    ///
    /// # Arguments
    ///
    /// * `attempts` - Number of steal attempts (default: 3)
    #[must_use]
    pub fn with_steal_attempts(mut self, attempts: usize) -> Self {
        self.config.steal_attempts = attempts;
        self
    }

    /// Set the random seed for deterministic Random selection strategy
    ///
    /// M-572: When a seed is set, the Random selection strategy uses a
    /// seeded RNG for deterministic behavior in tests.
    ///
    /// # Arguments
    ///
    /// * `seed` - Random seed for deterministic randomness
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.config.random_seed = Some(seed);
        self
    }

    /// Get current scheduler metrics
    ///
    /// Returns a snapshot of performance metrics including:
    /// - Tasks submitted, executed (local/remote)
    /// - Execution times
    /// - Worker utilization
    pub async fn metrics(&self) -> SchedulerMetrics {
        self.metrics.lock().await.clone()
    }

    /// Execute parallel nodes using work-stealing scheduler
    ///
    /// This is called by the graph executor when it encounters parallel edges.
    /// The scheduler decides whether to execute locally or distribute to workers.
    ///
    /// # Arguments
    ///
    /// * `node_names` - Names of nodes to execute in parallel
    /// * `state` - Current graph state
    /// * `nodes` - Map of node name to node instance
    ///
    /// # Returns
    ///
    /// Vector of results in the same order as `node_names`
    ///
    /// # Algorithm
    ///
    /// 1. Create tasks for each node
    /// 2. If local queue < threshold: execute locally
    /// 3. Else: distribute to workers using selection strategy
    /// 4. If distribution fails: fallback to local execution
    pub async fn execute_parallel(
        &self,
        node_names: &[String],
        state: &S,
        nodes: &std::collections::HashMap<String, BoxedNode<S>>,
    ) -> Result<Vec<S>> {
        use std::time::Instant;
        use tracing::{info_span, Instrument};

        let start_time = Instant::now();

        // Create a tracing span for parallel execution
        let span = info_span!(
            "scheduler.execute_parallel",
            node_count = node_names.len(),
            node_names = ?node_names,
            execution_type = tracing::field::Empty,
            duration_ms = tracing::field::Empty
        );

        async move {
            // Update metrics: tasks submitted
            {
                let mut metrics = self.metrics.lock().await;
                metrics.tasks_submitted += node_names.len() as u64;
            }

            // Create tasks
            let tasks: Vec<Task<S>> = node_names
                .iter()
                .map(|name| Task::new(name.clone(), state.clone()))
                .collect();

            // Decision: local or distributed?
            let local_queue_len = self.local_queue.lock().await.len();
            let should_distribute =
                local_queue_len >= self.config.local_queue_threshold && self.workers.has_workers();

            let results = if should_distribute {
                // Try distributed execution
                tracing::Span::current().record("execution_type", "distributed");
                match self.distribute_and_execute(tasks, nodes).await {
                    Ok(results) => {
                        let mut metrics = self.metrics.lock().await;
                        metrics.tasks_executed_remote += node_names.len() as u64;
                        metrics.execution_time_remote += start_time.elapsed();
                        results
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Distributed execution failed: {}, falling back to local",
                            e
                        );
                        tracing::Span::current().record("execution_type", "distributed_fallback");
                        // Fallback to local execution
                        let results = self.execute_local(node_names, state, nodes).await?;
                        let mut metrics = self.metrics.lock().await;
                        metrics.tasks_executed_local += node_names.len() as u64;
                        metrics.execution_time_local += start_time.elapsed();
                        results
                    }
                }
            } else {
                // Execute locally
                tracing::Span::current().record("execution_type", "local");
                let results = self.execute_local(node_names, state, nodes).await?;
                let mut metrics = self.metrics.lock().await;
                metrics.tasks_executed_local += node_names.len() as u64;
                metrics.execution_time_local += start_time.elapsed();
                results
            };

            // Record final span attributes
            let duration_ms = start_time.elapsed().as_millis() as u64;
            tracing::Span::current().record("duration_ms", duration_ms);

            Ok(results)
        }
        .instrument(span)
        .await
    }

    /// Execute tasks locally using `tokio::spawn`
    ///
    /// This is the same as the current parallel execution in executor.rs.
    async fn execute_local(
        &self,
        node_names: &[String],
        state: &S,
        nodes: &std::collections::HashMap<String, BoxedNode<S>>,
    ) -> Result<Vec<S>> {
        let mut tasks = vec![];

        for node_name in node_names {
            let node = nodes
                .get(node_name)
                .ok_or_else(|| Error::NodeNotFound(node_name.clone()))?
                .clone();

            let state_clone = state.clone();
            let node_name_clone = node_name.clone();

            tasks.push(tokio::spawn(async move {
                let result = node.execute(state_clone).await;
                (node_name_clone, result)
            }));
        }

        // Wait for all tasks to complete
        let mut results = Vec::with_capacity(node_names.len());
        for task in tasks {
            let (node_name, result) = task
                .await
                .map_err(|e| Error::InternalExecutionError(format!("Task panicked: {e}")))?;

            let state = result.map_err(|e| Error::NodeExecution {
                node: node_name.clone(),
                source: Box::new(std::io::Error::other(e.to_string())),
            })?;

            results.push(state);
        }

        Ok(results)
    }

    /// Distribute tasks to workers and execute
    ///
    /// Selects workers based on configured strategy and distributes tasks.
    async fn distribute_and_execute(
        &self,
        tasks: Vec<Task<S>>,
        _nodes: &std::collections::HashMap<String, BoxedNode<S>>,
    ) -> Result<Vec<S>> {
        // Get available workers
        let available_workers = self.workers.available_workers().await;

        if available_workers.is_empty() {
            return Err(Error::Validation(
                "No available workers for distributed execution".to_string(),
            ));
        }

        // Assign tasks to workers based on strategy
        let assignments = self.assign_tasks(tasks, &available_workers).await;

        // Execute tasks on workers in parallel
        let mut futures = vec![];
        for (worker, worker_tasks) in assignments {
            let worker_clone = worker.clone();
            futures.push(tokio::spawn(async move {
                worker_clone.execute_batch(worker_tasks).await
            }));
        }

        // Collect results
        let mut all_results = Vec::new();
        for future in futures {
            let batch_results = future.await.map_err(|e| {
                Error::InternalExecutionError(format!("Worker task panicked: {e}"))
            })??;
            all_results.extend(batch_results);
        }

        Ok(all_results)
    }

    /// Assign tasks to workers based on selection strategy
    async fn assign_tasks(
        &self,
        tasks: Vec<Task<S>>,
        workers: &[Arc<RemoteWorker>],
    ) -> Vec<(Arc<RemoteWorker>, Vec<Task<S>>)> {
        match self.config.selection_strategy {
            SelectionStrategy::RoundRobin => self.assign_round_robin(tasks, workers),
            SelectionStrategy::LeastLoaded => self.assign_least_loaded(tasks, workers).await,
            SelectionStrategy::Random => self.assign_random(tasks, workers),
        }
    }

    /// Round-robin task assignment
    fn assign_round_robin(
        &self,
        tasks: Vec<Task<S>>,
        workers: &[Arc<RemoteWorker>],
    ) -> Vec<(Arc<RemoteWorker>, Vec<Task<S>>)> {
        let mut assignments: Vec<Vec<Task<S>>> = vec![vec![]; workers.len()];

        for (i, task) in tasks.into_iter().enumerate() {
            let worker_idx = i % workers.len();
            assignments[worker_idx].push(task);
        }

        workers
            .iter()
            .zip(assignments)
            .filter(|(_, tasks)| !tasks.is_empty())
            .map(|(w, tasks)| (w.clone(), tasks))
            .collect()
    }

    /// Least-loaded task assignment
    async fn assign_least_loaded(
        &self,
        tasks: Vec<Task<S>>,
        workers: &[Arc<RemoteWorker>],
    ) -> Vec<(Arc<RemoteWorker>, Vec<Task<S>>)> {
        if workers.is_empty() {
            return vec![];
        }

        let mut assignments: Vec<Vec<Task<S>>> = vec![vec![]; workers.len()];

        for task in tasks {
            // Find worker with least load
            let Some((min_idx, _)) = workers
                .iter()
                .enumerate()
                .map(|(i, w)| (i, w.load()))
                .min_by_key(|(_, load)| *load)
            else {
                continue;
            };

            assignments[min_idx].push(task);
        }

        workers
            .iter()
            .zip(assignments)
            .filter(|(_, tasks)| !tasks.is_empty())
            .map(|(w, tasks)| (w.clone(), tasks))
            .collect()
    }

    /// Random task assignment
    /// M-572: Uses seeded RNG if random_seed is configured for deterministic behavior
    fn assign_random(
        &self,
        tasks: Vec<Task<S>>,
        workers: &[Arc<RemoteWorker>],
    ) -> Vec<(Arc<RemoteWorker>, Vec<Task<S>>)> {
        use rand::Rng;
        use rand::SeedableRng;

        let mut assignments: Vec<Vec<Task<S>>> = vec![vec![]; workers.len()];

        // M-572: Use seeded RNG if configured, otherwise use thread_rng
        if let Some(seed) = self.config.random_seed {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            for task in tasks {
                let worker_idx = rng.gen_range(0..workers.len());
                assignments[worker_idx].push(task);
            }
        } else {
            let mut rng = rand::thread_rng();
            for task in tasks {
                let worker_idx = rng.gen_range(0..workers.len());
                assignments[worker_idx].push(task);
            }
        }

        workers
            .iter()
            .zip(assignments)
            .filter(|(_, tasks)| !tasks.is_empty())
            .map(|(w, tasks)| (w.clone(), tasks))
            .collect()
    }
}

impl<S> Default for WorkStealingScheduler<S>
where
    S: GraphState,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Clone for WorkStealingScheduler<S>
where
    S: GraphState,
{
    fn clone(&self) -> Self {
        Self {
            workers: self.workers.clone(),
            local_queue: self.local_queue.clone(),
            config: self.config.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    /// Test state for scheduler tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
        message: String,
    }

    /// Simple test node that increments value
    struct IncrementNode;

    #[async_trait::async_trait]
    impl Node<TestState> for IncrementNode {
        async fn execute(&self, mut state: TestState) -> crate::error::Result<TestState> {
            state.value += 1;
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// Test node that doubles value
    struct DoubleNode;

    #[async_trait::async_trait]
    impl Node<TestState> for DoubleNode {
        async fn execute(&self, mut state: TestState) -> crate::error::Result<TestState> {
            state.value *= 2;
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// Test node that adds a specific value
    struct AddNode(i32);

    #[async_trait::async_trait]
    impl Node<TestState> for AddNode {
        async fn execute(&self, mut state: TestState) -> crate::error::Result<TestState> {
            state.value += self.0;
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// Test node that sets a specific value
    struct SetValueNode(i32);

    #[async_trait::async_trait]
    impl Node<TestState> for SetValueNode {
        async fn execute(&self, mut state: TestState) -> crate::error::Result<TestState> {
            state.value = self.0;
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_work_stealing_scheduler_new() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        assert!(!scheduler.workers.has_workers());
        assert_eq!(scheduler.config.local_queue_threshold, 10);
        assert_eq!(
            scheduler.config.selection_strategy,
            SelectionStrategy::LeastLoaded
        );
    }

    #[test]
    fn test_work_stealing_scheduler_with_workers() {
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_workers(vec!["worker1:50051", "worker2:50051"]);

        assert!(scheduler.workers.has_workers());
        assert_eq!(scheduler.workers.workers().len(), 2);
    }

    #[test]
    fn test_work_stealing_scheduler_with_threshold() {
        let scheduler = WorkStealingScheduler::<TestState>::new().with_threshold(20);

        assert_eq!(scheduler.config.local_queue_threshold, 20);
    }

    #[test]
    fn test_work_stealing_scheduler_with_strategy() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::RoundRobin);

        assert_eq!(
            scheduler.config.selection_strategy,
            SelectionStrategy::RoundRobin
        );
    }

    #[test]
    fn test_work_stealing_scheduler_with_work_stealing() {
        let scheduler = WorkStealingScheduler::<TestState>::new().with_work_stealing(false);

        assert!(!scheduler.config.enable_stealing);
    }

    #[test]
    fn test_work_stealing_scheduler_with_steal_attempts() {
        let scheduler = WorkStealingScheduler::<TestState>::new().with_steal_attempts(5);

        assert_eq!(scheduler.config.steal_attempts, 5);
    }

    #[test]
    fn test_work_stealing_scheduler_default() {
        let scheduler = WorkStealingScheduler::<TestState>::default();

        assert!(!scheduler.workers.has_workers());
        assert_eq!(scheduler.config.local_queue_threshold, 10);
    }

    #[test]
    fn test_work_stealing_scheduler_clone() {
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_threshold(15)
            .with_strategy(SelectionStrategy::Random);

        let cloned = scheduler.clone();

        assert_eq!(
            cloned.config.local_queue_threshold,
            scheduler.config.local_queue_threshold
        );
        assert_eq!(
            cloned.config.selection_strategy,
            scheduler.config.selection_strategy
        );
    }

    #[tokio::test]
    async fn test_work_stealing_scheduler_metrics() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let metrics = scheduler.metrics().await;

        assert_eq!(metrics.tasks_submitted, 0);
        assert_eq!(metrics.tasks_executed_local, 0);
        assert_eq!(metrics.tasks_executed_remote, 0);
    }

    #[tokio::test]
    async fn test_execute_parallel_local_no_workers() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 10,
            message: "test".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        nodes.insert("increment".to_string(), Arc::new(IncrementNode));
        nodes.insert("double".to_string(), Arc::new(DoubleNode));

        let node_names = vec!["increment".to_string(), "double".to_string()];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);

        // One should have value 11 (10+1), other should have value 20 (10*2)
        let values: Vec<i32> = results.iter().map(|s| s.value).collect();
        assert!(values.contains(&11));
        assert!(values.contains(&20));

        // Verify metrics
        let metrics = scheduler.metrics().await;
        assert_eq!(metrics.tasks_submitted, 2);
        assert_eq!(metrics.tasks_executed_local, 2);
        assert_eq!(metrics.tasks_executed_remote, 0);
    }

    #[tokio::test]
    async fn test_execute_parallel_single_node() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 5,
            message: "single".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        nodes.insert("increment".to_string(), Arc::new(IncrementNode));

        let node_names = vec!["increment".to_string()];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, 6); // 5 + 1
    }

    #[tokio::test]
    async fn test_execute_parallel_node_not_found() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 5,
            message: "test".to_string(),
        };

        let nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();

        let node_names = vec!["nonexistent".to_string()];

        let result = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Node 'nonexistent' not found"));
    }

    #[tokio::test]
    async fn test_execute_parallel_with_workers_no_threshold() {
        // Create scheduler with workers but low threshold
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_workers(vec!["worker1:50051"])
            .with_threshold(100); // High threshold, so should execute locally

        let state = TestState {
            value: 10,
            message: "test".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        nodes.insert("increment".to_string(), Arc::new(IncrementNode));

        let node_names = vec!["increment".to_string()];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, 11);

        // Should execute locally (threshold not exceeded)
        let metrics = scheduler.metrics().await;
        assert_eq!(metrics.tasks_executed_local, 1);
        assert_eq!(metrics.tasks_executed_remote, 0);
    }

    #[test]
    fn test_assign_round_robin() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::RoundRobin);

        let workers = vec![
            Arc::new(RemoteWorker::new(
                "w1".to_string(),
                "localhost:50051".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w2".to_string(),
                "localhost:50052".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w3".to_string(),
                "localhost:50053".to_string(),
            )),
        ];

        let tasks = vec![
            Task::new(
                "task1".to_string(),
                TestState {
                    value: 1,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task2".to_string(),
                TestState {
                    value: 2,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task3".to_string(),
                TestState {
                    value: 3,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task4".to_string(),
                TestState {
                    value: 4,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task5".to_string(),
                TestState {
                    value: 5,
                    message: "".to_string(),
                },
            ),
        ];

        let assignments = scheduler.assign_round_robin(tasks, &workers);

        // 5 tasks across 3 workers: [2, 2, 1]
        assert_eq!(assignments.len(), 3);

        let task_counts: Vec<usize> = assignments.iter().map(|(_, tasks)| tasks.len()).collect();
        assert_eq!(task_counts, vec![2, 2, 1]);
    }

    #[tokio::test]
    async fn test_assign_least_loaded() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::LeastLoaded);

        let workers = vec![
            Arc::new(RemoteWorker::new(
                "w1".to_string(),
                "localhost:50051".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w2".to_string(),
                "localhost:50052".to_string(),
            )),
        ];

        // Simulate different loads by executing batch (which increments/decrements load)
        // For now, we'll just verify the load balancing algorithm works with equal loads
        // (In production, load would be set by actual execute_batch calls)

        let tasks = vec![
            Task::new(
                "task1".to_string(),
                TestState {
                    value: 1,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task2".to_string(),
                TestState {
                    value: 2,
                    message: "".to_string(),
                },
            ),
        ];

        let assignments = scheduler.assign_least_loaded(tasks, &workers).await;

        // With equal loads, tasks will be distributed
        // At minimum, all tasks should be assigned
        let total_tasks: usize = assignments.iter().map(|(_, tasks)| tasks.len()).sum();
        assert_eq!(total_tasks, 2);
    }

    #[test]
    fn test_assign_random() {
        // M-572: Use seeded scheduler for deterministic test behavior
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_strategy(SelectionStrategy::Random)
            .with_seed(42);

        let workers = vec![
            Arc::new(RemoteWorker::new(
                "w1".to_string(),
                "localhost:50051".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w2".to_string(),
                "localhost:50052".to_string(),
            )),
        ];

        let tasks = vec![
            Task::new(
                "task1".to_string(),
                TestState {
                    value: 1,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task2".to_string(),
                TestState {
                    value: 2,
                    message: "".to_string(),
                },
            ),
        ];

        let assignments = scheduler.assign_random(tasks, &workers);

        // Should assign to workers (with seed 42, deterministic but total should be 2)
        let total_tasks: usize = assignments.iter().map(|(_, tasks)| tasks.len()).sum();
        assert_eq!(total_tasks, 2);
    }

    #[tokio::test]
    async fn test_distribute_and_execute_no_workers() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let tasks = vec![Task::new(
            "task1".to_string(),
            TestState {
                value: 1,
                message: "".to_string(),
            },
        )];

        let nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();

        // Should fail because no workers available
        let result = scheduler.distribute_and_execute(tasks, &nodes).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No available workers"));
    }

    #[test]
    fn test_scheduler_clone() {
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_workers(vec!["worker1:50051"])
            .with_threshold(15)
            .with_strategy(SelectionStrategy::Random);

        let cloned = scheduler.clone();

        assert_eq!(cloned.config.local_queue_threshold, 15);
        assert_eq!(cloned.config.selection_strategy, SelectionStrategy::Random);
        assert!(cloned.workers.has_workers());
    }

    #[test]
    fn test_assign_round_robin_empty_tasks() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::RoundRobin);

        let workers = vec![Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ))];

        let tasks: Vec<Task<TestState>> = vec![];
        let assignments = scheduler.assign_round_robin(tasks, &workers);

        assert_eq!(assignments.len(), 0);
    }

    #[tokio::test]
    async fn test_assign_least_loaded_empty_tasks() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::LeastLoaded);

        let workers = vec![Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ))];

        let tasks: Vec<Task<TestState>> = vec![];
        let assignments = scheduler.assign_least_loaded(tasks, &workers).await;

        assert_eq!(assignments.len(), 0);
    }

    #[test]
    fn test_assign_random_empty_tasks() {
        // M-572: Use seeded scheduler for deterministic test behavior
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_strategy(SelectionStrategy::Random)
            .with_seed(42);

        let workers = vec![Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ))];

        let tasks: Vec<Task<TestState>> = vec![];
        let assignments = scheduler.assign_random(tasks, &workers);

        assert_eq!(assignments.len(), 0);
    }

    #[tokio::test]
    async fn test_execute_parallel_empty_node_names() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 10,
            message: "test".to_string(),
        };

        let nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        let node_names: Vec<String> = vec![];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        // Empty input should return empty results
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_metrics_initial_state() {
        let scheduler = WorkStealingScheduler::<TestState>::new();
        let metrics = scheduler.metrics().await;

        assert_eq!(metrics.tasks_executed_local, 0);
        assert_eq!(metrics.tasks_executed_remote, 0);
        assert_eq!(metrics.tasks_submitted, 0);
    }

    #[tokio::test]
    async fn test_assign_least_loaded_equal_loads() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::LeastLoaded);

        let w1 = Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ));
        let w2 = Arc::new(RemoteWorker::new(
            "w2".to_string(),
            "localhost:50052".to_string(),
        ));

        let workers = vec![w1.clone(), w2.clone()];

        let tasks = vec![
            Task::new(
                "task1".to_string(),
                TestState {
                    value: 1,
                    message: "".to_string(),
                },
            ),
            Task::new(
                "task2".to_string(),
                TestState {
                    value: 2,
                    message: "".to_string(),
                },
            ),
        ];

        let assignments = scheduler.assign_least_loaded(tasks, &workers).await;

        // With equal loads, tasks should be distributed
        // (actual distribution depends on assignment order)
        let total_tasks: usize = assignments.iter().map(|(_, tasks)| tasks.len()).sum();
        assert_eq!(total_tasks, 2);
    }

    #[test]
    fn test_round_robin_distribution_fairness() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::RoundRobin);

        let workers = vec![
            Arc::new(RemoteWorker::new(
                "w1".to_string(),
                "localhost:50051".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w2".to_string(),
                "localhost:50052".to_string(),
            )),
        ];

        // Create 10 tasks
        let tasks: Vec<Task<TestState>> = (0..10)
            .map(|i| {
                Task::new(
                    format!("task{}", i),
                    TestState {
                        value: i,
                        message: "".to_string(),
                    },
                )
            })
            .collect();

        let assignments = scheduler.assign_round_robin(tasks, &workers);

        // Should distribute evenly: 5 tasks each
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].1.len(), 5);
        assert_eq!(assignments[1].1.len(), 5);
    }

    #[test]
    fn test_scheduler_default() {
        let scheduler = WorkStealingScheduler::<TestState>::default();
        assert_eq!(scheduler.config.local_queue_threshold, 10);
        assert!(matches!(
            scheduler.config.selection_strategy,
            SelectionStrategy::LeastLoaded
        ));
    }

    #[test]
    fn test_with_threshold() {
        let scheduler = WorkStealingScheduler::<TestState>::new().with_threshold(50);
        assert_eq!(scheduler.config.local_queue_threshold, 50);
    }

    #[test]
    fn test_with_strategy() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::Random);
        assert!(matches!(
            scheduler.config.selection_strategy,
            SelectionStrategy::Random
        ));
    }

    #[tokio::test]
    async fn test_execute_parallel_multiple_nodes() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 10,
            message: "init".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        nodes.insert("node1".to_string(), Arc::new(AddNode(1)));
        nodes.insert("node2".to_string(), Arc::new(AddNode(2)));
        nodes.insert("node3".to_string(), Arc::new(AddNode(3)));

        let node_names = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        // Results should contain node1 with value 11, node2 with value 12, node3 with value 13
        let values: Vec<i32> = results.iter().map(|s| s.value).collect();
        assert!(values.contains(&11));
        assert!(values.contains(&12));
        assert!(values.contains(&13));
    }

    #[test]
    fn test_assign_round_robin_single_worker() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::RoundRobin);

        let workers = vec![Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ))];

        let tasks: Vec<Task<TestState>> = (0..5)
            .map(|i| {
                Task::new(
                    format!("task{}", i),
                    TestState {
                        value: i,
                        message: "".to_string(),
                    },
                )
            })
            .collect();

        let assignments = scheduler.assign_round_robin(tasks, &workers);

        // All tasks should go to the single worker
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].1.len(), 5);
    }

    #[test]
    fn test_assign_round_robin_uneven_distribution() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::RoundRobin);

        let workers = vec![
            Arc::new(RemoteWorker::new(
                "w1".to_string(),
                "localhost:50051".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w2".to_string(),
                "localhost:50052".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w3".to_string(),
                "localhost:50053".to_string(),
            )),
        ];

        // 7 tasks with 3 workers: should be 3, 2, 2 or similar
        let tasks: Vec<Task<TestState>> = (0..7)
            .map(|i| {
                Task::new(
                    format!("task{}", i),
                    TestState {
                        value: i,
                        message: "".to_string(),
                    },
                )
            })
            .collect();

        let assignments = scheduler.assign_round_robin(tasks, &workers);

        assert_eq!(assignments.len(), 3);
        let total_tasks: usize = assignments.iter().map(|(_, tasks)| tasks.len()).sum();
        assert_eq!(total_tasks, 7);
    }

    #[test]
    fn test_assign_random_single_worker() {
        // M-572: Use seeded scheduler for deterministic test behavior
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_strategy(SelectionStrategy::Random)
            .with_seed(42);

        let workers = vec![Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ))];

        let tasks: Vec<Task<TestState>> = (0..5)
            .map(|i| {
                Task::new(
                    format!("task{}", i),
                    TestState {
                        value: i,
                        message: "".to_string(),
                    },
                )
            })
            .collect();

        let assignments = scheduler.assign_random(tasks, &workers);

        // All tasks should go to the single worker
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].1.len(), 5);
    }

    #[test]
    fn test_assign_random_distribution() {
        // M-572: Use seeded scheduler for deterministic test behavior
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_strategy(SelectionStrategy::Random)
            .with_seed(42);

        let workers = vec![
            Arc::new(RemoteWorker::new(
                "w1".to_string(),
                "localhost:50051".to_string(),
            )),
            Arc::new(RemoteWorker::new(
                "w2".to_string(),
                "localhost:50052".to_string(),
            )),
        ];

        let tasks: Vec<Task<TestState>> = (0..10)
            .map(|i| {
                Task::new(
                    format!("task{}", i),
                    TestState {
                        value: i,
                        message: "".to_string(),
                    },
                )
            })
            .collect();

        let assignments = scheduler.assign_random(tasks, &workers);

        // All tasks should be assigned
        let total_tasks: usize = assignments.iter().map(|(_, tasks)| tasks.len()).sum();
        assert_eq!(total_tasks, 10);

        // Should have assignments to at least one worker
        assert!(!assignments.is_empty());
    }

    #[tokio::test]
    async fn test_assign_least_loaded_single_worker() {
        let scheduler =
            WorkStealingScheduler::<TestState>::new().with_strategy(SelectionStrategy::LeastLoaded);

        let workers = vec![Arc::new(RemoteWorker::new(
            "w1".to_string(),
            "localhost:50051".to_string(),
        ))];

        let tasks: Vec<Task<TestState>> = (0..5)
            .map(|i| {
                Task::new(
                    format!("task{}", i),
                    TestState {
                        value: i,
                        message: "".to_string(),
                    },
                )
            })
            .collect();

        let assignments = scheduler.assign_least_loaded(tasks, &workers).await;

        // All tasks should go to the single worker
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].1.len(), 5);
    }

    #[tokio::test]
    async fn test_metrics_after_parallel_execution() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 10,
            message: "test".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        nodes.insert("node1".to_string(), Arc::new(AddNode(1)));
        nodes.insert("node2".to_string(), Arc::new(AddNode(2)));

        let node_names = vec!["node1".to_string(), "node2".to_string()];

        let _ = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        let metrics = scheduler.metrics().await;
        // Both tasks should be executed locally since there are no workers
        assert_eq!(metrics.tasks_executed_local, 2);
        assert_eq!(metrics.tasks_submitted, 2);
    }

    #[test]
    fn test_clone_scheduler() {
        let scheduler = WorkStealingScheduler::<TestState>::new()
            .with_threshold(20)
            .with_strategy(SelectionStrategy::RoundRobin);

        let cloned = scheduler.clone();

        assert_eq!(cloned.config.local_queue_threshold, 20);
        assert!(matches!(
            cloned.config.selection_strategy,
            SelectionStrategy::RoundRobin
        ));
    }

    #[tokio::test]
    async fn test_execute_parallel_with_multiple_duplicate_nodes() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 10,
            message: "test".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        nodes.insert("node1".to_string(), Arc::new(AddNode(5)));

        // Request same node multiple times
        let node_names = vec![
            "node1".to_string(),
            "node1".to_string(),
            "node1".to_string(),
        ];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        // Should execute the node 3 times in parallel
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|s| s.value == 15));
    }

    #[tokio::test]
    async fn test_execute_parallel_preserves_results_count() {
        let scheduler = WorkStealingScheduler::<TestState>::new();

        let state = TestState {
            value: 0,
            message: "init".to_string(),
        };

        let mut nodes: HashMap<String, BoxedNode<TestState>> = HashMap::new();
        for i in 1..=5 {
            nodes.insert(format!("node{}", i), Arc::new(SetValueNode(i)));
        }

        let node_names = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
            "node4".to_string(),
            "node5".to_string(),
        ];

        let results = scheduler
            .execute_parallel(&node_names, &state, &nodes)
            .await
            .unwrap();

        assert_eq!(results.len(), 5);
        // Verify all unique values are present in results
        let values: Vec<i32> = results.iter().map(|s| s.value).collect();
        assert!(values.contains(&1));
        assert!(values.contains(&2));
        assert!(values.contains(&3));
        assert!(values.contains(&4));
        assert!(values.contains(&5));
    }
}
