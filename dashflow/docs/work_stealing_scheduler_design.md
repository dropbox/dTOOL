# Work-Stealing Scheduler Design

## Overview

Implements a work-stealing scheduler for distributing parallel DashFlow node execution across multiple remote workers. Based on classic work-stealing algorithms (Cilk, Rayon).

## Architecture

### Components

1. **WorkStealingScheduler** - Orchestrates task distribution
2. **WorkerPool** - Manages remote worker connections
3. **TaskQueue** - Local task queue with deque semantics
4. **WorkStealingStrategy** - Load balancing heuristics

### Design Principles

- **Locality First**: Try local execution first, distribute only when overloaded
- **Random Victim Selection**: Workers randomly select victims to steal from
- **LIFO for owner, FIFO for thieves**: Owner pops from tail (local), thieves steal from head (oldest tasks)
- **Adaptive Stealing**: Adjust stealing threshold based on queue depth

## Data Structures

```rust
pub struct WorkStealingScheduler {
    /// Pool of remote workers
    workers: Arc<WorkerPool>,
    /// Local task queue (deque)
    local_queue: Arc<Mutex<VecDeque<Task>>>,
    /// Scheduler configuration
    config: SchedulerConfig,
    /// Metrics
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

pub struct WorkerPool {
    /// Remote worker endpoints
    workers: Vec<RemoteWorker>,
    /// Worker health status
    health: Arc<RwLock<HashMap<String, WorkerHealth>>>,
}

pub struct RemoteWorker {
    /// Worker ID
    id: String,
    /// gRPC endpoint
    endpoint: String,
    /// gRPC client
    client: Arc<Mutex<Option<RemoteNodeServiceClient<Channel>>>>,
    /// Current load estimate
    load: Arc<AtomicU32>,
}

pub struct Task {
    /// Node name to execute
    node_name: String,
    /// State to pass to node
    state: Vec<u8>, // serialized
    /// Task priority
    priority: u8,
}

pub struct SchedulerConfig {
    /// Maximum local queue size before distributing
    local_queue_threshold: usize,
    /// Enable work stealing
    enable_stealing: bool,
    /// Steal attempts per cycle
    steal_attempts: usize,
    /// Worker selection strategy
    selection_strategy: SelectionStrategy,
}

pub enum SelectionStrategy {
    /// Random victim selection
    Random,
    /// Least loaded worker
    LeastLoaded,
    /// Round robin
    RoundRobin,
}
```

## Algorithm

### Task Submission (Producer)

```
function submit_parallel_tasks(nodes: Vec<String>, state: State):
    tasks = nodes.map(|node| Task { node, state.clone() })

    if local_queue.len() < threshold:
        // Execute locally
        for task in tasks:
            local_queue.push_back(task)
        return execute_local()
    else:
        // Distribute to workers
        return distribute_tasks(tasks)
```

### Local Execution

```
function execute_local():
    results = Vec::new()

    while let Some(task) = local_queue.pop_back():  // LIFO for owner
        result = execute_task(task).await
        results.push(result)

    return results
```

### Distributed Execution

```
function distribute_tasks(tasks: Vec<Task>):
    // Sort workers by load
    available_workers = workers.filter(|w| w.is_healthy())
                                .sort_by_key(|w| w.load)

    if available_workers.is_empty():
        // Fallback to local execution
        return execute_local()

    // Distribute tasks round-robin
    assignments = assign_tasks(tasks, available_workers)

    // Execute in parallel
    futures = assignments.map(|(worker, tasks)| {
        worker.execute_batch(tasks)
    })

    results = join_all(futures).await
    return flatten(results)
```

### Work Stealing (Idle Worker)

```
function steal_work(idle_worker: RemoteWorker):
    for attempt in 0..steal_attempts:
        // Random victim selection
        victim = workers.choose_random().exclude(idle_worker)

        if let Some(task) = victim.local_queue.pop_front():  // FIFO for thief
            // Execute stolen task
            result = idle_worker.execute_task(task).await
            return Some(result)

    return None  // No work stolen
```

## Integration with ParallelEdge

### Current Execution (Local)

```rust
// In executor.rs
if current_nodes.len() > 1 {
    // Parallel execution
    let mut tasks = vec![];
    for node_name in &current_nodes {
        let node = self.nodes.get(node_name)?;
        tasks.push(tokio::spawn(async move {
            node.execute(state).await
        }));
    }
    let results = join_all(tasks).await;
}
```

### New Execution (With Scheduler)

```rust
// In executor.rs
if current_nodes.len() > 1 {
    // Check if scheduler is configured
    if let Some(scheduler) = &self.scheduler {
        // Use work-stealing scheduler
        let results = scheduler.execute_parallel(
            &current_nodes,
            &state,
            &self.nodes
        ).await?;
    } else {
        // Fallback to local parallel execution
        let mut tasks = vec![];
        for node_name in &current_nodes {
            // ... existing code
        }
    }
}
```

### API

```rust
// Configure scheduler
let scheduler = WorkStealingScheduler::new()
    .with_workers(vec![
        "worker1:50051",
        "worker2:50051",
        "worker3:50051",
    ])
    .with_threshold(10)  // Distribute after 10 queued tasks
    .with_strategy(SelectionStrategy::LeastLoaded);

// Add to compiled graph
let app = graph.compile()?
    .with_scheduler(scheduler);

// Execution automatically uses scheduler for parallel edges
let result = app.invoke(state).await?;
```

## Metrics

Track the following metrics:

- **tasks_submitted**: Total tasks submitted
- **tasks_executed_local**: Tasks executed locally
- **tasks_executed_remote**: Tasks executed remotely
- **tasks_stolen**: Tasks stolen by workers
- **worker_idle_time**: Worker idle time
- **task_distribution_latency**: Time to distribute tasks
- **execution_time_local**: Local execution time
- **execution_time_remote**: Remote execution time

## Testing Strategy

### Unit Tests

1. **test_local_execution**: All tasks execute locally when under threshold
2. **test_distribution**: Tasks distribute when over threshold
3. **test_worker_selection**: Verify selection strategies (random, least loaded, round robin)
4. **test_work_stealing**: Verify stealing mechanism
5. **test_fallback_to_local**: Fallback when no workers available

### Integration Tests

1. **test_multiple_workers**: Real gRPC workers, verify load balancing
2. **test_worker_failure**: Worker fails mid-execution, verify recovery
3. **test_heterogeneous_tasks**: Mix of fast and slow tasks

### Benchmarks

1. **bench_local_vs_distributed**: Compare throughput
2. **bench_scaling**: Measure speedup with worker count
3. **bench_overhead**: Measure scheduling overhead

## Implementation Plan

1. **Commit 1**: WorkStealingScheduler core structure
2. **Commit 2**: WorkerPool and RemoteWorker implementation
3. **Commit 3**: Task queue and distribution logic
4. **Commit 4**: Work stealing algorithm
5. **Commit 5**: Integration with executor.rs parallel edge execution
6. **Commit 6**: Unit tests for scheduler logic
7. **Commit 7**: Integration tests with real workers
8. **Commit 8**: Benchmarks
9. **Commit 9**: Example demonstrating work-stealing scheduler
10. **Commit 10**: Documentation and cleanup

## References

- Cilk work-stealing scheduler
- Rayon work-stealing implementation
- Java Fork/Join framework
- Tokio task scheduler internals
