# Prometheus Metrics for DashFlow

This document provides an overview of Prometheus metrics in the DashFlow ecosystem.

## Canonical Source

**The authoritative documentation for metrics is in the crate that emits them:**

- **Quality Monitoring Metrics**: See [`crates/dashflow-prometheus-exporter/README.md`](../crates/dashflow-prometheus-exporter/README.md)
- **Registry API Metrics**: See the registry documentation
- **WebSocket Server Metrics**: See `crates/dashflow-observability/src/bin/websocket_server/main.rs`

## Quality Monitor Metrics (dashstream_*)

The Prometheus Exporter bridges DashFlow Streaming quality events from Kafka to Prometheus.

### Build Info

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashflow_build_info` | Gauge | `version`, `commit`, `build_date`, `rust_version` | Build and version information (always 1, metadata on labels) |

### Core Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_quality_monitor_quality_score` | Gauge | - | Current quality score (0.0-1.0 scale) |
| `dashstream_quality_monitor_queries_total` | Counter | - | Total queries processed |
| `dashstream_quality_monitor_queries_passed_total` | Counter | - | Queries that passed quality threshold |
| `dashstream_quality_monitor_queries_failed_total` | CounterVec | `category` | Queries that failed quality threshold (category: Simple, Medium, Complex, Edge, Unknown) |
| `dashstream_query_latency_ms` | Histogram | - | Query latency distribution (buckets: 10-30000ms) |
| `dashstream_quality_retry_count` | HistogramVec | `status` | Retry count distribution (status: passed, failed) |

### Granular Quality Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `dashstream_quality_accuracy` | Gauge | Quality accuracy score (0.0-1.0) |
| `dashstream_quality_relevance` | Gauge | Quality relevance score (0.0-1.0) |
| `dashstream_quality_completeness` | Gauge | Quality completeness score (0.0-1.0) |

### Per-Model Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_quality_score_by_model` | HistogramVec | `model` | Quality score distribution by model (0.0-1.0). Buckets configurable via `PROMETHEUS_QUALITY_SCORE_BUCKETS` env var (default: 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0). M-527: Changed from Gauge to Histogram to track distribution instead of just last value. |
| `dashstream_queries_by_model_total` | CounterVec | `model` | Total queries by model |
| `dashstream_latency_by_model_ms` | HistogramVec | `model` | Query latency by model (buckets: 10-30000ms) |

### Session Tracking

| Metric | Type | Description |
|--------|------|-------------|
| `dashstream_turns_by_session` | Histogram | Number of turns per session (buckets: 1, 2, 5, 10, 20, 50). M-528: Now tracks max turns per session and observes only on session timeout (`PROMETHEUS_SESSION_TIMEOUT_SECS` env var, default 300s) or shutdown. Sessions are tracked by session_id to prevent lower bucket inflation. |

### Application-Specific Metrics (Librarian)

**Source:** Prometheus Exporter (derived from Kafka quality events)

The librarian metrics are aggregated from Kafka events. Prefixed with `dashstream_` (S-9) to distinguish from direct app instrumentation.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_librarian_requests_total` | Counter | - | Total librarian requests derived from Kafka |
| `dashstream_librarian_iterations` | Gauge | - | Last observed librarian iterations (turn_number) |
| `dashstream_librarian_tests_total` | CounterVec | `status` | Librarian test results (status: passed, failed) |
| `dashstream_librarian_request_duration_seconds` | Histogram | - | Request duration in seconds (buckets: 0.01-10s) |

### Exporter Self-Monitoring Metrics

**Source:** Prometheus Exporter (self-monitoring for operational visibility)

These metrics monitor the health of the prometheus-exporter itself. Prefixed with `dashstream_exporter_` to distinguish from data pipeline metrics.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_exporter_process_start_time_seconds` | Gauge | - | Unix timestamp when exporter started (detects restarts) |
| `dashstream_exporter_metrics_endpoint_duration_seconds` | Histogram | - | Time to encode /metrics response (detects high-cardinality slowdowns) |
| `dashstream_exporter_messages_received_total` | Counter | - | Total Kafka messages received (for throughput: `rate(...)`) |
| `dashstream_exporter_messages_failed_total` | CounterVec | `error_type` | Message processing failures (error_type: decode/process/unknown) |
| `dashstream_exporter_kafka_consumer_errors_total` | Counter | - | Kafka consumer errors (connectivity/protocol issues) |
| `dashstream_exporter_offset_store_errors_total` | Counter | - | Offset storage failures (may cause duplicates) |
| `dashstream_exporter_last_event_timestamp_seconds` | Gauge | - | Timestamp of last processed event (for staleness alerting) |
| `dashstream_exporter_messages_wrong_scope_total` | Counter | - | Messages with non-quality scope (high count indicates misconfiguration) |
| `dashstream_exporter_messages_missing_header_total` | Counter | - | Quality messages missing header field (protocol errors) |
| `dashstream_exporter_gauges_last_update_timestamp_seconds` | Gauge | - | Timestamp of last gauge update (for staleness detection) |
| `dashstream_exporter_kafka_consumer_lag` | Gauge | - | Sum of consumer lag across all assigned partitions |

**Recommended Alerts:**
- `rate(dashstream_exporter_messages_failed_total[5m]) > 0.1` - Processing errors
- `time() - dashstream_exporter_last_event_timestamp_seconds > 300` - No events for 5 minutes
- `time() - dashstream_exporter_gauges_last_update_timestamp_seconds > 300` - Stale gauge metrics (no updates for 5 minutes)
- `dashstream_exporter_kafka_consumer_errors_total > 0` - Kafka connectivity issues
- `dashstream_exporter_kafka_consumer_lag > 10000` - High consumer lag (falling behind)
- `dashstream_exporter_messages_wrong_scope_total > 100` - Possible misconfiguration (many non-quality messages)

## WebSocket Server Metrics (websocket_*)

Emitted by `/metrics` endpoint on the websocket server (port 3002 by default).

### Core Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `websocket_kafka_messages_total` | Counter | `status` | Total Kafka messages processed (status=success/error/old_data_error; success=decoded, error=new decode failure, old_data_error=skipped old data decode failure) |
| `websocket_decode_errors_total` | CounterVec | `error_type` | Protobuf decode errors by type (payload_too_large, buffer_underflow, invalid_protobuf, schema_version_mismatch, unknown_decode_error) |
| `websocket_connected_clients` | Gauge | - | Current number of connected WebSocket clients |
| `websocket_dropped_messages_total` | Counter | `reason` | Messages dropped (reason=lagged_receiver) |
| `websocket_uptime_seconds` | Gauge | - | Server uptime in seconds |
| `websocket_kafka_consumer_lag` | GaugeVec | `partition` | Kafka consumer lag (high watermark - current offset) per partition (M-419) |
| `websocket_kafka_lag_poll_failures_total` | Counter | - | Total failures fetching Kafka watermarks for lag calculation (M-437) |
| `websocket_kafka_lag_poll_duration_seconds` | HistogramVec | `status` | Duration of Kafka watermark fetch operations (status=success/error) (M-437) |
| `websocket_kafka_lag_offset_age_seconds` | GaugeVec | `partition` | Seconds since last offset update per partition - high values indicate stale partitions (M-437) |

**Consumer Lag (M-419):** The `websocket_kafka_consumer_lag` metric shows how many messages the consumer is behind the latest Kafka offset. Updated every 10 seconds (configurable via `KAFKA_LAG_CHECK_INTERVAL_SECS`). Use alerts `KafkaConsumerLagHigh` (>10K messages) and `KafkaConsumerLagCritical` (>100K messages) to detect when the consumer is falling behind.

**Lag Monitor Health (M-437):** Three metrics provide visibility into the lag monitor's health:
- `websocket_kafka_lag_poll_failures_total`: Rising counter indicates Kafka connectivity issues
- `websocket_kafka_lag_poll_duration_seconds`: High latency suggests Kafka metadata slowness
- `websocket_kafka_lag_offset_age_seconds`: High age indicates a stale partition (no offset updates). Alert via `KafkaPartitionStale` (>120s) / `KafkaPartitionStaleCritical` (>300s). `KAFKA_LAG_STALE_PARTITION_SECS` controls when the server logs a warning; it does not change alert thresholds.

### Client Lag Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `websocket_client_lag_events_total` | CounterVec | `severity` | Total client lag events (severity=warning/critical) |
| `websocket_client_lag_messages_total` | CounterVec | `severity` | Total lagged messages by severity |

### Error Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `websocket_kafka_errors_by_type_total` | CounterVec | `error_type` | rdkafka client errors by type (dns_failure, connection_timeout, broker_down, decode_error, unknown); separate from `websocket_decode_errors_total` |
| `websocket_infrastructure_errors_total` | Counter | - | Infrastructure errors (network, Kafka connection failures) |
| `websocket_old_data_decode_errors_total` | Counter | - | Decode errors from old/pre-cached data (also exported as `websocket_kafka_messages_total{status="old_data_error"}`) |

### Latency Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `websocket_e2e_latency_ms` | HistogramVec | `stage` | End-to-end latency in milliseconds (stage=kafka_to_websocket) |

### DLQ Metrics (WebSocket Server)

**Service:** `websocket-server` (port 3002)

The WebSocket server implements its own DLQ handling (not using the library's `DlqHandler`). These metrics are **actively exported** by the production WebSocket server.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `websocket_dlq_sends_total` | CounterVec | `error_type` | Messages sent to DLQ (error_type=decode_error, decompression_failure, etc.) |
| `websocket_dlq_send_failures_total` | CounterVec | `reason` | Failed DLQ sends (reason=timeout, kafka_error, backpressure) |

### DLQ Metrics (Streaming Library)

**Library:** `dashflow-streaming` crate (`dlq.rs`)

These metrics are exported by any service that uses the library's `Consumer::with_dlq()` or `Producer::with_dlq()` builder methods. The `DlqHandler` class emits these metrics when handling failed messages.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_dlq_sends_total` | Counter | - | Library-level DLQ sends (total count, no error_type breakdown) |
| `dashstream_dlq_send_failures_total` | Counter | - | Library-level DLQ send failures (total count, no reason breakdown) |
| `dashstream_dlq_dropped_total` | Counter | - | Messages dropped due to DLQ backpressure (lazy metric - only appears after first drop) |
| `dashstream_dlq_send_retries_total` | Counter | - | DLQ send retry attempts |

**Current Deployment Status (M-412):**
- **WebSocket server**: Exports `websocket_dlq_*` metrics ✅
- **quality_aggregator**: Does NOT use library DLQ (no `dashstream_dlq_*` metrics)
- **prometheus-exporter**: Does NOT use library DLQ (no `dashstream_dlq_*` metrics)

**Alert Applicability:**
- `WebSocketDlqHighRate`, `WebSocketDlqBroken` → Fire when WebSocket server has DLQ issues
- `HighDLQRate`, `DLQItselfBroken`, `DLQBackpressureDrops`, `DLQSendFailures` → Fire ONLY if a service using the library's `DlqHandler` is deployed and scraped. These alerts are for future/custom services that opt into library DLQ support.

## Sequence Validation Metrics (dashstream_*)

Emitted by websocket server for detecting message loss, duplicates, and reordering.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_sequence_gaps_total` | Counter | - | Total sequence gaps detected (message loss) |
| `dashstream_sequence_duplicates_total` | Counter | - | Total duplicate sequences detected |
| `dashstream_sequence_reorders_total` | Counter | - | Total out-of-order sequences detected |

**Note:** These metrics do NOT have `thread_id` labels (removed in P0.4 to prevent unbounded cardinality). Per-thread debugging info is logged to traces instead.

## Redis/Replay Buffer Metrics

M-647: Redis metrics are component-scoped to allow different semantics per component.

### Rate Limiter Redis Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_rate_limiter_redis_errors_total` | CounterVec | `operation` | Redis errors in rate limiting (operation=new_connection, rate_limit_check, fallback_to_local, rate_limit_check_timeout) |
| `dashstream_rate_limiter_redis_latency_ms` | HistogramVec | `operation` | Redis latency for rate limiting (buckets: 0.1-500ms) |

### WebSocket Replay Buffer Redis Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `dashstream_websocket_redis_errors_total` | Counter | - | Total Redis errors in websocket replay |
| `dashstream_websocket_redis_latency_ms` | HistogramVec | `operation` | Redis latency for replay (operation=read, write; buckets: 1-1000ms) |
| `dashstream_ws_retry_count` | HistogramVec | `operation` | WebSocket operation retry counts |
| `replay_buffer_memory_hits_total` | Counter | - | Replay requests served from memory |
| `replay_buffer_redis_hits_total` | Counter | - | Replay requests served from Redis |
| `replay_buffer_redis_misses_total` | Counter | - | Replay requests not found in Redis |
| `replay_buffer_redis_write_dropped_total` | Counter | - | Redis writes dropped due to concurrency limiting |
| `replay_buffer_redis_write_failures_total` | Counter | - | Redis write failures |

## Grafana Dashboards

See `grafana/dashboards/` for Grafana dashboard JSON files:
- `grafana_quality_dashboard.json` - Quality monitoring dashboard

## Prometheus Configuration

Example scrape config:

```yaml
scrape_configs:
  - job_name: 'dashstream-quality'
    static_configs:
      - targets: ['dashstream-prometheus-exporter:9190']
    scrape_interval: 5s

  - job_name: 'websocket-server'
    static_configs:
      - targets: ['websocket-server:3002']
    scrape_interval: 5s
    metrics_path: /metrics
```

## Alert Rules

See `monitoring/alert_rules.yml` for production alert rules.

---

**Note:** This document is a summary. For complete and up-to-date metric details, always refer to the source crate documentation.

**Last Updated:** 2025-12-30 (Worker #2197 - M-2067 Fix stale file paths)
