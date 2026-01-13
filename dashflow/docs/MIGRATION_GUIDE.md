# DashFlow Migration Guide

**Version:** 1.11.3
**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

This guide provides migration paths for all deprecated APIs in DashFlow. Each section explains what changed, why, and how to update your code.

---

## Table of Contents

1. [Graph API Changes (v1.1.0)](#graph-api-changes-v110)
2. [Chat Model Construction (v1.0.1)](#chat-model-construction-v101)
3. [Tool Binding (v1.9.0)](#tool-binding-v190)
4. [Streaming/Codec APIs (v1.1.0, v1.11.0)](#streamingcodec-apis-v110-v1110)
5. [Cost Monitoring (v1.11.3)](#cost-monitoring-v1113)
6. [Trace Types (v1.11.3)](#trace-types-v1113)
7. [Agent APIs (v1.9.0, v1.11.3)](#agent-apis-v190-v1113)
8. [Retrievers (v1.11.0)](#retrievers-v1110)
9. [Self-Improvement Plugins (v1.11.20)](#self-improvement-plugins-v11120)
10. [Streaming Metrics (v1.11.0)](#streaming-metrics-v1110)
11. [Zapier Integration (v1.0.0)](#zapier-integration-v100)
12. [Prometheus Timeout Constants (v1.11.0)](#prometheus-timeout-constants-v1110)

---

## Graph API Changes (v1.1.0)

### `add_conditional_edge` → `add_conditional_edges`

**Deprecated since:** v1.1.0
**Status:** Functional with deprecation warning

The singular form was renamed to plural for API consistency with Python LangGraph.

**Before:**
```rust
graph.add_conditional_edge(
    "router",
    |state| route_condition(state),
    vec!["path_a", "path_b"],
);
```

**After:**
```rust
graph.add_conditional_edges(
    "router",
    |state| route_condition(state),
    vec!["path_a", "path_b"],
);
```

**Quick fix:**
```bash
rg "add_conditional_edge\(" --files-with-matches | xargs sed -i 's/add_conditional_edge(/add_conditional_edges(/g'
```

### `add_parallel_edge` → `add_parallel_edges`

**Deprecated since:** v1.1.0
**Status:** Functional with deprecation warning

**Before:**
```rust
graph.add_parallel_edge("fan_out", vec!["worker_a", "worker_b", "worker_c"]);
```

**After:**
```rust
graph.add_parallel_edges("fan_out", vec!["worker_a", "worker_b", "worker_c"]);
```

**Quick fix:**
```bash
rg "add_parallel_edge\(" --files-with-matches | xargs sed -i 's/add_parallel_edge(/add_parallel_edges(/g'
```

---

## Chat Model Construction (v1.0.1)

### Direct `::new()` Constructors → Config-Driven Instantiation

**Deprecated since:** v1.0.1
**Reason:** Config-driven instantiation is more flexible and production-friendly

All chat model `new()` constructors are deprecated in favor of the `build_chat_model(&config)` pattern.

### OpenAI

**Before:**
```rust
use dashflow_openai::ChatOpenAI;

let model = ChatOpenAI::new()
    .with_model("gpt-4o-mini")
    .with_api_key(api_key);
```

**After:**
```rust
use dashflow_openai::{build_chat_model, ChatOpenAIConfig};

let config = ChatOpenAIConfig {
    model: "gpt-4o-mini".to_string(),
    api_key: Some(api_key),
    ..Default::default()
};
let model = build_chat_model(&config);
```

### Azure OpenAI

**Before:**
```rust
use dashflow_openai::ChatAzureOpenAI;

let model = ChatAzureOpenAI::new()
    .with_deployment("my-deployment")
    .with_api_key(api_key);
```

**After:**
```rust
use dashflow_azure_openai::{build_chat_model, ChatAzureOpenAIConfig};

let config = ChatAzureOpenAIConfig {
    deployment_name: "my-deployment".to_string(),
    api_key: Some(api_key),
    endpoint: endpoint,
    ..Default::default()
};
let model = build_chat_model(&config);
```

### Fireworks

**Before:**
```rust
use dashflow_fireworks::ChatFireworks;

let model = ChatFireworks::new()
    .with_model("accounts/fireworks/models/llama-v3-70b-instruct")
    .with_api_key(api_key);
```

**After:**
```rust
use dashflow_fireworks::{build_chat_model, ChatFireworksConfig};

let config = ChatFireworksConfig {
    model: "accounts/fireworks/models/llama-v3-70b-instruct".to_string(),
    api_key: Some(api_key),
    ..Default::default()
};
let model = build_chat_model(&config);
```

### XAI (Grok)

**Before:**
```rust
use dashflow_xai::ChatXAI;

let model = ChatXAI::new()
    .with_model("grok-beta")
    .with_api_key(api_key);
```

**After:**
```rust
use dashflow_xai::{build_chat_model, ChatXAIConfig};

let config = ChatXAIConfig {
    model: "grok-beta".to_string(),
    api_key: Some(api_key),
    ..Default::default()
};
let model = build_chat_model(&config);
```

### Perplexity

**Before:**
```rust
use dashflow_perplexity::ChatPerplexity;

let model = ChatPerplexity::new()
    .with_model("llama-3.1-sonar-small-128k-online")
    .with_api_key(api_key);
```

**After:**
```rust
use dashflow_perplexity::{build_chat_model, ChatPerplexityConfig};

let config = ChatPerplexityConfig {
    model: "llama-3.1-sonar-small-128k-online".to_string(),
    api_key: Some(api_key),
    ..Default::default()
};
let model = build_chat_model(&config);
```

---

## Tool Binding (v1.9.0)

### Provider-Specific `with_tools()` → Unified `bind_tools()`

**Deprecated since:** v1.9.0
**Reason:** `bind_tools()` provides type-safe, consistent tool binding across all providers

Provider-specific `with_tools()` methods have been deprecated in favor of the unified `bind_tools()` trait method from `ChatModelToolBindingExt`.

**Before:**
```rust
use dashflow_openai::ChatOpenAI;

let model = ChatOpenAI::new()
    .with_model("gpt-4o-mini")
    .with_tools(vec![tool_def]);  // Provider-specific, not type-safe
```

**After:**
```rust
use dashflow::core::language_models::ChatModelToolBindingExt;
use dashflow_openai::{build_chat_model, ChatOpenAIConfig};
use std::sync::Arc;

let config = ChatOpenAIConfig::default();
let model = build_chat_model(&config);

// Type-safe, works consistently across all providers
let bound_model = model.bind_tools(vec![Arc::new(my_tool)], None);
```

**Benefits of `bind_tools()`:**
- Type-safe: Accepts `Arc<dyn Tool>` instead of raw JSON
- Consistent: Same API across OpenAI, Anthropic, Fireworks, Together, etc.
- Extensible: Second parameter for tool choice configuration

**Providers supporting `bind_tools()`:**
- `dashflow-openai`
- `dashflow-anthropic`
- `dashflow-fireworks`
- `dashflow-together`
- `dashflow-xai`

---

## Streaming/Codec APIs (v1.1.0, v1.11.0)

### Message Decoding: Legacy → Strict Mode

**Deprecated since:** v1.1.0
**Reason:** Security - legacy functions accept messages without headers

The following functions accept legacy messages without header validation, which is a security risk for untrusted input:

- `decode_message_with_decompression()`
- `decode_message_with_decompression_and_limit()`

**Before:**
```rust
use dashflow_streaming::codec::decode_message_with_decompression;

let message = decode_message_with_decompression(&bytes)?;
```

**After:**
```rust
use dashflow_streaming::codec::decode_message_strict;

let message = decode_message_strict(&bytes)?;
```

**When to use legacy functions:**
- Migrating from old data where messages lack headers
- Explicitly trusted internal sources

```rust
// If you must use legacy (with documented justification):
#[allow(deprecated)]
use dashflow_streaming::codec::decode_message_with_decompression;
// SAFETY: These messages come from pre-v1.1 internal backup, trusted source
let message = decode_message_with_decompression(&bytes)?;
```

### Kafka Consumer Config: Unused Fields

**Deprecated since:** v1.11.0
**Reason:** rskafka does not support consumer groups

The `group_id` and `session_timeout_ms` fields in `ConsumerConfig` are ignored because rskafka uses partition-based consumption rather than consumer groups.

**Before:**
```rust
use dashflow_streaming::ConsumerConfig;

let config = ConsumerConfig {
    group_id: "my-consumer-group".to_string(),  // Ignored!
    session_timeout_ms: 30000,                   // Ignored!
    ..Default::default()
};
```

**After:**
```rust
use dashflow_streaming::ConsumerConfig;

// For multi-partition topics, create one consumer per partition
let config = ConsumerConfig {
    bootstrap_servers: vec!["localhost:9092".to_string()],
    topic: "my-topic".to_string(),
    partition: 0,  // Explicit partition assignment
    ..Default::default()
};
```

**Multi-partition consumption:**
```rust
// Create consumers for each partition
let partitions = vec![0, 1, 2];
let consumers: Vec<Consumer> = partitions
    .iter()
    .map(|&p| {
        Consumer::new(ConsumerConfig {
            partition: p,
            ..base_config.clone()
        })
    })
    .collect();
```

---

## Cost Monitoring (v1.11.3)

### `optimize::cost_monitoring` → `dashflow_observability::cost`

**Deprecated since:** v1.11.3
**Reason:** Cost monitoring moved to the observability crate for better integration with metrics/tracing

The entire `dashflow::optimize::cost_monitoring` module has been replaced by `dashflow_observability::cost`.

| Old Type | New Type |
|----------|----------|
| `optimize::cost_monitoring::TokenUsage` | `dashflow_observability::cost::TokenUsage` |
| `optimize::cost_monitoring::ModelPrice` | `dashflow_observability::cost::ModelPrice` |
| `optimize::cost_monitoring::ModelPricing` | `dashflow_observability::cost::ModelPricing` |
| `optimize::cost_monitoring::UsageRecord` | `dashflow_observability::cost::CostRecord` |
| `optimize::cost_monitoring::CostReport` | `dashflow_observability::cost::CostReport` |
| `optimize::cost_monitoring::CostMonitor` | `dashflow_observability::cost::CostTracker` |
| `optimize::cost_monitoring::CostMonitorError` | `dashflow_observability::error::Error` (uses `Error::Metrics` variant) |
| `optimize::cost_monitoring::AlertLevel` | `dashflow_observability::cost::AlertLevel` |
| `optimize::cost_monitoring::BudgetConfig` | `dashflow_observability::cost::BudgetConfig` |
| `optimize::cost_monitoring::BudgetEnforcer` | `dashflow_observability::cost::BudgetEnforcer` |

**Before:**
```rust
use dashflow::optimize::cost_monitoring::{
    CostMonitor, TokenUsage, BudgetConfig, BudgetEnforcer
};

let monitor = CostMonitor::new();
let usage = TokenUsage {
    input_tokens: 100,
    output_tokens: 50,
};
monitor.record_usage("gpt-4o", usage).await?;

let enforcer = BudgetEnforcer::new(BudgetConfig::daily(10.0));
```

**After:**
```rust
use dashflow_observability::cost::{
    CostTracker, ModelPricing, Pricing, BudgetConfig, BudgetEnforcer
};

// Create tracker with pricing configuration
let pricing = ModelPricing::new()
    .with_model("gpt-4o", Pricing::per_1m(2.50, 10.00));
let tracker = CostTracker::new(pricing);

// Record usage (synchronous API)
tracker.record_usage("gpt-4o", 100, 50)?;

// Or use comprehensive defaults for all major providers
let tracker = CostTracker::with_defaults();
let enforcer = BudgetEnforcer::new(tracker, BudgetConfig::with_daily_limit(10.0));
```

**Add dependency:**
```toml
[dependencies]
dashflow-observability = "1.11"
```

---

## Trace Types (v1.11.3)

### `optimize::trace` → Introspection Module

**Deprecated since:** v1.11.3
**Reason:** Unified execution tracing in the introspection module

The trace collection types have been consolidated into the introspection module for a unified tracing experience.

| Old Type | New Type |
|----------|----------|
| `optimize::trace_types::TraceEntry` | `ExecutionTrace::to_trace_entries()` |
| `optimize::trace::TraceCollector` | `ExecutionTrace` + `ExecutionTraceBuilder` |
| `optimize::optimizers::simba::TraceStep` | `NodeExecution` from introspection |

**Before:**
```rust
use dashflow::optimize::trace::TraceCollector;
use dashflow::optimize::trace_types::TraceEntry;

let collector = TraceCollector::new(kafka_config)?;
let entries: Vec<TraceEntry> = collector.collect_traces(session_id).await?;
```

**After:**
```rust
use dashflow::introspection::{ExecutionTrace, ExecutionTraceBuilder};

// Building traces locally
let mut builder = ExecutionTraceBuilder::new("my-session");
builder.start_node("classifier");
builder.end_node("classifier", state.clone());
let trace: ExecutionTrace = builder.build();

// Converting to legacy format if needed
let entries = trace.to_trace_entries();

// For Kafka streaming, use DashStream directly:
use dashflow_streaming::Consumer;
// ... consume StateDiff messages from Kafka
```

---

## Agent APIs (v1.9.0, v1.11.3)

### `AgentExecutor` → `create_react_agent()`

**Deprecated since:** v1.9.0
**Reason:** New API integrates with DashFlow features (checkpointing, streaming, human-in-the-loop)

The legacy `AgentExecutor` pattern has been replaced by the more idiomatic `create_react_agent()` function.

**Before:**
```rust
use dashflow::core::agents::{AgentExecutor, ZeroShotAgent};

let agent = ZeroShotAgent::new(model, tools);
let executor = AgentExecutor::new(agent)
    .with_max_iterations(10)
    .with_handle_parsing_errors(true);

let result = executor.run("What is 2+2?").await?;
```

**After:**
```rust
use dashflow::prebuilt::create_react_agent;
use dashflow::graph::GraphConfig;

let agent = create_react_agent(model, tools)?;
let config = GraphConfig::default()
    .with_recursion_limit(10);

let result = agent.invoke(input, Some(config)).await?;
```

### `ZeroShotAgent` / `MRKLAgent` → `ReActAgent`

**Deprecated since:** v1.11.3
**Status:** Type aliases that will be removed in v2.0

These type aliases exist for backwards compatibility but should be replaced with `ReActAgent` directly.

**Before:**
```rust
use dashflow::core::agents::ZeroShotAgent;
// or
use dashflow::core::agents::MRKLAgent;

let agent = ZeroShotAgent::new(model, tools);
```

**After:**
```rust
use dashflow::core::agents::ReActAgent;

let agent = ReActAgent::new(model, tools);
```

**Quick fix:**
```bash
sed -i 's/ZeroShotAgent/ReActAgent/g' src/**/*.rs
sed -i 's/MRKLAgent/ReActAgent/g' src/**/*.rs
```

---

## Retrievers (v1.11.0)

### `PineconeHybridSearchRetriever` → `PineconeVectorStore`

**Deprecated since:** v1.11.0
**Reason:** Hybrid search (vector + BM25) not implemented in Rust Pinecone client

The hybrid search retriever was a stub. Use the vector-only store instead.

**Before:**
```rust
use dashflow::core::retrievers::PineconeHybridSearchRetriever;

let retriever = PineconeHybridSearchRetriever::new(config)?;
```

**After:**
```rust
use dashflow_pinecone::PineconeVectorStore;

let store = PineconeVectorStore::new(config)?;
let retriever = store.as_retriever();
```

### `ElasticsearchBM25Retriever` (old location)

**Deprecated since:** v1.11.0
**Reason:** Moved to dedicated crate

**Before:**
```rust
use dashflow::core::retrievers::ElasticsearchBM25Retriever;
```

**After:**
```rust
use dashflow_elasticsearch::ElasticsearchBM25Retriever;
```

### `WeaviateHybridSearchRetriever` → `WeaviateVectorStore`

**Deprecated since:** v1.11.0
**Reason:** Python baseline deprecated, use Rust implementation

**Before:**
```rust
use dashflow::core::retrievers::WeaviateHybridSearchRetriever;
```

**After:**
```rust
use dashflow_weaviate::WeaviateVectorStore;

let store = WeaviateVectorStore::new(config)?;
let retriever = store.as_retriever();
```

---

## Self-Improvement Plugins (v1.11.20)

### `PluginConfig::to_analyzer_registry()` → `run_all_analyzers()`

**Deprecated since:** v1.11.20
**Reason:** Method always returned empty registry; was never functional

**Before:**
```rust
use dashflow::self_improvement::PluginConfig;

let config = PluginConfig::default();
let registry = config.to_analyzer_registry();  // Always empty!
```

**After:**
```rust
use dashflow::self_improvement::{PluginConfig, run_all_analyzers};

let config = PluginConfig::default();
let results = run_all_analyzers(&config, traces).await?;
```

### `PluginConfig::to_planner_registry()` → `run_planner()`

**Deprecated since:** v1.11.20
**Reason:** Method always returned empty registry; was never functional

**Before:**
```rust
let registry = config.to_planner_registry();  // Always empty!
```

**After:**
```rust
use dashflow::self_improvement::run_planner;

let plan = run_planner(&config, analysis).await?;
```

---

## Streaming Metrics (v1.11.0)

### Process-Local Loss Metrics (M-649)

**Deprecated since:** v1.11.0
**Reason:** Process-local computation meaningless in distributed systems

The following metrics monitor types are deprecated because they track process-local statistics that have no meaning when consumers run across multiple processes:

- `MetricsMonitor::record_message_loss()`
- `MetricsMonitor::get_loss_rate()`
- `MetricsMonitor::reset_loss_counters()`

**Migration:** For loss tracking in distributed systems, use Kafka consumer lag metrics:
```rust
use dashflow_streaming::metrics::consumer_lag;

// External monitoring via Prometheus
// dashflow_kafka_consumer_lag{topic, partition}
```

### Unimplemented Metric Constants (M-697)

**Deprecated since:** v1.11.0
**Reason:** Constants defined but metrics never collected

The following metric constants are deprecated because they were never implemented:

| Constant | Use Instead |
|----------|-------------|
| `METRIC_STREAM_BUFFER_SIZE` | Component-specific buffer metrics |
| `METRIC_STREAM_QUEUE_DEPTH` | `dashflow_observability::queue_depth` |
| `METRIC_CONSUMER_LAG` | `websocket_kafka_consumer_lag` |
| `METRIC_PRODUCER_BATCH_SIZE` | Component-specific batch metrics |

**Migration:** Use the observability crate's metrics or implement component-specific metrics as needed.

---

## Zapier Integration (v1.0.0)

### Entire Crate Deprecated (API Sunset)

**Deprecated since:** v1.0.0
**Status:** Removed from this repository (dead API)
**Reason:** Zapier NLA API was sunset on 2023-11-17

The entire `dashflow-zapier` crate is deprecated because the underlying Zapier NLA API no longer exists.

**See:** https://nla.zapier.com/sunset/

**Affected types:**
- `ZapierNLAWrapper`
- `ZapierAction`
- `ZapierNLAListActions`
- `ZapierNLARunAction`

**Migration:** There is no direct replacement. For workflow automation:
- Use direct API integrations with individual services
- Use `dashflow-langserve` for custom API endpoints
- Use the MCP (Model Context Protocol) tools for extensible integrations

---

## Prometheus Timeout Constants (v1.11.0)

### Module-Specific Constants → Centralized Constants

**Deprecated since:** v1.11.0
**Scheduled for removal:** v2.0
**Reason:** DashFlow now uses centralized timeout constants in `crate::constants` for consistency

The following constants in `prometheus_client` module are deprecated:

| Deprecated Constant | Replacement |
|---------------------|-------------|
| `prometheus_client::DEFAULT_REQUEST_TIMEOUT` | `constants::DEFAULT_HTTP_CONNECT_TIMEOUT` |
| `prometheus_client::DEFAULT_CONNECT_TIMEOUT` | `constants::DEFAULT_HTTP_CONNECT_TIMEOUT` |
| `prometheus_client::DEFAULT_HEALTH_CHECK_TIMEOUT` | `constants::SHORT_TIMEOUT` |

**Before:**
```rust
use dashflow::prometheus_client::DEFAULT_REQUEST_TIMEOUT;

let config = PrometheusClientConfig {
    request_timeout: DEFAULT_REQUEST_TIMEOUT,
    ..Default::default()
};
```

**After:**
```rust
use dashflow::constants::DEFAULT_HTTP_CONNECT_TIMEOUT;

let config = PrometheusClientConfig {
    request_timeout: DEFAULT_HTTP_CONNECT_TIMEOUT,
    ..Default::default()
};
```

**Quick fix:**
```bash
# Replace imports
sed -i 's/prometheus_client::DEFAULT_REQUEST_TIMEOUT/constants::DEFAULT_HTTP_CONNECT_TIMEOUT/g' src/**/*.rs
sed -i 's/prometheus_client::DEFAULT_CONNECT_TIMEOUT/constants::DEFAULT_HTTP_CONNECT_TIMEOUT/g' src/**/*.rs
sed -i 's/prometheus_client::DEFAULT_HEALTH_CHECK_TIMEOUT/constants::SHORT_TIMEOUT/g' src/**/*.rs
```

---

## Automated Migration

### Scanning for Deprecated Usage

Find all deprecated API usage in your codebase:

```bash
# Compile with warnings for deprecated APIs
cargo build 2>&1 | grep -E "deprecated|warning.*since"

# Or use clippy
cargo clippy -- -W deprecated
```

### Find-and-Replace Patterns

```bash
# Graph API (v1.1.0)
sed -i 's/add_conditional_edge(/add_conditional_edges(/g' src/**/*.rs
sed -i 's/add_parallel_edge(/add_parallel_edges(/g' src/**/*.rs

# Cost monitoring imports (v1.11.3)
sed -i 's/dashflow::optimize::cost_monitoring/dashflow_observability::cost/g' src/**/*.rs
sed -i 's/CostMonitor/CostTracker/g' src/**/*.rs
sed -i 's/UsageRecord/CostRecord/g' src/**/*.rs
```

---

## Deprecation Timeline

| API | Deprecated | Will Remove | Migration Effort |
|-----|------------|-------------|------------------|
| `add_conditional_edge` | v1.1.0 | v2.0 | Low (find/replace) |
| `add_parallel_edge` | v1.1.0 | v2.0 | Low (find/replace) |
| `ChatOpenAI::new()` et al. | v1.0.1 | v2.0 | Medium (config pattern) |
| Provider `with_tools()` | v1.9.0 | v2.0 | Medium (trait adoption) |
| Legacy codec functions | v1.1.0 | v2.0 | Low (function rename) |
| `ConsumerConfig.group_id` | v1.11.0 | v2.0 | Low (remove field) |
| `cost_monitoring` module | v1.11.3 | v2.0 | Medium (import changes) |
| Trace types | v1.11.3 | v2.0 | Medium (new types) |
| `AgentExecutor` | v1.9.0 | v2.0 | Medium (API change) |
| `ZeroShotAgent`/`MRKLAgent` | v1.11.3 | v2.0 | Low (find/replace) |
| Hybrid retrievers | v1.11.0 | v2.0 | Medium (crate change) |
| Self-improvement plugins | v1.11.20 | v2.0 | Low (function change) |
| Streaming loss metrics | v1.11.0 | v2.0 | Low (use Prometheus) |
| Prometheus timeout constants | v1.11.0 | v2.0 | Low (constant rename) |
| `dashflow-zapier` | v1.0.0 | Already broken | High (no replacement) |

**Policy:** Deprecated APIs remain functional for at least 2 minor versions or 6 months, whichever is longer. Removal only happens in major versions (v2.0+).

---

## See Also

- [API Stability Policy](API_STABILITY.md) - Our stability guarantees
- [Release Notes v1.11.0](RELEASE_NOTES_v1.11.0.md) - Latest release changes
- [Changelog](../CHANGELOG.md) - Full version history

---

**Author:** DashFlow Core Team
**Questions?** Open an issue at https://github.com/dropbox/dTOOL/dashflow/issues
