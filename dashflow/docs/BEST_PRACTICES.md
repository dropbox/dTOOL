# DashFlow Best Practices

**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)

Production-ready patterns and recommendations for building reliable LLM applications with DashFlow.

## Table of Contents

1. [Error Handling](#error-handling)
2. [Performance Optimization](#performance-optimization)
3. [Security](#security)
4. [Testing](#testing)
5. [Deployment](#deployment)
6. [Advanced DashFlow Patterns](#advanced-dashflow-patterns)

## Error Handling

### Use Result Types Consistently

```rust
use anyhow::Result;

async fn generate_response(prompt: &str) -> Result<String> {
    let chat = ChatOpenAI::new().with_model("gpt-4o-mini");
    let messages = vec![Message::human(prompt)];
    let result = chat.generate(&messages, None, None).await?;
    Ok(result.generations[0].message.as_text().to_string())
}
```

### Handle API Failures Gracefully

```rust
use dashflow::core::runnable::Runnable;

// Use built-in retry with defaults (3 retries, exponential backoff with jitter)
let retryable_chat = chat.with_retry(None, None, None, None, None, None);
let result = retryable_chat.invoke(messages, None).await?;

// Or customize retry settings
let retryable_chat = chat.with_retry(
    Some(5),      // max_retries
    Some(true),   // wait_exponential_jitter
    Some(100),    // initial_delay_ms
    Some(10000),  // max_delay_ms
    Some(2.0),    // exp_base
    Some(1000),   // jitter_ms
);
let result = retryable_chat.invoke(messages, None).await?;
```

### Validate Inputs

```rust
fn validate_prompt(prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() {
        return Err(anyhow!("Prompt cannot be empty"));
    }
    if prompt.len() > 100_000 {
        return Err(anyhow!("Prompt exceeds maximum length"));
    }
    Ok(())
}
```

## Performance Optimization

### Use Arc for Shared State

```rust
use std::sync::Arc;

// ✅ Correct - share embeddings across threads
let embeddings = Arc::new(OpenAIEmbeddings::new());
let vectorstore1 = ChromaVectorStore::new("http://localhost:8000", embeddings.clone());
let vectorstore2 = ChromaVectorStore::new("http://localhost:8000", embeddings.clone());
```

### Batch Operations

```rust
// ❌ Inefficient - one API call per text
for text in texts {
    let embedding = embeddings.embed_query(&text).await?;
}

// ✅ Efficient - single API call
let batch_embeddings = embeddings.embed_documents(&texts).await?;
```

### Use Release Builds

```bash
# Development (faster compilation, slower runtime)
cargo build

# Production (slower compilation, 10-100× faster runtime)
cargo build --release
```

### Reuse HTTP Clients

```rust
// ✅ Correct - reuse client
let chat = ChatOpenAI::new(); // Creates client once
for prompt in prompts {
    let result = chat.generate(&[Message::human(prompt)], None, None).await?;
}
```

### Parallel Processing

```rust
use futures::future::join_all;
use tokio::task;

// Process multiple prompts concurrently
let tasks: Vec<_> = prompts
    .iter()
    .map(|prompt| {
        let chat = chat.clone();
        let prompt = prompt.clone();
        task::spawn(async move {
            let messages = vec![Message::human(&prompt)];
            chat.generate(&messages, None, None).await
        })
    })
    .collect();

let results = join_all(tasks).await;
```

## Security

### Never Hardcode API Keys

```rust
// ❌ Wrong
let chat = ChatOpenAI::new_with_api_key("sk-proj-...");

// ✅ Correct - use environment variables
use std::env;
dotenv::dotenv().ok();
let api_key = env::var("OPENAI_API_KEY")?;
let chat = ChatOpenAI::new_with_api_key(&api_key);

// ✅ Better - let library read from env
let chat = ChatOpenAI::new(); // Reads OPENAI_API_KEY automatically
```

### Sanitize User Inputs

```rust
fn sanitize_input(input: &str) -> String {
    // Remove potential injection attempts
    input
        .replace("</s>", "")  // Remove special tokens
        .replace("<|endoftext|>", "")
        .trim()
        .to_string()
}

let user_input = sanitize_input(&raw_input);
let messages = vec![Message::human(&user_input)];
```

### Rate Limiting

```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

// Limit to 10 requests per second
let limiter = RateLimiter::direct(
    Quota::per_second(NonZeroU32::new(10).unwrap())
);

// Before each API call
limiter.until_ready().await;
let result = chat.generate(&messages, None, None).await?;
```

### Timeout Long Operations

```rust
use tokio::time::{timeout, Duration};

let result = timeout(
    Duration::from_secs(30),
    chat.generate(&messages, None, None)
).await??;
```

## Testing

### Unit Tests for Core Logic

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prompt_validation() {
        assert!(validate_prompt("Valid prompt").is_ok());
        assert!(validate_prompt("").is_err());
        assert!(validate_prompt(&"x".repeat(100_001)).is_err());
    }

    #[tokio::test]
    async fn test_message_creation() {
        let msg = Message::human("test");
        assert_eq!(msg.content, "test");
    }
}
```

### Integration Tests with Mocking

```rust
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_llm_integration() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"choices":[{"message":{"content":"test"}}]}"#)
        )
        .mount(&mock_server)
        .await;

    let chat = ChatOpenAI::new()
        .with_base_url(&mock_server.uri());

    let messages = vec![Message::human("test")];
    let result = chat.generate(&messages, None, None).await;

    assert!(result.is_ok());
}
```

### Running Workspace Tests

For large workspaces (100+ crates), avoid `cargo test --workspace` due to runner timeouts:

```bash
# ❌ May timeout on large workspaces
cargo test --workspace

# ✅ Test by crate for faster feedback
cargo test -p dashflow
cargo test -p dashflow-evals
cargo test -p dashflow-openai

# ✅ Test specific functionality
cargo test multi_model
cargo test checkpoint

# ✅ Run ignored integration tests separately (require API keys)
cargo test -p dashflow-evals -- --ignored
```

**For parallel execution**, use `cargo-nextest`:

```bash
# Install nextest (one-time)
cargo install cargo-nextest

# Run all tests in parallel
cargo nextest run --workspace

# Run with output
cargo nextest run -p dashflow-evals --nocapture
```

**Why:** Cumulative test execution time across 100+ crates can trigger runner timeouts (SIGKILL).
**Benefit:** Faster feedback, isolated failures, better parallelization.

**See Also:** [Integration Testing Guide](./INTEGRATION_TESTING.md) - Comprehensive patterns for testcontainers and mock servers.

### Benchmark Critical Paths

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_embedding(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let embeddings = OpenAIEmbeddings::new();

    c.bench_function("embed_query", |b| {
        b.to_async(&rt).iter(|| async {
            embeddings.embed_query(black_box("test")).await
        });
    });
}

criterion_group!(benches, benchmark_embedding);
criterion_main!(benches);
```

## Deployment

### Environment Configuration

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    openai_api_key: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
}

fn load_config() -> Result<Config> {
    dotenv::dotenv().ok();
    envy::from_env::<Config>()
        .context("Failed to load configuration")
}
```

### Structured Logging

```rust
use tracing::{info, error, instrument};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = run_app().await {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}

#[instrument]
async fn generate_response(prompt: &str) -> Result<String> {
    info!("Generating response for prompt");
    let chat = ChatOpenAI::new();
    // ... implementation
}
```

### Health Checks

```rust
use axum::{routing::get, Router, Json};
use serde_json::json;

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

let app = Router::new()
    .route("/health", get(health_check))
    .route("/api/generate", post(generate_endpoint));
```

### Graceful Shutdown

```rust
use tokio::signal;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Received shutdown signal");
}
```

### Docker Deployment

```dockerfile
# Multi-stage build for minimal image size
FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/my-app /usr/local/bin/
CMD ["my-app"]
```

### Monitoring with LangSmith

```rust
use dashflow_langsmith::Client;

let client = Client::builder()
    .api_key(&std::env::var("LANGSMITH_API_KEY")?)
    .build()?;

// Use the client to create runs manually
// Note: Automatic callback integration is not yet available
```

### Production Observability

#### Prometheus Metrics

Expose application metrics in Prometheus format for monitoring and alerting:

```rust
use prometheus::{Registry, Counter, Histogram, HistogramOpts};
use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder};

// Create metrics
let registry = Registry::new();
let requests_total = Counter::new("requests_total", "Total requests")?;
let request_duration = Histogram::with_opts(
    HistogramOpts::new("request_duration_seconds", "Request latency")
        .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0])
)?;

registry.register(Box::new(requests_total.clone()))?;
registry.register(Box::new(request_duration.clone()))?;

// Metrics endpoint
async fn metrics_handler(registry: Registry) -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

let app = Router::new()
    .route("/metrics", get(metrics_handler))
    .route("/health", get(health_check));
```

**Key Metrics to Track:**
- **Request rate**: `requests_total` (counter)
- **Latency**: `request_duration_seconds` (histogram with p50, p95, p99)
- **Error rate**: `errors_total{error_type}` (counter)
- **Token usage**: `llm_tokens_total` (counter)
- **Cache hit rate**: `cache_hits_total / cache_requests_total` (ratio)

#### OpenTelemetry Distributed Tracing

Set up distributed tracing with OpenTelemetry for request flow visibility:

```rust
use opentelemetry::{global, sdk::trace::TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, Registry};
use tracing_opentelemetry;

// Initialize OpenTelemetry
let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
    .unwrap_or_else(|_| "http://localhost:4317".to_string());

let tracer = opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_exporter(
        opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(otlp_endpoint)
    )
    .install_batch(opentelemetry::runtime::Tokio)?;

// Integrate with tracing
let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
let subscriber = Registry::default()
    .with(tracing_subscriber::fmt::layer())
    .with(telemetry);

tracing::subscriber::set_global_default(subscriber)?;

// Use with tracing macros
#[tracing::instrument]
async fn process_request(query: &str) -> Result<String> {
    tracing::info!("Processing query: {}", query);
    // Spans automatically created and propagated
    let result = llm_call(query).await?;
    tracing::info!("Query processed successfully");
    Ok(result)
}
```

**Benefits:**
- ✅ End-to-end request flow visibility
- ✅ Automatic span creation from `#[instrument]` macro
- ✅ Async context propagation across tasks
- ✅ Integration with Jaeger, Zipkin, or Honeycomb
- ✅ Zero overhead when `OTEL_EXPORTER_OTLP_ENDPOINT` not set

#### Observability Stack Setup

Use Docker Compose for local development observability:

```yaml
# docker-compose.yml
version: '3.8'
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # Jaeger UI
      - "4317:4317"    # OTLP gRPC receiver
    environment:
      - COLLECTOR_OTLP_ENABLED=true

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
```

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'dashflow-app'
    static_configs:
      - targets: ['host.docker.internal:9090']
```

**Access:**
- Jaeger UI: http://localhost:16686 (traces)
- Prometheus: http://localhost:9090 (metrics)
- Grafana: http://localhost:3000 (dashboards, login: admin/admin)

**Production Deployment:**
- Use managed services (Datadog, New Relic, Honeycomb)
- Configure sampling (e.g., 10% of traces) for cost control
- Set up alerting rules (error rate > 5%, latency p95 > 10s)
- Implement log aggregation (ELK Stack, Loki, CloudWatch)

**See Also:**
- [Observability Guide](../examples/apps/README_OBSERVABILITY.md) - Complete setup with pre-configured dashboards
- [Production Deployment Guide](PRODUCTION_DEPLOYMENT.md) - Full deployment patterns

## Resource Management

### Connection Pooling

```rust
// Vector store clients typically use connection pools internally
let vectorstore = PgVectorStore::new(&config.database_url).await?;
// Connection pool managed automatically
```

### Limit Concurrent Requests

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

let semaphore = Arc::new(Semaphore::new(10)); // Max 10 concurrent

for prompt in prompts {
    let permit = semaphore.clone().acquire_owned().await?;
    let chat = chat.clone();

    tokio::spawn(async move {
        let _permit = permit; // Hold permit until task completes
        let result = chat.generate(&[Message::human(&prompt)], None, None).await;
        // Permit automatically released when dropped
    });
}
```

### Memory Management

```rust
// Use streaming for large responses
let mut stream = chat.stream(&messages, None, None, None, None).await?;
while let Some(chunk) = stream.next().await {
    // Process chunk immediately, don't accumulate in memory
    process_chunk(chunk?).await?;
}
```

## Advanced DashFlow Patterns

### Implementing MergeableState for Parallel Execution

When using parallel node execution in DashFlow, implement `MergeableState` to correctly combine state from parallel branches:

```rust
use dashflow::MergeableState;
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ResearchState {
    query: String,
    findings: Vec<Finding>,
    insights: Vec<Insight>,
    agent_outputs: HashMap<String, serde_json::Value>,
    errors: Vec<String>,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        // Merge findings from parallel branches
        self.findings.extend(other.findings.clone());
        self.insights.extend(other.insights.clone());

        // Merge agent outputs
        for (key, value) in &other.agent_outputs {
            self.agent_outputs.insert(key.clone(), value.clone());
        }

        // Merge errors
        self.errors.extend(other.errors.clone());
    }
}
```

**Key Points:**
- Merge collections additively (findings, errors, etc.)
- For maps, decide merge strategy (overwrite, keep first, or custom logic)
- Handle optional fields carefully (prefer non-empty values)
- Test merge logic with realistic parallel execution scenarios

### Circuit Breaker Pattern for Error Recovery

Implement circuit breakers to prevent cascading failures when external services are unavailable:

```rust
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Stop calling service
    HalfOpen, // Test if service recovered
}

async fn fetch_with_circuit_breaker(
    mut state: ErrorRecoveryState
) -> Result<ErrorRecoveryState> {
    let service_name = "external_api";
    let breaker_state = state.circuit_breaker_states
        .get(service_name)
        .unwrap_or(&CircuitBreakerState::Closed);

    match breaker_state {
        CircuitBreakerState::Open => {
            // Service is down, use fallback
            state.fallback_used.insert(service_name.to_string(), true);
            return use_cached_data(state).await;
        }
        CircuitBreakerState::HalfOpen => {
            // Test if service recovered
            match try_service_call().await {
                Ok(data) => {
                    // Success! Close circuit breaker
                    state.circuit_breaker_states.insert(
                        service_name.to_string(),
                        CircuitBreakerState::Closed
                    );
                }
                Err(_) => {
                    // Still failing, reopen circuit
                    state.circuit_breaker_states.insert(
                        service_name.to_string(),
                        CircuitBreakerState::Open
                    );
                }
            }
        }
        CircuitBreakerState::Closed => {
            // Normal operation
            match try_service_call().await {
                Ok(data) => state.fetched_data = Some(data),
                Err(e) => {
                    // Increment failure count
                    let retry_count = state.retry_counts
                        .entry(service_name.to_string())
                        .or_insert(0);
                    *retry_count += 1;

                    // Open circuit after N failures
                    if *retry_count >= 3 {
                        state.circuit_breaker_states.insert(
                            service_name.to_string(),
                            CircuitBreakerState::Open
                        );
                    }
                }
            }
        }
    }

    Ok(state)
}
```

**Benefits:**
- Prevents cascading failures when services are down
- Automatic recovery testing with HalfOpen state
- Fail fast instead of wasting resources on doomed requests
- Graceful degradation with fallback mechanisms

### Dead Letter Queue for Unrecoverable Failures

Capture items that cannot be processed after retries for later investigation:

```rust
#[derive(Clone, Serialize, Deserialize, Debug)]
struct DeadLetterItem {
    item_id: String,
    error_type: ErrorType,
    error_message: String,
    timestamp: i64,
    original_data: serde_json::Value,
}

async fn process_with_dlq(mut state: ErrorRecoveryState) -> Result<ErrorRecoveryState> {
    let max_retries = 3;

    for item in state.validated_data.unwrap_or_default() {
        let retry_count = state.retry_counts
            .get(&item.id)
            .copied()
            .unwrap_or(0);

        if retry_count >= max_retries {
            // Send to dead letter queue
            state.dead_letter_items.push(DeadLetterItem {
                item_id: item.id.clone(),
                error_type: ErrorType::ProcessingTimeout,
                error_message: "Exceeded max retries".to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                original_data: serde_json::to_value(&item)?,
            });
            continue;
        }

        match process_item(&item).await {
            Ok(result) => {
                state.processed_data.get_or_insert_with(Vec::new).push(result);
            }
            Err(e) => {
                *state.retry_counts.entry(item.id.clone()).or_insert(0) += 1;
            }
        }
    }

    Ok(state)
}
```

**Benefits:**
- Prevents data loss for items that fail processing
- Enables offline investigation and reprocessing
- Separates transient failures from permanent failures
- Maintains pipeline throughput for successful items

### Conditional Routing Based on State

Use conditional edges to implement dynamic workflow routing:

```rust
use dashflow::{StateGraph, END};

fn should_retry(state: &ErrorRecoveryState) -> &'static str {
    if let Some(last_error) = &state.last_error {
        // Check if error is retryable
        let is_retryable = matches!(
            last_error.error_type,
            ErrorType::NetworkTimeout | ErrorType::RateLimited
        );

        let retry_count = state.retry_counts
            .get(&last_error.node_name)
            .copied()
            .unwrap_or(0);

        if is_retryable && retry_count < 3 {
            return &last_error.node_name; // Retry the failed node
        }
    }

    if state.dead_letter_items.is_empty() {
        "write" // Continue to write node
    } else {
        END // End if we have unrecoverable failures
    }
}

// Build graph with conditional routing
let graph = StateGraph::new()
    .add_node("fetch", fetch_node)
    .add_node("validate", validate_node)
    .add_node("process", process_node)
    .add_conditional_edges("process", should_retry, vec!["fetch", "validate", "process", "write", END])
    .compile()?;
```

**Key Patterns:**
- Examine error types to determine if retry is appropriate
- Track retry counts to prevent infinite loops
- Route to different recovery strategies based on error classification
- Use END node for terminal error states

### Iterative Refinement with Quality Evaluation

Implement feedback loops to iteratively improve outputs:

```rust
#[derive(Clone, Serialize, Deserialize, Debug)]
struct ResearchState {
    query: String,
    final_report: String,
    iteration: u32,
    max_iterations: u32,
    quality_score: f64,
    quality_threshold: f64,
    feedback: String,
}

async fn evaluate_quality(mut state: ResearchState) -> Result<ResearchState> {
    // Evaluate report quality (simplified - use LLM in production)
    let quality_score = calculate_quality_score(&state.final_report)?;
    state.quality_score = quality_score;

    if quality_score < state.quality_threshold && state.iteration < state.max_iterations {
        // Generate feedback for improvement
        state.feedback = format!(
            "Quality score {:.2} below threshold {:.2}. Needs improvement.",
            quality_score,
            state.quality_threshold
        );
        state.iteration += 1;
    }

    Ok(state)
}

fn should_refine(state: &ResearchState) -> &'static str {
    if state.quality_score < state.quality_threshold
        && state.iteration < state.max_iterations {
        "writer" // Regenerate report with feedback
    } else {
        END // Quality sufficient or max iterations reached
    }
}

// Build graph with refinement loop
let graph = StateGraph::new()
    .add_node("orchestrator", orchestrator_node)
    .add_node("researcher", researcher_node)
    .add_node("writer", writer_node)
    .add_node("quality_evaluator", evaluate_quality)
    .add_edge("orchestrator", "researcher")
    .add_edge("researcher", "writer")
    .add_edge("writer", "quality_evaluator")
    .add_conditional_edges("quality_evaluator", should_refine, vec!["writer", END])
    .compile()?;
```

**Benefits:**
- Automatically improve outputs without manual intervention
- Bounded iteration with `max_iterations` prevents infinite loops
- Feedback mechanism guides improvement
- Quantitative quality thresholds enable objective evaluation

**See also:** Production examples at `examples/apps/librarian` (production RAG paragon demonstrating best practices). See [docs/EXAMPLE_APPS.md](EXAMPLE_APPS.md) for details.

## Production Checklist

**Security & Configuration:**
- [ ] All API keys loaded from environment variables
- [ ] Input validation on all user inputs
- [ ] Secrets management configured (vault, AWS Secrets Manager)
- [ ] Release build used (`--release` flag)

**Reliability:**
- [ ] Retry logic implemented for API calls
- [ ] Timeout configured for all external calls
- [ ] Rate limiting in place
- [ ] Circuit breakers configured for external services
- [ ] Dead letter queue for unrecoverable failures
- [ ] Graceful shutdown handling

**Observability:**
- [ ] Structured logging configured (JSON format)
- [ ] Prometheus metrics endpoint (`/metrics`)
- [ ] OpenTelemetry distributed tracing enabled
- [ ] Health check endpoint implemented (`/health`)
- [ ] Error monitoring (Sentry, DataDog, etc.)
- [ ] LangSmith tracing enabled (optional)
- [ ] Alerting rules configured (error rate, latency)

**Testing & Quality:**
- [ ] Integration tests passing
- [ ] Load testing completed
- [ ] Quality thresholds defined and tested
- [ ] MergeableState implemented for parallel nodes

**Deployment:**
- [ ] Docker image < 50 MB
- [ ] Multi-stage Dockerfile for minimal size
- [ ] Container health checks configured
- [ ] Resource limits set (CPU, memory)

## Additional Resources

- [Getting Started Guide](../QUICKSTART.md)
- [Golden Path Guide](GOLDEN_PATH.md) - Recommended API patterns
- [Embedding Provider Comparison](EMBEDDING_PROVIDERS_COMPARISON.md)
- [Architecture Guide](ARCHITECTURE.md)

---

**Questions?** Check the [examples/](../examples/) directory for production-ready code samples!

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
