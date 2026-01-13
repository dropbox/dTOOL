# dashflow-streaming

Ultra-efficient streaming telemetry for DashFlow using Protocol Buffers.

## Features

- **Protobuf Encoding**: Compact binary serialization (<100μs encoding target)
- **Kafka Integration**: Stream events to Kafka for real-time analytics
- **Compression**: Zstd/LZ4 support (5:1 compression ratio target)
- **Zero-Copy**: Efficient serialization with minimal allocations
- **Type-Safe**: Full Rust type system integration
- **Distributed Rate Limiting**: Per-tenant rate limiting with optional Redis backend for multi-server deployments

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-streaming = "1.11"
```

## Quick Start

### Producer: Sending Events to Kafka

```rust
use dashflow_streaming::{
    producer::DashStreamProducer,
    Event, EventType, Header, MessageType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create producer
    let producer = DashStreamProducer::new("localhost:9092", "dashstream_events").await?;

    // Create an event
    let event = Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "my-app".to_string(),
            thread_id: "session-123".to_string(),
            sequence: 1,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::GraphStart as i32,
        node_id: "".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    };

    // Send event
    producer.send_event(event).await?;
    producer.flush(std::time::Duration::from_secs(5)).await?;

    Ok(())
}
```

### Consumer: Reading Events from Kafka

```rust
use dashflow_streaming::consumer::DashStreamConsumer;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create consumer for a specific partition.
    // DashStreamConsumer consumes a single partition (no consumer-group rebalancing with rskafka).
    let mut consumer = DashStreamConsumer::new_for_partition(
        "localhost:9092",
        "dashstream_events",
        0,
    )
    .await?;

    // Consume messages.
    // Note: `next_timeout()` returns `None` on timeout (not end-of-stream),
    // so treat `None` as "no message yet" and keep waiting.
    loop {
        match consumer.next_timeout(Duration::from_secs(30)).await {
            Some(Ok(msg)) => println!("Received: {:?}", msg),
            Some(Err(e)) => eprintln!("Error: {}", e),
            None => continue, // timeout
        }
    }

    Ok(())
}
```

### Rate Limiting

DashFlow Streaming supports per-tenant rate limiting to prevent resource exhaustion and ensure fair usage across multiple tenants.

#### In-Memory Rate Limiting (Single Server)

For single-server deployments, use in-memory rate limiting:

```rust
use dashflow_streaming::{
    producer::DashStreamProducer,
    rate_limiter::RateLimit,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create producer with rate limiting (100 msg/sec, burst up to 1000)
    let producer = DashStreamProducer::new_with_rate_limiting(
        "localhost:9092",
        "dashstream_events",
        RateLimit {
            messages_per_second: 100.0,
            burst_capacity: 1000,
        },
        None, // In-memory mode (no Redis)
    ).await?;

    // Send events (automatically rate-limited per tenant)
    for i in 0..1000 {
        let event = create_event(&format!("tenant_{}", i % 10));
        producer.send_event(event).await?;
    }

    Ok(())
}
```

#### Distributed Rate Limiting (Multi-Server)

For multi-server deployments, use Redis-backed rate limiting to enforce shared quotas across all servers:

```rust
use dashflow_streaming::{
    producer::DashStreamProducer,
    rate_limiter::RateLimit,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create producer with Redis-backed rate limiting
    let producer = DashStreamProducer::new_with_rate_limiting(
        "localhost:9092",
        "dashstream_events",
        RateLimit {
            messages_per_second: 100.0,
            burst_capacity: 1000,
        },
        Some("redis://localhost:6379"), // Distributed mode with Redis
    ).await?;

    // Rate limits enforced across ALL servers sharing this Redis instance
    // If 3 servers @ 100 msg/sec each → 100 msg/sec total (not 300!)

    Ok(())
}
```

**Why Distributed Rate Limiting?**

Without Redis, each server enforces rate limits independently:
- **Problem**: 3 servers @ 100 msg/sec each = 300 msg/sec actual (3× quota violation)
- **Solution**: Redis-backed token bucket shared across all servers = 100 msg/sec total

**Capacity Planning:**
- **Memory**: ~100 bytes per tenant in Redis
- **Recommended**: 64 MB for development, 256 MB for production (handles 2.5M tenants)
- **Latency**: 1-5ms for Redis operations
- **TTL**: Inactive tenant data expires after 1 hour

For more details, see [OBSERVABILITY_INFRASTRUCTURE.md](../../docs/OBSERVABILITY_INFRASTRUCTURE.md).

## Message Partitioning and Ordering Guarantees

DashFlow Streaming uses `thread_id` as the Kafka partition key to ensure message ordering within a session/thread.

### Partitioning Contract

| Field | Role | Guarantee |
|-------|------|-----------|
| `thread_id` | Partition key | All messages with the same `thread_id` go to the same Kafka partition |
| `sequence` | Per-thread sequence number | Monotonically increasing within a `thread_id` |
| `tenant_id` | Logical tenant identifier | Used for rate limiting, not partitioning |

### Ordering Guarantees

1. **Within a `thread_id`**: Messages are **strictly ordered** by Kafka offset. The consumer receives them in the exact order they were produced.

2. **Across `thread_id`s**: **No ordering guarantee**. Messages from different threads may be on different partitions and can arrive in any relative order.

3. **Sequence Validation**: The consumer validates `sequence` numbers to detect:
   - **Gaps**: Missing messages (sequence jump > 1)
   - **Duplicates**: Same sequence number received twice
   - **Reordering**: Sequence number less than previously seen

### Design Rationale

- **Why `thread_id`?** Each thread represents a conversation/session that requires strict ordering (e.g., state diffs must apply in order).
- **Why not `tenant_id`?** Tenants can have many concurrent threads; partitioning by tenant would create hotspots.
- **Partition count**: Should be ≥ expected concurrent threads for parallelism; typically 3-16 for development, 64+ for production.

### Consumer Considerations

```rust
// Single-partition consumer (no rebalancing complexity)
let consumer = DashStreamConsumer::new_for_partition(broker, topic, partition).await?;

// To consume all partitions, spawn one consumer per partition:
for partition_id in 0..partition_count {
    tokio::spawn(async move {
        let mut consumer = DashStreamConsumer::new_for_partition(
            broker, topic, partition_id
        ).await.unwrap();
        // Process messages...
    });
}
```

**Note**: DashFlow uses `rskafka` which does not support consumer groups with automatic partition assignment. Each consumer explicitly claims a partition.

## Command-Line Tool: `parse_events`

This crate includes a command-line tool to decode and display DashFlow Streaming events from Kafka in human-readable JSON format.

### Installation

```bash
cargo install --path crates/dashflow-streaming
```

Or run directly:

```bash
cargo run --bin parse_events
```

### Usage

```bash
# View all events from the beginning (default topic: dashstream_events)
parse_events

# Specify custom topic
parse_events --topic my_topic

# Tail mode: only show new events
parse_events --tail

# With custom broker
parse_events --broker localhost:9092

# Limit number of messages
parse_events --limit 10

# Get help
parse_events --help
```

### Example Output

```json
{
  "type": "Event",
  "header": {
    "message_id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp_us": 1699564800000000,
    "timestamp_iso": "2023-11-09T12:00:00+00:00",
    "tenant_id": "my-app",
    "thread_id": "session-123",
    "sequence": 1,
    "type": "EVENT",
    "parent_id": null,
    "schema_version": 1
  },
  "event_type": "GRAPH_START",
  "node_id": null,
  "attributes": {
    "graph_name": "librarian"
  },
  "duration_us": null,
  "duration_ms": null,
  "llm_request_id": null
}
```

## Command-Line Tool: `analyze_events`

This crate includes an analytics tool that aggregates metrics from parsed DashFlow Streaming events. It reads JSON output from `parse_events` and generates comprehensive reports.

### Installation

Same as `parse_events` above.

### Usage

```bash
# Basic text report (pipe from parse_events)
parse_events | analyze_events

# JSON output
parse_events | analyze_events --format json

# Markdown report
parse_events | analyze_events --format markdown

# Analyze last 1000 events
parse_events --limit 1000 | analyze_events --format markdown > report.md
```

### What It Analyzes

**Session Metrics:**
- Total duration per session
- Event counts
- Node execution counts
- Tool calls
- Error counts
- Token usage (prompt, completion, total)

**Node Performance:**
- Execution count
- Average, min, max durations
- P50, P95, P99 latency percentiles
- Total duration (to identify bottlenecks)

**Tool Performance:**
- Total calls
- Success rate
- Retry rate
- Average and P95 duration

**Error Analysis:**
- Error counts by severity
- Percentage distribution

### Example Output (Text Format)

```
=== DashFlow Streaming Analytics Report ===

SUMMARY
  Total Sessions:      3
  Total Events:        247
  Total Duration:      12453.50 ms
  Total Tokens:        15234
  Prompt Tokens:       8234
  Completion Tokens:   7000
  Total Errors:        2

SESSIONS (Top 3):
  1. session-abc123
     Events: 150, Duration: 8234.20 ms, Nodes: 5, Tool Calls: 12, Errors: 1, Tokens: 9234
  2. session-def456
     Events: 75, Duration: 3219.30 ms, Nodes: 4, Tool Calls: 8, Errors: 0, Tokens: 4500
  3. session-ghi789
     Events: 22, Duration: 1000.00 ms, Nodes: 3, Tool Calls: 2, Errors: 1, Tokens: 1500

NODE PERFORMANCE (Top 10 by total duration):
  1. retriever (count: 15)
     Avg: 245.50 ms, P50: 230.00 ms, P95: 450.00 ms, P99: 520.00 ms
  2. llm_call (count: 12)
     Avg: 1850.30 ms, P50: 1750.00 ms, P95: 2500.00 ms, P99: 2800.00 ms
  3. reranker (count: 10)
     Avg: 125.20 ms, P50: 120.00 ms, P95: 180.00 ms, P99: 200.00 ms

TOOL PERFORMANCE:
  retriever_tool (calls: 15)
     Success Rate: 93.3%, Avg Duration: 245.50 ms, P95: 450.00 ms
  search_tool (calls: 8)
     Success Rate: 100.0%, Avg Duration: 150.20 ms, P95: 220.00 ms

ERRORS:
  ERROR: 1 (50.0%)
  WARNING: 1 (50.0%)
```

### Example Workflow

```bash
# 1. Start Kafka
docker-compose -f docker-compose-kafka.yml up -d

# 2. Run your DashFlow application
cargo run -p librarian -- query "test"

# 3. Parse and analyze events
cargo run --bin parse_events | cargo run --bin analyze_events --format markdown > report.md

# 4. View report
cat report.md
```

### Use Cases

**Performance Optimization:**
- Identify slow nodes (high P95/P99)
- Find bottlenecks (high total duration)
- Track improvements over time

**Quality Monitoring:**
- Tool success rates
- Error frequency and severity
- Retry rates

**Cost Tracking:**
- Token usage per session
- LLM costs (with pricing data)
- Resource utilization

**Debugging:**
- Session-level diagnostics
- Error patterns
- Tool failure analysis

## Message Types

DashFlow Streaming supports the following message types:

- **Event**: Lifecycle events (graph start/end, node execution, etc.)
- **StateDiff**: Incremental state updates using JSON Patch
- **TokenChunk**: LLM streaming tokens
- **ToolExecution**: Tool call tracking
- **Checkpoint**: Full state snapshots
- **Metrics**: Performance metrics
- **Error**: Error tracking and reporting

## Event Types

- **Graph Lifecycle**: `GRAPH_START`, `GRAPH_END`, `GRAPH_ERROR`
- **Node Lifecycle**: `NODE_START`, `NODE_END`, `NODE_ERROR`
- **Edge Traversal**: `EDGE_TRAVERSAL`, `CONDITIONAL_BRANCH`
- **LLM Lifecycle**: `LLM_START`, `LLM_END`, `LLM_ERROR`, `LLM_RETRY`
- **Tool Lifecycle**: `TOOL_START`, `TOOL_END`, `TOOL_ERROR`
- **Checkpoint**: `CHECKPOINT_SAVE`, `CHECKPOINT_LOAD`, `CHECKPOINT_DELETE`
- **Memory**: `MEMORY_SAVE`, `MEMORY_LOAD`
- **Human-in-the-loop**: `HUMAN_INTERRUPT`, `HUMAN_RESUME`

## Integration with DashFlow

DashFlow applications automatically emit DashFlow Streaming events when using the `DashStreamCallback`:

```rust
use dashflow::dashstream_callback::DashStreamCallback;

// Create callback
let callback = DashStreamCallback::new("localhost:9092", "dashstream_events").await?;

// Add to graph
let graph = StateGraph::new()
    .add_callbacks(vec![Arc::new(callback)])
    // ... rest of graph setup
    .compile()?;
```

## Examples

See the `examples/` directory for complete examples:

- `kafka_streaming.rs` - Producer and consumer example
- `state_diff.rs` - State diff example

See also the sample application:
- `examples/apps/librarian/` - Production RAG with DashFlow Streaming

## Command-Line Tool: `eval_runner`

The `eval_runner` tool provides automated evaluation of DashFlow applications using metrics from `analyze_events`.

### Overview

The evaluation framework enables:
- **Baseline comparison**: Compare current runs to previous baselines
- **Regression detection**: Identify performance/quality regressions automatically
- **Cost tracking**: Monitor token usage and API costs
- **CI/CD integration**: Run automated evaluations in test pipelines

### Installation

```bash
cargo build --bin eval_runner
```

### Workflow

```
parse_events → analyze_events → eval_runner → Evaluation Report
```

### Usage

#### 1. Save a baseline

Capture metrics from a good run and save as baseline:

```bash
# Run app (emits events to Kafka)
cargo run -p librarian -- query "What is the capital of France?"

# Parse + analyze + save baseline
cargo run --bin parse_events | \
  cargo run --bin analyze_events --format json | \
  cargo run --bin eval_runner -- \
    --app-name librarian \
    --version 1.0.0 \
    --save-baseline baselines/librarian_v1.0.0.json
```

#### 2. Compare to baseline

Run the app and compare to baseline:

```bash
# Run app
cargo run -p librarian -- query "What is the capital of France?"

# Parse + analyze + compare
cargo run --bin parse_events | \
  cargo run --bin analyze_events --format json | \
  cargo run --bin eval_runner -- \
    --app-name librarian \
    --baseline baselines/librarian_v1.0.0.json
```

Output:
```
=== Evaluation Report ===

App: librarian
Baseline: baselines/librarian_v1.0.0.json

QUALITY:
  (not evaluated)

PERFORMANCE:
  ✓ P95 Latency:  1850.00ms (baseline: 1850.00ms, 1.0x)
  ✓ Avg Latency:  1047.50ms (baseline: 1047.50ms, 1.0x)
  ✓ Success Rate: 100.0% (baseline: 100.0%, delta: +0.0%)
  ✓ Error Rate:   0.0% (baseline: 0.0%, delta: +0.0%)

COST:
  ✓ Total Tokens: 150 (baseline: 150, 1.0x)
  ✓ Cost per Run: $0.00075 (baseline: $0.00075, 1.0x)
  ✓ Tool Calls:   4 (baseline: 4, 1.0x)
```

#### 3. CLI options

```bash
cargo run --bin eval_runner -- --help

# Options:
#   --save-baseline <PATH>    Save metrics as baseline
#   --baseline <PATH>         Load baseline for comparison
#   --app-name <NAME>         Application name (default: "app")
#   --version <VERSION>       Version (default: "1.0.0")
#   --pricing <MODEL>         LLM pricing model (gpt4o, gpt35, claude35)
#   --format <FORMAT>         Output format (text, json)
```

### Convenience Script

Use the provided script for easier invocation:

```bash
# Compare to baseline
./scripts/run_eval.sh --baseline baselines/librarian_v1.0.0.json

# Save new baseline
./scripts/run_eval.sh --save-baseline baselines/new.json --app-name my_app
```

### Metrics

The evaluation framework tracks:

**Quality Metrics** (optional, requires manual scoring):
- Correctness: Did the app produce correct output?
- Relevance: Are retrieved documents relevant?
- Safety: Does output contain harmful content?
- Hallucination rate: Percentage of made-up facts

**Performance Metrics** (from analytics):
- P95 latency: 95th percentile response time
- Avg latency: Average response time
- Success rate: Percentage of successful completions
- Error rate: Percentage of errors

**Cost Metrics** (from analytics):
- Total tokens: Prompt + completion tokens
- Cost per run: Estimated API cost (USD)
- Tool calls: Number of tool invocations

### LLM Pricing

Built-in pricing models (as of Nov 2025):

- **GPT-4o** (default): $2.50/1M input, $10.00/1M output
- **GPT-3.5-turbo**: $0.50/1M input, $1.50/1M output
- **Claude 3.5 Sonnet**: $3.00/1M input, $15.00/1M output

Specify with `--pricing`:

```bash
cargo run --bin eval_runner -- --baseline baselines/x.json --pricing gpt35
```

### Integration with CI/CD

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The workflow example below is provided as a template for teams using GitHub Actions.

Example GitHub Actions workflow:

```yaml
name: Evaluations

on: [push, pull_request]

jobs:
  eval:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Start Kafka
        run: docker-compose -f docker-compose-kafka.yml up -d
      - name: Run app
        run: cargo run -p librarian -- query "test query"
      - name: Run eval
        run: |
          cargo run --bin parse_events | \
          cargo run --bin analyze_events --format json | \
          cargo run --bin eval_runner -- \
            --baseline baselines/librarian_v1.0.0.json
```

### Regression Detection

The `eval_runner` now includes automated regression detection with configurable thresholds.

#### Pass/Fail Logic

When comparing to a baseline, the tool automatically detects regressions in:

**Quality Metrics** (Critical severity - test fails):
- Correctness dropped > 5%
- Relevance dropped > 10%
- Safety dropped > 5%
- Hallucination rate increased > 5%

**Performance Metrics** (Warning severity - test passes with warnings):
- P95 latency increased > 20%
- Average latency increased > 20%
- Success rate dropped > 5%
- Error rate increased > 5%

**Cost Metrics** (Warning severity - test passes with warnings):
- Token usage increased > 50%
- Cost per run increased > 50%
- Tool calls increased > 50%

#### Exit Codes

- **Exit 0**: No regressions or only warnings
- **Exit 1**: Critical regressions detected (quality degradation)

#### Threshold Modes

Control strictness with `--threshold-mode`:

```bash
# Default thresholds (balanced)
cargo run --bin eval_runner -- --baseline baselines/x.json

# Strict thresholds (2-10% tolerance)
cargo run --bin eval_runner -- --baseline baselines/x.json --threshold-mode strict

# Lenient thresholds (10-100% tolerance)
cargo run --bin eval_runner -- --baseline baselines/x.json --threshold-mode lenient
```

#### Example Output with Regression

```
=== Evaluation Report ===

App: librarian
Baseline: baselines/librarian_v1.0.0.json

PERFORMANCE:
  ✓ P95 Latency:  1850.00ms (baseline: 1850.00ms)
  ⚠ Avg Latency:  1500.00ms (baseline: 1000.00ms, 1.5x slower)

COST:
  ⚠ Total Tokens: 225 (baseline: 150, 1.5x more tokens)

⚠ PASSED (with warnings): 2 warning(s)
```

#### JSON Output

Get structured results with `--format json`:

```bash
cargo run --bin eval_runner -- --baseline baselines/x.json --format json
```

Output:
```json
{
  "status": "PASSED_WITH_WARNINGS",
  "summary": {
    "critical_regressions": 0,
    "warnings": 2,
    "info": 0
  },
  "current": { ... },
  "baseline": { ... },
  "regressions": [
    {
      "metric": "avg_latency",
      "baseline_value": 1000.0,
      "current_value": 1500.0,
      "threshold": 1.2,
      "severity": "Warning",
      "description": "1.5x slower"
    }
  ]
}
```

#### CI/CD Integration with Regression Checks

Updated GitHub Actions workflow:

```yaml
- name: Run eval with regression check
  run: |
    cargo run --bin parse_events | \
    cargo run --bin analyze_events --format json | \
    cargo run --bin eval_runner -- \
      --baseline baselines/librarian_v1.0.0.json \
      --fail-on-regression true
  # This step will fail (exit 1) if critical regressions are detected
```

### Golden Dataset Support

Evaluate applications against reference query/answer pairs for reproducible quality testing.

#### Creating an Eval Suite

Create a JSON file with test cases:

```json
{
  "name": "librarian",
  "description": "Librarian RAG test cases",
  "version": "1.0.0",
  "cases": [
    {
      "id": "basic_001",
      "query": "What is the capital of France?",
      "expected_answer": "Paris",
      "metadata": {
        "category": "geography",
        "difficulty": "easy"
      }
    },
    {
      "id": "technical_001",
      "query": "What is the time complexity of binary search?",
      "expected_answer": "O(log n)",
      "metadata": {
        "category": "computer_science",
        "difficulty": "medium"
      }
    }
  ]
}
```

Example suites are available in `examples/apps/librarian/data/` directory.

#### Running Eval Suites

Score actual answers against the eval suite:

```bash
# Create answers JSON (one answer per test case, in order)
echo '["Paris", "O(log n)"]' | \
  cargo run --bin eval_runner -- \
    --eval-suite evals/librarian.json

# With custom scoring method
echo '["Paris", "O(log n)"]' | \
  cargo run --bin eval_runner -- \
    --eval-suite evals/librarian.json \
    --scoring-method fuzzy \
    --correctness-threshold 0.8
```

**Scoring Methods:**
- `exact` - Exact string match (case-sensitive)
- `case-insensitive` - Case-insensitive exact match
- `fuzzy` - Normalized Levenshtein distance (default)
- `contains` - Expected answer is substring of actual

#### Example Output

```
=== Eval Suite Results ===

Suite: librarian v1.0.0
Description: Librarian RAG test cases
Test Cases: 2

[✓] Case 1: basic_001 (100.0%)
    Query:    What is the capital of France?
    Expected: Paris
    Actual:   Paris

[✓] Case 2: technical_001 (100.0%)
    Query:    What is the time complexity of binary search?
    Expected: O(log n)
    Actual:   O(log n)

=== Summary ===
Average Correctness: 100.0%
Threshold: 80.0%
Passed: 2 / 2
Failed: 0 / 2

✓ PASSED
```

#### Programmatic Usage

Use golden datasets in Rust code:

```rust
use dashflow_streaming::evals::{EvalSuite, ScoringMethod, score_suite, average_correctness};

// Load eval suite
let suite = EvalSuite::load("evals/librarian.json")?;

// Get actual answers from your application
let actual_answers = vec![
    "Paris".to_string(),
    "O(log n)".to_string(),
];

// Score with fuzzy matching
let scores = score_suite(&suite, &actual_answers, ScoringMethod::FuzzyMatch)?;
let avg = average_correctness(&scores);

println!("Average correctness: {:.1}%", avg * 100.0);

// Check threshold
if avg < 0.8 {
    eprintln!("✗ FAILED: Correctness below 80%");
    std::process::exit(1);
}
```

### Test Harness Integration

Integrate evals with `cargo test` for automated testing in your CI/CD pipeline. This allows you to run evaluations as part of your regular test suite and catch regressions automatically.

#### Using the Test Harness

The test harness provides helpers, assertion macros, and fixtures to make evaluation testing seamless with Rust's built-in test framework.

**Example integration test** (`tests/my_app_evals.rs`):

```rust
use dashflow_streaming::{
    assert_no_critical_regressions,
    assert_metric_within_threshold,
    assert_quality_maintained,
};
use dashflow_streaming::evals::{
    EvalTestRunner, EvalMetrics, RegressionThresholds,
    mock_metrics, mock_baseline,
};
use std::io::Write;
use tempfile::NamedTempFile;

// Helper to create baseline file
fn create_baseline_file(baseline: &Baseline) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    let json = serde_json::to_string_pretty(baseline).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn test_librarian_quality() {
    // Setup baseline
    let baseline = mock_baseline("librarian", "1.0.0", 1850.0, 1047.5, 150);
    let baseline_file = create_baseline_file(&baseline);

    // Create test runner
    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Simulate current metrics (e.g., from a test run)
    let metrics = mock_metrics(1800.0, 1000.0, 145);

    // Run evaluation
    let regressions = runner.run(&metrics);

    // Assert: No critical regressions
    assert!(regressions.is_empty(), "Unexpected regressions: {:?}", regressions);
}

#[test]
fn test_librarian_with_macros() {
    // Setup
    let baseline = mock_baseline("librarian", "1.0.0", 1850.0, 1047.5, 150);
    let baseline_file = create_baseline_file(&baseline);
    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Current metrics
    let current = EvalMetrics {
        p95_latency: 1620.0,      // Improved!
        avg_latency: 980.0,        // Improved!
        success_rate: 100.0,
        error_rate: 0.0,
        total_tokens: 145,
        cost_per_run: 0.000725,
        tool_calls: 2,
        correctness: Some(0.96),   // Improved!
        relevance: Some(0.93),
        safety: Some(1.0),
        hallucination_rate: Some(0.02),
    };

    let baseline_metrics = runner.baseline().metrics.clone();
    let thresholds = RegressionThresholds::default();

    // Use assertion macros for clear test failures
    assert_no_critical_regressions!(current, baseline_metrics, thresholds);

    assert_metric_within_threshold!(
        current.p95_latency,
        baseline_metrics.p95_latency,
        1.2,  // 20% slower allowed
        "P95 latency"
    );

    assert_quality_maintained!(
        current.correctness.unwrap(),
        baseline_metrics.correctness.unwrap(),
        0.05,  // 5% absolute drop allowed
        "Correctness"
    );
}

#[test]
fn test_with_strict_thresholds() {
    // Use strict thresholds for production deployments
    let baseline = mock_baseline("prod_app", "2.0.0", 1000.0, 500.0, 150);
    let baseline_file = create_baseline_file(&baseline);

    let mut runner = EvalTestRunner::with_thresholds(
        baseline_file.path(),
        RegressionThresholds::strict()  // Tighter thresholds
    );

    let metrics = mock_metrics(1050.0, 525.0, 150);
    let result = runner.run_and_check(&metrics);

    assert!(result.is_ok(), "Strict threshold regression: {:?}", result.err());
}
```

#### Assertion Macros

Three convenient macros for different regression checks:

**1. `assert_no_critical_regressions!`**

Fails the test if any critical regressions (quality degradation) are detected:

```rust
assert_no_critical_regressions!(current_metrics, baseline_metrics, thresholds);
```

**2. `assert_metric_within_threshold!`**

Checks a specific metric against a threshold:

```rust
assert_metric_within_threshold!(
    current.p95_latency,
    baseline.p95_latency,
    1.2,  // threshold multiplier
    "P95 latency"
);
```

**3. `assert_quality_maintained!`**

Checks quality metrics (0.0-1.0 scores) haven't dropped below threshold:

```rust
assert_quality_maintained!(
    current.correctness.unwrap(),
    baseline.correctness.unwrap(),
    0.05,  // 5% absolute drop allowed
    "Correctness"
);
```

#### Mock Fixtures

Helper functions for creating test fixtures:

```rust
use dashflow_streaming::evals::{mock_metrics, mock_baseline};

// Create mock metrics
let metrics = mock_metrics(
    1800.0,  // p95_latency
    1000.0,  // avg_latency
    150      // total_tokens
);

// Create mock baseline
let baseline = mock_baseline(
    "my_app",
    "1.0.0",
    1850.0,  // p95_latency
    1047.5,  // avg_latency
    150      // total_tokens
);
```

#### CI/CD Integration with Cargo Test

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The workflow example below is provided as a template for teams using GitHub Actions.

Run evaluations as part of your test suite:

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run unit tests
        run: cargo test

      - name: Run evaluation tests
        run: cargo test --test evals_integration

      # Tests fail automatically if regressions detected
```

#### Benefits of Test Harness Integration

- **Automated**: Runs with `cargo test`, no manual intervention
- **CI/CD Ready**: Fails builds on critical regressions
- **Fast Feedback**: Test failures show exact regression details
- **Type Safe**: Rust's type system catches configuration errors
- **Composable**: Mix eval tests with regular unit/integration tests

### Performance Benchmarking

Run performance benchmarks with statistical analysis to track latency, throughput, and cost metrics over time.

#### Benchmark Runner CLI

Run benchmarks from the command line:

```bash
# Run a benchmark with 50 iterations
cargo run --bin benchmark_runner -- \
  --analytics analytics.json \
  --iterations 50 \
  --output benchmark_result.json

# Compare against baseline
cargo run --bin benchmark_runner -- \
  --analytics analytics.json \
  --baseline baseline_benchmark.json \
  --threshold 0.2

# Save as new baseline
cargo run --bin benchmark_runner -- \
  --analytics analytics.json \
  --iterations 100 \
  --save-baseline new_baseline.json
```

#### Benchmark Output

```
=== Benchmark Report ===

Samples: 50

Latency (ms):
  Min:     950.00
  P50:     1000.00
  Mean:    1005.50
  P95:     1050.00
  P99:     1100.00
  Max:     1150.00
  StdDev:  45.20
  95% CI:  [992.45, 1018.55]

Throughput: 0.99 req/s

Tokens:
  Min:     140
  Median:  150
  Mean:    152
  Max:     165
  StdDev:  6.50

Cost (USD):
  Min:     $0.000700
  Median:  $0.000750
  Mean:    $0.000760
  Max:     $0.000825
  StdDev:  $0.000033

Quality:
  Correctness: 0.95
  Relevance:   0.90
  Safety:      1.00
```

#### Programmatic Benchmarking

Use benchmarks in Rust code:

```rust
use dashflow_streaming::evals::{
    BenchmarkConfig, BenchmarkRunner, BenchmarkResult,
    detect_performance_regression, format_benchmark_report
};
use dashflow_streaming::evals::metrics::EvalMetrics;

// Configure benchmark
let config = BenchmarkConfig {
    iterations: 50,
    warmup_iterations: 5,
    confidence_level: 0.95,
    parallel: false,
};

let mut runner = BenchmarkRunner::new(config);

// Run application multiple times and collect metrics
for _ in 0..55 {  // 5 warmup + 50 iterations
    let metrics = run_and_collect_metrics().await?;
    runner.add_sample(metrics);
}

// Analyze results
let result = runner.analyze();

println!("{}", format_benchmark_report(&result));

// Check for regression
let baseline = load_baseline_benchmark("baseline.json")?;
if detect_performance_regression(&baseline, &result, 0.2) {
    eprintln!("Performance regression detected!");
    std::process::exit(1);
}
```

#### Statistical Metrics

The benchmark framework provides:

- **Percentiles**: P50 (median), P95, P99 latencies
- **Confidence Intervals**: 95% or 99% CI for mean latency
- **Standard Deviation**: Measure of variability
- **Throughput**: Requests per second
- **Cost Analysis**: Token usage and API costs with statistics
- **Quality Metrics**: Average correctness, relevance, safety scores

#### Regression Detection

Performance regressions are detected using statistical tests:

```rust
// Regression if current P95 exceeds baseline upper CI by threshold
let regression = current.p95_latency > baseline.latency_ci_upper * (1.0 + threshold);
```

Example comparison report:

```
=== Benchmark Comparison ===

P95 Latency:
  Baseline: 1000.00ms (95% CI: [986.00, 1014.00])
  Current:  1050.00ms (95% CI: [1035.00, 1065.00])
  Change:   +5.00% ✅ ACCEPTABLE

Throughput:
  Baseline: 1.00 req/s
  Current:  0.95 req/s
  Change:   -5.00%

Tokens:
  Baseline: 150
  Current:  152
  Change:   +1.33%

Cost:
  Baseline: $0.000750
  Current:  $0.000760
  Change:   +1.33%
```

#### Benefits of Benchmarking

- **Statistical Rigor**: Confidence intervals and percentiles, not single measurements
- **Warmup Phase**: Excludes JIT compilation and cache warming from statistics
- **Regression Detection**: Automated pass/fail based on statistical significance
- **Cost Tracking**: Monitor token usage and API costs across runs
- **Production Insights**: P95/P99 latencies for SLA monitoring

### Programmatic Usage

Use the evals module in Rust code:

```rust
use dashflow_streaming::evals::{
    AnalyticsConverter, Baseline, LlmPricing,
    detect_regressions, has_critical_regressions, RegressionThresholds
};

// Load analytics JSON
let analytics_json = std::fs::read_to_string("analytics.json")?;

// Convert to metrics
let metrics = AnalyticsConverter::from_json(&analytics_json, Some(LlmPricing::GPT_4O))?;

// Load baseline
let baseline = Baseline::load("baselines/app_v1.0.0.json")?;

// Detect regressions
let thresholds = RegressionThresholds::default();
let regressions = detect_regressions(&baseline.metrics, &metrics, &thresholds);

// Check results
if has_critical_regressions(&regressions) {
    eprintln!("Critical regressions detected:");
    for reg in &regressions {
        eprintln!("  {}", reg.format_plain());
    }
    std::process::exit(1);
} else if !regressions.is_empty() {
    println!("Warnings:");
    for reg in &regressions {
        println!("  {}", reg.format_plain());
    }
} else {
    println!("✓ No regressions detected");
}

// Save new baseline
let new_baseline = Baseline::new("app".to_string(), "1.1.0".to_string(), metrics);
new_baseline.save("baselines/app_v1.1.0.json")?;
```

## Testing

DashFlow Streaming includes comprehensive test coverage at multiple levels:

### Unit Tests (427 tests)

Located in `src/`, covering:
- Protocol buffer encoding/decoding (`codec.rs`)
- ZSTD compression/decompression (`compression.rs`)
- Consumer and producer configuration (`consumer.rs`, `producer.rs`)
- State diffing algorithm (`diff.rs`)
- Error handling (`errors.rs`)
- Kafka configuration (`kafka.rs`)
- Evaluation framework (`evals/`)

Run unit tests:
```bash
cargo test --package dashflow-streaming --lib
```

### Mock Integration Tests (20 tests) - NEW!

Located in `tests/mock_integration.rs`, these tests verify DashFlow Streaming functionality **without requiring Kafka**. They enable fast CI/CD testing without external dependencies.

**Coverage:**
- Message serialization/deserialization
- Compression with automatic threshold (512 bytes)
- State diffing and patch application
- Producer configuration validation
- Message sequence ordering
- Streaming token sequences
- Error handling (invalid data, decompression failures)
- Full message lifecycle (GraphStart → NodeStart → NodeEnd → GraphEnd)
- Batch processing (1000 messages)

Run mock integration tests:
```bash
cargo test --package dashflow-streaming --test mock_integration
```

**Key characteristics:**
- **No external dependencies**: Tests run in isolation without Kafka
- **Fast execution**: < 10ms for entire suite
- **CI/CD friendly**: Can run in any environment
- **Comprehensive**: Covers serialization, compression, diffing, and configuration

### Evals Integration Tests (20 tests)

Located in `tests/evals_integration.rs`, covering:
- Eval runner with regression detection
- Quality, performance, cost, and safety metrics
- Baseline comparison and thresholds
- Assertion macros (`assert_no_critical_regressions!`, etc.)

Run evals integration tests:
```bash
cargo test --package dashflow-streaming --test evals_integration
```

### Kafka Integration Tests (5 tests - require Kafka)

Located in `tests/kafka_integration.rs`, these tests require a running Kafka instance:
- Topic management (create, list, delete)
- Producer-consumer roundtrip
- Compression roundtrip
- Partition ordering
- Producer send event

Run Kafka integration tests:
```bash
# Start Kafka
docker-compose -f docker-compose-kafka.yml up -d

# Run tests (marked #[ignore] by default)
cargo test --package dashflow-streaming --test kafka_integration -- --ignored

# Stop Kafka
docker-compose -f docker-compose-kafka.yml down
```

### Testcontainers Integration Tests (15 tests) ⚡

Located in `tests/kafka_testcontainers.rs`, these tests **automatically start Kafka in Docker** and clean up afterward. No manual Kafka setup required!

**Coverage:**
- Producer-consumer roundtrip
- Compression roundtrip
- Partition ordering
- Additional Kafka integration scenarios

Run testcontainers integration tests:
```bash
# On macOS with Colima, set DOCKER_HOST:
export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
cargo test --package dashflow-streaming --test kafka_testcontainers

# Or on systems with standard Docker socket:
cargo test --package dashflow-streaming --test kafka_testcontainers
```

**Key advantages:**
- **Zero manual setup**: Kafka starts automatically in Docker
- **Automatic cleanup**: Containers removed after test completion
- **CI/CD friendly**: No pre-existing Kafka required
- **Isolated**: Each test gets a fresh Kafka instance

### Additional Integration Tests (64 tests)

Located in `tests/`, covering specialized scenarios:
- `decode_error_detection.rs` (7 tests) - Error detection and handling
- `dlq_integration_test.rs` (8 tests) - Dead letter queue integration
- `e2e_integration_test.rs` (5 tests) - End-to-end workflows
- `format_validation_tests.rs` (13 tests) - Message format validation
- `quality_gate_integration_test.rs` (5 tests) - Quality gates
- `redis_integration_test.rs` (7 tests) - Redis backend integration
- `schema_evolution_tests.rs` (10 tests) - Schema versioning
- `smoke_tests.rs` (9 tests) - Basic smoke tests

### Test Summary

| Test Type | Count | External Deps | CI/CD Ready | Run Command |
|-----------|-------|---------------|-------------|-------------|
| Unit tests | 427 | None | ✅ Yes | `cargo test --lib` |
| Mock integration | 20 | None | ✅ Yes | `cargo test --test mock_integration` |
| Evals integration | 20 | None | ✅ Yes | `cargo test --test evals_integration` |
| Additional integration | 64 | Varies | ⚠️ Some | `cargo test --test <name>` |
| Testcontainers | 15 | Docker | ✅ Yes | `cargo test --test kafka_testcontainers` |
| Kafka integration | 5 | Kafka | ⚠️ Manual | `cargo test --test kafka_integration -- --ignored` |
| **Total** | **551** | - | **546/551** | `cargo test` |

### Testing Strategy

**For CI/CD pipelines:**
- Use unit tests + mock integration tests (467 tests, no external dependencies)
- Run with `cargo test --package dashflow-streaming` (excludes ignored Kafka tests)

**For local development:**
- Run all tests including Kafka integration tests
- Requires Docker Compose for Kafka

**For production validation:**
- Use evals integration tests with baseline comparison
- Monitor regressions in quality, performance, and cost metrics

## Protocol Documentation

See `proto/dashstream.proto` for the complete protocol buffer schema.

See `docs/DASHSTREAM_PROTOCOL.md` for protocol design and rationale.

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-streaming --open`
- **[Main Repository](../../README.md)** - Full project documentation
