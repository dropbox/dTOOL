# Re-Audit v7: Kafka + Streaming Metrics (DashStream) — 2025-12-23

This extends `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-22_v6.md` after re-checking current HEAD (worker commits up to `#1493`).

The intent is skeptical: assume “fixed” items may have regressed or only be partially fixed, and look for adjacent/structurally-similar gaps.

---

## 0) What’s Changed Since v6 (Re-verified)

### ✅ Lag polling no longer blocks the websocket hot path (M-430)
The WebSocket server now performs consumer lag polling in a background task using a separate metadata consumer, and the main consumer only sends offset updates over a channel:
- `crates/dashflow-observability/src/bin/websocket_server.rs` (search “M-430”)

This fixes the earlier positive-feedback failure mode where lag monitoring itself could cause lag.

### ✅ Ops coverage for consumer lag exists (part of M-428)
Runbook coverage for `KafkaConsumerLagHigh/Critical` is present:
- `docs/OBSERVABILITY_RUNBOOK.md` (sections “7c/7d”)

Grafana panels exist (but in the quality dashboard, not the streaming dashboard):
- `grafana/dashboards/grafana_quality_dashboard.json` (search “Kafka Consumer Lag” / `websocket_kafka_consumer_lag`)

---

## 1) Kafka Client Inventory (What Talks to Kafka)

### 1.1 Long-running services
- `crates/dashflow-observability/src/bin/websocket_server.rs`
  - rdkafka consumer (streams to WebSocket, emits websocket_* metrics)
  - rdkafka producer (DLQ JSON payloads)
  - Lag monitoring: background metadata consumer (M-430)
- `crates/dashflow-prometheus-exporter/src/main.rs`
  - rdkafka consumer (bridges quality events → dashstream_quality_* metrics)
- `crates/dashflow-streaming/src/bin/quality_aggregator.rs`
  - rskafka-based consumer/producer pipeline (partition-based)

### 1.2 Operator tooling / debugging
- `crates/dashflow-cli/src/commands/*.rs`
  - multiple rdkafka consumers (tail/replay/inspect/diff/export/profile/flamegraph/watch/costs)

### 1.3 Admin/topic tooling (library helpers)
- `crates/dashflow-streaming/src/kafka.rs`
  - rdkafka AdminClient for create/delete topics
  - rdkafka BaseConsumer for metadata operations (list_topics/topic_exists/partition_count)

---

## 2) Metrics Inventory (Emitters → Consumers of Metrics)

### 2.1 WebSocket server
Canonical docs:
- `monitoring/PROMETHEUS_METRICS.md`
- `docs/OBSERVABILITY_INFRASTRUCTURE.md`

Critical metrics:
- `websocket_kafka_messages_total{status="success"|"error"}` (processing success/error counters)
- `websocket_decode_errors_total{error_type=...}` (protobuf decode failures)
- `websocket_infrastructure_errors_total` + `websocket_kafka_errors_by_type_total{error_type=...}` (rdkafka client/infra failures)
- `websocket_kafka_consumer_lag{partition=...}` (background lag monitor)
- `websocket_dlq_*` (best-effort forensic DLQ publishing)
- `dashstream_sequence_*` (gap/dup/reorder detection)

### 2.2 Prometheus exporter
- `dashstream_quality_monitor_*`, `dashstream_query_latency_ms`, etc. (see `crates/dashflow-prometheus-exporter/README.md`)

---

## 3) Skeptical Findings (Remaining Bugs / Footguns)

### 3.1 ✅ Fixed: Kafka TLS/SASL unification now covers CLI + admin helpers

Even though the websocket-server and prometheus-exporter already apply `KafkaSecurityConfig`, two additional Kafka client surfaces needed the same treatment:

1) **dashflow-cli** consumers
   - Fixed by centralizing rdkafka config in `crates/dashflow-cli/src/kafka_config.rs` and using it from all Kafka commands.

2) **dashflow-streaming Kafka admin + metadata helpers**
   - Fixed by applying `KafkaSecurityConfig::from_env().validate()?` + `apply_to_rdkafka()` to every AdminClient/BaseConsumer builder.

**Verification:**
- With `KAFKA_SECURITY_PROTOCOL=sasl_ssl` + CA path set, CLI commands and `ensure_topics_with_dlq()` succeed.

### 3.2 ✅ Fixed: Prometheus exporter supports `KAFKA_AUTO_OFFSET_RESET`

The exporter now reads `KAFKA_AUTO_OFFSET_RESET` (default `earliest`) and documents the override in `crates/dashflow-prometheus-exporter/README.md`.

### 3.3 ✅ Fixed: CLI defaults + env-var wiring match the DashStream stack

All Kafka-backed CLI commands now:
- Support `KAFKA_BROKERS` + `KAFKA_TOPIC` env vars
- Default `--topic` to `dashstream-quality` (matching websocket-server + exporter defaults)

### 3.4 P2: DLQ durability semantics still unresolved (M-429)

Current websocket DLQ behavior is best-effort/async. Offsets are stored/committed regardless of DLQ send failures:
- Outcome: you can lose the forensic payload for a bad message while still advancing the consumer offset.

**Fix direction (document-first, then optional code):**
- Explicitly document the chosen behavior (best-effort vs fail-closed) in `docs/OBSERVABILITY_RUNBOOK.md`.
- Optional: add `DLQ_MODE=best_effort|fail_closed` (fail-closed blocks offset store/commit until DLQ succeeds with bounded retries).

---

## 4) Worker Priority List (Do Next)

### P2
1) **Decide and document DLQ durability semantics (M-429)**
2) **Add explicit infra-error alert**
   - Alert on `rate(websocket_infrastructure_errors_total[5m])` (or `websocket_kafka_errors_by_type_total`) to page on Kafka/network outages (separate from decode failures).

---

## 5) Verification Commands (Worker Must Run)

```bash
# Prevent alert-rule drift
./scripts/check_dashstream_alert_rules_sync.sh
./scripts/check_dashstream_alert_rules_sync.sh --promtool

# Build
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-cli
```
