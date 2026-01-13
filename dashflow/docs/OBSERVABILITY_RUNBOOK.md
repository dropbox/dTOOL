# Observability Operational Runbook

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Version**: 1.3
**Purpose**: Response procedures for all observability alerts
**Audience**: AI workers

---

## Table of Contents

1. [Alert Response Overview](#alert-response-overview)
2. [DLQ Metrics: Which Service Exports What?](#dlq-metrics-which-service-exports-what-m-412)
3. [DLQ Durability Semantics](#dlq-durability-semantics-m-429)
4. [Critical Alerts (P0)](#critical-alerts-p0)
5. [High Severity Alerts (P1)](#high-severity-alerts-p1)
6. [Medium Severity Alerts (P2)](#medium-severity-alerts-p2)
7. [Forensic Analysis Tools](#forensic-analysis-tools)
8. [Escalation Matrix](#escalation-matrix)

---

## Alert Response Overview

**Alert Severities**:
- **Critical (P0)**: Immediate action required - data loss or system down
- **High (P1)**: Action within 15 minutes - significant degradation
- **Medium (P2)**: Action within 1 hour - performance degradation

**General Response Flow**:
1. **Acknowledge**: Confirm alert is firing
2. **Investigate**: Use diagnostic commands to identify root cause
3. **Remediate**: Apply fix or workaround
4. **Verify**: Confirm alert clears
5. **Document**: Note findings in git commit

---

## DLQ Metrics: Which Service Exports What? (M-412)

There are **two distinct DLQ metric namespaces** that serve different purposes:

### WebSocket Server DLQ (`websocket_dlq_*`)

**Service:** `websocket-server` (port 3002)
**Status:** Actively exported in production

The WebSocket server implements its own DLQ handling inline. When decode/validation fails, messages are written to the Kafka DLQ topic and these metrics are incremented.

| Metric | Description |
|--------|-------------|
| `websocket_dlq_sends_total` | Messages written to DLQ, by `error_type` label |
| `websocket_dlq_send_failures_total` | Failed DLQ writes, by `reason` label (timeout, kafka_error, backpressure) |

**Alerts that use these metrics:**
- `WebSocketDlqBroken` - DLQ writes are failing (P0 data loss)
- `WebSocketDlqHighRate` - High rate of messages going to DLQ (P1)

### Streaming Library DLQ (`dashstream_dlq_*`)

**Library:** `dashflow-streaming` crate (`DlqHandler` class)
**Status:** Exported only if a service uses `Consumer::with_dlq()` or `Producer::with_dlq()`

These metrics are emitted by the library's reusable `DlqHandler` component, designed for custom services.

| Metric | Description |
|--------|-------------|
| `dashstream_dlq_sends_total` | Library-level DLQ sends |
| `dashstream_dlq_send_failures_total` | Library-level send failures |
| `dashstream_dlq_dropped_total` | Messages dropped due to backpressure |
| `dashstream_dlq_send_retries_total` | Retry attempts |

**Current deployment note:** As of December 2025, no production services (quality_aggregator, prometheus-exporter) use the library's `DlqHandler`. The `dashstream_dlq_*` alerts exist for future/custom services that opt into this pattern.

**Alerts that use these metrics:**
- `HighDLQRate` - High library DLQ volume (fires only if service exports these metrics)
- `DLQItselfBroken` - Library DLQ handler broken
- `DLQBackpressureDrops` - Library DLQ dropping messages
- `DLQSendFailures` - Library DLQ send failures

---

## DLQ Durability Semantics (M-429)

The WebSocket server implements a **FAIL-OPEN** DLQ design. Understanding this is critical for operators.

### How It Works

1. When a message fails to decode/validate, it is sent to the DLQ **asynchronously** via `tokio::spawn`
2. The Kafka consumer **offset is committed regardless** of whether the DLQ send succeeds
3. DLQ sends are bounded by `MAX_CONCURRENT_DLQ_SENDS` (default: 100)

### Implications

| Scenario | Behavior |
|----------|----------|
| DLQ send succeeds | Message appears in DLQ topic for forensics |
| DLQ send times out | Message **lost from DLQ**; main pipeline continues |
| DLQ backpressure | Message **dropped**; `websocket_dlq_send_failures_total{reason="backpressure"}` incremented |
| Kafka DLQ topic down | Messages **lost from DLQ**; `websocket_dlq_send_failures_total{reason="kafka_error"}` incremented |

### Design Rationale

This design prioritizes **pipeline availability** over **forensic completeness**:

- A broken DLQ does NOT block real-time observability
- The primary data path (Kafka → WebSocket → UI) continues regardless of DLQ health
- Operators should monitor DLQ failure metrics but a DLQ incident is **not** a P0 data loss for the main pipeline

### Monitoring DLQ Health

```bash
# Check DLQ success rate
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_dlq_sends_total[5m])' | jq

# Check DLQ failure rate by reason
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_dlq_send_failures_total[5m])' | \
  jq '.data.result[] | {reason: .metric.reason, rate: .value[1]}'
```

### Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `MAX_CONCURRENT_DLQ_SENDS` | 100 | Max concurrent async DLQ send tasks |
| `DLQ_SEND_TIMEOUT_SECS` | 5 | Timeout for each DLQ producer send |
| `KAFKA_DLQ_TOPIC` | `${KAFKA_TOPIC}-dlq` | DLQ topic name |

### Future Consideration

A "fail-closed" mode (`DLQ_FAIL_CLOSED=true`) that blocks offset commit until DLQ succeeds is not currently implemented. This would trade availability for durability - DLQ failures would cause consumer lag.

---

## Critical Alerts (P0)

### 1. SequenceGapsDetected

**Meaning**: Messages are being lost between producer and consumer. This indicates potential data corruption or Kafka broker issues.

**Alert Definition**:
```yaml
expr: sum(rate(dashstream_sequence_gaps_total[5m])) > 0
for: 2m
severity: critical
```

**Investigation Steps**:

```bash
# 1. Check current gap rate
curl -s 'http://localhost:9090/api/v1/query?query=sum(rate(dashstream_sequence_gaps_total[5m]))' | \
  jq '.data.result[] | {gaps_per_sec: .value[1]}'

# 2. Check Kafka broker health
docker logs dashstream-kafka --tail=100 | grep -E "ERROR|WARN|partition"

# 3. Check WebSocket server health
curl -s http://localhost:3002/health | jq '{status, kafka_status, websocket_status, alert}'

# 4. Find affected traces in Jaeger
curl -s "http://localhost:16686/api/traces?service=websocket-server&lookback=15m&limit=20" | \
  jq '.data[] | select(.spans[].tags[] | select(.key=="sequence_gap")) | {traceID, duration}'

# 5. Check Kafka topic health
docker exec dashstream-kafka kafka-topics \
  --describe --bootstrap-server localhost:9092 \
  --topic dashstream-quality
```

**Common Causes**:
- Kafka broker restart or crash
- Network partition between producer and Kafka
- Consumer group rebalancing
- Producer acknowledgment misconfiguration

**Remediation**:

```bash
# If Kafka broker is unhealthy
docker restart dashstream-kafka
sleep 10
docker logs dashstream-kafka --tail=50

# If consumer is lagged
docker restart dashstream-websocket-server
sleep 5
curl http://localhost:3002/health

# If producer is misconfigured (check acks setting)
# Edit producer config to set acks=all
docker restart dashstream-quality-monitor
```

**Verification**:
```bash
# Alert should clear within 5 minutes if fixed
curl -s http://localhost:9090/api/v1/alerts | \
  jq '.data.alerts[] | select(.labels.alertname == "SequenceGapsDetected") | {state, value}'
```

**Escalation**: If gaps persist > 10 minutes with healthy Kafka, this is a critical bug requiring immediate developer attention.

---

### 2. WebSocketDlqBroken

**Meaning**: The WebSocket server’s DLQ cannot write failed messages. Failed decode/validation messages are being **LOST**. This is a P0 data loss incident.

**Alert Definition**:
```yaml
expr: sum(rate(websocket_dlq_send_failures_total[5m])) > 0
for: 1m
severity: critical
```

**Investigation Steps**:

```bash
# 1. Check DLQ failure rate and reasons
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_dlq_send_failures_total[5m])' | \
  jq '.data.result[] | {reason: .metric.reason, failures_per_sec: .value[1]}'

# 2. Verify DLQ topic exists
docker exec dashstream-kafka kafka-topics \
  --list --bootstrap-server localhost:9092 | grep dlq

# 3. Check WebSocket server DLQ producer logs
docker logs dashstream-websocket-server 2>&1 | \
  grep -i "dlq" | tail -50

# 4. Check Kafka broker capacity
docker exec dashstream-kafka df -h /var/lib/kafka

# 5. Test DLQ topic write access
docker exec dashstream-kafka kafka-console-producer \
  --broker-list localhost:9092 \
  --topic dashstream-quality-dlq <<< "test message"  # default: ${KAFKA_TOPIC:-dashstream-quality}-dlq
```

**Common Causes**:
- DLQ topic doesn't exist (misconfigured)
- Kafka broker out of disk space
- WebSocket server DLQ producer connection failed
- Kafka broker crashed

**Remediation**:

```bash
# Create DLQ topic if missing
docker exec dashstream-kafka kafka-topics \
  --create --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --partitions 1 --replication-factor 1

# Restart WebSocket server if producer broken
docker restart dashstream-websocket-server
sleep 5
curl http://localhost:3002/health | jq '.dlq_health'

# If Kafka out of space - emergency cleanup
docker exec dashstream-kafka kafka-configs \
  --bootstrap-server localhost:9092 \
  --alter --entity-type topics \
  --entity-name dashstream-quality \
  --add-config retention.ms=86400000  # Reduce to 1 day
```

**Verification**:
```bash
# Send test message and verify DLQ write works
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning --max-messages 1 --timeout-ms 5000
```

**Escalation**: **IMMEDIATE** - This is P0 data loss. Notify on-call engineer immediately.

---

### 3. ServiceDown

**Meaning**: A critical service (Prometheus, WebSocket server, Kafka) is unreachable.

**Alert Definition**:
```yaml
expr: up == 0
for: 1m
severity: critical
```

**Investigation Steps**:

```bash
# 1. Check which services are down
curl -s http://localhost:9090/api/v1/targets | \
  jq '.data.activeTargets[] | select(.health != "up") | {job: .labels.job, lastError: .lastError}'

# 2. Check Docker container status
docker-compose -f docker-compose.dashstream.yml ps

# 3. Check container logs for crashed service
docker logs dashstream-websocket-server --tail=100
docker logs dashstream-prometheus --tail=100
docker logs dashstream-kafka --tail=100
```

**Remediation**:

```bash
# Restart affected service
docker restart <container-name>

# If entire stack is down
docker-compose -f docker-compose.dashstream.yml restart

# If restart fails, check for port conflicts
lsof -i :3002  # WebSocket server
lsof -i :9090  # Prometheus
lsof -i :9092  # Kafka
```

**Escalation**: If service won't restart after 3 attempts, requires developer investigation.

---

## High Severity Alerts (P1)

### 4. HighMessageProcessingErrorRate (formerly HighKafkaErrorRate - M-416)

**Meaning**: More than 1% of Kafka messages are failing to PROCESS (decode errors, bad protobuf). This is NOT a Kafka infrastructure alert - it measures message content issues.

> **Note (M-416)**: This alert was renamed from `HighKafkaErrorRate` because the metric
> (`websocket_kafka_messages_total{status="error"}`) actually counts decode/processing
> failures, NOT Kafka broker errors. For actual Kafka infrastructure issues, check
> `websocket_infrastructure_errors_total` or the health endpoint's `kafka_status`.

**Alert Definition**:
```yaml
# M-416: Renamed to HighMessageProcessingErrorRate for clarity
# Uses sum() for label aggregation and clamp_min to prevent div-by-zero
expr: sum(rate(websocket_kafka_messages_total{status="error"}[5m])) / clamp_min(sum(rate(websocket_kafka_messages_total[5m])), 1e-9) > 0.01
for: 5m
severity: critical
```

**Investigation Steps**:

```bash
# 1. Check message processing error rate (decode failures)
curl -s 'http://localhost:9090/api/v1/query?query=sum(rate(websocket_kafka_messages_total{status="error"}[5m]))/clamp_min(sum(rate(websocket_kafka_messages_total[5m])),1e-9)' | \
  jq '.data.result[0].value[1]'

# 2. Check decode error breakdown by type (more specific)
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_decode_errors_total[5m])' | \
  jq '.data.result[] | {error_type: .metric.error_type, rate: .value[1]}'

# 3. Check WebSocket server logs for decode errors
docker logs dashstream-websocket-server --tail=100 | grep -i "decode\|protobuf\|error"

# 4. Check DLQ for failed message details (forensic analysis)
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning --max-messages 5 | jq '.'

# 5. Check producer version compatibility
docker logs dashstream-quality-monitor --tail=50 | grep -i "version\|schema"
```

**Common Causes**:
- Protobuf schema version mismatch between producer and consumer
- Corrupted messages from producer bugs
- Network corruption (rare)
- Producer sending non-protobuf data
- Old/stale messages with outdated schema

**Remediation**:

```bash
# 1. Check producer and consumer use same protobuf schema version
# Both should use dashflow-streaming = "1.11.x"

# 2. If schema mismatch, restart producer with correct version
docker restart dashstream-quality-monitor
sleep 10
docker logs dashstream-quality-monitor --tail=20

# 3. If old data is causing issues, skip old messages by restarting consumer
# (WebSocket server skips messages older than session_start by default)
docker restart dashstream-websocket-server
sleep 5
curl http://localhost:3002/health | jq '.status'

# 4. Verify error rate returns to 0
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_kafka_messages_total{status="error"}[5m])' | \
  jq '.data.result'
```

**Verification**:
```bash
# Alert should clear within 10 minutes if fixed
curl -s http://localhost:9090/api/v1/alerts | \
  jq '.data.alerts[] | select(.labels.alertname == "HighMessageProcessingErrorRate") | {state, value}'
```

**Escalation**: If error rate persists > 15 minutes after remediation, check producer code for bugs or schema incompatibility.

---

### 5. HighDecodeErrorRate

**Meaning**: More than 5% of Kafka messages cannot be decoded. Indicates protobuf schema mismatch or data corruption.

**Alert Definition**:
```yaml
# S-20: Uses sum() for aggregation, total messages (all statuses) in denominator, clamp_min for safety
expr: sum(rate(websocket_decode_errors_total[5m])) / clamp_min(sum(rate(websocket_kafka_messages_total[5m])), 1e-9) > 0.05
for: 2m
severity: high
```

**Investigation Steps**:

```bash
# 1a. Check aggregate decode error rate (matches alert PromQL)
curl -s 'http://localhost:9090/api/v1/query?query=sum(rate(websocket_decode_errors_total[5m]))/clamp_min(sum(rate(websocket_kafka_messages_total[5m])),1e-9)' | \
  jq '.data.result[0].value[1]'

# 1b. Check decode errors by type (for root cause analysis)
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_decode_errors_total[5m])' | \
  jq '.data.result[] | {error_type: .metric.error_type, rate: .value[1]}'

# 2. Check DLQ for failed messages
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning --max-messages 10 | jq .

# 3. Verify protobuf schema version
docker logs dashstream-quality-monitor | grep "protobuf version"
docker logs dashstream-websocket-server | grep "protobuf version"

# 4. Sample raw Kafka messages
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality \
  --max-messages 5 | xxd | head -50
```

**Remediation**:
- If schema mismatch: Rebuild and restart producer/consumer with matching protobuf
- If data corruption: Restart Kafka broker and verify disk health
- If persistent: Skip corrupted offset range and alert developer

---

### 6. HighDroppedMessageRate

**Meaning**: WebSocket server is dropping messages due to slow clients (lagged receivers).

**Alert Definition**:
```yaml
expr: rate(websocket_dropped_messages_total{reason="lagged_receiver"}[5m]) > 0
for: 2m
severity: high
```

**Investigation Steps**:

```bash
# 1. Check drop rate
curl -s http://localhost:3002/metrics | grep websocket_dropped_messages_total

# 2. Check client lag metrics
curl -s 'http://localhost:9090/api/v1/query?query=websocket_client_lag_messages_total' | \
  jq '.data.result'

# 3. Check connected clients
curl -s http://localhost:3002/health | jq '.connected_clients'
```

**Remediation**:
- Increase broadcast channel buffer size (requires code change)
- Disconnect slow clients (automatic)
- Optimize client processing speed

---

### 7. WebSocketDlqHighRate

**Meaning**: More than 0.1 messages/second are failing and being written to DLQ. High failure rate requiring investigation.

**Alert Definition**:
```yaml
expr: sum(rate(websocket_dlq_sends_total[5m])) > 0.1
for: 2m
severity: high
```

**Investigation Steps**:

```bash
# 1. Check DLQ write rate by error type
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_dlq_sends_total[5m])' | \
  jq '.data.result[] | {error_type: .metric.error_type, rate: .value[1]}'

# 2. Read recent DLQ messages
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning --max-messages 20 | jq '.error_type' | sort | uniq -c

# 3. Check if producer is sending bad data
docker logs dashstream-quality-monitor --tail=100 | grep ERROR
```

**Remediation**:
- Identify error pattern from DLQ messages
- Fix producer if sending malformed data
- Update consumer to handle new message formats

---

### 7b. DashStreamDlqBackpressureDrops

**Meaning**: Messages are being dropped because the DLQ handler is overwhelmed. Forensic data is being permanently lost.

**Alert Definition**:
```yaml
expr: rate(dashstream_dlq_dropped_total[5m]) > 0
for: 5m
severity: high
```

**Note**: `dashstream_dlq_dropped_total` is a "lazy" metric - it only appears after the first drop event. If no data exists in Prometheus, that's normal (no drops have occurred).

**WebSocket server note**: The WebSocket DLQ does not export a separate “dropped” counter; DLQ backpressure appears as `websocket_dlq_send_failures_total{reason="backpressure"}`.

**Investigation Steps**:

```bash
# 1. Check drop rate
curl -s 'http://localhost:9090/api/v1/query?query=rate(dashstream_dlq_dropped_total[5m])' | \
  jq '.data.result'

# WebSocket server backpressure failures (if using websocket DLQ)
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_dlq_send_failures_total{reason=\"backpressure\"}[5m])' | \
  jq '.data.result'

# 2. Check DLQ send rate vs failures
curl -s 'http://localhost:9090/api/v1/query?query=rate(dashstream_dlq_sends_total[5m])' | \
  jq '.data.result'
curl -s 'http://localhost:9090/api/v1/query?query=rate(dashstream_dlq_send_failures_total[5m])' | \
  jq '.data.result'

# 3. Check Kafka DLQ topic lag and throughput
docker exec dashstream-kafka kafka-consumer-groups \
  --bootstrap-server localhost:9092 \
  --describe --all-groups | grep dlq
```

**Remediation**:
- Increase DLQ handler concurrency/capacity
- Check Kafka DLQ topic for partition count and throughput limits
- Investigate root cause of high DLQ volume (fix upstream decode errors)

---

### 7c. KafkaConsumerLagHigh (M-419/M-428)

**Meaning**: The WebSocket server is >10,000 messages behind the Kafka high watermark. Real-time observability data is delayed.

**Alert Definition**:
```yaml
# M-419: Kafka consumer lag monitoring
expr: max(websocket_kafka_consumer_lag) > 10000
for: 5m
severity: high
```

**Dashboard Panel**: See "Kafka Consumer Lag by Partition" in Grafana Quality Dashboard (panels 24-26).

**Investigation Steps**:

```bash
# 1. Check current lag by partition
curl -s 'http://localhost:9090/api/v1/query?query=websocket_kafka_consumer_lag' | \
  jq '.data.result[] | {partition: .metric.partition, lag: .value[1]}'

# 2. Check max lag across all partitions
curl -s 'http://localhost:9090/api/v1/query?query=max(websocket_kafka_consumer_lag)' | \
  jq '.data.result[0].value[1]'

# 3. Check lag trend (positive = falling behind, negative = catching up)
curl -s 'http://localhost:9090/api/v1/query?query=deriv(max(websocket_kafka_consumer_lag)[5m:1m])' | \
  jq '.data.result[0].value[1]'

# 4. Check WebSocket server resource usage
docker stats dashstream-websocket-server --no-stream

# 5. Check Kafka message throughput
curl -s 'http://localhost:9090/api/v1/query?query=rate(websocket_kafka_messages_total[5m])' | \
  jq '.data.result'

# 6. Check E2E latency (should correlate with lag)
curl -s 'http://localhost:9090/api/v1/query?query=histogram_quantile(0.99,rate(websocket_e2e_latency_ms_bucket[5m]))' | \
  jq '.data.result'
```

**Common Causes**:
- **Consumer CPU-bound**: Too many messages, consumer can't keep up
- **Network latency**: Slow Kafka fetch due to network issues
- **Slow processing**: Complex message handling (decode, transform, broadcast)
- **Producer burst**: Sudden spike in message production
- **Kafka broker issues**: Slow broker responses

**Remediation**:

```bash
# 1. Check if consumer is CPU-bound
docker stats dashstream-websocket-server --no-stream
# If CPU > 80%, consider scaling or optimizing

# 2. Restart consumer to reset state and catch up
docker restart dashstream-websocket-server
sleep 5
curl http://localhost:3002/health

# 3. If lag persists after restart, check producer rate
# May need to scale consumers or add partitions

# 4. Verify lag is decreasing after remediation
curl -s 'http://localhost:9090/api/v1/query?query=deriv(max(websocket_kafka_consumer_lag)[2m:30s])' | \
  jq '.data.result[0].value[1]'
# Value should be negative (catching up)
```

**Verification**:
```bash
# Alert should clear when lag drops below 10K
curl -s http://localhost:9090/api/v1/alerts | \
  jq '.data.alerts[] | select(.labels.alertname == "KafkaConsumerLagHigh") | {state, value}'
```

**Escalation**: If lag continues to grow after restart, investigate producer throughput and consider scaling.

---

### 7d. KafkaConsumerLagCritical (M-419/M-428)

**Meaning**: The WebSocket server is >100,000 messages behind. Real-time observability is severely degraded. This is a P0 (critical) alert that requires immediate attention.

**Alert Definition**:
```yaml
# M-419: Critical lag - real-time observability broken
expr: max(websocket_kafka_consumer_lag) > 100000
for: 2m
severity: critical
```

**Investigation Steps**:
Same as KafkaConsumerLagHigh above, but with urgency.

```bash
# Quick status check
curl -s 'http://localhost:9090/api/v1/query?query=max(websocket_kafka_consumer_lag)' | \
  jq '.data.result[0].value[1]'

# Check consumer health
curl -s http://localhost:3002/health | jq '.'
```

**Remediation**:

```bash
# 1. Immediate: Restart consumer
docker restart dashstream-websocket-server
sleep 5

# 2. Monitor catch-up rate
watch -n 5 'curl -s "http://localhost:9090/api/v1/query?query=max(websocket_kafka_consumer_lag)" | jq ".data.result[0].value[1]"'

# 3. If not catching up, consider:
#    - Skipping old messages (restart with fresh consumer offset)
#    - Scaling consumers (if using multiple partitions)
#    - Reducing producer throughput temporarily
```

**Critical Note**: At 100K+ lag, clients are receiving stale data. Consider alerting stakeholders that real-time observability is degraded until lag recovers.

**Escalation**: If lag > 100K persists for > 10 minutes, escalate to on-call for production impact assessment.

---

## Medium Severity Alerts (P2)

### 8. E2ELatencyHigh

**Meaning**: P99 end-to-end latency from Kafka to WebSocket exceeds 500ms for 5 minutes.

**Investigation Steps**:

```bash
# Check latency percentiles
# Note: websocket_e2e_latency_ms is a "lazy" metric - it only appears after events are processed.
# If no data is returned, this is normal for idle systems.
curl -s 'http://localhost:9090/api/v1/query?query=histogram_quantile(0.99, rate(websocket_e2e_latency_ms_bucket[5m]))' | \
  jq '.data.result'

# Check Kafka consumer group lag (set KAFKA_GROUP_ID if customized)
docker exec dashstream-kafka kafka-consumer-groups \
  --bootstrap-server localhost:9092 \
  --group ${KAFKA_GROUP_ID:-websocket-server-v4} \
  --describe
```

**Remediation**:
- Restart WebSocket server if CPU-bound
- Scale Kafka partitions if throughput-limited
- Optimize message processing

---

### 9. HighDuplicateRate

**Meaning**: More than 0.1 duplicate messages/second detected. Indicates Kafka at-least-once redelivery.

**Investigation Steps**:

```bash
# Check duplicate rate
curl -s 'http://localhost:9090/api/v1/query?query=rate(dashstream_sequence_duplicates_total[5m])' | \
  jq '.data.result'

# Check Kafka consumer group rebalances
docker exec dashstream-kafka kafka-consumer-groups \
  --bootstrap-server localhost:9092 \
  --group ${KAFKA_GROUP_ID:-websocket-server-v4} \
  --describe
```

**Remediation**:
- Normal if consumer group rebalancing
- Check for network instability if persistent
- Implement deduplication if required

---

### 10. HighReorderRate

**Meaning**: More than 0.1 out-of-order messages/second detected. Partition ordering may be compromised.

**Investigation Steps**:

```bash
# Check reorder rate
curl -s 'http://localhost:9090/api/v1/query?query=rate(dashstream_sequence_reorders_total[5m])' | \
  jq '.data.result'

# Verify single partition (ordering guarantee)
docker exec dashstream-kafka kafka-topics \
  --describe --bootstrap-server localhost:9092 \
  --topic dashstream-quality
```

**Remediation**:
- Should not occur with single partition
- If multi-partition: messages from different partitions may interleave
- Check producer partition key if ordering critical

---

## Forensic Analysis Tools

### DLQ Message Inspection

```bash
# Read all DLQ messages
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning | jq . > /tmp/dlq_messages.json

# Analyze error types
cat /tmp/dlq_messages.json | jq -r '.error_type' | sort | uniq -c | sort -rn

# Find messages from specific thread
cat /tmp/dlq_messages.json | jq 'select(.original_message.thread_id == "thread-1")'
```

### Jaeger Trace Correlation

```bash
# Find trace by ID from DLQ message
TRACE_ID="<trace_id_from_dlq>"
curl -s "http://localhost:16686/api/traces/${TRACE_ID}" | \
  jq '.data[0].spans[] | {operationName, startTime, duration, tags}'

# Search for traces with errors
curl -s "http://localhost:16686/api/traces?service=websocket-server&tags={\"error\":\"true\"}" | \
  jq '.data[] | {traceID, duration, spans: .spans | length}'
```

### Kafka Offset Analysis

```bash
# Check consumer group offsets
docker exec dashstream-kafka kafka-consumer-groups \
  --bootstrap-server localhost:9092 \
  --group websocket-server-v4 \
  --describe

# Get topic offset range
docker exec dashstream-kafka kafka-run-class kafka.tools.GetOffsetShell \
  --broker-list localhost:9092 \
  --topic dashstream-quality --time -1

# Manually consume from specific offset
docker exec dashstream-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream-quality \
  --partition 0 --offset 1000 --max-messages 10
```

### Prometheus Query Examples

```bash
# All firing alerts
curl -s http://localhost:9090/api/v1/alerts | \
  jq '.data.alerts[] | select(.state == "firing")'

# Metric cardinality (check for label explosion)
curl -s 'http://localhost:9090/api/v1/query?query=count by (__name__) ({__name__=~".+"})' | \
  jq '.data.result | sort_by(-.value[1] | tonumber) | .[:20]'

# Scrape target health
curl -s http://localhost:9090/api/v1/targets | \
  jq '.data.activeTargets[] | {job: .labels.job, health: .health, scrapeInterval: .scrapeInterval}'
```

---

## Escalation Matrix

| Alert | Severity | Response Time | Escalation Threshold |
|-------|----------|--------------|---------------------|
| SequenceGapsDetected | P0 | Immediate | > 10 min persistent |
| WebSocketDlqBroken | P0 | Immediate | Immediate (data loss) |
| ServiceDown | P0 | Immediate | After 3 restart attempts |
| HighMessageProcessingErrorRate | P1 | 15 min | > 15 min persistent |
| HighDecodeErrorRate | P1 | 15 min | > 30 min persistent |
| HighDroppedMessageRate | P1 | 15 min | > 1 hour persistent |
| WebSocketDlqHighRate | P1 | 15 min | > 30 min persistent |
| E2ELatencyHigh | P2 | 1 hour | > 2 hours persistent |
| HighDuplicateRate | P2 | 1 hour | > 4 hours persistent |
| HighReorderRate | P2 | 1 hour | If gaps also detected |

**Escalation Contact**: Open GitHub issue with logs, metrics, and investigation summary.

---

## Quick Reference Commands

```bash
# Check all alert statuses
curl -s http://localhost:9090/api/v1/alerts | jq '.data.alerts[] | {alert: .labels.alertname, state: .state}'

# Check system health
curl http://localhost:3002/health | jq .
curl http://localhost:9090/-/healthy
curl http://localhost:16686/api/services | jq '.data | length'

# Restart entire stack
docker-compose -f docker-compose.dashstream.yml restart

# View all logs
docker-compose -f docker-compose.dashstream.yml logs -f

# Clear Prometheus WAL (if corrupted)
docker stop dashstream-prometheus
docker exec dashstream-prometheus rm -rf /prometheus/wal/*
docker start dashstream-prometheus
```

---

## Alert: GapDetectedWithHalt

**Severity**: Critical
**Trigger**: Sequence gap detected with Halt policy - consumption stopped for a thread

**Symptoms**:
- Logs show "❌ HALTED thread <thread_id> due to gap"
- Specific thread stops processing messages
- Metric `dashstream_sequence_gaps_total` incrementing

**Response Steps**:

1. **Assess Gap Size**:
```bash
grep "HALTED thread" /var/log/websocket-server.log
```

2. **Check if Messages Recoverable from Kafka**:
```bash
# Get current offset for the thread
kafka-consumer-groups --bootstrap-server localhost:9092 \
  --group websocket-server-v4 --describe
```

3. **Reset Halted Thread** (if data loss acceptable):
```bash
# Restart the consumer to reset state
docker-compose restart dashstream-websocket-server
```

---

## Alert: SchemaVersionMismatch

**Severity**: Warning
**Trigger**: Message with incompatible schema version detected

**Response Steps**:

1. **Check DLQ for schema mismatch messages**:
```bash
kafka-console-consumer --bootstrap-server localhost:9092 \
  --topic dashstream-quality-dlq \
  --from-beginning | jq 'select(.error_type=="schema_mismatch")'
```

2. **Coordinate rolling upgrade or adjust compatibility policy**

---

## Alert: OversizedMessage

**Severity**: Warning
**Trigger**: Message exceeds 1MB size limit

**Response Options**:
- Increase `max_message_size` in config (if justified)
- Enable aggressive compression
- Use state diffs instead of full checkpoints
- Externalize large payloads to S3/Redis

---

## Configuration Reference

### Gap Recovery Policies

```rust
// Default: WarnAndContinue (log but continue)
SequenceValidator::new()

// Halt on gaps (require manual reset)
SequenceValidator::with_policy(GapRecoveryPolicy::Halt)

// Continue (accept data loss)
SequenceValidator::with_policy(GapRecoveryPolicy::Continue)
```

### Offset Reset Policy

```bash
# Production (no data loss on restart)
KAFKA_AUTO_OFFSET_RESET=earliest

# Testing (skip old messages)
KAFKA_AUTO_OFFSET_RESET=latest
```

---

## Alert: TenantRateLimitExceeded

**Severity**: Warning
**Trigger**: Tenant hitting rate limit >10 times in 5 minutes

**Symptoms**:
- Producer errors: "Rate limit exceeded for tenant: <tenant_id>"
- Metric `dashstream_rate_limit_exceeded_total{tenant_id}` incrementing
- Messages being dropped from specific tenant

**Response Steps**:

1. **Identify Affected Tenant**:
```bash
curl -s 'http://localhost:9090/api/v1/query?query=rate(dashstream_rate_limit_exceeded_total[5m])' | \
  jq '.data.result[] | {tenant: .metric.tenant_id, rate: .value[1]}'
```

2. **Assess if Legitimate Traffic**:
```bash
# Check total message volume for tenant
curl -s 'http://localhost:9090/api/v1/query?query=sum(rate(dashstream_rate_limit_allowed_total{tenant_id="TENANT"}[5m]))' | \
  jq '.data.result[0].value[1]'
```

3. **Remediation Options**:

**Option A**: Increase tenant quota (if justified)
```rust
rate_limiter.set_tenant_limit(
    "tenant-id".to_string(),
    RateLimit {
        messages_per_second: 500.0,  // Increased from 100
        burst_capacity: 5000,         // Increased from 1000
    },
).await;
```

**Option B**: Contact tenant to reduce send rate
```bash
# Document current usage and communicate limits
```

**Option C**: Investigate if misconfiguration causing excessive sends
```bash
# Check for retry loops or infinite loops in tenant code
```

4. **Monitor Resolution**:
```bash
# Verify rate limit exceeded stops incrementing
watch 'curl -s http://localhost:9090/api/v1/query?query=dashstream_rate_limit_exceeded_total | jq'
```

---

## Replay Buffer Operations

### Check Replay Buffer Health

```bash
# Check memory vs Redis hit rates
curl http://localhost:3002/metrics | grep replay_buffer

# Expected: memory_hits >> redis_hits (95%+ from memory)
```

### Manual Redis Inspection

```bash
# Connect to Redis
redis-cli -h localhost -p 6379

# M-691: Replay buffer keys are namespaced to prevent cross-topic/cluster collisions.
# Fetch the namespace from the websocket-server:
RESUME_NAMESPACE=$(curl -s http://localhost:3002/version | jq -r '.resume_namespace // empty')

# Check replay buffer keys (prefer SCAN over KEYS)
SCAN 0 MATCH "dashstream-replay:${RESUME_NAMESPACE}:partition:*:offsets" COUNT 100
SCAN 0 MATCH "dashstream-replay:${RESUME_NAMESPACE}:thread:*:sequences" COUNT 100

# Get a specific replayed message (partition+offset)
GET "dashstream-replay:${RESUME_NAMESPACE}:partition:0:offset:42"

# Check per-partition offset index
ZRANGE "dashstream-replay:${RESUME_NAMESPACE}:partition:0:offsets" 0 -1 WITHSCORES

# Clear replay buffer (if needed)
DEL "dashstream-replay:${RESUME_NAMESPACE}:partition:0:offsets"
```

---

**For Questions**: See `docs/OBSERVABILITY_INFRASTRUCTURE.md` for system architecture and component details.
