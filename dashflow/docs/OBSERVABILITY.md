# DashFlow Observability Guide

**Status:** Active (v1.11+)
**Author:** Andrew Yates © 2026
**Last Updated:** 2026-01-03 (Worker #2400 - Remove stale '(Future)' labels from LLM/Checkpointer metrics)

This guide explains how to use DashFlow's observability features for distributed tracing, monitoring, and performance analysis.

---

## Overview

DashFlow integrates OpenTelemetry for enterprise-grade observability:

- **Distributed Tracing**: Track request flows across graph execution, nodes, checkpoints, and distributed workers
- **Automatic Instrumentation**: Zero-configuration span creation for all graph operations
- **Performance Monitoring**: State size tracking, execution duration, and resource utilization metrics
- **Flexible Backends**: Export to Jaeger, Zipkin, DataDog, New Relic, or custom OTLP endpoints
- **Low Overhead**: < 2% performance impact when no subscriber active (measured via benchmarks)

---

## Quick Start

### 1. Enable the `observability` Feature

Add to your `Cargo.toml`:

```toml
[dependencies]
dashflow = { version = "1.11", features = ["observability"] }
dashflow-observability = "1.11"
```

### 2. Initialize Tracing (Jaeger Example)

```rust
use dashflow_observability::{init_tracing, TracingConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize OpenTelemetry with Jaeger backend
    let config = TracingConfig::new()
        .with_service_name("my-dashflow-app")
        .with_endpoint("http://localhost:4317") // Jaeger OTLP endpoint
        .with_sampling_rate(1.0); // Sample 100% of traces

    init_tracing(config)?;

    // Your graph execution code here
    let result = app.invoke(state).await?;

    // Gracefully shut down tracing to flush all spans
    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}
```

### 3. Run Jaeger Locally (Docker)

```bash
docker run -d \
  --name jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p 4317:4317 \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest
```

Access Jaeger UI at [http://localhost:16686](http://localhost:16686)

### 4. Execute Your Graph

Graph execution is automatically instrumented. No code changes required:

```rust
use dashflow::{StateGraph, END};

let mut graph = StateGraph::new();

graph.add_node_from_fn("researcher", |state| {
    Box::pin(async move {
        // Research logic here
        Ok(state)
    })
});

graph.add_node_from_fn("analyzer", |state| {
    Box::pin(async move {
        // Analysis logic here
        Ok(state)
    })
});

graph.add_edge("researcher", "analyzer");
graph.add_edge("analyzer", END);
graph.set_entry_point("researcher");

let app = graph.compile()?.with_name("research-agent");

// Automatically traced!
let result = app.invoke(initial_state).await?;
```

---

## Span Hierarchy

DashFlow creates the following span hierarchy:

```
graph.invoke                                    (Root span)
├── node.execute [node.name="researcher"]       (Node execution)
│   ├── input_size_bytes: 256
│   └── output_size_bytes: 512
├── node.execute [node.name="analyzer"]
│   ├── input_size_bytes: 512
│   └── output_size_bytes: 768
├── checkpoint.save [node="researcher"]         (Checkpoint operation)
│   ├── thread_id: "session-123"
│   ├── state_size_bytes: 512
│   └── duration_ms: 5
└── checkpoint.save [node="analyzer"]
    ├── thread_id: "session-123"
    ├── state_size_bytes: 768
    └── duration_ms: 6
```

### Parallel Execution Spans

For parallel node execution:

```
graph.invoke
├── scheduler.execute_parallel [node_count=3]
│   ├── execution_type: "local" | "distributed" | "distributed_fallback"
│   ├── duration_ms: 45
│   ├── node.execute [node.name="worker1"]
│   ├── node.execute [node.name="worker2"]
│   └── node.execute [node.name="worker3"]
```

---

## Span Attributes Reference

### `graph.invoke` Span

| Attribute | Type | Description |
|-----------|------|-------------|
| `graph.name` | string | Graph name (set via `.with_name()`) |
| `graph.entry_point` | string | Entry point node name |
| `graph.duration_ms` | u64 | Total execution duration |
| `graph.nodes_executed` | u64 | Number of nodes executed |

### `node.execute` Span

| Attribute | Type | Description |
|-----------|------|-------------|
| `node.name` | string | Node name |
| `input_size_bytes` | u64 | Serialized input state size |
| `output_size_bytes` | u64 | Serialized output state size |

### `checkpoint.save` Span

| Attribute | Type | Description |
|-----------|------|-------------|
| `thread_id` | string | Thread ID for checkpoint isolation |
| `node` | string | Node that triggered checkpoint |
| `state_size_bytes` | u64 | Serialized state size |
| `duration_ms` | u64 | Checkpoint save duration |

### `scheduler.execute_parallel` Span

| Attribute | Type | Description |
|-----------|------|-------------|
| `node_count` | usize | Number of parallel nodes |
| `node_names` | [string] | List of node names |
| `execution_type` | string | "local", "distributed", or "distributed_fallback" |
| `duration_ms` | u64 | Parallel execution duration |

---

## Configuration Options

### TracingConfig

```rust
pub struct TracingConfig {
    service_name: String,
    endpoint: String,
    sampling_rate: f64,
    attributes: HashMap<String, String>,
}

impl TracingConfig {
    pub fn new() -> Self;
    pub fn with_service_name(self, name: impl Into<String>) -> Self;
    pub fn with_endpoint(self, endpoint: impl Into<String>) -> Self;
    pub fn with_sampling_rate(self, rate: f64) -> Self;
    pub fn with_attribute(self, key: impl Into<String>, value: impl Into<String>) -> Self;
}
```

**Example:**

```rust
let config = TracingConfig::new()
    .with_service_name("recommendation-engine")
    .with_endpoint("http://jaeger:4317")
    .with_sampling_rate(0.1) // Sample 10% of traces
    .with_attribute("environment", "production")
    .with_attribute("version", "1.11");

init_tracing(config)?;
```

---

## Backend Integration

### Jaeger (OTLP)

```rust
let config = TracingConfig::new()
    .with_service_name("my-service")
    .with_endpoint("http://localhost:4317");

init_tracing(config)?;
```

### Zipkin

```rust
// Use Zipkin OTLP endpoint
let config = TracingConfig::new()
    .with_service_name("my-service")
    .with_endpoint("http://localhost:9411/api/v2/spans");

init_tracing(config)?;
```

### DataDog

```rust
// DataDog Agent OTLP endpoint
let config = TracingConfig::new()
    .with_service_name("my-service")
    .with_endpoint("http://localhost:4318") // DataDog Agent OTLP HTTP
    .with_attribute("env", "production")
    .with_attribute("service", "dashflow-app");

init_tracing(config)?;
```

### New Relic

```rust
// New Relic OTLP endpoint (requires API key)
let config = TracingConfig::new()
    .with_service_name("my-service")
    .with_endpoint("https://otlp.nr-data.net:4317")
    .with_attribute("api-key", env::var("NEW_RELIC_API_KEY")?);

init_tracing(config)?;
```

### Custom OTLP Endpoint

```rust
let config = TracingConfig::new()
    .with_service_name("my-service")
    .with_endpoint("https://my-telemetry-backend.com:4317");

init_tracing(config)?;
```

---

## Prometheus Metrics

DashFlow provides Prometheus-compatible metrics for real-time monitoring and alerting on graph execution performance.

### Quick Start

**1. Enable the `metrics-server` feature:**

```toml
[dependencies]
dashflow = { version = "1.11", features = ["observability"] }
dashflow-observability = { version = "1.11", features = ["metrics-server"] }
```

**2. Initialize metrics and start HTTP server:**

```rust
use dashflow_observability::metrics::init_default_recorder;
use dashflow_observability::metrics_server::serve_metrics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize metrics recorder
    init_default_recorder()?;

    // Start metrics HTTP server (non-blocking)
    tokio::spawn(async move {
        serve_metrics(9091).await
    });

    // Your graph execution code
    let result = app.invoke(state).await?;

    Ok(())
}
```

**3. Scrape metrics:**

```bash
# Manual scraping
curl http://localhost:9091/metrics

# Health check
curl http://localhost:9091/health
```

### Available Metrics

#### Graph Execution Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `graph_invocations_total` | Counter | `graph_name`, `status` | Total number of graph invocations (status: `success` or `error`) |
| `graph_duration_seconds` | Histogram | `graph_name` | Graph execution duration in seconds (buckets: 1ms to 10s) |
| `graph_active_executions` | Gauge | `graph_name` | Number of currently executing graph instances |

#### Node Execution Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `node_executions_total` | Counter | `graph_name`, `node_name`, `status` | Total number of node executions |
| `node_duration_seconds` | Histogram | `graph_name`, `node_name` | Node execution duration in seconds (buckets: 1ms to 5s) |

#### LLM Call Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `llm_requests_total` | Counter | `provider`, `model`, `status` | Total LLM API requests |
| `llm_tokens_total` | Counter | `provider`, `model`, `token_type` | Total tokens consumed (prompt/completion) |
| `llm_request_duration_seconds` | Histogram | `provider`, `model` | LLM request duration |

#### Checkpointer Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `checkpoint_save_duration_seconds` | Histogram | `checkpointer_type` | Checkpoint save duration |
| `checkpoint_load_duration_seconds` | Histogram | `checkpointer_type` | Checkpoint load duration |
| `checkpoint_size_bytes` | Histogram | `checkpointer_type` | Checkpoint size in bytes |

### Prometheus Configuration

Add this to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'dashflow'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:9091']
```

**Run Prometheus with Docker:**

```bash
docker run -d \
  --name prometheus \
  -p 9090:9090 \
  -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus
```

Access Prometheus UI at [http://localhost:9090](http://localhost:9090)

### Grafana Dashboard (Example Queries)

**Graph Execution Rate:**
```promql
rate(graph_invocations_total[5m])
```

**Graph Execution Duration (p95):**
```promql
histogram_quantile(0.95, rate(graph_duration_seconds_bucket[5m]))
```

**Active Graphs:**
```promql
graph_active_executions
```

**Node Execution Rate by Node:**
```promql
sum(rate(node_executions_total[5m])) by (node_name)
```

**Error Rate:**
```promql
rate(graph_invocations_total{status="error"}[5m])
```

### Example

See `examples/custom_metrics_observability.rs` for a complete working example:

```bash
cargo run --example custom_metrics_observability
```

---

## Cost Tracking

Track LLM API costs across graph execution with automatic cost calculation and reporting.

### Features

- **Per-Model Pricing**: Configure pricing for multiple LLM models
- **Automatic Cost Calculation**: Track input/output token costs per call
- **Cost Attribution**: Break down costs by node and model
- **Cumulative Tracking**: Aggregate costs across multiple graph invocations
- **Flexible Reporting**: Generate detailed cost reports with breakdowns

### Quick Start

```rust
use dashflow_observability::cost::{CostTracker, ModelPricing, Pricing};
use std::sync::{Arc, Mutex};

// Configure pricing for your models
let pricing = ModelPricing::new()
    .with_model("gpt-4", Pricing::new(0.03, 0.06))           // $0.03/$0.06 per 1K tokens
    .with_model("gpt-4-turbo", Pricing::new(0.01, 0.03))     // $0.01/$0.03 per 1K tokens
    .with_model("gpt-3.5-turbo", Pricing::new(0.0005, 0.0015)); // $0.0005/$0.0015 per 1K tokens

// Create cost tracker
let cost_tracker = Arc::new(Mutex::new(CostTracker::new(pricing)));

// Record LLM calls during execution
let cost = cost_tracker.lock().unwrap().record_llm_call(
    "gpt-4",
    1500,  // input tokens
    800,   // output tokens
    Some("researcher_node"),
).unwrap();

println!("Call cost: ${:.4}", cost);

// Generate cost report
let report = cost_tracker.lock().unwrap().report();
println!("{}", report.format());
```

### Default Pricing

Use `ModelPricing::openai_defaults()` for standard OpenAI models:

```rust
let cost_tracker = CostTracker::default(); // Uses OpenAI defaults
```

Includes pricing for:
- `gpt-4`: $0.03 input, $0.06 output per 1K tokens
- `gpt-4-turbo`: $0.01 input, $0.03 output per 1K tokens
- `gpt-3.5-turbo`: $0.0005 input, $0.0015 output per 1K tokens

### Cost Report

The `CostReport` provides multiple views of cost data:

```rust
let report = cost_tracker.lock().unwrap().report();

// Summary statistics
println!("Total Calls: {}", report.total_calls());
println!("Total Cost: ${:.4}", report.total_cost());
println!("Total Tokens: {}", report.total_tokens());
println!("Average Cost/Call: ${:.4}", report.average_cost_per_call());

// Cost breakdown by model
for (model, cost) in report.cost_by_model() {
    println!("  {}: ${:.4}", model, cost);
}

// Cost breakdown by node
for (node, cost) in report.cost_by_node() {
    println!("  {}: ${:.4}", node, cost);
}

// Or use formatted output
println!("{}", report.format());
```

### Example Output

```text
Cost Report
===========
Total Calls: 9
Total Cost: $0.3585
Total Tokens: 21300 (input: 14100, output: 7200)
Average Cost/Call: $0.0398

Cost by Model:
  gpt-4: $0.2520
  gpt-4-turbo: $0.0990
  gpt-3.5-turbo: $0.0075

Cost by Node:
  researcher: $0.2520
  analyzer: $0.0990
  writer: $0.0075
```

### Integration with DashFlow

Pass the cost tracker to your node functions:

```rust
use dashflow::{StateGraph, END};
use std::sync::{Arc, Mutex};

let cost_tracker = Arc::new(Mutex::new(CostTracker::default()));

let mut graph = StateGraph::new();

let tracker_clone = cost_tracker.clone();
graph.add_node_from_fn("researcher", move |state| {
    let tracker = tracker_clone.clone();
    Box::pin(async move {
        // Simulate LLM call
        let response = call_llm("gpt-4", &state.query).await?;

        // Record cost
        tracker.lock().unwrap().record_llm_call(
            "gpt-4",
            response.input_tokens,
            response.output_tokens,
            Some("researcher"),
        ).unwrap();

        Ok(state)
    })
});

// ... add more nodes ...

let app = graph.compile()?;
let result = app.invoke(initial_state).await?;

// Generate final cost report
let report = cost_tracker.lock().unwrap().report();
println!("Final cost: ${:.4}", report.total_cost());
```

### Custom Pricing

Configure pricing for any LLM model:

```rust
let pricing = ModelPricing::new()
    .with_model("claude-3-opus", Pricing::new(0.015, 0.075))
    .with_model("claude-3-sonnet", Pricing::new(0.003, 0.015))
    .with_model("claude-3-haiku", Pricing::new(0.00025, 0.00125))
    .with_model("mistral-large", Pricing::new(0.008, 0.024));

let tracker = CostTracker::new(pricing);
```

### Cost Budget Enforcement

Implement budget limits in your application:

```rust
const MAX_COST: f64 = 1.0; // $1.00 budget

let report = cost_tracker.lock().unwrap().report();
if report.total_cost() > MAX_COST {
    return Err("Budget exceeded!".into());
}
```

### Usage Notes

The cost tracking API demonstrated above supports:
- Multi-node graphs with cost tracking per node
- Different models per node (GPT-4, GPT-4 Turbo, GPT-3.5 Turbo, etc.)
- Cost attribution by node and model
- Comprehensive cost reporting via `CostReport`

See also `examples/apps/librarian/` for a complete application with cost tracking integration.

---

## Performance Impact

### Overhead Measurements

The tracing instrumentation has been benchmarked to verify minimal overhead:

**Run benchmarks:**

```bash
cargo bench --package dashflow tracing_overhead --features observability
```

**Measured Results (no subscriber active):**

Benchmarks measure complete graph execution time with tracing instrumentation present but no subscriber active. This represents the baseline overhead of having instrumentation code compiled in:

| Benchmark | Execution Time | Description |
|-----------|----------------|-------------|
| 5_nodes_no_subscriber | 11.5 µs | 5-node sequential graph with metadata operations |
| parallel_3_workers_no_subscriber | 24.1 µs | 3-worker parallel fan-out/fan-in pattern |
| checkpoint_3_nodes_no_subscriber | 3.6 µs | 3-node sequential with memory checkpointer |

**Key Findings:**

- **Instrumentation overhead**: Negligible when no subscriber active (< 2% estimated)
- **Production impact**: When tracing is enabled with a subscriber, expect < 5% overhead for span export
- **Recommendation**:
  - Development: Sample 100% of traces (`sampling_rate = 1.0`)
  - Production: Use sampling rates 0.1-0.5 for high-traffic systems
  - Critical paths: Disable tracing or use very low sampling rates (0.01-0.05)

---

## Advanced Usage

### Custom Span Attributes

Add custom attributes to graph execution:

```rust
use tracing::Span;

graph.add_node_from_fn("custom_node", |state| {
    Box::pin(async move {
        // Add custom attribute to current span
        Span::current().record("custom_metric", 42);
        Span::current().record("user_id", &state.user_id);

        // Your node logic
        Ok(state)
    })
});
```

### Conditional Sampling

Sample traces based on runtime conditions:

```rust
use tracing::Level;

// Sample all ERROR level spans, 10% of INFO
let config = TracingConfig::new()
    .with_service_name("my-service")
    .with_endpoint("http://localhost:4317")
    .with_sampling_rate(0.1);

init_tracing(config)?;
```

### Distributed Context Propagation

When using distributed workers (via scheduler), trace context is automatically propagated:

```rust
use dashflow::scheduler::WorkStealingScheduler;

let scheduler = WorkStealingScheduler::new()
    .with_workers(vec!["worker1:50051", "worker2:50051"]);

let app = graph.compile()?
    .with_scheduler(scheduler)
    .with_name("distributed-graph");

// Trace context automatically propagated to remote workers
let result = app.invoke(state).await?;
```

Remote workers will continue the trace, creating child spans visible in Jaeger/Zipkin.

---

## Troubleshooting

### No Spans Appearing in Jaeger

1. **Check Jaeger is running:**
   ```bash
   curl http://localhost:4317
   ```

2. **Verify OTLP is enabled:**
   Ensure Jaeger was started with `COLLECTOR_OTLP_ENABLED=true`

3. **Check initialization:**
   ```rust
   match init_tracing(config) {
       Ok(_) => println!("Tracing initialized successfully"),
       Err(e) => eprintln!("Tracing init failed: {}", e),
   }
   ```

4. **Flush on shutdown:**
   ```rust
   opentelemetry::global::shutdown_tracer_provider(); // Flush all spans
   ```

### High Memory Usage

**Issue:** Memory grows with long-running traces

**Solution:** Use sampling or reduce span attribute verbosity

```rust
// Sample 10% of traces
let config = TracingConfig::new()
    .with_sampling_rate(0.1);
```

### Performance Degradation

**Issue:** Application slower with tracing enabled

**Diagnosis:** Run overhead benchmarks:

```bash
cargo bench --package dashflow tracing_overhead
```

**Solutions:**

1. **Lower sampling rate:**
   ```rust
   .with_sampling_rate(0.01) // 1% sampling
   ```

2. **Use async export** (default in dashflow-observability)

3. **Disable tracing in hot paths** (use `#[cfg(feature = "observability")]`)

---

## Examples

### Complete Example: Traced Agent Workflow

See `examples/traced_agent.rs`:

```rust
use dashflow::{StateGraph, END};
use dashflow_observability::{init_tracing, TracingConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    let config = TracingConfig::new()
        .with_service_name("research-agent")
        .with_endpoint("http://localhost:4317")
        .with_sampling_rate(1.0);

    init_tracing(config)?;

    // Build graph
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("researcher", researcher_fn);
    graph.add_node_from_fn("analyzer", analyzer_fn);
    graph.add_node_from_fn("writer", writer_fn);

    graph.add_edge("researcher", "analyzer");
    graph.add_edge("analyzer", "writer");
    graph.add_edge("writer", END);
    graph.set_entry_point("researcher");

    let app = graph.compile()?.with_name("research-agent");

    // Execute (automatically traced)
    let result = app.invoke(initial_state).await?;

    println!("Research complete! View trace at http://localhost:16686");

    // Flush spans
    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}
```

**Run example:**

```bash
cargo run --example traced_agent --features observability
```

---

## Best Practices

1. **Name your graphs** - Use `.with_name()` for better trace filtering:
   ```rust
   let app = graph.compile()?.with_name("recommendation-engine");
   ```

2. **Use sampling in production** - Avoid tracing 100% of requests:
   ```rust
   .with_sampling_rate(0.1) // 10% sampling
   ```

3. **Add custom attributes** - Include business context:
   ```rust
   Span::current().record("user_id", user_id);
   Span::current().record("request_type", "premium");
   ```

4. **Flush on shutdown** - Always call shutdown to ensure spans are exported:
   ```rust
   opentelemetry::global::shutdown_tracer_provider();
   ```

5. **Monitor overhead** - Run benchmarks periodically:
   ```bash
   cargo bench --package dashflow tracing_overhead
   ```

6. **Use feature flags** - Make observability optional:
   ```toml
   [features]
   default = []
   observability = ["dashflow-observability"]
   ```

---

## Related Documentation

- **OpenTelemetry Integration** - Built into the dashflow observability infrastructure
- **OpenTelemetry Docs:** [https://opentelemetry.io/docs/](https://opentelemetry.io/docs/)
- **Jaeger Docs:** [https://www.jaegertracing.io/docs/](https://www.jaegertracing.io/docs/)
- **Tracing Crate:** [https://docs.rs/tracing/](https://docs.rs/tracing/)

---

## Completed Features

All major observability features have been implemented:

- ✅ **Prometheus Metrics** - See [Prometheus Metrics](#prometheus-metrics) section above
- ✅ **Cost Tracking** - See [Cost Tracking](#cost-tracking) section above
- ✅ **Redis/DynamoDB Checkpointers** - Both have full tracing integration (`dashflow-redis-checkpointer`, `dashflow-dynamodb-checkpointer`)
- ✅ **Comprehensive Observability Proof** - See `cargo test -p codex-dashflow --test comprehensive_observability_test -- --ignored --nocapture`

---

**Questions?** File an issue at [github.com/dropbox/dTOOL/dashflow](https://github.com/dropbox/dTOOL/dashflow)
