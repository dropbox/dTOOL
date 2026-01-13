# [MANAGER] CRITICAL: Next 5 Observability & Reliability Gaps

**Date**: November 21, 2025
**Priority**: ✅ COMPLETE (as of N=80-87, verified at N=94)
**Context**: After fixing streaming bugs + alerting, system investigation reveals critical observability and reliability gaps
**Current Status**: ✅ ALL 5 ISSUES COMPLETE - Production-ready observability stack operational

**COMPLETION NOTE (N=94, November 22, 2025)**: All 5 issues described in this directive were completed at commits N=80-87 (November 21-22, 2025). Comprehensive verification performed at N=94 confirms all features implemented, tested, and operational. See `reports/main/n94_issues_11-15_already_complete_verification.md` for complete evidence.

---

## Executive Summary

Rigorous investigation of the RUNNING SYSTEM reveals 5 CRITICAL gaps preventing production deployment:

1. **No Consumer-Side Sequence Validation** - Producer tracks sequences, consumer doesn't validate
2. **Missing Infrastructure Metrics** - Alert rules reference non-existent metrics
3. **No Dead Letter Queue (DLQ) Handling** - Failed messages disappear silently
4. **No Distributed Tracing Integration** - Jaeger running but unused
5. **Observability UI Build Broken** - React dashboard cannot build

**Evidence**: System logs, metric queries, code inspection, build failures

---

## Issue #11: NO CONSUMER-SIDE SEQUENCE VALIDATION (CRITICAL - Data Integrity)

### Evidence

**Producer Has Sequence Tracking** (implemented in N=57):
```rust
// crates/dashflow-streaming/src/producer.rs:345-353
fn next_sequence(&self, thread_id: &str) -> u64 {
    let mut counters = self.sequence_counters.lock().unwrap();
    let counter = counters.entry(thread_id.to_string()).or_insert(0);
    *counter += 1;
    *counter
}
```

**Consumer Has NO Validation**:
```bash
$ find crates/dashflow-streaming -name "*.rs" -exec grep -l "SequenceValidator\|sequence.*validate\|gap.*detect" {} \;
(no results)
```

**Sequence Numbers Being Sent**:
```bash
$ cargo test --lib producer::tests::test_sequence
running 4 tests
test producer::tests::test_sequence_numbers_increment_per_thread ... ok
test producer::tests::test_sequence_numbers_per_thread_isolation ... ok
test producer::tests::test_sequence_numbers_multiple_threads ... ok
test producer::tests::test_sequence_numbers_thread_id_format ... ok
test result: ok. 4 passed
```

### Impact

**Severity**: CRITICAL - Cannot detect message loss/reordering

1. **Message Loss Undetected**
   - Producer sends: 1, 2, 3, 5, 6 (message 4 lost)
   - Consumer receives: 1, 2, 3, 5, 6
   - Gap never detected, no alert, silent data loss

2. **Message Reordering Undetected**
   - Producer sends: 1, 2, 3, 4, 5
   - Kafka partitions cause: 1, 3, 2, 4, 5
   - Out-of-order delivery never detected

3. **Duplicate Messages Undetected**
   - Kafka at-least-once semantics
   - Retries can cause: 1, 2, 2, 3, 4
   - Duplicates processed twice

4. **No Metrics for Data Integrity**
   - Cannot measure:
     - Gap rate per thread
     - Duplicate rate per thread
     - Out-of-order rate per thread

### Root Cause

Sequence numbers added to producer (N=57) but consumer validation was never implemented.

### Fix Required

**Step 1: Add Consumer Sequence Validator** (1 hour):
```rust
// crates/dashflow-streaming/src/consumer.rs (NEW FILE)
use std::collections::HashMap;
use crate::errors::{Error, Result};

#[derive(Debug, Clone)]
pub enum SequenceError {
    Gap {
        thread_id: String,
        expected: u64,
        received: u64,
        gap_size: u64,
    },
    Duplicate {
        thread_id: String,
        sequence: u64,
        expected: u64,
    },
    Reordered {
        thread_id: String,
        sequence: u64,
        expected: u64,
    },
}

pub struct SequenceValidator {
    /// Map of thread_id -> next expected sequence number
    expected_next: HashMap<String, u64>,
}

impl SequenceValidator {
    pub fn new() -> Self {
        Self {
            expected_next: HashMap::new(),
        }
    }

    pub fn validate(&mut self, thread_id: &str, sequence: u64) -> Result<(), SequenceError> {
        let expected = self.expected_next.entry(thread_id.to_string()).or_insert(1);

        if sequence < *expected {
            // Duplicate or reordered message
            if sequence == *expected - 1 {
                return Err(SequenceError::Duplicate {
                    thread_id: thread_id.to_string(),
                    sequence,
                    expected: *expected,
                });
            } else {
                return Err(SequenceError::Reordered {
                    thread_id: thread_id.to_string(),
                    sequence,
                    expected: *expected,
                });
            }
        }

        if sequence > *expected {
            // Gap detected - missing messages
            let gap_size = sequence - *expected;
            let error = SequenceError::Gap {
                thread_id: thread_id.to_string(),
                expected: *expected,
                received: sequence,
                gap_size,
            };
            *expected = sequence + 1;
            return Err(error);
        }

        // Sequence is exactly what we expected
        *expected = sequence + 1;
        Ok(())
    }

    pub fn reset(&mut self, thread_id: &str) {
        self.expected_next.remove(thread_id);
    }
}
```

**Step 2: Add Sequence Metrics** (30 min):
```rust
lazy_static! {
    static ref SEQUENCE_GAPS_TOTAL: Counter = register_counter!(
        "dashstream_sequence_gaps_total",
        "Total sequence gaps detected (message loss)"
    ).unwrap();

    static ref SEQUENCE_DUPLICATES_TOTAL: Counter = register_counter!(
        "dashstream_sequence_duplicates_total",
        "Total duplicate sequences detected"
    ).unwrap();

    static ref SEQUENCE_REORDERS_TOTAL: Counter = register_counter!(
        "dashstream_sequence_reorders_total",
        "Total out-of-order sequences detected"
    ).unwrap();

    static ref SEQUENCE_GAP_SIZE: Histogram = register_histogram!(
        "dashstream_sequence_gap_size",
        "Size of detected sequence gaps"
    ).unwrap();
}
```

**Step 3: Add Alert Rules** (15 min):
```yaml
# monitoring/alert_rules.yml
- alert: SequenceGapsDetected
  expr: rate(dashstream_sequence_gaps_total[5m]) > 0
  for: 2m
  labels:
    severity: critical
  annotations:
    summary: "Message loss detected via sequence gaps"
    description: "{{ $value }} sequence gaps/second detected. Messages are being lost."

- alert: HighDuplicateRate
  expr: rate(dashstream_sequence_duplicates_total[5m]) / rate(dashstream_messages_received_total[5m]) > 0.01
  for: 5m
  labels:
    severity: medium
  annotations:
    summary: "High duplicate message rate: {{ $value | humanizePercentage }}"
```

### Acceptance Criteria

- [ ] SequenceValidator implemented with gap/duplicate/reorder detection
- [ ] Metrics: `dashstream_sequence_gaps_total`, `dashstream_sequence_duplicates_total`, `dashstream_sequence_reorders_total`
- [ ] Alert rules: SequenceGapsDetected, HighDuplicateRate
- [ ] Tests: 10+ tests covering all sequence error cases
- [ ] Integration: Validator used in WebSocket server consumer loop

### Time Estimate

**Total**: 2 hours
- Add SequenceValidator (1 hour)
- Add metrics (30 min)
- Add alert rules (15 min)
- Write tests (15 min)

---

## Issue #12: MISSING INFRASTRUCTURE METRICS (CRITICAL - Monitoring Broken)

### Evidence

**Alert Rules Reference Non-Existent Metrics**:
```bash
$ cat monitoring/alert_rules.yml | grep "expr:" | head -6
expr: (dashstream_messages_sent_total - dashstream_messages_received_total) / dashstream_messages_sent_total > 0.01
expr: rate(dashstream_decode_errors_total[5m]) / rate(dashstream_messages_received_total[5m]) > 0.05
expr: up{job="dashstream"} == 0
expr: kafka_consumer_lag_messages > 1000
expr: histogram_quantile(0.99, rate(dashstream_e2e_latency_ms_bucket[5m])) > 500
expr: rate(dashstream_decompression_failures_total[5m]) / rate(dashstream_messages_received_total[5m]) > 0.01
```

**Metrics Don't Exist**:
```bash
$ curl -s http://localhost:8080/metrics | grep -E "messages_(sent|received)_total|decode_errors_total|decompression_failures"
(no results)
```

**Available Metrics Are Quality-Only**:
```bash
$ curl -s http://localhost:8080/metrics | grep "# HELP" | grep -v "process\|go_"
# HELP dashstream_quality_monitor_quality_score Current quality score (0-1000)
# HELP dashstream_quality_monitor_queries_failed_total Queries that failed quality threshold
# HELP dashstream_quality_monitor_queries_passed_total Queries that passed quality threshold
# HELP dashstream_quality_score_by_model Quality score by model
# HELP dashstream_queries_by_model_total Total queries by model
# HELP dashstream_query_latency_ms Query latency in milliseconds
(only quality metrics, NO infrastructure metrics)
```

### Impact

**Severity**: CRITICAL - Alerting system non-functional

1. **6 Alert Rules Useless**
   - MessageLossDetected: references `dashstream_messages_sent_total` (doesn't exist)
   - HighDecodeErrorRate: references `dashstream_decode_errors_total` (doesn't exist)
   - HighConsumerLag: references `kafka_consumer_lag_messages` (doesn't exist)
   - E2ELatencyHigh: references `dashstream_e2e_latency_ms_bucket` (doesn't exist)
   - DecompressionFailureRate: references `dashstream_decompression_failures_total` (doesn't exist)
   - Only ServiceDown might work (depends on `up` metric)

2. **Cannot Monitor System Health**
   - No visibility into:
     - Message throughput (sent/received)
     - Decode error rates
     - Decompression failure rates
     - Consumer lag
     - E2E latency distribution

3. **Incidents Go Undetected**
   - System could be losing messages (no alert)
   - Decode errors could spike to 50% (no alert)
   - Consumer could fall behind 10,000 messages (no alert)
   - P99 latency could hit 10 seconds (no alert)

### Root Cause

Prometheus-exporter only exports quality monitor metrics. Producer/consumer/infrastructure metrics are registered but never exported.

### Fix Required

**Step 1: Add Infrastructure Metrics to Prometheus-Exporter** (1 hour):
```rust
// Quality aggregator or WebSocket server needs to expose infrastructure metrics

// In producer.rs (already exists):
lazy_static! {
    static ref MESSAGES_SENT_TOTAL: Counter = register_counter!(
        "dashstream_messages_sent_total",
        "Total number of messages successfully sent to Kafka"
    ).unwrap();
}

// In consumer/websocket (MISSING - add this):
lazy_static! {
    static ref MESSAGES_RECEIVED_TOTAL: Counter = register_counter!(
        "dashstream_messages_received_total",
        "Total number of messages received from Kafka"
    ).unwrap();

    static ref DECODE_ERRORS_TOTAL: Counter = register_counter!(
        "dashstream_decode_errors_total",
        "Total number of message decode errors"
    ).unwrap();

    static ref DECOMPRESSION_FAILURES_TOTAL: Counter = register_counter!(
        "dashstream_decompression_failures_total",
        "Total number of decompression failures"
    ).unwrap();

    static ref CONSUMER_LAG_MESSAGES: Gauge = register_gauge!(
        "dashstream_consumer_lag_messages",
        "Current consumer lag in messages"
    ).unwrap();

    static ref E2E_LATENCY_MS: Histogram = register_histogram!(
        "dashstream_e2e_latency_ms",
        "End-to-end message latency in milliseconds",
        vec![10.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0]
    ).unwrap();
}
```

**Step 2: Export Metrics from Prometheus-Exporter** (30 min):
Currently prometheus-exporter only bridges Kafka quality events. Need to also collect metrics from WebSocket server health endpoint or expose metrics endpoint from WebSocket server.

**Option A**: WebSocket server exports Prometheus metrics endpoint
**Option B**: Prometheus-exporter scrapes WebSocket health endpoint and converts to metrics

**Step 3: Update Prometheus Scrape Config** (15 min):
```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'dashstream'
    static_configs:
      - targets:
          - 'dashstream-prometheus-exporter:9090'  # Quality metrics
          - 'dashstream-websocket-server:9091'      # Infrastructure metrics (NEW)
```

### Acceptance Criteria

- [ ] All alert rule metrics exist and have data
- [ ] `curl localhost:8080/metrics | grep dashstream_messages_sent_total` returns data
- [ ] `curl localhost:8080/metrics | grep dashstream_decode_errors_total` returns data
- [ ] `curl localhost:8080/metrics | grep dashstream_e2e_latency_ms_bucket` returns histogram
- [ ] Grafana dashboards show live infrastructure data
- [ ] All 6 alert rules evaluate successfully in Prometheus

### Time Estimate

**Total**: 2 hours
- Add missing metrics (1 hour)
- Configure export (30 min)
- Update Prometheus config (15 min)
- Verify all alerts work (15 min)

---

## Issue #13: NO DEAD LETTER QUEUE (DLQ) HANDLING (HIGH - Data Loss)

### Evidence

**DLQ Topics Exist But Unused**:
```bash
$ docker exec dashstream-kafka kafka-topics --bootstrap-server localhost:9092 --list | grep dlq
dashstream-dlq
dashstream-quality-dlq

$ docker exec dashstream-kafka kafka-run-class kafka.tools.GetOffsetShell --broker-list localhost:9092 --topic dashstream-dlq
dashstream-dlq:0:0
(offset 0 = empty topic, nothing ever sent to DLQ)
```

**No DLQ Code**:
```bash
$ find crates/dashflow-streaming -name "*.rs" -exec grep -l "dlq\|dead.*letter\|DeadLetter" {} \;
(no results)
```

**Failed Messages Disappear**:
```rust
// WebSocket server decode error handling (current):
match decode_message(&payload) {
    Ok(message) => process(message),
    Err(e) => {
        error!("❌ Decode error: {}", e);
        DECODE_ERRORS.inc();
        // Message is LOST - not sent to DLQ!
    }
}
```

### Impact

**Severity**: HIGH - Silent data loss for failed messages

1. **Failed Messages Lost Forever**
   - Decode errors: message discarded
   - Decompression failures: message discarded
   - Sequence validation failures: message discarded
   - No way to recover/replay failed messages

2. **Cannot Debug Production Issues**
   - Why did message fail to decode?
   - What was the malformed payload?
   - Which thread/tenant had issues?
   - No forensics available

3. **No Manual Recovery**
   - Cannot replay failed messages after fixing bug
   - Cannot inspect failed message patterns
   - Cannot test fixes against real failures

4. **Compliance Risk**
   - Some regulations require retention of all data
   - Silently dropping messages = compliance violation

### Root Cause

DLQ topics created but no producer code to send failed messages to them.

### Fix Required

**Step 1: Add DLQ Producer** (30 min):
```rust
// crates/dashflow-streaming/src/dlq.rs (NEW FILE)
use crate::producer::DashFlow StreamingProducer;
use crate::DashFlow StreamingMessage;
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqMessage {
    /// Original message payload (raw bytes)
    pub payload: Vec<u8>,

    /// Error that caused DLQ
    pub error: String,

    /// Error type (decode, decompression, validation, etc.)
    pub error_type: String,

    /// Timestamp of failure
    pub timestamp_us: i64,

    /// Original thread_id (if available)
    pub thread_id: Option<String>,

    /// Original tenant_id (if available)
    pub tenant_id: Option<String>,

    /// Kafka topic the message came from
    pub source_topic: String,

    /// Kafka partition
    pub source_partition: i32,

    /// Kafka offset
    pub source_offset: i64,
}

pub struct DlqHandler {
    producer: DashFlow StreamingProducer,
    dlq_topic: String,
}

impl DlqHandler {
    pub async fn new(bootstrap_servers: &str, dlq_topic: &str) -> Result<Self> {
        let producer = DashFlow StreamingProducer::new(bootstrap_servers, dlq_topic).await?;
        Ok(Self {
            producer,
            dlq_topic: dlq_topic.to_string(),
        })
    }

    pub async fn send_to_dlq(&self, dlq_message: DlqMessage) -> Result<()> {
        // Serialize DLQ message as JSON for easy inspection
        let json = serde_json::to_vec(&dlq_message)?;

        // Send to DLQ topic using thread_id as key (for ordering)
        let key = dlq_message.thread_id.as_deref().unwrap_or("unknown");

        // Use raw Kafka producer to avoid recursion
        // (don't use DashFlow StreamingProducer.send_event which could fail and cause infinite DLQ loop)

        self.producer.producer.send(
            FutureRecord::to(&self.dlq_topic)
                .key(key.as_bytes())
                .payload(&json),
            Timeout::After(Duration::from_secs(5))
        ).await?;

        Ok(())
    }
}
```

**Step 2: Use DLQ in WebSocket Server** (30 min):
```rust
// Update decode error handling:
match decode_message(&payload) {
    Ok(message) => process(message),
    Err(e) => {
        error!("❌ Decode error: {}", e);
        DECODE_ERRORS.inc();

        // Send to DLQ
        let dlq_message = DlqMessage {
            payload: payload.to_vec(),
            error: e.to_string(),
            error_type: "decode_error".to_string(),
            timestamp_us: Utc::now().timestamp_micros(),
            thread_id: None, // Can't extract from failed decode
            tenant_id: None,
            source_topic: "dashstream-events".to_string(),
            source_partition: message.partition(),
            source_offset: message.offset(),
        };

        if let Err(dlq_err) = dlq_handler.send_to_dlq(dlq_message).await {
            error!("❌ Failed to send to DLQ: {}", dlq_err);
            // Don't fail the consumer - log and continue
        }
    }
}
```

**Step 3: Add DLQ Metrics** (15 min):
```rust
lazy_static! {
    static ref DLQ_MESSAGES_TOTAL: Counter = register_counter!(
        "dashstream_dlq_messages_total",
        "Total messages sent to dead letter queue"
    ).unwrap();

    static ref DLQ_FAILURES_TOTAL: Counter = register_counter!(
        "dashstream_dlq_failures_total",
        "Failed attempts to send to DLQ (DLQ itself is broken)"
    ).unwrap();
}
```

**Step 4: Add DLQ Alert Rule** (15 min):
```yaml
- alert: HighDlqRate
  expr: rate(dashstream_dlq_messages_total[5m]) > 10
  for: 2m
  labels:
    severity: high
  annotations:
    summary: "High DLQ rate: {{ $value }} messages/second"
    description: "Messages are failing and being sent to DLQ at high rate. Investigate decode/decompression issues."

- alert: DlqItself Broken
  expr: rate(dashstream_dlq_failures_total[5m]) > 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "DLQ is broken - cannot send failed messages"
    description: "DLQ handler is failing. Failed messages are being LOST."
```

### Acceptance Criteria

- [ ] DlqHandler implemented and tested
- [ ] WebSocket server sends decode errors to DLQ
- [ ] Metrics: `dashstream_dlq_messages_total`, `dashstream_dlq_failures_total`
- [ ] Alert rules: HighDlqRate, DlqItselfBroken
- [ ] Can inspect DLQ messages: `docker exec kafka kafka-console-consumer --topic dashstream-dlq`
- [ ] Tests: 5+ tests covering DLQ scenarios

### Time Estimate

**Total**: 2 hours
- Add DLQ handler (30 min)
- Integrate with WebSocket server (30 min)
- Add metrics (15 min)
- Add alert rules (15 min)
- Write tests (30 min)

---

## Issue #14: NO DISTRIBUTED TRACING INTEGRATION (HIGH - Observability Gap)

### Evidence

**Jaeger Running But Unused**:
```bash
$ docker ps | grep jaeger
dashstream-jaeger  Up 2 days (healthy)  0.0.0.0:16686->16686/tcp, 0.0.0.0:4317-4318->4317-4318/tcp

$ curl -s "http://localhost:16686/api/services" | jq '.data[]'
jq: error (at <stdin>:0): Cannot iterate over null (null)
(no services registered - nothing is sending traces)
```

**No OpenTelemetry Integration**:
```bash
$ grep -r "opentelemetry\|tracing::instrument\|TracerProvider" crates/dashflow-streaming/src/ --include="*.rs"
(no results)
```

**No Trace Context Propagation**:
```rust
// Producer sends messages but no trace context in headers
// Consumer receives messages but no trace context extraction
// End-to-end trace broken
```

### Impact

**Severity**: HIGH - Cannot debug cross-service issues

1. **Cannot Trace Message Flow**
   - Producer → Kafka → Consumer → WebSocket → UI
   - Each hop is isolated, no end-to-end visibility
   - Cannot answer: "Where is the latency coming from?"

2. **Cannot Debug Latency Issues**
   - Quality monitor shows 25.5s average latency
   - But which component is slow?
     - Producer encode time?
     - Kafka transmission time?
     - Consumer decode time?
     - Quality aggregation time?
   - No way to know without distributed tracing

3. **Cannot Track Multi-Tenant Requests**
   - User query spans multiple services
   - Cannot correlate logs across services
   - Cannot filter by tenant in trace UI

4. **Missing Production Debugging Tool**
   - When user reports "slow query"
   - Cannot pull up trace to see what happened
   - Cannot identify bottleneck service

### Root Cause

Jaeger deployed but no code instrumentation with OpenTelemetry SDK.

### Fix Required

**Step 1: Add OpenTelemetry Dependencies** (15 min):
```toml
# crates/dashflow-streaming/Cargo.toml
[dependencies]
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
opentelemetry-semantic-conventions = "0.13"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-opentelemetry = "0.22"
```

**Step 2: Initialize OpenTelemetry** (30 min):
```rust
// Initialize tracer at application startup
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry(service_name: &str) -> Result<()> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", service_name.to_string()),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(())
}
```

**Step 3: Instrument Producer** (30 min):
```rust
// Add trace context to Kafka message headers
#[tracing::instrument(skip(self, message))]
pub async fn send_message(&self, message: DashFlow StreamingMessage, thread_id: &str) -> Result<()> {
    let span = tracing::Span::current();
    let context = span.context();

    // Serialize trace context into Kafka headers
    let trace_headers = extract_trace_headers(&context);

    let record = FutureRecord::to(&self.config.topic)
        .key(thread_id.as_bytes())
        .payload(&encoded_payload);

    // Add trace context headers
    for (key, value) in trace_headers {
        record = record.headers(
            OwnedHeaders::new().insert(Header {
                key: &key,
                value: Some(&value),
            })
        );
    }

    self.producer.send(record, timeout).await
}
```

**Step 4: Instrument Consumer** (30 min):
```rust
// Extract trace context from Kafka message headers
#[tracing::instrument(skip(message))]
async fn process_message(message: BorrowedMessage) {
    // Extract parent trace context from headers
    let parent_context = extract_trace_context_from_headers(message.headers());

    // Create child span linked to parent
    let span = tracing::span!(
        tracing::Level::INFO,
        "process_message",
        thread_id = ?extract_thread_id(&message)
    );

    let _enter = span.enter();

    // Process message (automatically adds to trace)
    decode_and_forward(message).await;
}
```

**Step 5: Add Trace Links to Metrics** (15 min):
```rust
// Link exemplars (example traces) to metrics
E2E_LATENCY_MS.observe_with_exemplar(
    latency_ms,
    &[(
        "trace_id",
        format!("{}", span.context().span().span_context().trace_id())
    )]
);
```

### Acceptance Criteria

- [ ] OpenTelemetry initialized in all services
- [ ] Traces visible in Jaeger UI: http://localhost:16686
- [ ] Can filter by service: quality-monitor, websocket-server, prometheus-exporter
- [ ] End-to-end trace shows: Producer → Kafka → Consumer → Processing
- [ ] Trace context propagated through Kafka message headers
- [ ] Can correlate traces by thread_id
- [ ] Can search traces by tenant_id

### Time Estimate

**Total**: 2.5 hours
- Add dependencies (15 min)
- Initialize telemetry (30 min)
- Instrument producer (30 min)
- Instrument consumer (30 min)
- Add exemplars (15 min)
- Test end-to-end (30 min)

---

## Issue #15: OBSERVABILITY UI BUILD BROKEN (MEDIUM - Dashboard Unavailable)

### Evidence

**Build Fails**:
```bash
$ cd observability-ui && npm run build
> vite build

vite v4.5.14 building for production...
✓ 0 modules transformed.
✓ built in 3ms
Could not resolve entry module "index.html".
error during build:
RollupError: Could not resolve entry module "index.html".
```

**Missing index.html**:
```bash
$ ls observability-ui/
node_modules/  package.json  package-lock.json  src/  vite.config.js
(no index.html file!)
```

**Cannot Access Dashboard**:
```bash
$ npm run dev
# Likely also fails
```

### Impact

**Severity**: MEDIUM - Cannot use React observability dashboard

1. **No Real-Time Dashboard**
   - Grafana exists but complex to configure
   - Observability UI designed for real-time event stream
   - Cannot visualize WebSocket events in UI

2. **Poor Developer Experience**
   - Cannot demo system to stakeholders
   - Cannot visually debug issues
   - Forced to use curl + jq for everything

3. **Missing User-Facing Feature**
   - Observability UI is a deliverable
   - Currently completely broken
   - Users cannot view their data

### Root Cause

Observability UI was created but never finished. Missing entry point (index.html).

### Fix Required

**Step 1: Create index.html** (15 min):
```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>DashFlow Streaming Observability</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 2: Create Main Entry Point** (15 min):
```typescript
// observability-ui/src/main.tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
```

**Step 3: Create App Component** (30 min):
```typescript
// observability-ui/src/App.tsx
import React, { useEffect, useState } from 'react';
import { DashFlow StreamingEvent } from './types';

function App() {
  const [events, setEvents] = useState<DashFlow StreamingEvent[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const ws = new WebSocket('ws://localhost:3002');

    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      setEvents(prev => [data, ...prev].slice(0, 100)); // Keep last 100
    };

    return () => ws.close();
  }, []);

  return (
    <div className="App">
      <header>
        <h1>DashFlow Streaming Observability</h1>
        <div className={`status ${connected ? 'connected' : 'disconnected'}`}>
          {connected ? '🟢 Connected' : '🔴 Disconnected'}
        </div>
      </header>

      <main>
        <div className="event-stream">
          {events.map((event, i) => (
            <div key={i} className="event">
              <span className="timestamp">{new Date(event.timestamp).toLocaleTimeString()}</span>
              <span className="type">{event.type}</span>
              <span className="thread">{event.thread_id}</span>
              <span className="quality">{event.quality?.toFixed(2)}</span>
            </div>
          ))}
        </div>
      </main>
    </div>
  );
}

export default App;
```

**Step 4: Verify Build** (15 min):
```bash
cd observability-ui
npm run build
# Should succeed and create dist/ directory

npm run dev
# Should start dev server at http://localhost:5173
```

### Acceptance Criteria

- [ ] `npm run build` succeeds without errors
- [ ] `npm run dev` starts dev server
- [ ] Can access UI at http://localhost:5173
- [ ] WebSocket connection established to localhost:3002
- [ ] Real-time events displayed in UI
- [ ] UI shows connection status indicator

### Time Estimate

**Total**: 1.5 hours
- Create index.html (15 min)
- Create main.tsx (15 min)
- Create App.tsx (30 min)
- Add styling (15 min)
- Test and verify (15 min)

---

## Worker Directive: Fix All 5 In Priority Order

### Priority Order

**Fix in this order** (most critical first):

1. **Issue #12 (Missing Infrastructure Metrics)**: CRITICAL - Alerting completely broken
2. **Issue #11 (Consumer Sequence Validation)**: CRITICAL - Cannot detect message loss
3. **Issue #13 (DLQ Handling)**: HIGH - Failed messages lost forever
4. **Issue #14 (Distributed Tracing)**: HIGH - Cannot debug cross-service issues
5. **Issue #15 (Observability UI)**: MEDIUM - Dashboard unavailable

### Estimated Total Time

- Issue #12: 2 hours
- Issue #11: 2 hours
- Issue #13: 2 hours
- Issue #14: 2.5 hours
- Issue #15: 1.5 hours

**Total**: 10 hours of work

### Commit Template

```
# <N++>: Fix Issue #<X> (of next 5) - [Title]

**Current Plan**: [MANAGER]_NEXT_5_CRITICAL_OBSERVABILITY_GAPS.md
**Issue**: #<X> - [Title]

## Evidence BEFORE Fix
[Show logs/metrics/errors demonstrating issue]

## Changes Made
- File: path/to/file.rs - [Description]
- [List changes]

## Evidence AFTER Fix
[Show issue resolved - metrics working, tests passing, etc.]

## Status
Previous 5 issues (#6-10): ✅ ALL FIXED
Next 5 issues: #<X> [✅ FIXED / ❌ NOT STARTED], remaining: #<...>
```

---

## Success Criteria (ALL MUST PASS)

### Issue #11: Consumer Sequence Validation ✅
```bash
cargo test --package dashflow-streaming sequence_validator
# Result: All tests pass

curl localhost:8080/metrics | grep dashstream_sequence_gaps_total
# Result: Metric exists with value

# Simulate gap: manually skip sequence number in test
# Result: Gap detected and logged, metric incremented
```

### Issue #12: Infrastructure Metrics ✅
```bash
curl localhost:8080/metrics | grep dashstream_messages_sent_total
# Result: dashstream_messages_sent_total{...} 12345

curl localhost:8080/metrics | grep dashstream_decode_errors_total
# Result: dashstream_decode_errors_total{...} 0

curl http://localhost:9090/api/v1/rules | jq '.data.groups[].rules[] | select(.state == "inactive")'
# Result: All rules have data sources, none are inactive
```

### Issue #13: DLQ Handling ✅
```bash
# Inject malformed message to trigger DLQ
docker exec dashstream-kafka kafka-console-producer --topic dashstream-events --broker-list localhost:9092
> {"invalid":"json","not":"protobuf"}

# Check DLQ
docker exec dashstream-kafka kafka-console-consumer --topic dashstream-dlq --from-beginning --max-messages 1
# Result: DLQ message with error details visible

curl localhost:8080/metrics | grep dashstream_dlq_messages_total
# Result: dashstream_dlq_messages_total 1
```

### Issue #14: Distributed Tracing ✅
```bash
# Send message through system
curl -X POST http://localhost:3003/query -d '{"query":"test"}'

# Check Jaeger
curl -s "http://localhost:16686/api/services" | jq '.data[]'
# Result: ["quality-monitor", "websocket-server", "prometheus-exporter"]

# View traces
open http://localhost:16686
# Result: Can see end-to-end trace from producer to consumer
```

### Issue #15: Observability UI ✅
```bash
cd observability-ui
npm run build
# Result: ✓ built in XXXms, dist/ directory created

npm run dev
# Result: Local: http://localhost:5173/

open http://localhost:5173
# Result: Dashboard loads, shows "🟢 Connected", events streaming
```

---

## Why This Matters

**User Goal**: "what are the next 5 serious issues to fix after these 5?"

**Current Reality**:
- Alerting system non-functional (metrics don't exist)
- Message loss undetectable (no consumer validation)
- Failed messages lost forever (no DLQ)
- Cannot debug cross-service issues (no tracing)
- Dashboard broken (build fails)

**Target State**:
- All alert rules functional with live data
- Message loss/gaps detected in real-time
- Failed messages preserved in DLQ for analysis
- End-to-end traces visible in Jaeger
- Real-time dashboard accessible to users

**These 5 issues must be fixed for production-grade observability.**

---

## Next Worker: Fix All 5 In Priority Order

Read this directive completely, fix Issues #11-15 in priority order, provide BEFORE/AFTER evidence for each.

**DO NOT skip issues. DO NOT claim "fixed" without runtime proof. SYSTEM MUST BE PRODUCTION-READY.**
