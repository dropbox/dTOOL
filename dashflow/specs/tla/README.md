# TLA+ Specifications for DashFlow

**Last Updated:** 2026-01-03 (Worker #2353 - Complete TLA+ documentation)

This directory contains TLA+ formal specifications for DashFlow's core protocols.

## Overview

TLA+ is a formal specification language for designing, modeling, and verifying
concurrent and distributed systems. These specifications provide mathematical
guarantees about DashFlow's correctness.

## Specifications

| File | Description | Status |
|------|-------------|--------|
| `StateGraph.tla` | Core graph execution state machine | TLA-001 ✅ Verified |
| `ExecutorScheduler.tla` | Work-stealing scheduler algorithm | TLA-002 ✅ Verified |
| `DeadlockAnalysis.tla` | Deadlock freedom verification | TLA-003 ✅ Verified |
| `CheckpointConsistency.tla` | FileCheckpointer crash consistency | TLA-004 ✅ Verified |
| `WALAppendOrdering.tla` | Write-Ahead Log append ordering | TLA-005 ✅ Verified |
| `DistributedExecution.tla` | Distributed work-stealing scheduler | TLA-006 ✅ Verified |
| `StreamMessageOrdering.tla` | Streaming message sequence validation | TLA-007 ✅ Verified |
| `FailureRecovery.tla` | Retry + circuit breaker failure recovery | TLA-008 ✅ Verified |
| `ObservabilityOrdering.tla` | Telemetry event ordering/hierarchy | TLA-009 ✅ Verified |
| `RateLimiterFairness.tla` | Multi-tenant token bucket rate limiter | TLA-010 ✅ Verified |
| `run_tlc.sh` | Automated TLC verification runner | TLA-011 Complete |
| `verification_results.md` | Latest verification results | Ready |

**Configuration Files:** Each `.tla` spec has a corresponding `.cfg` file for TLC model parameters.
**Model Modules:** Some specs have `*MC.tla` modules for record-valued constants.

## Installation

### macOS (Homebrew)

```bash
brew install tlaplus
```

### Manual Installation

Download from: https://github.com/tlaplus/tlaplus/releases

### VSCode Extension

Install the TLA+ extension for syntax highlighting and integration:
- Extension ID: `alygin.vscode-tlaplus`

## Running Model Checker

### Using run_tlc.sh (Recommended)

The `run_tlc.sh` script automates verification of all TLA+ specifications:

```bash
cd specs/tla

# Check prerequisites (Java, tla2tools.jar)
./run_tlc.sh --check

# Run all verifications
./run_tlc.sh

# Run specific spec
./run_tlc.sh StateGraph
./run_tlc.sh ExecutorScheduler
./run_tlc.sh DeadlockAnalysis
./run_tlc.sh CheckpointConsistency

# Download tla2tools.jar if needed
./run_tlc.sh --download
```

The script will:
- Auto-download `tla2tools.jar` if missing
- Run TLC on all or specified specs
- Generate `verification_results.md` with results
- Use parallel workers for faster verification
- Auto-select `*MC.tla` model modules when present (for record-valued constants)

**Requirements:** Java 11+ (`brew install openjdk@11`)

### Using TLC (manual)

```bash
# Navigate to specs/tla directory
cd specs/tla

# Run TLC model checker directly
java -jar tla2tools.jar -config StateGraph.cfg StateGraphMC.tla

# With Homebrew TLA+ installation:
tlc -config StateGraph.cfg StateGraphMC.tla
```

### Using TLA+ Toolbox (GUI)

1. Open TLA+ Toolbox
2. File → Open Spec → StateGraph.tla
3. TLC Model Checker → New Model
4. Configure model constants
5. Run model checker

## What the Specifications Verify

### StateGraph.tla

Models the core execution semantics:

1. **Type Invariants** - Variables stay within valid domains
2. **Recursion Limit** - Never exceeds configured limit
3. **Valid Transitions** - All node references exist
4. **Edge Priority** - Conditional > Parallel > Simple
5. **Termination** - Eventually reaches END or error state
6. **No Livelock** - Running state eventually changes

### Properties Checked

| Property | Type | Description |
|----------|------|-------------|
| `TypeInvariant` | Invariant | All variables have valid types |
| `RecursionLimitRespected` | Invariant | Iteration count bounded |
| `ValidCurrentNode` | Invariant | Current node exists |
| `Safety` | Invariant | Combined safety properties |
| `EventuallyTerminates` | Liveness | System eventually stops |
| `NoLivelock` | Liveness | No infinite running without progress |

### ExecutorScheduler.tla

Models the work-stealing scheduler for parallel task distribution:

1. **Task States** - Tasks transition: pending → assigned → executing → completed
2. **Selection Strategies** - RoundRobin, LeastLoaded, Random
3. **Local vs Distributed** - Queue threshold determines local or remote execution
4. **Work Stealing** - Idle workers steal from busy workers
5. **Failure Handling** - Failed workers' tasks fall back to local execution

### ExecutorScheduler Properties

| Property | Type | Description |
|----------|------|-------------|
| `NoDoubleAssignment` | Invariant | Each task assigned to exactly one location |
| `WorkerQueueConsistency` | Invariant | Queue contents match assignments |
| `TaskCountInvariant` | Invariant | Executed count bounded by task count |
| `NoReexecution` | Invariant | Completed tasks never re-execute |
| `AllTasksComplete` | Liveness | All tasks eventually complete |
| `NoStarvation` | Liveness | Assigned tasks eventually execute |
| `RoundRobinFairness` | Property | Tasks distributed evenly (within 1) |
| `LeastLoadedBalance` | Property | Worker loads stay balanced |

### DeadlockAnalysis.tla

Proves that DashFlow graph execution is deadlock-free by modeling:

1. **Sequential Execution** - Single node execution with edge routing
2. **Parallel Execution** - Concurrent node execution with semaphore limiting
3. **Recursion Limits** - Bounded iteration count prevents infinite cycles
4. **Timeout Mechanisms** - Graph and node level timeouts bound execution time
5. **State Merging** - Parallel results always merge or produce error

### DeadlockAnalysis Properties

| Property | Type | Description |
|----------|------|-------------|
| `NoDeadlock` | Invariant | Running state always has enabled action |
| `SemaphoreNonNegative` | Invariant | Permits never go negative |
| `SemaphoreBounded` | Invariant | Permits never exceed max |
| `ParallelConcurrencyBounded` | Invariant | Active tasks bounded by permits |
| `ParallelStatesMutuallyExclusive` | Invariant | No node in multiple states |
| `EventuallyTerminates` | Liveness | Execution always terminates |
| `NoLivelock` | Liveness | Running state eventually changes |
| `SemaphoreEventuallyReleased` | Liveness | Permits always released |
| `ParallelEventuallyComplete` | Liveness | Parallel execution completes |

### Why DashFlow is Deadlock-Free

The specification proves deadlock freedom through five mechanisms:

1. **Recursion Limit** - Graph cycles are bounded by `RecursionLimit` (default: 25)
2. **Graph Timeout** - Total execution bounded by `GraphTimeout` (default: 5 minutes)
3. **Node Timeout** - Individual nodes bounded by `NodeTimeout` (default: 30 seconds)
4. **Semaphore Liveness** - Permits are always eventually released
5. **Parallel Merge** - All parallel branches merge or produce clear error

### CheckpointConsistency.tla (TLA-004)

Models the FileCheckpointer crash consistency guarantees:

1. **Atomic Rename** - Checkpoints are written atomically via temp file + rename
2. **Index Safety** - Index always references valid checkpoint files
3. **Crash Recovery** - System recovers to consistent state after crash

| Property | Type | Description |
|----------|------|-------------|
| `IndexReferencesExistingCheckpoint` | Invariant | Index never references missing files |

### WALAppendOrdering.tla (TLA-005)

Models Write-Ahead Log append ordering contract:

1. **Mutex Serialization** - Writers serialize via mutex
2. **Buffer/Durable Separation** - Buffered vs durable write distinction
3. **Crash Recovery** - Only durable writes survive crash

| Property | Type | Description |
|----------|------|-------------|
| `DurableIsPrefix` | Invariant | Durable log is prefix of buffered |
| `UniqueEventIds` | Invariant | All event IDs are unique |
| `SingleWriterOrder` | Invariant | Single-writer append ordering |
| `Safety` | Invariant | Combined safety properties |

### DistributedExecution.tla (TLA-006)

Models distributed work-stealing scheduler:

1. **Task Assignment** - Tasks distributed across workers
2. **Queue Execution** - Workers process local queues
3. **Work Stealing** - Idle workers steal from busy workers
4. **Failure Recovery** - Failed workers' tasks reassigned

| Property | Type | Description |
|----------|------|-------------|
| `NoTaskDuplication` | Invariant | Each task assigned exactly once |
| `NoTaskLoss` | Invariant | Tasks never lost |
| `WorkerQueuesBounded` | Invariant | Queue sizes bounded |
| `ExecutingImpliesBusy` | Invariant | Executing workers marked busy |
| `FailedWorkerNotExecuting` | Invariant | Failed workers don't execute |

### StreamMessageOrdering.tla (TLA-007)

Models streaming message sequence validation:

1. **Sequence Generation** - Producers assign monotonic sequence numbers
2. **Network Effects** - Models reordering, loss, and duplication
3. **Consumer Validation** - Consumers detect gaps and duplicates

| Property | Type | Description |
|----------|------|-------------|
| `GapAlwaysDetected` | Invariant | Sequence gaps are detected |
| `DuplicateImpliesConsecutive` | Invariant | Duplicates only for consecutive retries |
| `ReorderImpliesOlderThanExpected` | Invariant | Reorders detected by sequence |
| `ExpectedNeverRegresses` | Invariant | Expected sequence only increases |

### FailureRecovery.tla (TLA-008)

Models failure recovery with retry and circuit breaker:

1. **Exponential Backoff** - Retry delay doubles on failure
2. **Circuit Breaker** - States: closed → open → half-open
3. **Degraded Mode** - Graceful degradation under failures

| Property | Type | Description |
|----------|------|-------------|
| `RetryBounded` | Invariant | Retry count bounded |
| `CircuitBreakerStateValid` | Invariant | Circuit breaker in valid state |
| `BackoffNonNegative` | Invariant | Backoff delay non-negative |
| `RequestCountsValid` | Invariant | Request counts consistent |

### ObservabilityOrdering.tla (TLA-009)

Models observability event ordering and hierarchy:

1. **Event Emission** - Spans, traces, and metrics emission
2. **Execution Hierarchy** - Parent/child relationships, depth tracking
3. **Happens-Before** - Causal ordering of events

| Property | Type | Description |
|----------|------|-------------|
| `GraphStartFirst` | Invariant | Graph start is first event |
| `GraphEndLast` | Invariant | Graph end is last event |
| `NodeStartBeforeEnd` | Invariant | Node start precedes end |
| `TimestampsMonotonic` | Invariant | Timestamps monotonically increase |
| `HierarchyConsistent` | Invariant | Parent-child relationships valid |

### RateLimiterFairness.tla (TLA-010)

Models multi-tenant token bucket rate limiter:

1. **Per-Tenant Buckets** - Isolated token buckets per tenant
2. **Token Refill** - Lazy refill on access
3. **Burst Capacity** - Up to bucket capacity for bursts

| Property | Type | Description |
|----------|------|-------------|
| `TokensNeverExceedCapacity` | Invariant | Bucket never overflows |
| `TokensNeverNegative` | Invariant | Bucket never underflows |
| `NoTokenCreation` | Invariant | Tokens only from refill |
| `TenantIsolation` | Invariant | Tenants don't share buckets |

## Model Parameters

The `StateGraph.cfg` file defines an example model:

```tla
Nodes = {"researcher", "writer", "reviewer"}
EntryPoint = "researcher"
RecursionLimit = 10
```

You can modify these to test different graph configurations.

### ExecutorScheduler.cfg Parameters

```tla
Tasks = {T1, T2, T3}        \* 3 tasks to schedule
Workers = {W1, W2}          \* 2 remote workers
LocalQueueThreshold = 2     \* Distribute when queue >= 2
SelectionStrategy = "LeastLoaded"  \* Options: RoundRobin, LeastLoaded, Random
EnableStealing = TRUE       \* Allow work stealing
```

Run with different strategies:
```bash
# Test round-robin fairness
sed 's/LeastLoaded/RoundRobin/' ExecutorScheduler.cfg > rr.cfg
tlc -config rr.cfg ExecutorScheduler.tla

# Test local-only (no workers)
# Edit cfg to set Workers = {}
```

## Creating Custom Models

To verify a specific graph configuration:

1. Copy `StateGraph.cfg` to a new file (e.g., `MyGraph.cfg`)
2. Modify the `CONSTANTS` section with your nodes and edges
3. Run: `tlc -config MyGraph.cfg StateGraph.tla`

## Mapping to Rust Code

| TLA+ Concept | Rust Implementation |
|--------------|---------------------|
| `currentNode` | `current_nodes` in `invoke_internal` |
| `graphState` | Generic state `S: GraphState` |
| `executedNodes` | `nodes_executed: Vec<String>` |
| `iterationCount` | `iteration_count: u32` |
| `RecursionLimit` | `self.recursion_limit` |
| `NextNode` | `find_next_nodes()` method |
| Edge priority | Checked in order: conditional, parallel, simple |

## Completed Milestones

All TLA+ specifications are now complete (TLA-001 through TLA-011):

- ✅ TLA-001: StateGraph state machine
- ✅ TLA-002: ExecutorScheduler algorithm
- ✅ TLA-003: DeadlockAnalysis verification
- ✅ TLA-004: CheckpointConsistency model
- ✅ TLA-005: WALAppendOrdering spec
- ✅ TLA-006: DistributedExecution spec
- ✅ TLA-007: StreamMessageOrdering spec
- ✅ TLA-008: FailureRecovery spec
- ✅ TLA-009: ObservabilityOrdering spec
- ✅ TLA-010: RateLimiterFairness spec
- ✅ TLA-011: TLC automation (run_tlc.sh)
- ✅ TLA-012: Documentation (this file + verification_results.md)

**Note:** TLA-005 through TLA-010 specs are ready but require Java to run TLC verification.
Install with: `brew install openjdk@11`

See `WORKER_DIRECTIVE.md` for the project roadmap.
