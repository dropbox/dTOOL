# Distributed Tracing Guide

**Last Updated:** 2026-01-03 (Worker #2361 - Document internal architecture guide)

**DashFlow v1.11**

This guide covers distributed tracing setup with OpenTelemetry, Jaeger, and LangSmith for comprehensive observability in production environments.

---

## Table of Contents

1. [Overview](#overview)
2. [OpenTelemetry Integration](#opentelemetry-integration)
3. [Jaeger Setup](#jaeger-setup)
4. [LangSmith Integration](#langsmith-integration)
5. [W3C Trace Context](#w3c-trace-context)
6. [Distributed Tracing Patterns](#distributed-tracing-patterns)
7. [Log Aggregation](#log-aggregation)
8. [Best Practices](#best-practices)
9. [Troubleshooting](#troubleshooting)

---

## Overview

Distributed tracing allows you to track requests across multiple services, understand latency bottlenecks, and debug complex LLM workflows. DashFlow provides first-class support for:

- **OpenTelemetry**: Industry-standard distributed tracing
- **W3C Trace Context**: Standard trace propagation across services
- **Jaeger**: Open-source tracing backend
- **LangSmith**: DashFlow-specific tracing and debugging
- **Structured Logging**: JSON-formatted logs with trace context

### Key Benefits

1. **End-to-end Visibility**: Track LLM calls, tool executions, and chain operations
2. **Performance Analysis**: Identify slow components and optimization opportunities
3. **Error Debugging**: Correlate errors with specific traces and spans
4. **Cost Tracking**: Monitor token usage and API costs per request
5. **Production Debugging**: Investigate issues with full request context

---

## OpenTelemetry Integration

### Installation

Add OpenTelemetry dependencies to your `Cargo.toml`:

```toml
[dependencies]
opentelemetry = "0.22"
opentelemetry-otlp = "0.15"
opentelemetry-semantic-conventions = "0.14"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-opentelemetry = "0.23"
```

### Basic Setup

```rust
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};
use opentelemetry_semantic_conventions as semcov;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    // Configure OpenTelemetry tracer
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://jaeger:4317"),
        )
        .with_trace_config(
            sdktrace::config().with_resource(Resource::new(vec![
                semcov::resource::SERVICE_NAME.string("dashflow-app"),
                semcov::resource::SERVICE_VERSION.string(env!("CARGO_PKG_VERSION")),
            ])),
        )
        .install_batch(runtime::Tokio)?;

    // Configure tracing subscriber
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    init_tracing()?;

    // Your application code here
    run_app().await?;

    // Shutdown tracer to flush remaining spans
    global::shutdown_tracer_provider();

    Ok(())
}
```

### Instrumenting Your Code

#### Automatic Instrumentation

Use the `#[tracing::instrument]` attribute for automatic span creation:

```rust
use tracing::instrument;

#[instrument(skip(chat_model), fields(model = %chat_model.model_name()))]
async fn call_llm(
    chat_model: &ChatOpenAI,
    messages: Vec<Message>,
) -> Result<String> {
    // Automatically creates a span named "call_llm"
    // with the model name as a field
    let response = chat_model.generate(&messages, None).await?;
    Ok(response.content)
}
```

#### Manual Span Creation

For finer control, create spans manually:

```rust
use tracing::{info, error, span, Level};

async fn process_chain(input: &str) -> Result<String> {
    let span = span!(Level::INFO, "process_chain", input_length = input.len());
    let _guard = span.enter();

    info!("Starting chain processing");

    // Your processing logic
    match run_chain(input).await {
        Ok(result) => {
            info!(result_length = result.len(), "Chain completed successfully");
            Ok(result)
        }
        Err(e) => {
            error!(error = %e, "Chain processing failed");
            Err(e)
        }
    }
}
```

#### Span Attributes

Add custom attributes to spans for better debugging:

```rust
use opentelemetry::trace::{Span, Tracer};
use opentelemetry::global;

async fn call_openai(prompt: &str, model: &str) -> Result<String> {
    let tracer = global::tracer("dashflow");
    let mut span = tracer.start("openai_call");

    // Add attributes
    span.set_attribute(semcov::trace::LLM_SYSTEM.string("openai"));
    span.set_attribute(semcov::trace::LLM_REQUEST_MODEL.string(model));
    span.set_attribute(semcov::trace::LLM_REQUEST_MAX_TOKENS.i64(1000));
    span.set_attribute(KeyValue::new("prompt_length", prompt.len() as i64));

    // Make API call
    let response = api_call(prompt, model).await?;

    // Record response attributes
    span.set_attribute(semcov::trace::LLM_RESPONSE_MODEL.string(&response.model));
    span.set_attribute(semcov::trace::LLM_USAGE_PROMPT_TOKENS.i64(response.usage.prompt_tokens as i64));
    span.set_attribute(semcov::trace::LLM_USAGE_COMPLETION_TOKENS.i64(response.usage.completion_tokens as i64));

    Ok(response.content)
}
```

### Environment Variables

Configure OpenTelemetry via environment variables:

```bash
# Service identification
export OTEL_SERVICE_NAME="dashflow-app"
export OTEL_SERVICE_VERSION="1.11"

# OTLP exporter endpoint
export OTEL_EXPORTER_OTLP_ENDPOINT="http://jaeger:4317"
export OTEL_EXPORTER_OTLP_PROTOCOL="grpc"

# Sampling (use for production to reduce overhead)
export OTEL_TRACES_SAMPLER="parentbased_traceidratio"
export OTEL_TRACES_SAMPLER_ARG="0.1"  # Sample 10% of traces

# Resource attributes
export OTEL_RESOURCE_ATTRIBUTES="deployment.environment=production,service.namespace=dashflow"

# Logging level
export RUST_LOG="info,dashflow_core=debug"
```

---

## Jaeger Setup

### Docker Compose

Deploy Jaeger for local development:

```yaml
# docker-compose.yml
version: '3.8'

services:
  jaeger:
    image: jaegertracing/all-in-one:1.55
    environment:
      - COLLECTOR_OTLP_ENABLED=true
    ports:
      - "16686:16686"  # Jaeger UI
      - "4317:4317"    # OTLP gRPC
      - "4318:4318"    # OTLP HTTP
    networks:
      - tracing

  dashflow-app:
    build: .
    environment:
      - OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317
      - OTEL_SERVICE_NAME=dashflow-app
    ports:
      - "8080:8080"
    depends_on:
      - jaeger
    networks:
      - tracing

networks:
  tracing:
```

```bash
# Start services
docker-compose up -d

# Access Jaeger UI
open http://localhost:16686
```

### Kubernetes Deployment

Deploy Jaeger with Kubernetes:

```yaml
# k8s/jaeger.yaml
apiVersion: v1
kind: Service
metadata:
  name: jaeger
  namespace: observability
spec:
  ports:
    - name: otlp-grpc
      port: 4317
      targetPort: 4317
    - name: otlp-http
      port: 4318
      targetPort: 4318
    - name: ui
      port: 16686
      targetPort: 16686
  selector:
    app: jaeger
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: jaeger
  namespace: observability
spec:
  replicas: 1
  selector:
    matchLabels:
      app: jaeger
  template:
    metadata:
      labels:
        app: jaeger
    spec:
      containers:
        - name: jaeger
          image: jaegertracing/all-in-one:1.55
          env:
            - name: COLLECTOR_OTLP_ENABLED
              value: "true"
          ports:
            - containerPort: 4317
              name: otlp-grpc
            - containerPort: 4318
              name: otlp-http
            - containerPort: 16686
              name: ui
```

```bash
# Deploy Jaeger
kubectl apply -f k8s/jaeger.yaml

# Port-forward to access UI
kubectl port-forward -n observability svc/jaeger 16686:16686
```

### Production Deployment

For production, use Jaeger Operator or managed services:

#### Jaeger Operator (Kubernetes)

```bash
# Install Jaeger Operator
kubectl create namespace observability
kubectl apply -f https://github.com/jaegertracing/jaeger-operator/releases/latest/download/jaeger-operator.yaml -n observability

# Deploy Jaeger instance
kubectl apply -f - <<EOF
apiVersion: jaegertracing.io/v1
kind: Jaeger
metadata:
  name: dashflow-tracing
  namespace: observability
spec:
  strategy: production
  storage:
    type: elasticsearch
    options:
      es:
        server-urls: http://elasticsearch:9200
  ingress:
    enabled: true
    hosts:
      - jaeger.example.com
EOF
```

#### Managed Services

Alternative: Use cloud-managed tracing services:

- **AWS X-Ray**: Native AWS distributed tracing
- **Google Cloud Trace**: GCP tracing service
- **Azure Monitor**: Azure application insights
- **Datadog APM**: Comprehensive APM with tracing
- **New Relic**: Full-stack observability

---

## LangSmith Integration

LangSmith provides DashFlow-specific tracing with LLM call visualization, token tracking, and cost analysis.

### Setup

1. **Create LangSmith Account**: Sign up at [smith.langchain.com](https://smith.langchain.com)

2. **Get API Key**: Generate an API key from the settings page

3. **Configure Environment**:

```bash
export LANGCHAIN_TRACING_V2=true
export LANGCHAIN_ENDPOINT="https://api.smith.langchain.com"
export LANGCHAIN_API_KEY="your-api-key"
export LANGCHAIN_PROJECT="my-dashflow-project"
```

### Using LangSmith Tracer

```rust
use dashflow::core::tracers::DashFlowTracer;
use dashflow::core::callbacks::CallbackManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize LangSmith tracer
    let tracer = DashFlowTracer::new("my-dashflow-project")?;
    let callbacks = CallbackManager::new().with_handler(tracer);

    // Use with any Runnable
    let chat = ChatOpenAI::new().with_model("gpt-4");
    let result = chat.generate(&messages, Some(&callbacks)).await?;

    Ok(())
}
```

### LangSmith Features

1. **Trace Visualization**: See full execution flow with timing
2. **LLM Call Details**: View prompts, responses, tokens, costs
3. **Dataset Management**: Create test sets for evaluation
4. **Feedback Collection**: Gather user feedback on responses
5. **Cost Tracking**: Monitor API costs per project
6. **Comparison**: Compare different model configurations

### LangSmith Dashboard

Access your traces at: `https://smith.langchain.com/{org}/{project}/traces`

The dashboard provides:
- Trace list with duration, status, cost
- Detailed trace tree with nested spans
- LLM inputs/outputs with token counts
- Error stack traces with context
- Performance metrics and trends

---

## W3C Trace Context

W3C Trace Context enables distributed tracing across services and languages.

### Propagation Headers

DashFlow automatically propagates trace context via HTTP headers:

- `traceparent`: Trace ID, parent span ID, trace flags
- `tracestate`: Vendor-specific trace state

### Extracting Trace Context

In HTTP handlers, extract and propagate trace context:

```rust
use opentelemetry::global;
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_http::HeaderExtractor;
use axum::{extract::Request, middleware::Next, response::Response};

pub async fn trace_propagation_middleware(
    request: Request,
    next: Next,
) -> Response {
    let tracer = global::tracer("dashflow");

    // Extract trace context from HTTP headers
    let parent_cx = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });

    // Create span with extracted context
    let span = tracer
        .span_builder("http_request")
        .with_parent_context(parent_cx)
        .start(&tracer);

    // Attach span to current context
    let cx = opentelemetry::Context::current_with_span(span);
    let _guard = cx.attach();

    // Process request
    next.run(request).await
}
```

### Injecting Trace Context

When making outgoing HTTP requests, inject trace context:

```rust
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use reqwest::header::HeaderMap;

async fn make_http_request(url: &str) -> Result<String> {
    let mut headers = HeaderMap::new();

    // Inject current trace context into headers
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &opentelemetry::Context::current(),
            &mut HeaderInjector(&mut headers),
        )
    });

    // Make HTTP request with trace headers
    let response = reqwest::Client::new()
        .get(url)
        .headers(headers)
        .send()
        .await?;

    Ok(response.text().await?)
}
```

### Cross-Service Tracing

Example: Tracing a request across multiple services:

```
┌─────────────┐   traceparent   ┌─────────────┐   traceparent   ┌─────────────┐
│   Gateway   │ ──────────────> │  DashFlow  │ ──────────────> │  OpenAI API │
│   Service   │                 │  Rust App   │                 │   Service   │
└─────────────┘                 └─────────────┘                 └─────────────┘
      │                                │                                │
      └────────────────── Same Trace ID ───────────────────────────────┘
```

All services share the same trace ID, enabling end-to-end request tracking.

---

## Distributed Tracing Patterns

### Pattern 1: LLM Call Tracing

Track individual LLM API calls with detailed attributes:

```rust
#[instrument(
    skip(client),
    fields(
        llm.system = "openai",
        llm.request.model = model,
        llm.request.temperature = temperature,
    )
)]
async fn call_openai(
    client: &ChatOpenAI,
    messages: Vec<Message>,
    model: &str,
    temperature: f32,
) -> Result<String> {
    let start = Instant::now();

    match client.generate(&messages, None).await {
        Ok(response) => {
            // Record success metrics
            tracing::info!(
                llm.response.model = %response.model,
                llm.usage.prompt_tokens = response.usage.prompt_tokens,
                llm.usage.completion_tokens = response.usage.completion_tokens,
                llm.usage.total_tokens = response.usage.total_tokens,
                duration_ms = start.elapsed().as_millis(),
                "LLM call succeeded"
            );
            Ok(response.content)
        }
        Err(e) => {
            // Record error
            tracing::error!(
                error.type = std::any::type_name_of_val(&e),
                error.message = %e,
                duration_ms = start.elapsed().as_millis(),
                "LLM call failed"
            );
            Err(e)
        }
    }
}
```

### Pattern 2: Agent Execution Tracing

Track multi-step agent reasoning:

```rust
#[instrument(skip(agent), fields(agent.type = "react"))]
async fn run_agent(agent: &Agent, task: &str) -> Result<String> {
    let mut iteration = 0;

    loop {
        iteration += 1;
        let span = tracing::info_span!("agent_iteration", iteration);
        let _guard = span.enter();

        // Think step
        let thought = agent.think(task).await?;
        tracing::info!(thought = %thought, "Agent thought");

        // Act step
        let action = agent.parse_action(&thought)?;
        tracing::info!(
            action.tool = %action.tool,
            action.input = %action.input,
            "Agent action"
        );

        // Execute tool
        let observation = agent.execute_tool(&action).await?;
        tracing::info!(
            observation_length = observation.len(),
            "Tool observation"
        );

        // Check if done
        if agent.is_final_answer(&observation) {
            tracing::info!("Agent completed task");
            return Ok(observation);
        }

        if iteration >= agent.max_iterations {
            tracing::warn!("Agent reached max iterations");
            return Err("Max iterations reached".into());
        }
    }
}
```

### Pattern 3: Chain Composition Tracing

Track complex chain pipelines:

```rust
#[instrument(skip_all, fields(chain.type = "rag"))]
async fn run_rag_chain(
    query: &str,
    retriever: &dyn Retriever,
    chat: &ChatOpenAI,
) -> Result<String> {
    // Retrieval span
    let docs = {
        let span = tracing::info_span!("retrieval", query_length = query.len());
        let _guard = span.enter();

        let docs = retriever.retrieve(query).await?;
        tracing::info!(docs_retrieved = docs.len(), "Documents retrieved");
        docs
    };

    // Context building span
    let context = {
        let span = tracing::info_span!("context_building");
        let _guard = span.enter();

        let context = docs.iter()
            .map(|d| &d.page_content)
            .collect::<Vec<_>>()
            .join("\n\n");
        tracing::info!(context_length = context.len(), "Context built");
        context
    };

    // LLM generation span
    let response = {
        let span = tracing::info_span!("generation");
        let _guard = span.enter();

        let prompt = format!("Context:\n{}\n\nQuestion: {}", context, query);
        let messages = vec![Message::human(&prompt)];
        chat.generate(&messages, None).await?
    };

    Ok(response.content)
}
```

---

## Log Aggregation

### Structured Logging Setup

Configure JSON logging for log aggregation systems:

```rust
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt};

fn init_logging() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
        )
        .init();
}
```

### Loki Integration

**Promtail Configuration** (`promtail-config.yml`):

```yaml
server:
  http_listen_port: 9080
  grpc_listen_port: 0

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: dashflow
    static_configs:
      - targets:
          - localhost
        labels:
          job: dashflow
          __path__: /var/log/dashflow/*.log
    pipeline_stages:
      - json:
          expressions:
            level: level
            message: message
            timestamp: timestamp
            trace_id: span.trace_id
            span_id: span.span_id
      - labels:
          level:
          trace_id:
      - timestamp:
          source: timestamp
          format: RFC3339
```

**Docker Compose with Loki**:

```yaml
version: '3.8'

services:
  loki:
    image: grafana/loki:2.9.3
    ports:
      - "3100:3100"
    command: -config.file=/etc/loki/local-config.yaml

  promtail:
    image: grafana/promtail:2.9.3
    volumes:
      - ./promtail-config.yml:/etc/promtail/config.yml
      - /var/log/dashflow:/var/log/dashflow
    command: -config.file=/etc/promtail/config.yml

  grafana:
    image: grafana/grafana:10.2.3
    ports:
      - "3000:3000"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
```

### ELK Stack Integration

**Filebeat Configuration** (`filebeat.yml`):

```yaml
filebeat.inputs:
  - type: log
    enabled: true
    paths:
      - /var/log/dashflow/*.log
    json.keys_under_root: true
    json.add_error_key: true
    processors:
      - add_fields:
          target: ''
          fields:
            service.name: dashflow
            service.version: 1.11

output.elasticsearch:
  hosts: ["elasticsearch:9200"]
  index: "dashflow-%{+yyyy.MM.dd}"

setup.ilm.enabled: false
setup.template.name: "dashflow"
setup.template.pattern: "dashflow-*"
```

### AWS CloudWatch Integration

Use `tracing-cloudwatch` for direct CloudWatch integration:

```toml
[dependencies]
tracing-cloudwatch = "0.4"
```

```rust
use tracing_cloudwatch::CloudWatchLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cloudwatch_layer = CloudWatchLayer::new(
        "dashflow-app",
        "/aws/dashflow",
        "us-east-1",
    ).await?;

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(cloudwatch_layer)
        .init();

    // Your application code

    Ok(())
}
```

---

## Best Practices

### 1. Sampling Strategy

In production, use sampling to reduce overhead:

```bash
# Sample 10% of traces
export OTEL_TRACES_SAMPLER="parentbased_traceidratio"
export OTEL_TRACES_SAMPLER_ARG="0.1"

# Always sample errors
export OTEL_TRACES_SAMPLER="parentbased_always_on"
```

Custom sampling based on conditions:

```rust
use opentelemetry_sdk::trace::{Sampler, SamplingDecision, SamplingResult};

struct CustomSampler;

impl Sampler for CustomSampler {
    fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
    ) -> SamplingResult {
        // Always sample errors
        if name.contains("error") || name.contains("fail") {
            return SamplingResult {
                decision: SamplingDecision::RecordAndSample,
                attributes: vec![],
                trace_state: Default::default(),
            };
        }

        // Sample 10% of normal requests
        if trace_id.to_bytes()[0] < 26 {  // ~10%
            SamplingResult {
                decision: SamplingDecision::RecordAndSample,
                attributes: vec![],
                trace_state: Default::default(),
            }
        } else {
            SamplingResult {
                decision: SamplingDecision::Drop,
                attributes: vec![],
                trace_state: Default::default(),
            }
        }
    }
}
```

### 2. Sensitive Data Redaction

Avoid logging sensitive information in traces:

```rust
#[instrument(
    skip(api_key, user_data),
    fields(
        api_key = "[REDACTED]",
        user_id = user_data.id,
        // Don't log email, password, tokens, etc.
    )
)]
async fn process_user_request(api_key: &str, user_data: &UserData) -> Result<()> {
    // Implementation
    Ok(())
}
```

### 3. Span Naming Conventions

Use consistent, hierarchical span names:

```
✅ Good:
- http.request
- llm.openai.chat_completion
- agent.react.iteration
- retrieval.vector_search

❌ Bad:
- request
- call
- iteration
- search
```

### 4. Attribute Standards

Follow OpenTelemetry semantic conventions:

```rust
use opentelemetry_semantic_conventions as semcov;

span.set_attribute(semcov::trace::HTTP_METHOD.string("POST"));
span.set_attribute(semcov::trace::HTTP_URL.string(url));
span.set_attribute(semcov::trace::HTTP_STATUS_CODE.i64(200));
span.set_attribute(semcov::trace::LLM_SYSTEM.string("openai"));
span.set_attribute(semcov::trace::LLM_REQUEST_MODEL.string("gpt-4"));
```

### 5. Error Handling

Always record errors in spans:

```rust
match operation().await {
    Ok(result) => {
        span.set_status(Status::Ok);
        Ok(result)
    }
    Err(e) => {
        span.set_status(Status::error(e.to_string()));
        span.record_exception(&e);
        Err(e)
    }
}
```

### 6. Performance Impact

Minimize tracing overhead:

- Use `#[instrument(skip(large_data))]` to avoid serializing large objects
- Set appropriate sampling rates (1-10% for high-traffic services)
- Use async batch exporters (not synchronous)
- Limit span attribute sizes (< 1KB per attribute)

---

## Troubleshooting

### Missing Traces

**Check exporter connectivity:**
```bash
# Test OTLP endpoint
curl -v http://jaeger:4317
```

**Verify environment variables:**
```bash
echo $OTEL_EXPORTER_OTLP_ENDPOINT
echo $OTEL_SERVICE_NAME
```

**Check logs for export errors:**
```bash
kubectl logs -l app=dashflow | grep -i "export\|otlp\|span"
```

### Broken Trace Context

**Symptom**: Traces appear as separate roots instead of connected spans

**Causes**:
1. Missing trace propagation middleware
2. Incorrect header extraction/injection
3. Async context not properly propagated

**Fix**: Ensure W3C trace context propagation:

```rust
// In HTTP middleware
let parent_cx = global::get_text_map_propagator(|propagator| {
    propagator.extract(&HeaderExtractor(request.headers()))
});

// In async code
let cx = Context::current();
tokio::spawn(async move {
    let _guard = cx.attach();
    // Work here inherits trace context
});
```

### High Memory Usage

**Symptom**: Increasing memory usage over time

**Causes**:
1. Batch exporter not flushing
2. Too many spans in memory
3. Large span attributes

**Fix**:
```rust
// Configure batch span processor with limits
use opentelemetry_sdk::trace::BatchConfig;

let batch_config = BatchConfig::default()
    .with_max_queue_size(2048)
    .with_max_export_batch_size(512)
    .with_scheduled_delay(std::time::Duration::from_secs(5));
```

### Slow Requests

**Symptom**: Requests slower with tracing enabled

**Solution**: Adjust sampling and export batch size:

```bash
# Reduce sampling
export OTEL_TRACES_SAMPLER_ARG="0.01"  # 1% sampling

# Increase batch export interval
export OTEL_BSP_SCHEDULE_DELAY=10000  # 10 seconds
```

---

## Additional Resources

- [OpenTelemetry Rust Documentation](https://docs.rs/opentelemetry/latest/opentelemetry/)
- [Jaeger Documentation](https://www.jaegertracing.io/docs/latest/)
- [LangSmith Documentation](https://docs.smith.langchain.com/)
- [W3C Trace Context Specification](https://www.w3.org/TR/trace-context/)
- [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/)
- [Tracing Crate Documentation](https://docs.rs/tracing/latest/tracing/)
