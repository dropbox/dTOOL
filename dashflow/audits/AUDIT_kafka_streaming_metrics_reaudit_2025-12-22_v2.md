# Re-Audit v2: Kafka + Streaming Metrics (DashStream) — 2025-12-22

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

**Goal:** Re-audit Kafka usage + streaming metrics with a skeptical, correctness-first lens. Identify bugs, config footguns, observability gaps, and metric/alert/document drift. Produce an actionable, prioritized fix list for the next AI worker.

**Context:** Many earlier “S-series” issues were fixed in `#1450` and `#1451`. This document focuses on what still looks wrong or fragile *now* (and what the design implies will regress again).

---

## 0) Executive Summary (What’s Still Concerning)

### P0 correctness risks (active bugs / broken scaling)
1. **WebSocket Kafka offset storage is likely wrong**: it stores the *current* offset, not the *next* offset. This can cause repeated re-processing and/or a “stuck at tail” loop on restart.
   - Evidence: `crates/dashflow-observability/src/bin/websocket_server.rs:2145`
2. **Kubernetes deploy scales websocket-server to 2 replicas without consumer-group semantics for broadcast**, which likely means clients see partial streams depending on which pod they hit (or Kafka load-balances partitions across pods).
   - Evidence: `deploy/kubernetes/base/websocket-server.yaml:12` (replicas=2), no `KAFKA_GROUP_ID` set.
3. **“HighKafkaErrorRate” alert is semantically mislabeled/redundant**: the metric used for “Kafka error rate” is actually a decode-error counter; infra errors are tracked elsewhere. This leads to incorrect runbook guidance and duplicate paging behavior.
   - Evidence: `monitoring/alert_rules.yml:7` vs decode error handling in `crates/dashflow-observability/src/bin/websocket_server.rs:2034`

### P1 configuration and operability gaps (will break in real environments)
4. **Kafka security knobs (TLS/SASL) are not wired for rdkafka-based services** (websocket server + prometheus exporter), even though the library producer/consumer have security config types.
   - Evidence: websocket server and exporter only read `KAFKA_BROKERS` (and topic), not TLS/SASL env vars.
5. **Topic provisioning/retention is still “best-effort”**: local compose enables auto-topic-create; production-like deployments lack a provisioning/init job using the existing admin tooling.
   - Evidence: `docker-compose.dashstream.yml` / `docker-compose-kafka.yml` and unused `crates/dashflow-streaming/src/kafka.rs`

### P2 design debt (high chance of future regressions)
6. **Metrics export pattern can be brittle** when services manually concatenate text or selectively export metrics. WebSocket server now gathers from one registry in `/metrics`; keep other services consistent to avoid drift.
   - Evidence: `crates/dashflow-observability/src/bin/websocket_server.rs` (`metrics_handler`)
7. **Mixed Kafka stacks (`rdkafka` vs `rskafka`) imply inconsistent semantics** (consumer groups vs partition client), requiring explicit design decisions about partitions, duplicates, and replay.
   - Evidence: `crates/dashflow-streaming/src/consumer.rs` vs websocket/exporter rdkafka consumers.

---

## 1) Kafka Inventory (Producers, Consumers, Topics)

### Producers
- **DashFlow producer path:** `DashStreamCallback` → `DashStreamProducer`
  - `crates/dashflow/src/dashstream_callback.rs`
  - `crates/dashflow-streaming/src/producer.rs`
  - **Notable**: callback config does not expose TLS/SASL knobs; producer supports them but callback doesn’t wire them.

- **WebSocket server DLQ producer:** rdkafka `FutureProducer` sending JSON forensic payloads
  - `crates/dashflow-observability/src/bin/websocket_server.rs`

### Consumers
- **WebSocket server:** rdkafka `StreamConsumer` with manual offset storage
  - Topic: `KAFKA_TOPIC` (default `dashstream-quality`)
  - Group: `KAFKA_GROUP_ID` (default `websocket-server-v4`)
  - `crates/dashflow-observability/src/bin/websocket_server.rs`

- **Prometheus exporter:** rdkafka `StreamConsumer` (consumer group)
  - Topic: `KAFKA_TOPIC` (default `dashstream-quality`)
- Group: `KAFKA_GROUP_ID` (default `prometheus-exporter`)
  - `crates/dashflow-prometheus-exporter/src/main.rs`

- **Quality “monitor” in DashStream stack:** actually `quality_aggregator` (rskafka per-partition fan-in)
  - `crates/dashflow-streaming/src/bin/quality_aggregator.rs`
  - **Note**: reads `KAFKA_GROUP` but does not use it (consumer groups unsupported in rskafka path).

- **Self-improvement daemon streaming consumer:** uses `DashStreamConsumer` and enforces single-partition topics (by design)
  - `crates/dashflow/src/self_improvement/streaming_consumer.rs`

### Topics (defaults and confusion points)
- DashStream stack defaults: `dashstream-quality` (consumer side) and `${KAFKA_TOPIC}-dlq` (websocket DLQ default).
- Library default topic for producer/consumer configs: `dashstream-events`.
  - This is a recurring “no data” trap if operators don’t set `KAFKA_TOPIC` consistently.

---

## 2) Metrics Inventory (Emitters → Scrapes → Alerts/Dashboards)

### Emitters
- WebSocket server: `/metrics` (same port as server; default `3002`) emits `websocket_*`, `dashstream_sequence_*`, `dashstream_redis_*`, `replay_buffer_*`, and `websocket_dlq_*`.
  - `crates/dashflow-observability/src/bin/websocket_server.rs`
- Prometheus exporter: `/metrics` (default `9090`) emits `dashstream_quality_*` and `dashstream_librarian_*`.
  - `crates/dashflow-prometheus-exporter/src/main.rs`

### Alerts
- `monitoring/alert_rules.yml` relies on:
  - WebSocket: `websocket_kafka_messages_total`, `websocket_decode_errors_total`, `websocket_e2e_latency_ms_bucket`, `websocket_dropped_messages_total`, `websocket_dlq_*`
  - Shared/library-ish: `dashstream_sequence_*`, `dashstream_redis_*`, `replay_buffer_*`
  - **Concern**: `HighKafkaErrorRate` uses `websocket_kafka_messages_total{status="error"}` which is *decode errors*, not infra/broker errors. See `monitoring/alert_rules.yml:7`.

### Dashboard
- Grafana dashboard uses `dashstream_quality_*` (exporter) and `websocket_dlq_*` (websocket).
  - `grafana/dashboards/grafana_quality_dashboard.json`

---

## 3) Design + Correctness Critique (What Can Go Wrong)

### 3.1 Offset/commit semantics are the #1 correctness risk

**WebSocket server uses `store_offset(&topic, partition, offset)`**. In Kafka, committed offsets generally represent the *next* record to read, not the last processed record. Many client APIs therefore expect `msg.offset()+1`, or provide `store_offset_from_message(&msg)` to do this correctly.

- **Evidence**: `crates/dashflow-observability/src/bin/websocket_server.rs:2145`
- **Likely failure modes**:
  - On restart, the consumer re-reads the last processed message repeatedly.
  - Client appears “stuck at tail” (always replaying the final message and never advancing).
  - Any derived metrics/replay-buffer behavior duplicates events at boundaries (hard to debug).

**Fix (P0)**: replace manual store call with `consumer.store_offset_from_message(&msg)` or store `offset + 1` (and add a small test or comment explaining the invariant).

### 3.2 Multi-replica WebSocket server is not safe with a shared consumer group

K8s deploy currently sets **2 websocket-server replicas** but does not set `KAFKA_GROUP_ID`. Default group ID is constant, so Kafka will balance partitions across pods.

- **Evidence**: `deploy/kubernetes/base/websocket-server.yaml:12` and websocket group default in `crates/dashflow-observability/src/bin/websocket_server.rs` near the `KAFKA_GROUP_ID` read.
- **Impact**:
  - A client connected to pod A only sees partitions assigned to pod A.
  - This violates the intuitive “everyone sees the same stream” model.
  - You get “ghost missing events” depending on load balancer routing.

**Fix options (pick one; document the choice):**
1. **Force single replica** (recommended unless you add a backplane): set replicas=1 in base K8s and make Helm default 1.
2. **Unique consumer group per pod** (broadcast model): set `KAFKA_GROUP_ID` to include pod name (Downward API) so every pod consumes the full stream. (Higher Kafka load, but correct.)
3. **Proper scale-out**: keep shared consumer group, but add a shared pubsub/backplane between websocket pods so each pod can serve any client with the full stream. (More work.)

### 3.3 Alert/metric naming drifts cause incorrect incident response

`HighKafkaErrorRate` currently measures `websocket_kafka_messages_total{status="error"}` which is driven by decode failures (not broker outages).

- **Evidence**:
  - Alert: `monitoring/alert_rules.yml:7`
  - Error increments: `crates/dashflow-observability/src/bin/websocket_server.rs:2034` (decode path)
  - Infra errors are separate: `websocket_infrastructure_errors_total` and `websocket_kafka_errors_by_type_total`

**Fix (P0/P1)**:
- Either rename the alert to “HighMessageProcessingErrorRate” (or similar), or change it to use infra error metrics and keep decode separate.
- Ensure `docs/OBSERVABILITY_RUNBOOK.md` text matches the actual semantics (consumer errors vs producer send errors).

### 3.4 Kafka security configuration is inconsistent across the stack

- Library producer/consumer structs have TLS/SASL config fields, but the rdkafka services (websocket + exporter) don’t support them via env vars.
- DashFlow callback doesn’t surface producer security options at all.

**Fix (P1)**: define a single “KafkaConnectionConfig” (env-driven) and reuse it everywhere:
- rdkafka producer (DLQ + core producer)
- rdkafka consumers (websocket + exporter)
- rskafka consumer path (if retained)

### 3.5 Topic provisioning and retention enforcement are missing

You have admin tooling (`crates/dashflow-streaming/src/kafka.rs`) but deploys rely on broker defaults.

**Fix (P1/P2)**: add an init/provisioning step that creates:
- `${KAFKA_TOPIC}` with expected partitions/retention
- `${KAFKA_TOPIC}-dlq` with longer retention (for forensics)
- (optional) `dashstream-dlq` if library DLQ metrics/handlers are intended to be used in prod

---

## 4) Prioritized Fix List (AI Worker Checklist)

### P0 — must fix (correctness)
1. **Fix websocket offset storage** (use `store_offset_from_message` or `offset+1`).
   - File: `crates/dashflow-observability/src/bin/websocket_server.rs:2145`
2. **Fix K8s websocket scaling semantics** (replicas/group ID).
   - File: `deploy/kubernetes/base/websocket-server.yaml:12`
   - File: `deploy/helm/dashflow/templates/websocket-server.yaml:11`
3. **Resolve “HighKafkaErrorRate” semantics** (rename or change metric), and align runbook wording.
   - File: `monitoring/alert_rules.yml:7`
   - File: `docs/OBSERVABILITY_RUNBOOK.md` (HighKafkaErrorRate section)

### P1 — should fix (production readiness)
4. **Unify Kafka security config** (TLS/SASL) across producer + consumers; ensure env-var story is consistent.
   - Files: `crates/dashflow-streaming/src/producer.rs`, `crates/dashflow-prometheus-exporter/src/main.rs`, `crates/dashflow-observability/src/bin/websocket_server.rs`, `crates/dashflow/src/dashstream_callback.rs`
5. **Make topic provisioning explicit** (stop relying on auto-create; enforce retention/partitions).
   - Files: `crates/dashflow-streaming/src/kafka.rs`, deploy/init scripts/manifests
6. **Remove/rename dead config**: `KAFKA_GROUP` in `quality_aggregator` is misleading (rskafka path doesn’t do consumer groups).
   - File: `crates/dashflow-streaming/src/bin/quality_aggregator.rs:249`

### P2 — nice-to-have / reduce future drift
7. **Make metrics export robust**: use a single `Registry` and `registry.gather()` for websocket server, avoiding manual text concatenation except for compatibility.
   - File: `crates/dashflow-observability/src/bin/websocket_server.rs` (`metrics_handler`)
8. **Add explicit monitoring for consumer lag** (requires kafka-exporter/JMX or a lag metric source).
   - Deploy: add `kafka-exporter` or JMX + scrape, then alert on lag.
9. **Define “partitioning contract”**: what key partitions events (thread_id?), and what ordering guarantees are expected by consumers/clients.

---

## 5) Verification Commands (Worker Must Run)

```bash
# Build checks
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter

# Alert syntax checks (promtool via docker)
docker run --rm --entrypoint promtool -v "$PWD/monitoring/alert_rules.yml:/rules.yml:ro" prom/prometheus:v2.49.1 check rules /rules.yml
docker run --rm --entrypoint promtool -v "$PWD/deploy/kubernetes/base/configs/alert_rules.yml:/rules.yml:ro" prom/prometheus:v2.49.1 check rules /rules.yml

# Kafka offset storage location (should be store_offset_from_message after fix)
rg -n "store_offset\\(&topic, partition, offset\\)" crates/dashflow-observability/src/bin/websocket_server.rs

# K8s websocket replication decision
rg -n "replicas:" deploy/kubernetes/base/websocket-server.yaml deploy/helm/dashflow/templates/websocket-server.yaml
```

---

## 6) Notes for the Next AI Worker (to avoid reintroducing drift)

- Any time you add/change a metric:
  1) verify it is **emitted**, **exported on `/metrics`**, **scraped**, **covered by dashboard/alerts**, and **documented** (or explicitly not).
- Avoid “semantic reuse” of metric names: if `*_kafka_*_error*` changes meaning, rename it and keep an adapter (or update all downstream queries in the same commit).
- Treat multi-replica websocket-server as a design feature, not a “scale slider”. Either commit to a backplane, or run 1 replica.
