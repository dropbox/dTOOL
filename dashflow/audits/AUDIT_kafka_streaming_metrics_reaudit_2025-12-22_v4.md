# Re-Audit v4: Kafka + Streaming Metrics (DashStream) — 2025-12-22

**Purpose:** Re-audit Kafka usage + all streaming metrics with a skeptical, correctness-first lens. This is an update/extension of `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-22_v2.md`, reflecting substantial repo changes since v2 and re-checking all “fixed” claims against current HEAD.

**Scope:** Kafka clients (producers/consumers/admin), streaming/observability metrics, alert rules, dashboards, and deployment config that affects correctness (consumer groups, offsets, lag, DLQ).

---

## 0) What Changed Since v2 (Key Deltas)

1. **WebSocket server is now a real binary** (no longer “just an example”): `crates/dashflow-observability/src/bin/websocket_server/main.rs` (commit `#1470`).
2. **Offset storage bug (M-414) is fixed in current code**: WebSocket server stores offsets using `consumer.store_offset_from_message(&msg)` (Kafka “offset+1” semantics).
3. **Consumer lag monitoring (M-419) exists**: `websocket_kafka_consumer_lag` gauge (per partition) + alerts `KafkaConsumerLagHigh/Critical`.
4. **Kubernetes base websocket scaling (M-415) is fixed**: `replicas: 1` with explicit rationale.
5. **ALERT-RULE DRIFT existed between Docker and K8s** and is a recurring risk:
   - Canonical: `monitoring/alert_rules.yml`
   - K8s copy: `deploy/kubernetes/base/configs/alert_rules.yml`
   - These files were observed drifting (K8s still had `HighKafkaErrorRate` and missing lag alerts); they must remain identical.

---

## 1) Kafka Inventory (Who Talks To Kafka)

### 1.1 Producers

- **DashFlow apps → DashStreamProducer** (library):
  - `crates/dashflow/src/dashstream_callback/mod.rs`
  - `crates/dashflow-streaming/src/producer.rs`
  - Partition key: `thread_id` (`FutureRecord::key(thread_id.as_bytes())`)

- **WebSocket server DLQ producer** (rdkafka direct):
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs`
  - Sends forensic JSON payloads to `KAFKA_DLQ_TOPIC` (default `${KAFKA_TOPIC}-dlq`)

### 1.2 Consumers

- **WebSocket server** (rdkafka StreamConsumer):
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs`
  - Topic: `KAFKA_TOPIC` (default `dashstream-quality`)
  - Group: `KAFKA_GROUP_ID` (default `websocket-server-v4`)
  - Offsets: `enable.auto.offset.store=false`, manual store with `store_offset_from_message`, `enable.auto.commit=true`

- **Prometheus exporter** (rdkafka StreamConsumer):
  - `crates/dashflow-prometheus-exporter/src/main.rs`
  - Topic: `KAFKA_TOPIC` (default `dashstream-quality`)
  - Group: `KAFKA_GROUP_ID` (default `prometheus-exporter`)
  - Offsets: same “store after processing” pattern

- **Quality monitor / aggregator** (streaming binary):
  - `crates/dashflow-streaming/src/bin/quality_aggregator.rs`
  - Note: does not support consumer groups (and intentionally does not read `KAFKA_GROUP`)

### 1.3 Deploy-Time Semantics That Matter

- **WebSocket server must be 1 replica** unless a shared backplane is implemented:
  - `deploy/kubernetes/base/websocket-server.yaml` pins `replicas: 1`
  - Helm chart defaults to `replicaCount: 1`, but any HPA/maxReplica setting must not allow scaling >1 without redesign.

- **Prometheus exporter is also effectively single-replica** (current manifests): scaling it via consumer groups breaks gauge semantics unless dashboards/recording rules explicitly aggregate safely.

---

## 2) Metrics Inventory (Emitters → Alerts/Dashboards)

### 2.1 WebSocket Server (`websocket_*`, `dashstream_sequence_*`, replay/redis)

- Export endpoint: `/metrics` on port 3002 (default)
- Metrics doc (summary): `monitoring/PROMETHEUS_METRICS.md`
- Alerts: `monitoring/alert_rules.yml`
- Dashboard: `grafana/dashboards/streaming_metrics_dashboard.json`

Key exported metrics:
- `websocket_kafka_messages_total{status="success"|"error"}` (status series derived from atomics)
- `websocket_decode_errors_total{error_type=...}`
- `websocket_kafka_consumer_lag{partition=...}`
- `websocket_infrastructure_errors_total`
- `websocket_dlq_sends_total{error_type=...}`, `websocket_dlq_send_failures_total{reason=...}`
- `dashstream_sequence_gaps_total`, `dashstream_sequence_duplicates_total`, `dashstream_sequence_reorders_total`
- replay/redis metrics: `replay_buffer_*`, `dashstream_redis_*`

### 2.2 Prometheus Exporter (`dashstream_quality_*`)

- Export endpoint: `/metrics` (port 9090 internal; mapped in docker-compose)
- README: `crates/dashflow-prometheus-exporter/README.md`
- Dashboard: `grafana/dashboards/grafana_quality_dashboard.json`

---

## 3) Skeptical Critique (Correctness + Config + Observability)

### 3.1 M-413: Kafka TLS/SASL config unification is now wired end-to-end (verify in your environment)

**What exists:** `KafkaSecurityConfig` (`from_env()`, `validate()`, `apply_to_rdkafka()`) in:
- `crates/dashflow-streaming/src/kafka.rs`

**What was missing in earlier audits (v2):** rdkafka-based binaries constructed `ClientConfig` directly without applying the unified security config.

**What changed:** WebSocket server and Prometheus exporter now validate + apply `KafkaSecurityConfig` when constructing rdkafka clients:
- `crates/dashflow-observability/src/bin/websocket_server/main.rs` (consumer + DLQ producer)
- `crates/dashflow-prometheus-exporter/src/main.rs` (consumer)

**Remaining doc gap:** `crates/dashflow-prometheus-exporter/README.md` should explicitly mention TLS/SASL env vars and `KAFKA_GROUP_ID`.

### 3.2 Alert rule duplication is a “time bomb” (and already drifted once)

**Two copies exist**:
- Canonical: `monitoring/alert_rules.yml` (Docker Compose / root `prometheus.yml`)
- K8s: `deploy/kubernetes/base/configs/alert_rules.yml` (mounted into Prometheus)

**Observed failure mode:** K8s copy lagged behind canonical (old alert name, missing lag alerts, missing clarifying comments). This causes “it works locally” observability, but production behaves differently.

**Fix direction (P0/P1):**
- Add a CI/check script (or a lightweight test) that fails if the files differ.
  - Example: `diff -u monitoring/alert_rules.yml deploy/kubernetes/base/configs/alert_rules.yml`
- Preferably: generate the K8s ConfigMap from the canonical file (single source of truth).

Acceptance criteria:
- A repo check prevents landing changes that update only one file.

### 3.3 Metric semantic risk: “synthetic counters” can violate Prometheus expectations

In the WebSocket server collector, `websocket_kafka_messages_total{status="success"}` is computed as:
- `success = total.saturating_sub(errors)`

Because `total` and `errors` are independent atomics loaded separately at scrape time, a scrape can observe a newer `errors` value but an older `total` value. That can make the “success counter” temporarily decrease between scrapes (appearing like a counter reset), which corrupts `rate()`/`increase()` math.

**Fix direction (P1):**
- Track explicit counters for success and error (increment on the corresponding paths), and export those directly.
  - Or: export only `total` and `errors`, and compute success in PromQL (`rate(total) - rate(errors)`), but then stop exposing a “success” counter series.

Acceptance criteria:
- `websocket_kafka_messages_total{status="success"}` never decreases across scrapes (even under load).

### 3.4 WebSocket scale-out remains a design constraint

Even with `replicas: 1`, the system is one “chart value flip” away from correctness failure:
- Helm HPA/minReplicas/maxReplicas must not permit >1 without a backplane.
- If you ever need HA/scale: decide explicitly whether Kafka is a *fan-out bus* (each replica consumes all partitions with a unique group) or a *work queue* (consumer group splits partitions). For WebSockets delivering a full stream to each connected client, consumer-group splitting is the wrong default.

### 3.5 Consumer lag monitoring exists but needs “operator story”

You have `websocket_kafka_consumer_lag` + alerts, but:
- The runbook doesn’t yet walk responders through what to do when lag is high.
- The streaming Grafana dashboard doesn’t yet show this metric prominently.

Fix direction (P2):
- Add dashboard panel(s) for lag by partition and overall max.
- Add runbook entry for `KafkaConsumerLagHigh/Critical`.

---

## 4) Updated Priority List (Worker Action Plan)

### P0 (stop-the-bleeding / prevent production drift)
1. **Enforce alert rule single-source-of-truth**
   - Add a repo check that `monitoring/alert_rules.yml` == `deploy/kubernetes/base/configs/alert_rules.yml`.

### P1 (production readiness / correctness)
2. **Fix synthetic “success” counter semantics**
   - Make `websocket_kafka_messages_total{status="success"}` monotonic under concurrent load.

### P2 (operability)
4. **Runbook + dashboard coverage for consumer lag**
   - Dashboard panel(s)
   - Runbook remediation steps
5. **DLQ durability semantics**
   - Decide whether offset commits may proceed when DLQ send fails (document; optionally add a “fail closed” mode).

---

## 5) Verification Checklist (Worker Must Run)

```bash
# Ensure alert rule copies match
diff -u monitoring/alert_rules.yml deploy/kubernetes/base/configs/alert_rules.yml

# Validate Prometheus rules
docker run --rm --entrypoint promtool -v "$PWD/monitoring/alert_rules.yml:/rules.yml:ro" prom/prometheus:v2.49.1 check rules /rules.yml

# Build the binaries that talk to Kafka
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
```

---

## 6) Status vs Roadmap (Reality Check)

- ✅ M-414: offset storage fixed in `crates/dashflow-observability/src/bin/websocket_server/main.rs`
- ✅ M-415: base K8s pins websocket-server to 1 replica; Helm defaults to 1 (but avoid HPA >1)
- ✅ M-416: canonical alerts use `HighMessageProcessingErrorRate` (ensure K8s copy matches canonical)
- ✅ M-413: unified TLS/SASL config is applied across DashStream Kafka clients
