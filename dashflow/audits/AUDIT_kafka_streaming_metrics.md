# Audit: Kafka + Streaming Metrics (DashStream)

**Status:** HISTORICAL / PARTIALLY SUPERSEDED (see update below)
**Scope:** Kafka usage + end-to-end streaming observability (DashStream topic(s), consumers, metrics, alerts, dashboards)
**Primary Outputs:** Prioritized fix list for next AI worker

**Update (2025-12-22):** Many items originally called out here have since been fixed (notably in `#1450` and `#1451`), including:
- WebSocket server topic/DLQ configurability, old-data decode classification, and busy-loop backoff
- WebSocket DLQ alerts (`websocket_dlq_*`) and ServiceDown label robustness
- Kubernetes/Helm scrape-port/rule-file deployment issues
- Prometheus exporter README/tests drift
- Multi-partition consumption in `quality_aggregator`

**Still-relevant gaps to prioritize now:**
- Kafka topic provisioning/retention enforcement (auto-create topics is still on in local compose; production needs explicit provisioning)
- Producer delivery semantics (application-level retry + idempotence duplicate risk) and consumer dedupe strategy
- Clarify/verify which scraped services actually export `dashstream_dlq_*` metrics (some alerts may be “dead” depending on deployment)
- Kafka security config unification (TLS/SASL knobs across producer + consumers)

---

## System Overview (what exists today)

**Main pipeline:**
- Producer(s): `crates/dashflow/src/dashstream_callback/mod.rs` → `DashStreamProducer` (`crates/dashflow-streaming/src/producer.rs`)
- Kafka topic: `dashstream-quality` (default in multiple components)
- Consumers:
- WebSocket server: `crates/dashflow-observability/src/bin/websocket_server/main.rs` (Docker: `Dockerfile.websocket-server`)
  - Prometheus exporter: `crates/dashflow-prometheus-exporter/src/main.rs` (Docker: `Dockerfile.prometheus-exporter`)
  - "Quality aggregator": `crates/dashflow-streaming/src/bin/quality_aggregator.rs` (Docker: `Dockerfile.quality-aggregator`)
- Metrics/alerts/dashboards:
  - Prometheus scrape + rules: `prometheus.yml`, `monitoring/alert_rules.yml`
  - Grafana: `grafana/dashboards/grafana_quality_dashboard.json`
  - Metrics summary doc: `monitoring/PROMETHEUS_METRICS.md`

**Local infra:**
- Kafka + ZK docker: `docker-compose-kafka.yml`, and full stack: `docker-compose.dashstream.yml`

---

## Findings (Kafka correctness + configuration)

### 1) “Quality monitor/aggregator” consumes only a single partition (data loss in multi-partition topics)

**Evidence**
- `crates/dashflow-streaming/src/consumer.rs` uses `rskafka::client::partition::PartitionClient` and is explicitly single-partition (`ConsumerConfig.partition`, default `0`).
- `crates/dashflow-streaming/src/bin/quality_aggregator.rs` uses `DashStreamConsumer::new(&kafka_brokers, &kafka_topic, &kafka_group)` and therefore reads partition `0` only.

**Impact**
- If `dashstream-quality` has `N>1` partitions, the aggregator silently misses ~((N-1)/N) of messages.
- The `KAFKA_GROUP` env var is misleading here: rskafka consumer-group commits aren’t used (API field exists but is unused).

**Fix**
- Replace aggregator consumer with `rdkafka::consumer::StreamConsumer` and a real consumer group, or implement a multi-partition fan-in (one `PartitionClient` per partition).
- If you keep `DashStreamConsumer`, remove/rename `group_id` to avoid implying consumer groups.

---

### 1b) Other `DashStreamConsumer` usages likely have the same partition-0 blind spot

**Evidence**
- `crates/dashflow/src/self_improvement/streaming_consumer.rs` (`start_streaming_consumer`) uses `DashStreamConsumer::new(...)` behind `feature = "dashstream"`.
- `crates/dashflow-streaming/src/bin/parse_events.rs` uses `DashStreamConsumer::new(...)`.
- `crates/dashflow/src/optimize/trace.rs` (deprecated `TraceCollector`) uses `DashStreamConsumer::new(...)` and documents the single-partition limitation.

**Impact**
- Any pipeline that depends on these consumers can silently miss messages if the topic has multiple partitions.

**Fix**
- Same as (1): switch to `rdkafka` consumer groups, implement a multi-partition fan-in, or enforce 1 partition for topics that must be fully consumed by these tools.

---

### 2) Topic config is not actually enforced in the default deployments (auto-create topics)

**Evidence**
- `docker-compose.dashstream.yml` and `docker-compose-kafka.yml` set `KAFKA_AUTO_CREATE_TOPICS_ENABLE: 'true'`.
- There is TopicConfig validation & admin tooling in `crates/dashflow-streaming/src/kafka.rs`, but nothing in the stack ensures topics are created via this path.

**Impact**
- Production-like configs (partitions, retention, `min.insync.replicas`, cleanup policy, compression type) will drift to broker defaults.
- Operational surprises: topic ends up with 1 partition, wrong retention, etc.

**Fix**
- Add explicit topic provisioning to the stack (startup init job or `dashflow-cli` command invoked by compose), and set auto-create off outside local dev.
- Make topic config (partitions/replication/retention/cleanup/minISR) an explicit deploy artifact.

---

### 3) Producer app-level retry can create duplicates even with `enable.idempotence=true`

**Evidence**
- `crates/dashflow-streaming/src/producer.rs` does an application-level retry loop around `FutureProducer::send(...)` (see `SEND_RETRIES_TOTAL` path and retry backoff).

**Why this is risky**
- If the first produce request succeeds but the client sees a timeout/transport error, the retry path can enqueue a *new* message. Kafka idempotence prevents duplicates from broker-side retries for the same produce sequence, but does not guarantee dedup across separate application sends after uncertain outcomes.

**Impact**
- Downstream can see duplicate events; any “exactly once” assumptions are invalid without transactions + idempotent processing.

**Fix**
- Prefer librdkafka’s internal retry handling; treat ambiguous delivery outcomes as “unknown” and surface to caller rather than blindly retrying.
- If duplicates are acceptable, make that explicit and ensure consumers are idempotent (e.g., dedupe by `message_id`).

---

### 4) Kafka consumer group IDs: ✅ FIXED (now configurable)

**Evidence (at HEAD)**
- WebSocket server reads `KAFKA_GROUP_ID` and uses it for both the main consumer and the lag-monitor consumer:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:486` (env + default)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:520` (consumer group.id)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:596` (lag monitor group.id)
- Prometheus exporter reads `KAFKA_GROUP_ID` and uses it for its consumer group:
  - `crates/dashflow-prometheus-exporter/src/main.rs:910-926`

**Impact**
- "Version bump" group IDs are an operational footgun: a group change rewinds offsets and reprocesses history, inflating derived counters/histograms.
- Hardcoded IDs impede multi-env deployments and blue/green.

**Fix**
- Commit the `KAFKA_GROUP_ID` change (and document it in `.env.template` / runbooks). Consider making `KAFKA_GROUP_ID` required in non-dev environments.

---

### 5) ~~WORKER_DIRECTIVE K-series appears stale vs current code~~ ✅ FIXED #1376

**Evidence**
- `WORKER_DIRECTIVE.md` lists K-1..K-10 as TODO (Kafka topic mgmt bugs).
- `crates/dashflow-streaming/src/kafka.rs` already contains explicit validation/retry/optimizations annotated with K-1..K-10 comments (suggesting the fixes landed but the directive wasn't updated).

**Impact**
- Next AI worker may waste cycles "fixing" already-fixed items or reintroduce drift.

**Fix**
- ✅ FIXED #1376: All K-series items (K-1 through K-10) now marked as FIXED in WORKER_DIRECTIVE.md.

---

## Findings (Streaming metrics correctness + alerting)

### 6) Alert rules: ✅ FIXED (HEAD uses label-robust PromQL)

**Evidence (at HEAD)**
- `monitoring/alert_rules.yml` uses `sum(rate(...)) / clamp_min(sum(rate(...)), 1e-9)` for error-rate alerts and avoids assuming optional labels in annotations.

**Follow-up**
- Keep `docs/OBSERVABILITY_RUNBOOK.md` aligned with `monitoring/alert_rules.yml` as PromQL evolves.

---

### 11) WebSocket `/metrics` exports decode errors correctly ✅ FIXED

**Evidence (at HEAD)**
- `crates/dashflow-observability/src/bin/websocket_server/main.rs:773-791` registers `websocket_decode_errors_total` as an `IntCounterVec` with label `error_type`.
- `/metrics` returns the registry output (so the vec metric is exported as-is):
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:360-370`

**Impact**
- Breaks per-error-type debugging and mislabels all decode errors as `buffer_underflow`.

**Fix**
- Export the actual `IntCounterVec` via `.collect()` (like other vec metrics in the handler) and remove the hardcoded decode-error line (or rename the unlabeled total to a distinct metric name).

---

### 12) WebSocket `/metrics` “success” counter is monotonic ✅ FIXED

**Evidence (at HEAD)**
- `websocket_kafka_messages_total{status="success"}` is tracked explicitly as a monotonic counter:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:408-416`

**Impact**
- If `kafka_errors > kafka_messages_received`, this subtraction wraps in release builds and emits a huge bogus counter.

**Fix**
- Use `saturating_sub` or maintain explicit `success`/`error` counters.

---

### 7) Metric name/label schema: partially harmonized (labels fixed; types still differ)

**Evidence (at HEAD)**
- WebSocket server sequence metrics are now unlabeled `IntCounter`s (thread_id label removed):
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1029-1129`
- `dashflow-streaming` library sequence metrics remain unlabeled `Counter`s:
  - `crates/dashflow-streaming/src/consumer.rs`
- WebSocket server DLQ metrics are namespaced to avoid collision with library `dashstream_dlq_*`:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1131+`

**Impact**
- Alerts/dashboards become non-portable between emitters.
- Humans reading dashboards can’t tell which semantic they’re looking at (“per-thread” vs “global”).

**Fix**
- Choose ONE canonical schema per metric name (type + labels), then:
  - Rename the non-canonical metrics, or
  - Harmonize all emitters to the canonical shape.

---

### 8) High-cardinality labels (`thread_id`) ✅ FIXED (removed)

**Evidence (at HEAD)**
- `crates/dashflow-observability/src/bin/websocket_server/main.rs:1029-1129` defines sequence counters without any `thread_id` label.

**Impact**
- Unbounded series cardinality (one time-series per thread/session) can exhaust Prometheus memory and make queries unusable.
- It also risks exposing sensitive identifiers via metrics.

**Result**
- Prometheus cardinality is bounded; per-thread debugging is via logs/traces, not metric labels.

---

### 9) Alert annotations/expressions should not assume optional labels (fix exists locally, uncommitted)

**Evidence**
- `monitoring/alert_rules.yml` references:
  - `{{ $labels.thread_id }}` for sequence alerts.
  - `{{ $labels.error_type }}` and `{{ $labels.reason }}` for DLQ alerts.
- These labels exist only if the WebSocket server emitter is the source-of-truth; they do not exist in the `dashflow-streaming` library DLQ/sequence emitters.

**Fix**
- Commit the alert rule hardening, and then update runbooks to stop referencing labels that may be absent.

---

### 10) Namespace collisions for derived vs direct app metrics

**Evidence**
- `crates/dashflow-prometheus-exporter/src/main.rs` defines metrics like `librarian_requests_total` and `librarian_request_duration_seconds`.
- Prometheus is also configured to scrape the real librarian app (`prometheus.yml` job `librarian`).

**Impact**
- Queries/dashboards that don’t scope by `job` can accidentally mix:
  - direct app instrumentation vs
  - Kafka-derived “quality monitor” aggregates

**Fix**
- Namespace Kafka-derived app metrics (e.g. `dashstream_librarian_requests_total`) or add a `source="kafka"` label and update dashboards accordingly.

---

## Prioritized Fix Plan (for Next AI worker)

### P0 (Correctness / "we're currently wrong")
1. **Commit the pending streaming alert/metrics fixes** currently in `git diff`:
   - `monitoring/alert_rules.yml` (PromQL + label-robustness)
   - `crates/dashflow-observability/src/bin/websocket_server/main.rs` (remove `thread_id` metric labels; make `KAFKA_GROUP_ID` configurable)
   - `crates/dashflow-prometheus-exporter/src/main.rs` (make `KAFKA_GROUP_ID` configurable)
2. **Fix WebSocket `/metrics` correctness** (Finding #11 and #12).
3. **Fix quality_aggregator consumption model** to read all partitions (or enforce single-partition topics).
4. **Resolve DLQ metric schema divergence** (decide canonical labels for `dashstream_dlq_*` and align emitters/dashboards/alerts).

### P1 (Operational safety / config drift / reduce footguns)
5. **Stop relying on Kafka auto-topic-creation** for anything but local dev; add explicit topic provisioning using `crates/dashflow-streaming/src/kafka.rs`.
6. **Document/decide producer delivery semantics** (app-level retry duplicate risk) and update producer/consumers accordingly.
7. ✅ **K-series already fixed in git**: `WORKER_DIRECTIVE.md` marks K-1..K-10 as ✅ FIXED #1376.

### P2 (Quality of observability + documentation)
8. **Update runbooks/docs to match reality** (`docs/OBSERVABILITY_RUNBOOK.md`, `monitoring/PROMETHEUS_METRICS.md`) after committing the alert/metric changes.
9. **Namespace derived "librarian_*" metrics** in the Kafka→Prometheus exporter to prevent cross-job ambiguity.
10. **Update docs** (`monitoring/PROMETHEUS_METRICS.md`, `docs/OBSERVABILITY_RUNBOOK.md`) to reflect the canonical metric schema and expected labels.
