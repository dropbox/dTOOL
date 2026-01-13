# Re-Audit v6: Kafka + Streaming Metrics (DashStream) — 2025-12-22

This extends `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-22_v5.md` with another skeptical pass emphasizing:
- correctness edge cases that survive “it works locally”
- configuration and deployment footguns
- observability design gaps (alerts/dashboards that don’t match emitted metrics)

It is written as a directive for the next AI worker: what to fix, why, and how to verify.

---

## 0) Quick Reality Check (What’s True *Right Now*)

### ✅ Kafka security env vars are now actually applied (M-413)
`KafkaSecurityConfig` is applied in:
- `crates/dashflow-observability/src/bin/websocket_server.rs` (consumer + DLQ producer)
- `crates/dashflow-prometheus-exporter/src/main.rs` (consumer)

### ✅ Prometheus counter monotonicity is now enforced for websocket message status
`websocket_kafka_messages_total{status=*}` is backed by explicit counters, not derived deltas, preventing “counter goes backwards” scrape corruption.

### ✅ Alert-rule drift has a guard (but only if used)
There’s a sync checker script; ensure it’s run in the worker “verify” path.

---

## 1) New/Remaining Correctness Risks

### 1.1 Consumer lag metric correctness under rebalances (assignment drift)

Current lag computation tracks `current_offsets` in a `HashMap<partition, offset>` and periodically calls `fetch_watermarks()` for each tracked partition.

**Failure mode:** after rebalances/assignment changes, the map can retain partitions you no longer own, and gauges for those partitions can remain stale/high indefinitely. This can page you for “lag” that is an artifact of stale state, not an actual stuck consumer.

**Fix (P0/P1):**
- Query current assignment (`consumer.assignment()`) and only compute lag for assigned partitions.
- On assignment changes, clear `current_offsets` for revoked partitions and set gauge to 0 (or remove series) for partitions no longer assigned.

**Verification:**
- Force a rebalance (start/stop a second consumer in the same group) and confirm lag gauges don’t remain stuck for revoked partitions.

### 1.2 Consumer lag computation can block the hot consume loop (performance correctness)

Lag polling currently happens in the main consume loop and can perform multiple blocking `fetch_watermarks(..., timeout=1s)` calls per interval.

**Failure mode:** under many partitions or Kafka slowness, lag polling steals time from consumption, which increases lag—a positive feedback loop.

**Fix (P1):**
- Move watermark polling into a background task with bounded concurrency/time budget.
- Emit an explicit metric for lag-update failures/latency (e.g., `websocket_kafka_lag_poll_failures_total`, `websocket_kafka_lag_poll_latency_ms`).

### 1.3 “Kafka errors by type” metric naming is misleading (semantic drift)

`websocket_kafka_errors_by_type_total{error_type=...}` is derived from `consumer.recv()` errors, with a label `decode_error` that can be confused with protobuf decode failures tracked by `websocket_decode_errors_total`.

**Fix (P2):**
- Rename to something like `websocket_kafka_client_errors_by_type_total` (and update dashboard/alerts/docs), or
- Remove/replace the `decode_error` label value to avoid collision with protobuf decode concepts.

### 1.4 DLQ durability vs offset commit is still a design hole

DLQ publishing is async/best-effort; offsets are stored/committed regardless of DLQ publish success.

**Failure mode:** you can permanently lose forensic payloads for the exact messages you most need to debug.

**Fix (P2, but operationally important):**
- Add `DLQ_MODE=best_effort|fail_closed`:
  - `best_effort`: current behavior (never block stream)
  - `fail_closed`: do not store/commit offset until DLQ send succeeds (with bounded retry/backoff)
- Add metrics for DLQ backlog and “fail closed stall” time.

---

## 2) Configuration/Deployment Footguns

### 2.1 Group ID collisions across environments

Defaults like `websocket-server-v4` and `prometheus-exporter` are fine locally, but in real deployments you often want group IDs scoped by environment/cluster.

**Fix (P1):**
- Recommend (or enforce) environment-specific group IDs via `KAFKA_GROUP_ID` in K8s `ConfigMap`/Helm values.

### 2.2 Topic mismatch remains an easy “no data” trap

Some components default to `dashstream-quality` while library defaults may differ. Operators frequently forget to set a consistent `KAFKA_TOPIC`.

**Fix (P1/P2):**
- Consolidate topic defaults in one place and ensure all DashStream components reference it (or fail fast with a clear error on mismatch).
- Add a startup log line for every component: `brokers`, `topic`, `group_id`, and security protocol.

### 2.3 Alert-rule duplication must be made impossible, not “discouraged”

Even with a checker script, drift can reappear if the script isn’t run.

**Fix (P0):**
- Wire the sync check into the repo’s standard verification workflow (or a pre-commit hook).
- Prefer single-source-of-truth: generate the K8s config from `monitoring/alert_rules.yml`.

---

## 3) Observability Gaps (Metrics exist but aren’t “operationalized”)

### 3.1 Consumer lag dashboards are missing

The metric and alerts exist, but Grafana doesn’t show `websocket_kafka_consumer_lag` prominently, and the runbook doesn’t have a clear playbook for lag.

**Fix (P1):**
- Add panels to `grafana/dashboards/streaming_metrics_dashboard.json`:
  - max lag across partitions
  - per-partition lag heatmap/time series
  - lag vs dropped messages vs E2E latency correlations
- Add runbook entry for `KafkaConsumerLagHigh/Critical` with concrete steps.

### 3.2 No explicit alert for infrastructure errors

You track `websocket_infrastructure_errors_total` and `websocket_kafka_errors_by_type_total`, but the current alert set focuses on decode/processing errors and lag/drops.

**Fix (P2):**
- Add an alert like `KafkaInfraErrorsHigh` based on rate of `websocket_infrastructure_errors_total` and/or `websocket_kafka_errors_by_type_total`.

---

## 4) Worker Priority List (Do Next)

### P0
1. Assignment-aware lag gauge: compute lag only for assigned partitions; clear stale partitions on rebalance.
2. Make alert rules single-source-of-truth (enforced in verification, not optional).

### P1
3. Move lag polling off hot consume loop; add lag poll failure/latency metrics.
4. Add Grafana lag panels + runbook playbook.
5. Standardize consumer config knobs (timeouts, poll intervals) across websocket-server and exporter.

### P2
6. DLQ durability mode (`best_effort` vs `fail_closed`) + metrics.
7. Rename/clarify `websocket_kafka_errors_by_type_total` semantics to avoid “decode” confusion.

---

## 5) Verification Commands (Worker Must Run)

```bash
# Prevent alert-rule drift
./scripts/check_dashstream_alert_rules_sync.sh

# promtool validation (docker)
./scripts/check_dashstream_alert_rules_sync.sh --promtool

# Build the Kafka-facing binaries
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-cli
```
