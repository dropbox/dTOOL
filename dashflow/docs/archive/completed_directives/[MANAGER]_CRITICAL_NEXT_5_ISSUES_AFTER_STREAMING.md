# [MANAGER] CRITICAL: Next 5 Issues After Streaming Bugs Fixed

**Date**: November 21, 2025
**Priority**: 🚨 CRITICAL
**Context**: User asked: "what are the next 5 serious issues to fix after these 5?"
**Current Status**: First 5 streaming bugs mostly fixed (Issues #1,2,5 ✅ | #3,4 in progress)

---

## Executive Summary

Rigorous investigation of the RUNNING SYSTEM reveals 5 MORE critical issues that will prevent production deployment:

1. **Prometheus-Exporter Total Failure** - 100% Kafka connection failure, 10.2GB network waste
2. **WebSocket Server "reconnecting" Status** - System not healthy, errors accumulating
3. **No Alerting System** - Zero automated alerts, issues found manually only
4. **Sequence Numbers Not Tracked** - Data integrity gap, cannot detect loss/replay
5. **Tenant ID Hardcoded** - Multi-tenancy impossible, configuration gap

**Evidence**: Runtime logs, Docker stats, health endpoints, source code analysis

---

## Issue #6: PROMETHEUS-EXPORTER TOTAL KAFKA FAILURE (CRITICAL - Correctness)

### Evidence

**Prometheus-Exporter Logs** (CONTINUOUS failures):
```bash
$ docker logs dashstream-prometheus-exporter 2>&1 | tail -50

[ERROR] librdkafka: Global error: Resolve (Local: Host resolution failure):
kafka:29092/1: Failed to resolve 'kafka:29092': Name or service not known
(after 7ms in state CONNECT, 30 identical error(s) suppressed)

[ERROR] prometheus_exporter: Kafka consumer error: Message consumption error:
Resolve (Local: Host resolution failure)

[... REPEATS EVERY FEW SECONDS CONTINUOUSLY ...]
```

**Docker Resource Usage**:
```bash
$ docker stats --no-stream

NAME                             CPU %     MEM USAGE       NET I/O
dashstream-prometheus-exporter   0.19%     23.29MiB        32.8MB / 10.2GB
                                                           ^^^^^^^^^^^^
                                                           10.2GB SENT!!!
```

**Configuration Bug** (`docker-compose.dashstream.yml:L16`):
```yaml
prometheus-exporter:
  environment:
    - KAFKA_BROKERS=kafka:29092  # ❌ WRONG! Should be localhost:9092
```

### Impact

**Severity**: CRITICAL - Service completely non-functional

1. **Metrics Collection 100% Broken**
   - Zero metrics being exported to Prometheus
   - Grafana dashboards showing no data
   - Cannot monitor system health

2. **Resource Waste**
   - 10.2GB network traffic from failed connection attempts
   - Continuous CPU usage on retry loops
   - Log storage filled with error spam

3. **Log Pollution**
   - "30 identical error(s) suppressed" messages flood logs
   - Signal-to-noise ratio destroyed
   - Real errors hidden by connection spam

4. **Alerting Impossible**
   - Cannot alert on metrics that don't exist
   - Grafana alerts won't trigger
   - Production incidents go undetected

### Root Cause Analysis

**Configuration Mismatch**:
- Docker compose uses internal Docker network name: `kafka:29092`
- Kafka is ONLY exposed on host: `localhost:9092`
- Internal port 29092 is NOT accessible from prometheus-exporter container
- Container cannot resolve `kafka:29092` hostname

**Why WebSocket Server Works But Prometheus-Exporter Doesn't**:
```bash
$ docker logs dashstream-websocket-server 2>&1 | grep "kafka:29092" | head -2
🔗 Connecting to Kafka brokers: kafka:29092
kafka:29092/bootstrap: Connect to ipv4#172.21.0.6:29092 failed: Connection refused

# WebSocket server CAN resolve "kafka" to IP, but connection refused
# Prometheus-exporter CANNOT resolve "kafka" at all (DNS failure)
```

Difference: Network configuration. WebSocket server is on correct Docker network, prometheus-exporter is NOT.

### Fix Options

**Option A: Fix Broker Address** (CORRECT fix, 5 min):
```yaml
# docker-compose.dashstream.yml
prometheus-exporter:
  environment:
    - KAFKA_BROKERS=localhost:9092  # ✅ Use host address
  network_mode: "host"  # ✅ Use host network to access localhost
```

**Option B: Fix Docker Network** (alternative, 10 min):
```yaml
# docker-compose.dashstream.yml
prometheus-exporter:
  networks:
    - dashflow_default  # ✅ Join Kafka's network
  environment:
    - KAFKA_BROKERS=kafka:9092  # ✅ Use port 9092 (internal Kafka port)
```

**Option C: Use IP Address** (workaround, 2 min):
```bash
# Get Kafka container IP
docker inspect kafka | grep IPAddress | head -1
# "IPAddress": "172.21.0.6"

# Update docker-compose.dashstream.yml
- KAFKA_BROKERS=172.21.0.6:9092
```

### Acceptance Criteria

- [ ] Prometheus-exporter logs: Zero "Failed to resolve" errors for 5 minutes
- [ ] `curl localhost:8080/metrics | grep dashstream` shows real metrics (values > 0)
- [ ] Grafana dashboards show live data
- [ ] Docker stats: Network I/O stops growing exponentially
- [ ] Prometheus scraping working: `curl localhost:9091/api/v1/targets` shows "up"

### Time Estimate

**Option A (Recommended)**: 10 minutes
1. Update docker-compose.dashstream.yml (2 min)
2. Restart prometheus-exporter (1 min)
3. Verify metrics (2 min)
4. Test Grafana dashboards (5 min)

---

## Issue #7: WEBSOCKET SERVER "RECONNECTING" STATUS (HIGH - Reliability)

### Evidence

**Health Endpoint** (`http://localhost:3002/health`):
```json
{
  "status": "reconnecting",  // ❌ Should be "healthy"
  "metrics": {
    "kafka_messages_received": 262,
    "kafka_errors": 2,          // ❌ Errors occurred
    "infrastructure_errors": 2, // ❌ Infrastructure issues
    "connected_clients": 1,
    "uptime_seconds": 4402,
    "last_kafka_message_ago_seconds": 12,
    "dropped_messages": 0,
    "decode_errors": 0
  },
  "kafka_status": "connected",
  "websocket_status": "1_clients_connected"
}
```

**WebSocket Server Logs** (Multiple Restarts):
```bash
$ docker logs dashstream-websocket-server 2>&1 | grep "Starting" | head -20

📡 Starting Kafka consumer loop...
🔄 Starting circuit breaker monitor (10min degraded = auto-restart)...
📡 Starting Kafka consumer loop...
🔄 Starting circuit breaker monitor (10min degraded = auto-restart)...
📡 Starting Kafka consumer loop...
[... 10+ "Starting" messages indicating restarts ...]
```

**Initial Connection Failures**:
```bash
$ docker logs dashstream-websocket-server 2>&1 | grep "kafka:29092" | head -5

🔗 Connecting to Kafka brokers: kafka:29092
GroupCoordinator: kafka:29092: Connect to ipv4#172.21.0.6:29092 failed:
Connection refused (after 0ms in state CONNECT)
kafka:29092/bootstrap: Connect to ipv4#172.21.0.6:29092 failed:
Connection refused (after 0ms in state CONNECT, 1 identical error(s) suppressed)
```

### Impact

**Severity**: HIGH - System operational but degraded

1. **Health Status Misleading**
   - Reports "reconnecting" even when working
   - Kubernetes/load balancers may mark pod as unhealthy
   - Could trigger unnecessary restarts

2. **Error Accumulation**
   - kafka_errors=2 (never resets)
   - infrastructure_errors=2 (never resets)
   - Counters only increment, never clear

3. **Circuit Breaker Sensitivity**
   - "10min degraded = auto-restart" policy
   - May restart unnecessarily due to transient errors
   - Causes message delivery interruptions

4. **Multiple Consumer Loop Starts**
   - Logs show 10+ "Starting Kafka consumer loop" messages
   - Suggests frequent restarts or multiple instances
   - Potential resource leak

### Root Cause Analysis

**Status Calculation Logic**:
```rust
// Likely in websocket_server source:
if kafka_errors > 0 || last_message_age > threshold {
    status = "reconnecting"  // ❌ Never clears even after recovery
} else {
    status = "healthy"
}
```

**Issues**:
1. Error counters never reset after recovery
2. Initial connection failures count as permanent errors
3. No "recovered" state transition
4. Circuit breaker too aggressive

### Fix Required

**Step 1: Add Error Counter Reset** (30 min):
```rust
// Reset error counters after N successful messages
if consecutive_successful_messages >= 10 {
    kafka_errors = 0;
    infrastructure_errors = 0;
}
```

**Step 2: Improve Status Logic** (30 min):
```rust
// Status based on recent health, not all-time errors
if recent_kafka_errors == 0 && last_message_age < 60 {
    status = "healthy"
} else if recent_kafka_errors < 5 && last_message_age < 300 {
    status = "degraded"
} else {
    status = "reconnecting"
}
```

**Step 3: Adjust Circuit Breaker** (15 min):
```rust
// Make circuit breaker less aggressive
circuit_breaker:
  failure_threshold: 10    // Was: 3
  recovery_timeout: 60s    // Was: 10min
  half_open_requests: 5    // Test recovery gradually
```

### Acceptance Criteria

- [ ] Health endpoint shows `status: "healthy"` after 1 minute of successful operation
- [ ] Error counters reset after 10 consecutive successful messages
- [ ] No "Starting Kafka consumer loop" messages after initial startup
- [ ] Circuit breaker only triggers after 10+ consecutive failures
- [ ] System recovers automatically within 60s of Kafka coming back online

### Time Estimate

**Total**: 1-2 hours
- Locate health status code (15 min)
- Implement error counter reset (30 min)
- Improve status logic (30 min)
- Adjust circuit breaker (15 min)
- Test and verify (30 min)

---

## Issue #8: NO ALERTING SYSTEM (HIGH - Observability)

### Evidence

**Grafana Alert Check**:
```bash
$ find grafana -name "*.json" | xargs grep -l "alert"
(no results)
```

**Script Check**:
```bash
$ ls scripts/*alert* scripts/*monitor*
ls: No such file or directory
```

**Prometheus AlertManager Check**:
```bash
$ docker ps | grep alertmanager
(no alertmanager container running)
```

### Impact

**Severity**: HIGH - Cannot detect production incidents automatically

1. **Manual Issue Discovery**
   - First 5 streaming bugs found by MANUAL investigation
   - Decompression failure (100% rate!) went undetected
   - Negative latency bug discovered by reading logs

2. **No Proactive Response**
   - System degradations go unnoticed
   - High error rates not caught until users complain
   - Resource exhaustion (10.2GB prometheus-exporter traffic) silent

3. **Cannot Meet SLA**
   - No alerting on message loss
   - No alerting on E2E latency > threshold
   - No alerting on consumer lag
   - No alerting on error rate spikes

4. **Incident Response Delayed**
   - Team doesn't know system is down until user reports
   - Mean time to detection (MTTD) measured in hours/days
   - Cannot alert on-call engineer

### What's Missing

**Critical Alerts Needed**:

1. **Message Loss Detection** (from Issue #4):
   - Alert: `(messages_sent - messages_received) / messages_sent > 0.01` (1% loss)
   - Severity: CRITICAL
   - Action: Page on-call engineer

2. **High Error Rate**:
   - Alert: `decode_errors / messages_received > 0.05` (5% decode errors)
   - Severity: HIGH
   - Action: Notify team Slack

3. **Consumer Lag**:
   - Alert: `kafka_consumer_lag_messages > 1000`
   - Severity: MEDIUM
   - Action: Auto-scale consumers

4. **E2E Latency**:
   - Alert: `e2e_latency_p99 > 500ms`
   - Severity: MEDIUM
   - Action: Investigate performance

5. **Service Health**:
   - Alert: `up{job="dashstream"} == 0` (service down)
   - Severity: CRITICAL
   - Action: Auto-restart + page

6. **Decompression Failures**:
   - Alert: `decompression_failures_rate > 0.01` (1% failure rate)
   - Severity: HIGH
   - Action: Rollback deployment

### Fix Required

**Step 1: Deploy Prometheus AlertManager** (30 min):
```yaml
# docker-compose.dashstream.yml
alertmanager:
  image: prom/alertmanager:latest
  container_name: dashstream-alertmanager
  ports:
    - "9093:9093"
  volumes:
    - ./monitoring/alertmanager.yml:/etc/alertmanager/alertmanager.yml
  command:
    - '--config.file=/etc/alertmanager/alertmanager.yml'
```

**Step 2: Create Alert Rules** (1 hour):
```yaml
# monitoring/alert_rules.yml
groups:
  - name: dashstream_critical
    interval: 30s
    rules:
      - alert: MessageLossDetected
        expr: (dashstream_messages_sent_total - dashstream_messages_received_total) / dashstream_messages_sent_total > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Message loss detected: {{ $value | humanizePercentage }}"

      - alert: HighDecodeErrorRate
        expr: rate(dashstream_decode_errors_total[5m]) / rate(dashstream_messages_received_total[5m]) > 0.05
        for: 2m
        labels:
          severity: high
        annotations:
          summary: "High decode error rate: {{ $value | humanizePercentage }}"

      - alert: ServiceDown
        expr: up{job="dashstream"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "DashFlow Streaming service {{ $labels.instance }} is down"
```

**Step 3: Configure Notification Channels** (30 min):
```yaml
# monitoring/alertmanager.yml
route:
  receiver: 'team-slack'
  group_by: ['alertname', 'severity']
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'

receivers:
  - name: 'team-slack'
    slack_configs:
      - api_url: 'YOUR_SLACK_WEBHOOK'
        channel: '#dashstream-alerts'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'YOUR_PAGERDUTY_KEY'
```

**Step 4: Update Prometheus Config** (15 min):
```yaml
# monitoring/prometheus.yml
rule_files:
  - "alert_rules.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']
```

### Acceptance Criteria

- [ ] Prometheus AlertManager accessible at http://localhost:9093
- [ ] 6 critical alert rules configured and loaded
- [ ] Test alert fires correctly: `curl -X POST http://localhost:9093/api/v1/alerts` (test payload)
- [ ] Slack notifications working (test in #dashstream-alerts channel)
- [ ] Grafana dashboards show alert status
- [ ] Alert fires within 1 minute of condition being met

### Time Estimate

**Total**: 2-3 hours
- Deploy AlertManager (30 min)
- Create 6 alert rules (1 hour)
- Configure Slack/PagerDuty (30 min)
- Update Prometheus config (15 min)
- Test alerts end-to-end (45 min)

---

## Issue #9: SEQUENCE NUMBERS NOT TRACKED PER THREAD (MEDIUM - Data Integrity)

### Evidence

**Source Code** (`crates/dashflow-streaming/src/producer.rs:L285`):
```rust
pub fn create_header(
    &self,
    thread_id: &str,
    message_type: crate::MessageType,
) -> crate::Header {
    crate::Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: "default".to_string(), // TODO: Make configurable
        thread_id: thread_id.to_string(),
        sequence: 0, // TODO: Track sequence numbers per thread  ❌
        r#type: message_type as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    }
}
```

**Message Header Definition** (`proto/dashstream.proto`):
```protobuf
message Header {
  bytes message_id = 1;      // ✅ Unique per message
  int64 timestamp_us = 2;    // ✅ Timestamp
  string tenant_id = 3;      // ❌ Always "default"
  string thread_id = 4;      // ✅ Thread identifier
  uint64 sequence = 5;       // ❌ Always 0!
  MessageType type = 6;
  bytes parent_id = 7;
  uint32 compression = 8;
  uint32 schema_version = 9;
}
```

### Impact

**Severity**: MEDIUM - Data integrity gap, no immediate failure

1. **Cannot Detect Message Loss**
   - Consumer receives messages with sequence: 0, 0, 0, 0, ...
   - Cannot detect if message #5 is missing
   - Gap detection impossible

2. **Cannot Detect Reordering**
   - Kafka guarantees ordering per partition
   - But cannot verify ordering was preserved
   - Cannot detect out-of-order delivery bugs

3. **Cannot Replay From Specific Point**
   - Want to replay "messages after sequence 1000"?
   - Impossible - all sequences are 0
   - Must replay entire thread from beginning

4. **Duplicate Detection Broken**
   - Cannot use (thread_id, sequence) to dedupe
   - Must rely only on message_id (UUID)
   - More complex deduplication logic needed

5. **Checkpoint Correlation Harder**
   - Checkpoints reference "sequence 0"
   - Cannot correlate checkpoint with specific message sequence
   - State recovery ambiguous

### Root Cause

**No Sequence Tracking Infrastructure**:
- Producer has no per-thread sequence counters
- No atomic increment on message send
- No persistence of sequence state
- No sequence number in metrics/monitoring

### Fix Required

**Step 1: Add Sequence State** (30 min):
```rust
// producer.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct DashFlow StreamingProducer {
    producer: FutureProducer,
    config: ProducerConfig,
    // NEW: Track sequence per thread
    sequence_counters: Arc<Mutex<HashMap<String, u64>>>,
}

impl DashFlow StreamingProducer {
    fn next_sequence(&self, thread_id: &str) -> u64 {
        let mut counters = self.sequence_counters.lock().unwrap();
        let counter = counters.entry(thread_id.to_string()).or_insert(0);
        *counter += 1;
        *counter
    }
}
```

**Step 2: Use Sequence in Headers** (15 min):
```rust
pub fn create_header(
    &self,
    thread_id: &str,
    message_type: crate::MessageType,
) -> crate::Header {
    let sequence = self.next_sequence(thread_id);  // ✅ Get next sequence

    crate::Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: "default".to_string(),
        thread_id: thread_id.to_string(),
        sequence,  // ✅ Real sequence number!
        r#type: message_type as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    }
}
```

**Step 3: Add Consumer Sequence Validation** (1 hour):
```rust
// consumer.rs
pub struct SequenceValidator {
    expected_next: HashMap<String, u64>,
}

impl SequenceValidator {
    pub fn validate(&mut self, thread_id: &str, sequence: u64) -> Result<(), SequenceError> {
        let expected = self.expected_next.entry(thread_id.to_string()).or_insert(1);

        if sequence < *expected {
            return Err(SequenceError::Duplicate {
                thread_id: thread_id.to_string(),
                sequence,
                expected: *expected
            });
        }

        if sequence > *expected {
            return Err(SequenceError::Gap {
                thread_id: thread_id.to_string(),
                expected: *expected,
                received: sequence,
                gap_size: sequence - *expected
            });
        }

        *expected = sequence + 1;
        Ok(())
    }
}
```

**Step 4: Add Sequence Metrics** (30 min):
```rust
lazy_static! {
    static ref SEQUENCE_GAPS: Counter = register_counter!(
        "dashstream_sequence_gaps_total",
        "Total sequence gaps detected"
    ).unwrap();

    static ref SEQUENCE_DUPLICATES: Counter = register_counter!(
        "dashstream_sequence_duplicates_total",
        "Total duplicate sequences detected"
    ).unwrap();
}
```

### Acceptance Criteria

- [ ] Producer: Sequence numbers increment per thread (1, 2, 3, ...)
- [ ] Consumer: Detects sequence gaps and logs warning
- [ ] Consumer: Detects duplicate sequences and logs warning
- [ ] Metrics: `dashstream_sequence_gaps_total` and `dashstream_sequence_duplicates_total` exposed
- [ ] Test: Send 100 messages, verify sequences 1-100
- [ ] Test: Delete message #50 from Kafka, verify gap detected

### Time Estimate

**Total**: 2-3 hours
- Add sequence state to producer (30 min)
- Update create_header to use sequences (15 min)
- Add consumer sequence validator (1 hour)
- Add sequence metrics (30 min)
- Write tests (45 min)

---

## Issue #10: TENANT ID HARDCODED TO "default" (MEDIUM - Configuration)

### Evidence

**Source Code** (`crates/dashflow-streaming/src/producer.rs:L283`):
```rust
pub fn create_header(
    &self,
    thread_id: &str,
    message_type: crate::MessageType,
) -> crate::Header {
    crate::Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: "default".to_string(), // TODO: Make configurable  ❌
        thread_id: thread_id.to_string(),
        sequence: 0,
        r#type: message_type as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    }
}
```

**All Messages Have Same Tenant**:
```bash
$ docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 \
    --topic dashstream-quality --from-beginning --max-messages 10 | \
    protoc --decode=Header proto/dashstream.proto | grep tenant_id

tenant_id: "default"
tenant_id: "default"
tenant_id: "default"
[... all messages have tenant_id="default" ...]
```

### Impact

**Severity**: MEDIUM - Blocks multi-tenancy, single-tenant works fine

1. **Multi-Tenant Deployments Impossible**
   - All messages tagged with tenant "default"
   - Cannot isolate Customer A from Customer B
   - Cannot filter/route by tenant
   - Cannot implement per-tenant SLAs

2. **Cannot Implement Tenant-Specific Features**
   - Per-tenant rate limiting: Impossible
   - Per-tenant retention policies: Impossible
   - Per-tenant data export: Must export everything
   - Per-tenant billing/metering: Cannot attribute usage

3. **Security/Compliance Risk**
   - All customer data mixed in same tenant
   - Cannot implement tenant data isolation
   - GDPR data deletion requests cannot be scoped to tenant
   - Audit logs cannot be filtered by tenant

4. **Scalability Limitation**
   - Cannot partition Kafka by tenant
   - Cannot route tenants to different regions
   - Cannot scale per-tenant (all tenants share resources)

### Root Cause

**No Tenant Context Infrastructure**:
- Producer has no tenant configuration
- No way to pass tenant ID when creating producer
- No tenant middleware/context
- No tenant validation

### Fix Required

**Step 1: Add Tenant to Producer Config** (15 min):
```rust
// producer.rs
#[derive(Debug, Clone)]
pub struct ProducerConfig {
    pub bootstrap_servers: String,
    pub topic: String,
    pub enable_compression: bool,
    pub timeout: Duration,
    pub enable_idempotence: bool,
    pub max_in_flight: i32,
    pub kafka_compression: String,
    pub tenant_id: String,  // ✅ NEW: Tenant ID
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic: "dashstream-events".to_string(),
            enable_compression: true,
            timeout: Duration::from_secs(30),
            enable_idempotence: true,
            max_in_flight: 5,
            kafka_compression: "none".to_string(),
            tenant_id: "default".to_string(),  // ✅ Default for backward compat
        }
    }
}
```

**Step 2: Use Tenant from Config** (5 min):
```rust
pub fn create_header(
    &self,
    thread_id: &str,
    message_type: crate::MessageType,
) -> crate::Header {
    crate::Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: self.config.tenant_id.clone(),  // ✅ Use config value
        thread_id: thread_id.to_string(),
        sequence: 0,
        r#type: message_type as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    }
}
```

**Step 3: Add Tenant to Producer Constructor** (10 min):
```rust
impl DashFlow StreamingProducer {
    /// Create a new producer with tenant ID
    pub async fn new_with_tenant(
        bootstrap_servers: &str,
        topic: &str,
        tenant_id: &str,
    ) -> Result<Self> {
        let config = ProducerConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            tenant_id: tenant_id.to_string(),  // ✅ Set tenant
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create a new producer (uses "default" tenant for backward compat)
    pub async fn new(bootstrap_servers: &str, topic: &str) -> Result<Self> {
        Self::new_with_tenant(bootstrap_servers, topic, "default").await
    }
}
```

**Step 4: Add Tenant Validation** (30 min):
```rust
// Validate tenant ID format
fn validate_tenant_id(tenant_id: &str) -> Result<(), Error> {
    if tenant_id.is_empty() {
        return Err(Error::InvalidTenantId("Tenant ID cannot be empty".to_string()));
    }

    if tenant_id.len() > 64 {
        return Err(Error::InvalidTenantId("Tenant ID too long (max 64 chars)".to_string()));
    }

    // Allow: alphanumeric, dash, underscore
    if !tenant_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(Error::InvalidTenantId("Tenant ID contains invalid characters".to_string()));
    }

    Ok(())
}
```

**Step 5: Add Tenant to Quality Aggregator** (30 min):
```rust
// Read tenant from environment or config
let tenant_id = std::env::var("TENANT_ID").unwrap_or_else(|_| "default".to_string());

let producer = DashFlow StreamingProducer::new_with_tenant(
    &kafka_brokers,
    &kafka_topic,
    &tenant_id,  // ✅ Pass tenant
).await?;
```

**Step 6: Add Tenant to Docker Compose** (10 min):
```yaml
# docker-compose.dashstream.yml
quality-aggregator:
  environment:
    - TENANT_ID=customer-123  # ✅ Configurable per deployment
```

### Acceptance Criteria

- [ ] Producer accepts `tenant_id` in config
- [ ] `new_with_tenant()` constructor creates producer with custom tenant
- [ ] Messages have correct `tenant_id` in header (verify with Kafka console consumer)
- [ ] Tenant validation rejects empty/invalid tenant IDs
- [ ] Quality aggregator reads `TENANT_ID` from environment
- [ ] Docker compose allows configuring tenant per deployment
- [ ] Backward compatibility: `new()` still uses "default" tenant

### Time Estimate

**Total**: 1-2 hours
- Add tenant to config (15 min)
- Use tenant from config (5 min)
- Add constructor with tenant (10 min)
- Add tenant validation (30 min)
- Update quality aggregator (30 min)
- Update Docker compose (10 min)
- Test multi-tenant scenarios (20 min)

---

## Worker Directive: Fix All 5 In Order

### Priority Order

**Fix in this order** (most critical first):

1. **Issue #6 (Prometheus-Exporter)**: CRITICAL - Service completely broken
2. **Issue #7 (WebSocket Status)**: HIGH - System degraded, errors accumulating
3. **Issue #8 (Alerting)**: HIGH - Cannot detect future issues automatically
4. **Issue #9 (Sequence Numbers)**: MEDIUM - Data integrity gap
5. **Issue #10 (Tenant ID)**: MEDIUM - Multi-tenancy blocked

### Estimated Total Time

- Issue #6: 10 minutes
- Issue #7: 1-2 hours
- Issue #8: 2-3 hours
- Issue #9: 2-3 hours
- Issue #10: 1-2 hours

**Total**: 6-10 hours of work

### Commit Template

```
# <N++>: Fix Issue #X (of next 5) - [Title]

**Current Plan**: [MANAGER]_CRITICAL_NEXT_5_ISSUES_AFTER_STREAMING.md
**Issue**: #X - [Title]

## Evidence BEFORE Fix
[Show logs/metrics/config demonstrating issue]

## Changes Made
- File: docker-compose.dashstream.yml - Fixed KAFKA_BROKERS address
- OR: File: src/producer.rs - Added sequence tracking
- [List changes]

## Evidence AFTER Fix
[Show issue resolved - metrics working, tests passing, etc.]

## Status
First 5 issues: ✅ ALL FIXED
Next 5 issues: #6 [✅ FIXED / ❌ NOT STARTED], remaining: #7-10
```

---

## Success Criteria (ALL MUST PASS)

### Issue #6: Prometheus-Exporter Fixed ✅
```bash
curl localhost:8080/metrics | grep dashstream_messages_received
# Result: dashstream_messages_received_total{...} 1234 (value > 0)

docker logs dashstream-prometheus-exporter 2>&1 | grep -c "Failed to resolve"
# Result: 0 (zero resolution errors)
```

### Issue #7: WebSocket Server Healthy ✅
```bash
curl http://localhost:3002/health | jq '.status'
# Result: "healthy" (not "reconnecting")

curl http://localhost:3002/health | jq '.metrics.kafka_errors'
# Result: 0 (errors reset after recovery)
```

### Issue #8: Alerting Active ✅
```bash
curl http://localhost:9093/api/v2/alerts | jq '. | length'
# Result: 6 (6 alert rules configured)

# Test alert fires
curl -X POST http://localhost:9093/api/v1/alerts -d '[{"labels":{"alertname":"test"}}]'
# Result: Slack notification received
```

### Issue #9: Sequence Numbers Working ✅
```bash
# Send 5 messages, verify sequences 1-5
cargo test --package dashflow-streaming test_sequence_numbers
# Result: ok. sequences: 1, 2, 3, 4, 5

curl localhost:8080/metrics | grep dashstream_sequence_gaps_total
# Result: dashstream_sequence_gaps_total 0 (no gaps detected)
```

### Issue #10: Tenant ID Configurable ✅
```bash
TENANT_ID=customer-123 cargo run --bin quality_aggregator &
sleep 5

docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 \
  --topic dashstream-quality --from-beginning --max-messages 1 | \
  protoc --decode=Header proto/dashstream.proto | grep tenant_id
# Result: tenant_id: "customer-123"
```

---

## Why This Matters

**User Goal**: "what are the next 5 serious issues to fix after these 5?"

**Current Reality**:
- Prometheus-exporter 100% broken (10.2GB network waste!)
- WebSocket server degraded ("reconnecting" status)
- Zero automated alerting (issues found manually only)
- Sequence numbers always 0 (data integrity gap)
- Tenant ID always "default" (multi-tenancy impossible)

**Target State**:
- All services healthy and operational
- Metrics collection working (Grafana dashboards live)
- Automated alerting on 6 critical conditions
- Sequence tracking for message loss detection
- Multi-tenant deployments possible

**These 5 issues must be fixed before production deployment.**

---

## Next Worker: Fix All 5 In Order

Read this directive completely, fix Issues #6-10 in priority order, provide BEFORE/AFTER evidence for each.

**DO NOT skip issues. DO NOT claim "fixed" without runtime proof. SYSTEM MUST BE PRODUCTION-READY.**
