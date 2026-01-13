# Re-Audit: Kafka + Streaming Metrics (DashStream) — 2025-12-22

**Scope**: Kafka usage + end-to-end streaming observability (DashStream topics, consumers, metrics, alerts, dashboards, runbooks).

This is a **verification pass against current `HEAD`** (see `git log --oneline -15`). It focuses on confirming which previously-raised S-series issues are actually addressed in code/config/docs, and flags additional correctness gaps of the same kind (metric semantics drift, counter monotonicity, stale runbook queries).

**Update (post-fix):** The major correctness/config gaps highlighted in this re-audit were addressed in `#1450` and `#1451` (WebSocket server metrics semantics, topic/DLQ configurability, old-data detection, k8s/helm scrape config, exporter README/tests drift). Treat the sections below as historical evidence of what was wrong prior to those fixes.

---

## Executive Summary

1. **Alerts (S-20) are fixed**, but **the WebSocket server metrics semantics (S-25) are still inconsistent**: infra errors are counted as “message errors”, breaking the “success = total - errors” invariant and making `/metrics` counters semantically unstable.
2. **Self-improvement streaming consumer (S-24) is still single-partition** by design (`DashStreamConsumer`/rskafka `PartitionClient`), so multi-partition topics will be silently under-consumed.
3. **Docs drift remains a recurring failure mode**:
   - `docs/OBSERVABILITY_RUNBOOK.md` investigation PromQL and health-field examples are stale even though alert rule blocks were updated (S-26 “fixed” is incomplete).
   - `monitoring/PROMETHEUS_METRICS.md` is still out of sync for multiple `websocket_*` metrics beyond `decode_errors_total` (S-27 “fixed” is incomplete).
   - `docs/OBSERVABILITY_INFRASTRUCTURE.md` currently lists metrics that **do not exist in the emitter** (doc drift in the other direction).

---

## “Fixed vs Not Fixed” Verification (S-20 → S-27)

### ✅ S-20: HighDecodeErrorRate denominator correctness

- **Alert rule is correct**: `monitoring/alert_rules.yml:18-20` uses total-message denominator with `sum()` and `clamp_min`.

### ⚠️ S-24: Self-improvement streaming consumer partition coverage

- **Not fixed (documented only)**: `crates/dashflow/src/self_improvement/streaming_consumer.rs:464` explicitly documents “partition 0 only”.
- **Why**: Uses `dashflow_streaming::consumer::DashStreamConsumer` which is a **single-partition** rskafka `PartitionClient` wrapper.
- **Impact**: For multi-partition topics, self-improvement triggers are computed from a partial stream (silent correctness failure).

### ❌ S-25: WebSocket `/metrics` success semantics / underflow and monotonicity

`WORKER_DIRECTIVE.md:857` claims S-25 is fixed “by documentation”, but code still violates the documented invariant.

**Evidence**
- `/metrics` computes `success = total - errors`: `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:358-370` (note: file split from single websocket_server.rs).
- Kafka infra errors increment `kafka_errors` without incrementing `kafka_messages_received`: `crates/dashflow-observability/src/bin/websocket_server/main.rs:2271` (note: file split from single websocket_server.rs).

**Why this is still wrong**
- The comment says `kafka_errors ⊆ kafka_messages_received`, but infra errors break this.
- `success_count` can become artificially low (or stuck at 0 via `saturating_sub`), and can appear to “reset” if the derived value decreases between scrapes.
- Alert semantics drift: `HighKafkaErrorRate` is described operationally as broker/network issues, but the underlying metric is mixing message failures and infra failures.

### ⚠️ S-26: Runbook drift vs alert rules

- **Partially fixed**: alert definition blocks match `monitoring/alert_rules.yml`, but investigation commands are still stale.
- **Evidence**:
  - Wrong PromQL (label-matching bug reintroduced): `docs/OBSERVABILITY_RUNBOOK.md:248-249` uses `rate(error)/rate(total)` without `sum()` or `clamp_min`.
  - References non-existent health fields: `docs/OBSERVABILITY_RUNBOOK.md:264` (`.kafka_producer` is not in `HealthResponse`).

### ⚠️ S-27: Metrics documentation drift (more than just decode errors)

- **Not fully fixed**: `monitoring/PROMETHEUS_METRICS.md:41-53` claims no labels for:
  - `websocket_client_lag_events_total` (actually has `severity`)
  - `websocket_client_lag_messages_total` (actually has `severity`)
  - `websocket_e2e_latency_ms` (actually a HistogramVec with `stage`)
- **Doc drift in the other direction**: `docs/OBSERVABILITY_INFRASTRUCTURE.md:252-265` lists:
  - `websocket_infrastructure_errors_total`
  - `websocket_old_data_decode_errors_total`
  but `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:358-370` does not export them.

---

## Additional Similar Bugs / Gaps (Correctness + Observability)

### 1) Health endpoint "recent infra errors" logic is incorrect

- **Evidence**: `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:160-185`.
- **Problem**: `recent_infrastructure_errors = infrastructure_errors > 0 && last_kafka_message_ago_seconds < 120`.
  - Once `infrastructure_errors` becomes non-zero, and messages are flowing, this remains true indefinitely.
- **Impact**: Health status can get stuck in `"reconnecting"` long after the last infra error.
- **Fix direction**: Track a timestamp for last infra error (e.g., `last_infrastructure_error_at`) and compute recency from that; export it in health JSON.

### 2) Circuit breaker double-counts errors

- **Evidence**: `crates/dashflow-observability/src/bin/websocket_server/main.rs:3094-3102`.
- **Problem**: `current_error_count = kafka_errors + decode_errors` even though decode errors already contribute to `kafka_errors` in the current design.

### 3) Prometheus emitter pattern is brittle (`/metrics` manually concatenates text)

- `/metrics` hand-builds `websocket_kafka_messages_total` and a few others, then selectively appends `TextEncoder` outputs for some registry metrics.
- **Risk**: new metrics get silently “lost” unless someone remembers to add them to `/metrics` handler.
- **Fix direction**: Put all metrics in a `Registry` and export via `registry.gather()`; avoid hand-built text except as a transitional compatibility layer.

### 4) Dashboards still assume removed labels

- Grafana legend still references `thread_id` even though sequence metrics no longer carry it (`grafana/dashboards/grafana_quality_dashboard.json`).
- **Fix direction**: Update legends/queries to match the current label schema (use logs/traces for thread-level debugging).

---

## Kafka Design Critique (Correctness + Configuration)

### Mixed Kafka stacks (rdkafka vs rskafka) with mismatched semantics

- Long-running services (`websocket_server`, `dashflow-prometheus-exporter`) use **rdkafka consumer groups + commits**.
- `DashStreamConsumer` (rskafka `PartitionClient`) is **single-partition** and does **not** implement consumer group semantics (despite taking `group_id`).

**Recommendation**
- Use rdkafka consumer groups for any production, long-lived consumer.
- Treat rskafka PartitionClient consumer as a low-level tool and require explicit multi-partition fan-in (as done for `quality_aggregator`).

### Producer application-level retry duplicate risk

- `DashStreamProducer` retries at the application layer; with idempotence this can still create duplicates if the first send succeeded but the client observed an error.
- **Recommendation**: Either (a) rely on broker/client retries + idempotence and remove app-level retry, or (b) treat duplicates as normal and ensure consumers dedupe using stable message IDs (sequence/thread_id + partition/offset).

---

## Worker Fix Priority (Actionable Checklist)

### P0 — correctness / alert safety

1. **Fix S-25 for real**:
   - Split infra errors vs message errors; do not increment `kafka_errors` on `consumer.recv()` errors (`crates/dashflow-observability/src/bin/websocket_server/main.rs:2254-2271`).
   - Export infra error counters explicitly (and optionally old-data decode errors), or remove them from docs.
   - Ensure `websocket_kafka_messages_total` counters remain monotonic and semantically meaningful.
2. **Fix health "recent infra errors"** (`crates/dashflow-observability/src/bin/websocket_server/handlers.rs:160-185`) by tracking last infra error timestamp.
3. **Fix circuit breaker error accounting** (`crates/dashflow-observability/src/bin/websocket_server/main.rs:3094-3102`) to avoid double counting.
4. **Repair runbook investigation PromQL + health fields** (`docs/OBSERVABILITY_RUNBOOK.md:248-265`) to match `monitoring/alert_rules.yml:7-25`.

### P1 — correctness (coverage) + operational safety

5. **Implement S-24**: multi-partition support for self-improvement streaming consumer:
   - Copy the fan-in pattern from `crates/dashflow-streaming/src/bin/quality_aggregator.rs` or switch to rdkafka consumer groups.
6. **Update `monitoring/PROMETHEUS_METRICS.md`** beyond decode errors (labels/types for lag + E2E histogram, plus “not exported” vs “exported” clarity).
7. **Update Grafana dashboards** to remove `thread_id` assumptions.

### P2 — maintainability

8. **Replace manual `/metrics` concatenation** with a registry-gather approach; keep the current text output only as a compatibility layer during migration.

---

## Notes for the Next AI Worker

- Treat `WORKER_DIRECTIVE.md` statuses for S-25/S-26/S-27 as **“needs re-validation”**. Several are “fixed by documentation” only, and the code/docs remain inconsistent.
- When you fix metrics semantics, update: `monitoring/alert_rules.yml`, `docs/OBSERVABILITY_RUNBOOK.md`, `monitoring/PROMETHEUS_METRICS.md`, and `grafana/dashboards/grafana_quality_dashboard.json` together to prevent the same drift recurring.
