# Example Apps Observability Guide

This guide explains how to use the observability features built into the DashFlow example applications: the `librarian` and `codex-dashflow` paragon apps.

> **Historical Note:** Previous example apps (document_search_streaming, advanced_rag, code_assistant)
> have been consolidated into the `librarian` application. The observability patterns described here
> apply to librarian and any custom DashFlow applications you build.

---

## Overview

The `librarian` application provides production-grade observability through:

1. **Structured Logging** (tracing + JSON output)
2. **Metrics Collection** (Prometheus exposition)
3. **Distributed Tracing** (OpenTelemetry + Jaeger)
4. **Health Checks** (HTTP endpoints for readiness/liveness probes)
5. **Performance Dashboards** (Grafana visualization)

---

## Quick Start

### 1. Start Observability Stack

```bash
# From repository root
docker-compose up -d

# Verify services are running
docker-compose ps
```

Access UIs:
- **Jaeger UI**: http://localhost:16686 (distributed tracing)
- **Prometheus UI**: http://localhost:9090 (metrics queries)
- **Grafana UI**: http://localhost:3000 (dashboards, login: admin/admin)

### 2. Run Librarian with Full Observability

```bash
# Set environment variables
export RUST_LOG=info                                      # Structured logging level
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317  # Jaeger endpoint
export OPENAI_API_KEY=your-key-here                       # LLM API key

# Run librarian (metrics on :9091)
cd examples/apps/librarian
cargo run --release -- query "What is async in Rust?"

# Run with streaming
cargo run --release --features dashstream -- query "Explain Rust ownership"

# Interactive mode
cargo run --release -- interactive
```

### 3. Run Codex DashFlow with Observability

```bash
# Set environment variables
export RUST_LOG=info                                      # Structured logging level
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317  # Jaeger endpoint
export OPENAI_API_KEY=your-key-here                       # LLM API key

# Run codex-dashflow (various commands)
cd examples/apps/codex-dashflow
cargo run --release -- generate "a fibonacci function"
cargo run --release -- chat --stream
cargo run --release -- exec "list files in this directory"

# Disable telemetry (local development)
cargo run --release -- --no-telemetry generate "hello world"
```

### 4. Explore Traces and Metrics

**Jaeger (Distributed Tracing):**
1. Open http://localhost:16686
2. Select service: `librarian` or `codex-dashflow`
3. Click "Find Traces"
4. Explore request timelines, span details, and dependencies

**Prometheus (Metrics):**
1. Open http://localhost:9090
2. Try example queries:
   ```promql
   # Request rate (requests per second)
   rate(librarian_requests_total[1m])

   # Average latency (milliseconds)
   histogram_quantile(0.5, rate(librarian_request_duration_seconds_bucket[5m])) * 1000

   # Error rate
   rate(librarian_errors_total[1m])
   ```

**App Endpoints:**
- Metrics: http://localhost:9091/metrics
- Health: http://localhost:8080/health

---

## Features

### Structured Logging

**Format**: JSON (production-ready, machine-parseable)

**Configuration**: `RUST_LOG` environment variable

```bash
# Examples
RUST_LOG=info      # Info level and above (default)
RUST_LOG=debug     # Debug level and above
RUST_LOG=warn      # Warn level and above
```

**Output Fields**:
- `timestamp`: ISO 8601 timestamp
- `level`: Log level (INFO, DEBUG, WARN, ERROR)
- `target`: Module path
- `message`: Log message
- `span`: Current trace span (if tracing enabled)
- `trace_id`: Distributed trace ID (if tracing enabled)
- Custom fields (query, latency, error_type, etc.)

### Metrics Collection

**Protocol**: Prometheus text exposition format

**Key Metrics**:

#### Librarian Metrics (:9091/metrics)
- `request_duration_seconds` (histogram): End-to-end latency
- `retrieval_duration_seconds` (histogram): Vector search latency
- `llm_duration_seconds` (histogram): LLM call latency
- `streaming_duration_seconds` (histogram): Streaming synthesis latency
- `ttft_seconds` (histogram): Time to first token
- `llm_tokens_total` (counter): Token usage
- `requests_total` (counter): Request count
- `errors_total{error_type}` (counter): Error count
- `documents_retrieved_total` (counter): Document retrieval count
- `quality_score` (histogram): Response quality scores (0.0-1.0)

### Distributed Tracing

**Protocol**: OpenTelemetry OTLP (gRPC)

**Configuration**: `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable

```bash
# Enable tracing (Jaeger endpoint)
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Disable tracing (unset or empty)
unset OTEL_EXPORTER_OTLP_ENDPOINT
```

**How it Works**:
- All `info!()`, `debug!()`, `warn!()`, `error!()` calls create spans automatically
- Spans inherit from parent context (async tasks)
- Trace IDs propagate across async boundaries
- Zero overhead when `OTEL_EXPORTER_OTLP_ENDPOINT` not set

**Sampling**: 100% (AlwaysOn sampler) - all traces collected

### Performance Dashboards

**Platform**: Grafana 10.3.3

**Access**: http://localhost:3000 (login: admin/admin)

**Pre-configured Dashboards**:

All dashboards are automatically loaded on startup via provisioning. Navigate to **Dashboards > DashFlow** folder in Grafana UI.

#### Librarian Dashboard

**UID**: `librarian`

**Panels**:
- **Request Latency** (p50, p95, p99): End-to-end request duration percentiles
- **Request Throughput**: Requests per second rate
- **Error Rate**: Error percentage over time
- **Time to First Token (TTFT)**: Streaming responsiveness
- **LLM Token Usage**: Token consumption rate
- **Pipeline Stage Durations**: Breakdown of retrieval, LLM, streaming phases
- **Quality Score**: Response quality over time
- **Errors by Type**: Pie chart of error distribution

**Key Metrics**:
- Target TTFT: <1s (streaming optimization)
- Target latency p95: <10s
- Target error rate: <1%
- Target quality score: >=0.80

**Dashboard Features**:
- **Auto-refresh**: 5s interval (live data)
- **Time range**: Default 15 minutes (adjustable)
- **Drill-down**: Click panels to explore raw Prometheus queries
- **Export**: Download dashboard JSON for backup/sharing

### Health Checks

**Protocol**: HTTP JSON responses

**Endpoint**: `http://localhost:8080/health`

**Response Format**:
```json
{
  "status": "healthy",
  "checks": {
    "metrics": "ok",
    "tracing": "ok",
    "llm_api": "ok"
  },
  "timestamp": "2025-01-12T10:30:00Z"
}
```

**Status Values**:
- `healthy`: All components operational
- `degraded`: Some components unavailable (e.g., LLM API key not set)
- `unhealthy`: Critical component failure

**Usage Examples**:

```bash
# Basic health check
curl http://localhost:8080/health

# Pretty-printed JSON
curl http://localhost:8080/health | jq

# Check status code (200 = healthy/degraded, 503 = unhealthy)
curl -w "\nHTTP Status: %{http_code}\n" http://localhost:8080/health

# Kubernetes liveness probe (example)
# livenessProbe:
#   httpGet:
#     path: /health
#     port: 8080
#   initialDelaySeconds: 30
#   periodSeconds: 10
```

---

## Alerting

**Status**: Pre-configured alert rules (auto-provisioned on Grafana startup)

**Access**: http://localhost:3000/alerting (Grafana UI)

### Pre-configured Alert Rules

#### Application Alerts

1. **High Error Rate** (Critical)
   - Condition: Error rate > 5% for 5 minutes
   - Severity: `critical`
   - Action: Investigate logs, check LLM API status

2. **High Latency p95** (Warning)
   - Condition: p95 latency > 15s for 5 minutes
   - Severity: `warning`
   - Action: Check LLM API latency, review system resources

3. **Slow Time to First Token** (Warning)
   - Condition: TTFT > 2s for 10 minutes
   - Severity: `warning`
   - Action: Check LLM API performance, review prompt complexity

4. **Low Quality Score** (Warning)
   - Condition: Average quality < 0.6 for 10 minutes
   - Severity: `warning`
   - Action: Review document corpus, check retrieval quality

### Configuring Notifications

Configure notification channels in Grafana UI:

1. Open http://localhost:3000/alerting/notifications
2. Click "Add contact point"
3. Choose notification type (Email, Slack, PagerDuty, Webhook, etc.)
4. Test the contact point
5. Create notification policy routing alerts by severity

---

## Troubleshooting

### Dashboard shows "No data"

**Cause**: App not running or Prometheus not scraping

**Fix**:
1. Verify app is running and exposing metrics:
   ```bash
   curl http://localhost:9091/metrics
   ```
2. Check Prometheus targets: http://localhost:9090/targets
3. Run librarian to generate metrics:
   ```bash
   cargo run -p librarian -- query "test"
   ```
4. Wait 5-10s for Prometheus to scrape, refresh dashboard

### Tracing not working

**Checklist**:
1. Is `OTEL_EXPORTER_OTLP_ENDPOINT` set?
   ```bash
   echo $OTEL_EXPORTER_OTLP_ENDPOINT  # Should show: http://localhost:4317
   ```
2. Is Jaeger running?
   ```bash
   docker-compose ps jaeger
   curl http://localhost:16686
   ```

### Health check shows degraded status

**Cause**: Usually missing OPENAI_API_KEY environment variable

**Fix**:
```bash
export OPENAI_API_KEY=your-key-here
# Restart app to pick up new environment variable
```

---

## Implementation Details

### Architecture

**Logging Flow:**
```
Application → tracing::info!() → tracing-subscriber (JSON) → stdout
                              ↓
                       OpenTelemetryLayer → OTLP Exporter → Jaeger
```

**Metrics Flow:**
```
Application → metrics::counter!()/histogram!() → prometheus exporter → :9091/metrics
                                                                              ↓
                                                                         Prometheus
```

### Dependencies

- `tracing` 0.1 + `tracing-subscriber` 0.3: Structured logging
- `metrics` 0.23 + `metrics-exporter-prometheus` 0.15: Metrics
- `opentelemetry` 0.27 + `opentelemetry-otlp` 0.27: Distributed tracing
- `tracing-opentelemetry` 0.28: Bridge between tracing and OpenTelemetry

### Code Structure

The librarian app has:
1. `init_tracing()`: Sets up structured logging and OpenTelemetry integration
2. `init_metrics()`: Starts Prometheus HTTP server
3. `init_health_check()`: Starts health check HTTP server
4. Instrumentation: `info!()` / `counter!()` / `histogram!()` calls throughout

---

## References

- **OpenTelemetry**: https://opentelemetry.io/
- **Jaeger**: https://www.jaegertracing.io/
- **Prometheus**: https://prometheus.io/
- **tracing crate**: https://docs.rs/tracing/
- **metrics crate**: https://docs.rs/metrics/

---

**Last Updated:** January 3, 2026
