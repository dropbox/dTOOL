# Re-Audit v9: Kafka + Streaming Metrics (DashStream) — 2025-12-23

This extends:
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v8.md`
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v7.md`

Re-checked against current `main` HEAD (through worker commit `#1507`).

Goal: correctness-first, skeptical audit of Kafka usage + streaming metrics. Treat “✅ FIXED” claims as hypotheses; verify in code + deploy config. This doc is written as a worker directive: what to fix next, why it matters, and how to verify.

---

## 0) What Changed Since v8 (Reality Check)

### ✅ Improvement: rdkafka security config is now applied in CLI + kafka admin helpers
Since v8, `KafkaSecurityConfig::create_client_config()` was introduced and adopted by:
- `crates/dashflow-streaming/src/kafka.rs` admin/metadata helpers (create/delete/list/topic_exists/get_partition_count)
- `crates/dashflow-cli/src/commands/*` streaming telemetry commands

This closes the specific v8 gap: “CLI/admin ignore TLS/SASL env vars”.

### ❌ Still true: highest-risk correctness problems are not solved
- WebSocket lag monitoring still has an unbounded-buffer + blocking-call design hazard (see Section 3.1).
- Prometheus exporter still hardcodes `auto.offset.reset=earliest` (deployment config drift).
- Env-var “unification” is still not end-to-end for rskafka consumer + producer paths (see Section 2.3/2.4).

---

## 1) Kafka Client Inventory (What Talks to Kafka)

### 1.1 Long-running services
- `crates/dashflow-observability/src/bin/websocket_server.rs` (rdkafka consumer + DLQ producer + lag-monitor metadata consumer)
- `crates/dashflow-prometheus-exporter/src/main.rs` (rdkafka consumer)
- `crates/dashflow-streaming/src/bin/quality_aggregator.rs` (rskafka consumers per partition; uses rdkafka only for metadata via `get_partition_count`)

### 1.2 Operator tooling
- `crates/dashflow-cli/src/commands/*.rs` (rdkafka StreamConsumer)

### 1.3 Core producer path (most likely to matter in real usage)
- `crates/dashflow/src/dashstream_callback.rs` → `dashflow_streaming::producer::DashStreamProducer` (rdkafka FutureProducer)

---

## 2) Kafka Security + Configuration Audit (Correctness + Drift)

### 2.1 `create_client_config()` does not validate the env config
`KafkaSecurityConfig` has `validate()`, but `create_client_config()` doesn’t call it and most call sites don’t either.

**Failure mode:** invalid env values won’t fail fast; they’ll fail later inside rdkafka with less actionable errors.

**Fix direction (P1):**
- Add a `create_client_config_checked(...) -> Result<ClientConfig>` that calls `validate()` and returns an error with context.
- Update all production-ish call sites (CLI + admin helpers + websocket-server + exporter) to use the checked builder.

**Concrete targets:**
- `crates/dashflow-streaming/src/kafka.rs` call sites of `KafkaSecurityConfig::from_env().create_client_config(...)`
- `crates/dashflow-cli/src/commands/*.rs` call sites of `KafkaSecurityConfig::from_env().create_client_config(...)`
- WebSocket server and exporter already call `validate()` today; keep them consistent with the checked builder for uniformity.

### 2.1.1 `broker.address.family=v4` is forced globally (may break IPv6-only environments)
`create_client_config()` unconditionally sets `broker.address.family` to `v4`.

**Failure mode:** in IPv6-only or IPv6-preferred environments, forcing v4 can make Kafka unreachable even with correct DNS/hosts.

**Fix direction (P2):**
- Only force IPv4 when `bootstrap_servers` contains `localhost`/`127.0.0.1` (the original Docker-local rationale), OR
- Add an env var override (e.g., `KAFKA_BROKER_ADDRESS_FAMILY=v4|v6|any`) and default to the safer behavior for production.

### 2.2 Deployment manifest drift: we set env vars that some services ignore
- Helm/K8s set `KAFKA_AUTO_OFFSET_RESET`, but exporter ignores it (hardcoded earliest).
- Helm/K8s do not expose `KAFKA_SECURITY_PROTOCOL`, `KAFKA_SASL_*`, `KAFKA_SSL_*` (secure Kafka requires patching manifests).

**Fix direction (P1/P2):**
- Exporter: honor `KAFKA_AUTO_OFFSET_RESET`.
- Helm/K8s: wire the security env vars via values + Secret references.

**Concrete targets:**
- Exporter: `crates/dashflow-prometheus-exporter/src/main.rs` in `KafkaConsumer::new()` (look for `.set("auto.offset.reset", "earliest")`)
- Helm: `deploy/helm/dashflow/templates/configmap.yaml` (add keys for security env vars; prefer Secret refs for credentials)
- K8s base: `deploy/kubernetes/base/configmap.yaml` + deployment yamls

### 2.3 rskafka path is not covered by the env-var “unification” story
`quality_aggregator` uses `DashStreamConsumer::new_for_partition()` (rskafka). `DashStreamConsumer` supports TLS/SASL via `ConsumerConfig`, but it does **not** load `KAFKA_SECURITY_PROTOCOL` / `KAFKA_SASL_*` / `KAFKA_SSL_*`.

**Failure mode:** in secure Kafka clusters, the aggregator can succeed in metadata calls (rdkafka path) but fail consuming partitions (rskafka path remains plaintext).

**Fix direction (P1):**
- Add `ConsumerConfig::from_env()` (or `DashStreamConsumer::from_env_for_partition()`) that maps `KafkaSecurityConfig` → rskafka settings.

### 2.4 Producer path is still not covered by the env-var “unification” story
`DashStreamProducer` uses `ProducerConfig.security_protocol`/TLS/SASL fields, but there is no `ProducerConfig::from_env()` and nothing populates these fields from unified env vars.

**Failure mode:** the default DashStream callback path can’t produce to secure Kafka unless the application constructs a custom `ProducerConfig` manually.

**Fix direction (P1):**
- Add `ProducerConfig::from_env()` that maps `KafkaSecurityConfig::from_env()` into producer TLS/SASL fields.

**Concrete targets:**
- Consumer: `crates/dashflow-streaming/src/consumer.rs` (`ConsumerConfig` + `DashStreamConsumer::with_config`)
- Producer: `crates/dashflow-streaming/src/producer.rs` (`ProducerConfig` + `DashStreamProducer::with_config`)
- Entry points that currently only set brokers/topic: `crates/dashflow/src/dashstream_callback.rs`

---

## 3) Streaming Metrics Correctness Audit (Bugs + Design Critique)

### 3.1 P0: WebSocket lag monitor can accumulate unbounded backlog and block runtime threads

Current design:
- main consumer sends `(partition, offset+1)` for every message on an **unbounded channel**
- background task does synchronous `fetch_watermarks(..., 1s)` inside an async task

**Failure modes:**
1) **Unbounded memory growth**: if consumption is fast and watermark fetch is slow, the channel grows without bound.
2) **Runtime starvation**: synchronous watermark fetch inside tokio task can block runtime worker threads.
3) **Stale partitions**: tracked partitions are never cleared; after rebalances/assignment changes, the gauge can page on stale partitions.
4) **Silent degradation**: no metric exists for “lag monitor is failing/slow”, so “lag gauge stopped updating” is invisible.

**Fix direction (P0):**
- Replace the channel with a “latest offset per partition” structure updated in the consume loop.
- Move watermark fetch off async worker threads (`spawn_blocking` or a dedicated thread).
- Make lag computation assignment-aware; clear revoked partitions and update gauges accordingly.
- Add lag-monitor health metrics (poll failures/latency + offset age).

**Concrete targets:**
- `crates/dashflow-observability/src/bin/websocket_server.rs` (search `unbounded_channel`, `fetch_watermarks`, `websocket_kafka_consumer_lag`)
- `monitoring/alert_rules.yml` (add alerts for lag monitor failures/offset staleness if you add those metrics)

**Acceptance criteria (must be testable):**
- Add a debug-only “high rate offsets” local harness (or a unit test around the lag-monitor data structure) that proves updates stay bounded (no unbounded queue).
- Force a rebalance (two consumers in the same group) and confirm gauges don’t retain revoked partitions indefinitely.

### 3.2 Missing infra paging: measured but not alerted
We have `websocket_infrastructure_errors_total` (and a Grafana panel), but no alert in `monitoring/alert_rules.yml`.

**Fix direction (P2):**
- Add `KafkaInfraErrorsHigh` alert on `sum(rate(websocket_infrastructure_errors_total[5m]))`.

**Concrete targets:**
- `monitoring/alert_rules.yml` and the K8s copy `deploy/kubernetes/base/configs/alert_rules.yml` (run `./scripts/check_dashstream_alert_rules_sync.sh --promtool`)

### 3.3 Exporter metrics: subtle semantics/cardinality hazards
1) `model` label cardinality risk (per-model metrics can explode series if model strings are unbounded).
2) `dashstream_librarian_iterations` is described as “average” but is set to “last observed”.

**Fix direction (P2):**
- Normalize models to a bounded set or gate per-model metrics behind a config flag.
- Replace “average iterations” gauge with a histogram (distribution of turn_number) or compute a real rolling average.

---

## 4) CLI Correctness + UX Footguns

### 4.1 CLI env-var wiring is still inconsistent (operators will run against the wrong topic)
Only `dashflow replay` wires `KAFKA_BROKERS` and `KAFKA_TOPIC` via clap `env = ...`. Most other commands hard-default to:
- brokers: `localhost:9092`
- topic: `dashstream`

**Fix direction (P1):**
- Standardize all Kafka-consuming CLI commands to accept `env="KAFKA_BROKERS"` + `env="KAFKA_TOPIC"`.
- Choose a single default topic for CLI telemetry commands (recommend `dashstream-quality`) and document it.

**Concrete targets:**
- `crates/dashflow-cli/src/commands/*.rs` `#[derive(Args)]` structs for: tail/watch/inspect/replay/diff/export/flamegraph/costs/profile
- Consider extracting a shared `KafkaArgs` struct to avoid drift.

### 4.2 `dashflow tail` commits offsets by default (debug tooling shouldn’t mutate Kafka state)
`dashflow tail` currently uses a fixed group id and `enable.auto.commit=true`.

**Fix direction (P1):**
- Default to `enable.auto.commit=false` + unique group id.
- Add `--group-id` and `--commit` flags for explicit opt-in.

**Concrete targets:**
- `crates/dashflow-cli/src/commands/tail.rs` (search `enable.auto.commit`)

---

## 5) Worker Priority List (Do Next)

### P0
1) Fix WebSocket lag monitor unbounded-buffer + blocking-call hazards (Section 3.1).

### P1
2) Exporter honors `KAFKA_AUTO_OFFSET_RESET` (config drift fix).
3) Add “checked” Kafka client config builder that validates `KafkaSecurityConfig` before creating rdkafka clients; update call sites.
4) Add `ProducerConfig::from_env()` and `ConsumerConfig::from_env()` (rskafka) so secure Kafka works for the core producer path + quality aggregator.
5) Standardize CLI env var wiring and safe defaults; remove “tail commits offsets by default” footgun.

### P2
6) Add infra-error alert (`websocket_infrastructure_errors_total`) and lag-monitor health alerts.
7) Bound exporter model-label cardinality; fix “average iterations” metric semantics.
8) Wire Kafka security env vars into Helm/K8s templates via values/Secrets.
9) Clean up stale env var names (`DASHSTREAM_*`, `DASHFLOW_*` Kafka vars) in docs + CLI example scripts.

---

## 6) Verification Commands (Worker Must Run)

```bash
./scripts/check_dashstream_alert_rules_sync.sh
./scripts/check_dashstream_alert_rules_sync.sh --promtool

CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-cli
```
