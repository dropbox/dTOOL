# Observability & Streaming Infrastructure

**Last Updated:** 2026-01-04 (Worker #2432 - Fix stale line counts)

**Status**: Production Ready
**Purpose**: Complete reference for all monitoring, metrics, alerting, and streaming systems

---

## Table of Contents

1. [System Architecture](#system-architecture)
2. [Monitoring Stack](#monitoring-stack)
3. [Streaming Infrastructure](#streaming-infrastructure)
4. [Alert Rules](#alert-rules)
5. [Dashboards](#dashboards)
6. [Validation Tests](#validation-tests)
7. [Access URLs](#access-urls)
8. [Operational Procedures](#operational-procedures)

---

## System Architecture

```
┌─────────────────┐
│  DashFlow App  │
└────────┬────────┘
         │ Events
         ▼
┌─────────────────┐
│ Quality Monitor │─────► Kafka (dashstream-quality topic)
└─────────────────┘                │
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
         ▼                         ▼                         ▼
┌──────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  WebSocket       │    │  Prometheus      │    │  DLQ Handler    │
│  Server          │    │  Exporter        │    │  (Errors)       │
└────────┬─────────┘    └────────┬─────────┘    └────────┬────────┘
         │                       │                       │
         │ Real-time             │ Metrics               │ Failed msgs
         ▼                       ▼                       ▼
┌──────────────────┐    ┌──────────────────┐    Kafka DLQ Topic
│  React UI        │    │  Prometheus      │    (dashstream-quality-dlq)
│  (Browser)       │    │  TSDB            │
└──────────────────┘    └────────┬─────────┘
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
                    ▼            ▼            ▼
            ┌──────────┐  ┌──────────┐  ┌──────────┐
            │ Grafana  │  │ Alert    │  │ Jaeger   │
            │ (Visual) │  │ Manager  │  │ (Traces) │
            └──────────┘  └──────────┘  └──────────┘
```

---

## Monitoring Stack

### 1. Prometheus (Metrics Database)

**Purpose**: Time-series database for all system metrics

**Container**: `dashstream-prometheus`
**Port**: 9090
**URL**: http://localhost:9090
**Data**: Stored in Docker volume `prometheus-data`

**Configuration**:
- `prometheus.yml` - Main configuration (root directory)
- `monitoring/alert_rules.yml` - Alert definitions (~36 rules, check file for current count)
- Scrape interval: 15 seconds
- Retention: 15 days

**Scrape Targets**:
- `dashstream-quality` job: Prometheus Exporter (port 9090)
- `dashstream-infrastructure` job: WebSocket Server (port 3002)

**Metrics Collected**:
- Kafka message counts (success/error)
- Decode error rates
- E2E latency histograms
- Client lag tracking
- Sequence anomalies (gaps/duplicates/reorders)
- DLQ activity (writes/failures)

**Health Check**:
```bash
curl http://localhost:9090/-/healthy
```

---

### 2. Grafana (Visualization)

**Purpose**: Visual dashboards for metrics and alerting

**Container**: `dashstream-grafana`
**Port**: 3000
**URL**: http://localhost:3000
**Default Login**: admin/admin

**Dashboards**:
- `grafana/dashboards/grafana_quality_dashboard.json` - Main quality metrics dashboard

**Data Sources**:
- Prometheus (primary)
- Jaeger (traces, read-only)

**Current Panels** (20 panels - Issue #19 COMPLETE):
- Success Rate (Target: ≥90%)
- Current Quality Score (Target: ≥0.90)
- Average Retries (Target: <1.5)
- Request Rate (QPS)
- Quality Components (Accuracy, Relevance, Completeness)
- Model Usage Distribution
- Latency Distribution (P50/P95/P99)
- Retry Count Percentiles
- Quality Score Trend (6h)
- Failure Breakdown by Category
- Quality Judge Usage
- Overall Failure Rate
- Retry Rate (p99)
- Sequence Gaps (Message Loss Detection)
- Duplicate Message Rate
- Out-of-Order Message Rate
- DLQ Write Rate by Error Type
- DLQ Health (Send Failures)
- Redis Connection Errors
- Redis Operation Latency (p95)

---

### 3. Alertmanager (Alert Routing)

**Purpose**: Routes Prometheus alerts to notification channels

**Container**: `dashstream-alertmanager`
**Port**: 9093
**URL**: http://localhost:9093
**Config**: `monitoring/alertmanager.yml`

**Currently Configured**: No external routing (alerts visible in Prometheus UI only)

**Future Integration Options**:
- Slack webhooks
- PagerDuty
- Email (SMTP)
- Webhook (generic)

---

### 4. Jaeger (Distributed Tracing)

**Purpose**: End-to-end request tracing across services

**Container**: `dashstream-jaeger`
**Ports**:
- 16686 (UI)
- 4317 (OTLP gRPC ingest)
- 14268 (Jaeger HTTP ingest)

**URL**: http://localhost:16686
**Protocol**: OpenTelemetry OTLP

**Services Registered**:
- `websocket-server` - WebSocket server traces
- `jaeger-all-in-one` - Jaeger internal traces

**Trace Data**:
- Operation: `process_kafka_message`
- Tags: partition, offset, busy_ns, idle_ns, thread_name
- Parent context: Propagated from Kafka message headers

**Query Traces**:
```bash
# List services
curl "http://localhost:16686/api/services" | jq

# Get traces for websocket-server
curl "http://localhost:16686/api/traces?service=websocket-server&limit=10" | jq
```

---

## Streaming Infrastructure

### 1. Kafka (Message Broker)

**Purpose**: Durable event streaming backbone

**Container**: `dashstream-kafka`
**Port**: 9092 (external), 29092 (internal)
**Zookeeper**: `dashstream-zookeeper` (port 2181)

**Topics**:

| Topic | Purpose | Partitions | Retention | Producer | Consumer |
|-------|---------|------------|-----------|----------|----------|
| `dashstream-quality` | Quality events | 1 | 7 days | Quality Monitor | WebSocket Server, Prometheus Exporter |
| `dashstream-quality-dlq` | Dead letter queue | 1 | 30 days | WebSocket Server | Manual inspection |

**Message Format**: Protobuf binary (DashFlow Streaming protocol)

**Access**:
```bash
# List topics
docker exec dashstream-kafka kafka-topics --list --bootstrap-server localhost:9092

# Check topic offset
docker exec dashstream-kafka kafka-run-class kafka.tools.GetOffsetShell \
  --broker-list localhost:9092 --topic dashstream-quality

# Consume messages (binary protobuf)
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality \
  --from-beginning --max-messages 10
```

---

### 2. WebSocket Server

**Purpose**: Real-time event streaming to web browsers

**Container**: `dashstream-websocket-server`
**Port**: 3002
**Code**: `crates/dashflow-observability/src/bin/websocket_server/main.rs` (~3400 lines)

> ⚠️ **Security Note (M-232)**: This is a development/example server **without authentication**.
> By default, it binds to `127.0.0.1` (localhost only). The Docker compose explicitly sets
> `WEBSOCKET_HOST=0.0.0.0` for container networking. For production deployments:
> - Place behind a reverse proxy (nginx, Traefik, HAProxy)
> - Enable TLS termination (HTTPS/WSS)
> - Add authentication (OAuth2, JWT, mTLS)
> - Restrict network access (firewall rules, internal VPC)

**Key Config Knobs**:
- `WEBSOCKET_BUFFER_SIZE` (default: 1000) - Broadcast channel buffer size (messages).
- `WEBSOCKET_MAX_PAYLOAD_BYTES` (default: 10MB) - Maximum accepted payload size for decode/decompression; oversized payloads increment `websocket_decode_errors_total{error_type="payload_too_large"}`.

**Endpoints**:
- **WebSocket**: `ws://localhost:3002/ws` - Binary protobuf stream
- **Health**: `http://localhost:3002/health` - JSON health status
- **Metrics**: `http://localhost:3002/metrics` - Prometheus text format
- **Version**: `http://localhost:3002/version` - Build metadata
- **UI**: `http://localhost:3002/` - React SPA (observability-ui)

**Features**:
- Kafka consumer (group: configurable via `KAFKA_GROUP_ID`)
- Broadcast channel (1000 message buffer)
- Sequence validation (detects gaps/duplicates/reorders)
- DLQ handler (fire-and-forget error logging)
- OpenTelemetry tracing (OTLP to Jaeger)
- Prometheus metrics export
- Replay buffer (1000 messages, 5-minute window)
- Circuit breaker (adaptive timeouts)

**Metrics Exported**:
```
websocket_kafka_messages_total{status="success|error"}
websocket_infrastructure_errors_total
websocket_decode_errors_total{error_type="..."}
websocket_old_data_decode_errors_total
websocket_kafka_consumer_lag{partition="..."}
websocket_connected_clients
websocket_dropped_messages_total{reason="lagged_receiver"}
websocket_e2e_latency_ms_bucket{stage="kafka_to_websocket"}  # Lazy: appears after events
websocket_kafka_errors_by_type_total{error_type="..."}
dashstream_sequence_gaps_total
dashstream_sequence_duplicates_total
dashstream_sequence_reorders_total
websocket_dlq_sends_total{error_type="..."}
websocket_dlq_send_failures_total{reason="..."}
```

**Note**: Some metrics like `websocket_e2e_latency_ms` are "lazy" - they only appear after events are processed. An empty result is normal for idle systems.

---

### 3. Prometheus Exporter

**Purpose**: Export Kafka messages as Prometheus metrics

**Container**: `dashstream-prometheus-exporter`
**Port**: 9090 (metrics endpoint)
**Code**: `crates/dashflow-prometheus-exporter/src/main.rs` (~2500 lines)

**Metrics Exported**:
- Quality event counts (pass/fail/skipped)
- Token usage statistics
- Latency measurements
- Error categorization

**Status**: Active and scraped by Prometheus every 15 seconds

---

### 4. Quality Monitor

**Purpose**: Generate synthetic quality events for testing

**Container**: `dashstream-quality-monitor`
**Code**: `crates/dashflow-streaming/src/bin/quality_aggregator.rs` (496 lines)

**Behavior**:
- Generates QualityEvent protobuf messages
- Publishes to Kafka topic `dashstream-quality`
- Rate: Configurable (default: 1 event/sec)
- Includes: thread_id, sequence numbers, timestamps, scores

**Use Case**: Testing, development, load generation

---

## Configuration Options

### Producer Configuration

```rust
use dashflow_streaming::producer::ProducerConfig;
use std::time::Duration;

ProducerConfig {
    bootstrap_servers: "localhost:9092".to_string(),
    topic: "dashstream-events".to_string(),
    enable_compression: true,            // Enable message-level Zstd compression
    compression_threshold: 512,          // Only compress messages >512 bytes
    compression_level: 3,               // Zstd level (1-22)
    timeout: Duration::from_secs(30),
    enable_idempotence: true,            // Exactly-once semantics
    max_in_flight: 5,
    kafka_compression: "none".to_string(),
    tenant_id: "customer-123".to_string(),
    max_message_size: 1_048_576,         // 1 MB limit (default)
    ..Default::default()
}
```

**Key Settings:**
- `max_message_size`: Prevents OOM from oversized messages (default: 1MB)
- `enable_compression`: Zstd compression for messages >`compression_threshold`
- `compression_threshold` / `compression_level`: Tune compression tradeoffs
- `enable_idempotence`: Exactly-once delivery guarantees
- `enable_dlq`: Emit failed sends to `dlq_topic` for forensics

### Consumer Configuration

```rust
use dashflow_streaming::consumer::{ConsumerConfig, GapRecoveryPolicy};
use dashflow_streaming::codec::SchemaCompatibility;

ConsumerConfig {
    bootstrap_servers: "localhost:9092".to_string(),
    topic: "dashstream-events".to_string(),
    partition: 0, // Consume a single partition (create one consumer per partition)
    group_id: "my-consumer".to_string(),
    auto_offset_reset: "earliest".to_string(), // No data loss on restart
    enable_decompression: true,
    max_message_size: 1_048_576,         // 1 MB limit (default)
    schema_compatibility: SchemaCompatibility::Exact,
    ..Default::default()
}
```

**Key Settings:**
- `auto_offset_reset: "earliest"`: Processes messages from downtime (production default)
- `auto_offset_reset: "latest"`: Skips old messages (testing only)
- `max_message_size`: Rejects oversized messages (default: 1MB)

### Gap Recovery Policies

Control how sequence gaps are handled:

| Policy | Behavior | Data Loss | Use Case |
|--------|----------|-----------|----------|
| `Continue` | Jump forward after gap | Yes | High-availability, accept some loss |
| `Halt` | Stop consumption | No further loss | Critical data, manual intervention |
| `WarnAndContinue` | Log critical warning, continue | Yes | Production default, balance |

```rust
use dashflow_streaming::consumer::{SequenceValidator, GapRecoveryPolicy};

// Default: WarnAndContinue
let validator = SequenceValidator::new();

// Explicit policy
let validator = SequenceValidator::with_policy(GapRecoveryPolicy::Halt);
```

### Schema Compatibility Policies

Control schema version validation:

| Policy | Accepts | Rejects | Use Case |
|--------|---------|---------|----------|
| `Exact` | Exact match only | v0, v2 | Production (safest) |
| `ForwardCompatible` | v1, v2, v3... | v0 (older) | Consumer lags behind producer |
| `BackwardCompatible` | v0, v1 | v2 (newer) | Producer lags behind consumer |

```rust
use dashflow_streaming::codec::{decode_message_with_validation, SchemaCompatibility};

// Default: Exact (safest)
let msg = decode_message_with_validation(&data, SchemaCompatibility::Exact)?;

// Forward compatible (accept newer schemas)
let msg = decode_message_with_validation(&data, SchemaCompatibility::ForwardCompatible)?;
```

### Replay Buffer Architecture

**Two-Tier Design:**
- **Memory (Fast)**: Last 1000 messages, 1ms latency
- **Redis (Persistent)**: Last 10,000 messages, 1h TTL, 5-10ms latency

**Benefits:**
- Survives server restarts
- Shared across multiple WebSocket servers
- Clients recover from network blips

**Configuration:**
```bash
REDIS_URL=redis://localhost:6379
```

**Metrics:**
- `replay_buffer_memory_hits_total` - Fast path
- `replay_buffer_redis_hits_total` - Persistent path
- `replay_buffer_redis_misses_total` - Not found

### Rate Limiting

**Per-Tenant Quotas** (Token Bucket):

```rust
use dashflow_streaming::producer::DashStreamProducer;
use dashflow_streaming::rate_limiter::RateLimit;

// Single-server deployment (in-memory rate limiting)
let producer = DashStreamProducer::new_with_rate_limiting(
    "localhost:9092",
    "dashstream-events",
    RateLimit {
        messages_per_second: 100.0,
        burst_capacity: 1000,
    },
    None,  // No Redis
).await?;

// Multi-server deployment (Redis-backed distributed rate limiting)
let producer = DashStreamProducer::new_with_rate_limiting(
    "localhost:9092",
    "dashstream-events",
    RateLimit {
        messages_per_second: 100.0,
        burst_capacity: 1000,
    },
    Some("redis://localhost:6379"),  // Shared Redis
).await?;
```

**Metrics:**
- `dashstream_rate_limit_exceeded_total{tenant_id}`
- `dashstream_rate_limit_allowed_total{tenant_id}`
- `dashstream_redis_connection_errors_total{operation}` (distributed mode only)
- `dashstream_redis_operation_latency_ms{operation}` (distributed mode only)

**Redis Capacity Planning** (Distributed Mode):

Memory usage per tenant:
- HASH with 2 fields (tokens, last_refill): ~100 bytes
- TTL: 1 hour (automatic cleanup for inactive tenants)

Capacity calculation:
- 64 MB Redis: ~650K concurrent tenants
- 256 MB Redis: ~2.5M concurrent tenants (recommended for production)
- Typical deployment (10K active tenants): ~1 MB

Performance:
- Redis EVALSHA (Lua script): 1-5ms latency
- Atomic operations prevent race conditions
- Fail-open on Redis errors (availability over strict enforcement)

Scaling:
- Single Redis instance: Up to 10M tenants
- Redis Cluster: For >10M tenants (future enhancement)

---

### 5. Observability UI (React)

**Purpose**: Browser-based real-time event viewer

**Location**: `observability-ui/`
**Framework**: React 18 + TypeScript + Vite
**Access**: http://localhost:3002/ (served by WebSocket server)

**Features**:
- WebSocket connection to server
- Binary protobuf decoding (protobufjs)
- Real-time event stream display
- Connection status indicator
- Event history buffer

**Build**:
```bash
cd observability-ui
npm install
npm run build  # Outputs to dist/
```

**Development**:
```bash
npm run dev  # Runs on port 5173
```

---

## Alert Rules

**Canonical file**: `monitoring/alert_rules.yml` (used by the Docker Compose stack)

**Kubernetes config**: `deploy/kubernetes/base/configs/alert_rules.yml` (must be kept in sync with the canonical file)

This file changes frequently; avoid hard-coding an exact count in docs. The most important alerts to understand for DashStream are:

### Highest Priority Alerts

1. **HighMessageProcessingErrorRate** (formerly `HighKafkaErrorRate` - M-416)
   - Meaning: Message *processing* failures (primarily decode failures), not Kafka infra outages

2. **ServiceDown**
   - Meaning: A scraped DashStream component is down

3. **SequenceGapsDetected** (Issue #11)
   - Meaning: Message loss detected via sequence validation

4. **WebSocketDlqBroken**
   - Meaning: WebSocket server cannot publish DLQ messages (forensic data loss)

5. **KafkaConsumerLagCritical** (M-419)
   - Meaning: WebSocket consumer is severely behind Kafka high watermark (real-time observability degraded)

9. **HighDuplicateRate** (Issue #11)
   - Condition: Duplicates > 0.1/sec for 5 minutes
   - Meaning: Kafka redelivery (at-least-once semantics)

10. **HighReorderRate** (Issue #11)
    - Condition: Reorders > 0.1/sec for 5 minutes
    - Meaning: Partition ordering compromised

**Verification**:
```bash
curl -s http://localhost:9090/api/v1/rules | jq '.data.groups[].rules[] | {alert: .name, health: .health, state: .state}'
```

---

## Dashboards

### Production Dashboard

**File**: `grafana/dashboards/grafana_quality_dashboard.json`
**Access**: http://localhost:3000/d/dashstream-quality
**Last Updated:** 2025-12-23 (commit 36e8192a1)

**Panels** (20 total - Issue #19 COMPLETE):

See Grafana section above for full panel list. All planned panels for Issue #19 have been implemented including:
- Sequence Gaps (Message Loss Detection)
- Duplicate Message Rate
- Out-of-Order Message Rate
- DLQ Write Rate by Error Type
- DLQ Health (Send Failures)

---

## Validation Tests

**Purpose**: LLM-as-judge automated validation of observability systems

**Location**: `scripts/`
**Framework**: Python 3.14 + Playwright + OpenAI GPT-4o-mini
**Environment**: `/tmp/playwright_venv/`

### Created Tests (Issue #16)

1. **`llm_validate_jaeger_traces.py`** (221 lines)
   - Queries Jaeger API for services and traces
   - Screenshots Jaeger UI with Playwright
   - GPT-4o-mini validates service registration, trace count, span quality
   - Returns: `{verdict: PASS/FAIL, confidence: 0-100, reasoning: ...}`

2. **`llm_validate_observability_ui.py`** (223 lines)
   - Starts UI dev server automatically
   - Screenshots React UI before/after connection
   - Validates: connection status, event stream, UI rendering
   - Returns: JSON verdict with confidence score

3. **`comprehensive_observability_tests.py`** (191 lines)
   - Runs all validation tests sequentially
   - Generates summary report
   - Saves detailed JSON results to `test_results_observability.json`
   - Exit code: 0 (all pass) or 1 (any fail)

4. **`llm_validate_grafana.py`** (301 lines)
   - Screenshots Grafana dashboard panels
   - Validates panels are visible and displaying data
   - Checks queries are working correctly
   - Returns: JSON verdict with panel validation results

### Running Tests

```bash
# Set API key
export OPENAI_API_KEY="sk-proj-..."

# Run individual tests
/tmp/playwright_venv/bin/python3 scripts/llm_validate_jaeger_traces.py
/tmp/playwright_venv/bin/python3 scripts/llm_validate_observability_ui.py
/tmp/playwright_venv/bin/python3 scripts/llm_validate_grafana.py

# Run comprehensive suite
/tmp/playwright_venv/bin/python3 scripts/comprehensive_observability_tests.py
```

**Current Status**: Tests exist but need refinement (LLM reasoning errors observed).

---

## Access URLs

| Service | URL | Purpose | Port |
|---------|-----|---------|------|
| **Prometheus** | http://localhost:9090 | Metrics database & alerts | 9090 |
| **Grafana** | http://localhost:3000 | Visual dashboards | 3000 |
| **Alertmanager** | http://localhost:9093 | Alert routing | 9093 |
| **Jaeger UI** | http://localhost:16686 | Distributed traces | 16686 |
| **WebSocket Server** | ws://localhost:3002/ws | Real-time event stream | 3002 |
| **Observability UI** | http://localhost:3002 | React browser UI | 3002 |
| **Health Check** | http://localhost:3002/health | Server health status | 3002 |
| **Prometheus Metrics** | http://localhost:3002/metrics | Metrics scrape endpoint | 3002 |
| **Prometheus Exporter** | http://dashstream-prometheus-exporter:9190/metrics | Kafka metrics export | 9190 |

---

## Operational Procedures

### Starting the Stack

```bash
# Start all services
docker-compose -f docker-compose.dashstream.yml up -d

# Check service health
docker-compose -f docker-compose.dashstream.yml ps

# View logs
docker-compose -f docker-compose.dashstream.yml logs -f
```

### Stopping the Stack

```bash
# Stop all services (preserve data)
docker-compose -f docker-compose.dashstream.yml down

# Stop and remove volumes (DESTROYS DATA)
docker-compose -f docker-compose.dashstream.yml down -v
```

### Restarting Individual Services

```bash
# Restart Prometheus (reload config)
docker restart dashstream-prometheus

# Restart WebSocket server (new binary)
docker restart dashstream-websocket-server

# Restart Grafana (reload dashboards)
docker restart dashstream-grafana
```

### Viewing Metrics

```bash
# Prometheus metrics (text format)
curl http://localhost:3002/metrics

# Health check (JSON)
curl http://localhost:3002/health | jq

# Alert status
curl http://localhost:9090/api/v1/alerts | jq

# Active alerts only
curl -s http://localhost:9090/api/v1/alerts | jq '.data.alerts[] | select(.state == "firing")'
```

### Inspecting Kafka

```bash
# List topics
docker exec dashstream-kafka kafka-topics --list --bootstrap-server localhost:9092

# Topic details
docker exec dashstream-kafka kafka-topics \
  --describe --bootstrap-server localhost:9092 \
  --topic dashstream-quality

# Check offset (message count)
docker exec dashstream-kafka kafka-run-class kafka.tools.GetOffsetShell \
  --broker-list localhost:9092 --topic dashstream-quality

# Read DLQ messages
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning --max-messages 10 | jq
```

### Debugging Issues

**No metrics in Prometheus**:
```bash
# Check scrape targets
curl http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job: .labels.job, health: .health, lastError: .lastError}'

# Check WebSocket server metrics endpoint
curl http://localhost:3002/metrics | head -50
```

**Alerts not firing**:
```bash
# Check alert rule status
curl http://localhost:9090/api/v1/rules | jq '.data.groups[].rules[] | select(.type == "alerting") | {alert: .name, health: .health, state: .state}'

# Check alert evaluation
curl "http://localhost:9090/api/v1/query?query=dashstream_sequence_gaps_total" | jq
```

**Grafana dashboard not loading**:
```bash
# Check Grafana logs
docker logs dashstream-grafana | tail -50

# Verify dashboard file mounted
docker exec dashstream-grafana ls -la /etc/grafana/provisioning/dashboards/
```

---

## File Locations

### Configuration Files

- `prometheus.yml` - Prometheus config (scrape targets, rules)
- `monitoring/alert_rules.yml` - Alert definitions
- `monitoring/alertmanager.yml` - Alertmanager routing
- `grafana/dashboards/grafana_quality_dashboard.json` - Grafana dashboard
- `docker-compose.dashstream.yml` - Container orchestration

### Source Code

- `crates/dashflow-observability/src/bin/websocket_server/main.rs` - WebSocket server
- `crates/dashflow-prometheus-exporter/src/main.rs` - Metrics exporter
- `crates/dashflow-streaming/src/bin/quality_aggregator.rs` - Event generator
- `crates/dashflow-streaming/src/consumer.rs` - Sequence validator
- `crates/dashflow-streaming/src/dlq.rs` - DLQ handler
- `proto/dashstream.proto` - Protocol definitions

### Frontend

- `observability-ui/src/App.tsx` - React main component
- `observability-ui/dist/` - Built static assets (served by WebSocket server)

### Documentation

- `docs/OBSERVABILITY_INFRASTRUCTURE.md` - This file
- `docs/OBSERVABILITY_RUNBOOK.md` - Operational runbook (1073 lines)
- `reports/main/n90_issues_16-20_status_2025-11-22.md` - Latest status report

### Validation

- `scripts/llm_validate_jaeger_traces.py` - Jaeger test (221 lines)
- `scripts/llm_validate_observability_ui.py` - UI test (223 lines)
- `scripts/comprehensive_observability_tests.py` - Test suite runner (191 lines)
- `scripts/llm_validate_grafana.py` - Grafana test (301 lines)

---

## Metrics Reference

### WebSocket Server Metrics

**Message Counters**:
- `websocket_kafka_messages_total{status="success"}` - Successfully processed messages
- `websocket_kafka_messages_total{status="error"}` - Message processing errors (primarily decode failures)
- `websocket_decode_errors_total{error_type="..."}` - Protobuf decode failures
- `websocket_old_data_decode_errors_total` - Decode failures from pre-session/old messages (excluded from processing error alerts)
- `websocket_dropped_messages_total{reason="lagged_receiver"}` - Messages dropped due to slow clients
- `websocket_kafka_consumer_lag{partition="..."}` - Consumer lag by partition (high watermark - current offset)

**Client Metrics**:
- `websocket_connected_clients` - Current WebSocket connections (gauge)
- `websocket_client_lag_events_total{severity="warning|critical"}` - Lag event count
- `websocket_client_lag_messages_total{severity="warning|critical"}` - Total messages lagged

**Latency Metrics** (lazy - appear after first event):
- `websocket_e2e_latency_ms_bucket{stage="kafka_to_websocket"}` - Histogram buckets
- `websocket_e2e_latency_ms_sum` - Total latency sum
- `websocket_e2e_latency_ms_count` - Sample count

**Error Classification**:
- `websocket_kafka_errors_by_type_total{error_type="dns_failure|connection_timeout|broker_down|decode_error|unknown"}` - rdkafka client error classification (separate from `websocket_decode_errors_total`)

**Sequence Validation** (Issue #11):
- `dashstream_sequence_gaps_total` - Message loss detection (thread_id logged to traces, not metrics)
- `dashstream_sequence_duplicates_total` - Duplicate messages (thread_id logged to traces, not metrics)
- `dashstream_sequence_reorders_total` - Out-of-order messages (thread_id logged to traces, not metrics)

**DLQ Metrics** (Issue #13):
- `websocket_dlq_sends_total{error_type="decode_error|decompression_failure|..."}` - Successful DLQ writes
- `websocket_dlq_send_failures_total{reason="timeout|kafka_error"}` - DLQ send failures

**System Metrics**:
- `websocket_uptime_seconds` - Server uptime (gauge)

---

## Implementation Status

| Component | Status | Commit | Notes |
|-----------|--------|--------|-------|
| Kafka Infrastructure | ✅ Complete | N=64 | Topics, producers, consumers |
| WebSocket Server | ✅ Complete | N=81 | Real-time streaming |
| Prometheus Stack | ✅ Complete | N=64 | Metrics collection |
| Jaeger Tracing | ✅ Complete | N=73 | Distributed tracing |
| Observability UI | ✅ Complete | N=73 | React frontend |
| Sequence Validation | ✅ Complete | N=81 | Gap/duplicate/reorder detection |
| DLQ Handler | ✅ Complete | N=81 | Dead letter queue |
| Alert Rules (#17-18) | ✅ Complete | N=90 | 5 new alerts deployed |
| LLM Validation Tests (#16) | ⚠️ Partial | N=65 | Tests exist, refinement needed |
| Grafana Dashboards (#19) | ✅ Complete | - | 24 panels at `grafana/dashboards/grafana_quality_dashboard.json` |
| Operational Runbook (#20) | ✅ Complete | N=90 | 1073 lines at `docs/OBSERVABILITY_RUNBOOK.md` |

---

## Next Steps

1. **Grafana Dashboard Panels** (Issue #19): ✅ COMPLETE
   - Dashboard: `grafana/dashboards/grafana_quality_dashboard.json` (24 panels)
   - LLM validation test: COMPLETE (`scripts/llm_validate_grafana.py`)

2. **Operational Runbook** (Issue #20): ✅ COMPLETE
   - See `docs/OBSERVABILITY_RUNBOOK.md` (1073 lines)
   - Documents response procedures for all alerts
   - Includes investigation commands and remediation steps

---

**For Questions or Issues**: See commit history (N=64, N=81, N=90) and reports in `reports/main/`
