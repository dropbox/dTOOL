# Observability Quick Start

**Last Updated:** 2025-12-30 (Worker #2161 - CLI deprecation warnings + unified --thread flag)

Get DashFlow's observability stack running in 5 minutes. This guide shows you how to start the infrastructure, run an instrumented app, and see telemetry in action.

## Prerequisites

- Docker and Docker Compose
- Rust toolchain (`cargo`)
- DashFlow binary built: `cargo build --release -p dashflow-cli`

## Step 1: Start the Observability Stack

Start Jaeger (tracing), Prometheus (metrics), and Grafana (dashboards):

```bash
docker-compose up -d
```

This starts:
- **Jaeger UI**: http://localhost:16686 - View distributed traces
- **Prometheus**: http://localhost:9090 - Query metrics
- **Grafana**: http://localhost:3000 - Dashboards (login: admin/admin)

Verify services are healthy:

```bash
docker-compose ps
# All services should show "healthy" status
```

Or use the DashFlow CLI:

```bash
dashflow status
```

## Step 2: Start Kafka (Optional - for Event Streaming)

If you want to use `dashflow timeline live` for live event streaming, start Kafka:

```bash
docker-compose -f docker-compose-kafka.yml up -d
```

This adds:
- **Kafka**: localhost:9092 - Event streaming
- **Kafka UI**: http://localhost:8080 - View topics and messages

## Step 3: Run an Instrumented App

### Option A: Use the Librarian Example

The Librarian app is pre-configured with telemetry:

```bash
# Set your OpenAI API key
export OPENAI_API_KEY="sk-..."

# Run a query (telemetry enabled by default)
cargo run -p librarian --release -- query "What is machine learning?"
```

### Option B: Add Telemetry to Your App

Add telemetry to any DashFlow app:

```rust
use dashflow::dashstream_callback::DashStreamCallback;
use dashflow::StateGraph;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create callback for streaming events to Kafka
    let callback = DashStreamCallback::new(
        "localhost:9092",       // Kafka broker
        "dashstream-events",    // Topic
        "my-tenant",            // Tenant ID
        "session-123"           // Session ID
    ).await?;

    // Create and compile your graph
    let mut graph = StateGraph::new();
    // ... add nodes ...

    // Attach callback and invoke
    let compiled = graph.compile()?.with_callback(callback);
    compiled.invoke(initial_state).await?;

    Ok(())
}
```

For OTLP tracing (sends to Jaeger), use OpenTelemetry:

```rust
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_tracing() -> Result<SdkTracerProvider, Box<dyn std::error::Error>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    let tracer = provider.tracer("my-app");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry)
        .init();

    Ok(provider)
}
```

## Step 4: View Telemetry

### Distributed Traces (Jaeger)

Open http://localhost:16686

1. Select your service from the dropdown (e.g., "librarian")
2. Click "Find Traces"
3. Click on a trace to see the execution timeline

Jaeger shows:
- Request flow across services
- Latency breakdown per operation
- Error locations and messages

### Metrics (Prometheus)

Open http://localhost:9090

Query example metrics:
```promql
# Librarian queries per second
rate(librarian_query_total[5m])

# Search latency histogram
histogram_quantile(0.95, librarian_search_duration_seconds_bucket)

# Telemetry drops (if any)
dashstream_telemetry_dropped_total
```

### Dashboards (Grafana)

Open http://localhost:3000 (admin/admin)

Pre-configured dashboards show:
- Request rates and latencies
- Error rates
- Resource utilization

### Live Event Stream (dashflow timeline live)

If Kafka is running, watch live graph execution:

```bash
# Watch all events
dashflow timeline live

# Watch specific thread
dashflow timeline live --thread my-thread-id

# Start from beginning of topic
dashflow timeline live --from-beginning
```

### Graph Visualization (dashflow timeline)

Visualize graph execution:

```bash
# Live execution view
dashflow timeline live

# Replay historical execution
dashflow timeline replay --thread my-thread-id

# Static graph visualization
dashflow timeline view graph.mmd

# Export to standalone HTML
dashflow timeline export graph.mmd -o graph.html
```

## Quick Reference

| Component | URL | Purpose |
|-----------|-----|---------|
| Jaeger | http://localhost:16686 | Distributed traces |
| Prometheus | http://localhost:9090 | Metrics queries |
| Grafana | http://localhost:3000 | Dashboards |
| Kafka UI | http://localhost:8080 | Event inspection |

| CLI Command | Purpose |
|-------------|---------|
| `dashflow status` | Check infrastructure health |
| `dashflow timeline live` | Live execution TUI |
| `dashflow timeline replay` | Replay historical execution |

## Troubleshooting

**No traces in Jaeger?**
- Check that OTLP endpoint is `http://localhost:4317`
- Ensure your app has the service name set
- Run `dashflow status --service jaeger`

**No events in dashflow timeline live?**
- Verify Kafka is running: `docker-compose -f docker-compose-kafka.yml ps`
- Check topic exists: visit http://localhost:8080
- Ensure your app uses `DashStreamCallback`

**Grafana shows no data?**
- Check Prometheus is scraping: http://localhost:9090/targets
- Verify your app exposes metrics on port 9091

## Next Steps

- [Core Concepts](./core-concepts.md) - Learn DashFlow architecture
- [Language Models](../core/language-models.md) - Configure LLM providers
- [Architecture Overview](../architecture/overview.md) - System design deep dive
