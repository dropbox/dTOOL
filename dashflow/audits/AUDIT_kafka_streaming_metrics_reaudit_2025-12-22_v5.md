# Re-Audit v5: Kafka + Streaming Metrics (DashStream) — 2025-12-22

This document extends `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-22_v4.md` with a second skeptical pass focused on:
- metric correctness (Prometheus counter monotonicity, scrape semantics)
- alert/runbook/CLI drift (same alert, different names/meaning across layers)
- Kafka config correctness (offsets/commits, TLS/SASL config, lag measurement cost)

It is written as a worker execution plan with explicit “why” + “how to fix” guidance.

---

## 0) What’s Already Fixed (Re-verified at HEAD)

### ✅ Offsets: WebSocket consumer stores correct committed offset
- WebSocket server stores offsets via `store_offset_from_message(&msg)` (offset+1 semantics).
- This prevents replaying the last message on restart (M-414).

### ✅ Single-replica semantics for websocket-server in K8s
- `deploy/kubernetes/base/websocket-server.yaml` pins `replicas: 1` with rationale (M-415).
- Helm defaults to `replicaCount: 1` (but see “autoscaling footguns” below).

### ✅ Kafka security config is now applied to rdkafka binaries (M-413)
`KafkaSecurityConfig::from_env().validate()` + `.apply_to_rdkafka()` is now applied in:
- `crates/dashflow-observability/src/bin/websocket_server.rs` (consumer + DLQ producer)
- `crates/dashflow-prometheus-exporter/src/main.rs` (consumer)

---

## 1) New Findings (This v5 Pass)

### 1.1 Counter correctness bug: “synthetic success counter” can go backwards between scrapes

**Symptom:** `websocket_kafka_messages_total{status="success"}` was previously computed as:
`success = total_messages_received - kafka_errors`.

Because `total_messages_received` and `kafka_errors` are independent atomics, Prometheus can observe an inconsistent snapshot and see `success` decrease, violating counter monotonicity. That corrupts `rate()`/`increase()` and can cause false alerts.

**Fix direction (P0):**
- Track explicit monotonic counters for `success` and `error` rather than deriving them at scrape-time.
- Only export counters that are individually monotonic.

**Status:** this should now be fixed by tracking explicit counters in `ServerMetrics` and exporting those series directly (see the patch that introduced `kafka_messages_success` / `kafka_messages_error`).

### 1.2 Alert-rule drift remains a structural risk (already happened once)

**Problem:** There are two alert rule files:
- canonical: `monitoring/alert_rules.yml`
- Kubernetes copy: `deploy/kubernetes/base/configs/alert_rules.yml`

They drifted previously (old alert names, missing lag alerts). This is a “works locally, fails in prod” trap.

**Fix direction (P0):**
- Add an explicit repo check to fail when these differ.
- Preferably remove duplication entirely (generate the K8s ConfigMap from canonical).

**Status:** a dedicated script should exist now: `scripts/check_dashstream_alert_rules_sync.sh`.

### 1.3 CLI alert explanations drifted from reality

`dashflow status` had a `HighKafkaErrorRate` explanation that described “Kafka connectivity issues”, but the system renamed the alert to `HighMessageProcessingErrorRate` and clarified it is primarily decode failures.

Additionally, lag alert names are `KafkaConsumerLagHigh`/`KafkaConsumerLagCritical`, but the CLI handled `KafkaConsumerLag`.

**Fix direction (P1):**
- Map both old/new alert names to a single explanation (backward compatibility).
- Ensure command snippets don’t require nonstandard tooling (use `grep`, not `rg`).

### 1.4 Lag monitoring correctness vs performance (still a risk)

The WebSocket server’s lag metric is computed by calling `fetch_watermarks()` in-line in the main consume loop.

**Risks:**
- With N partitions, worst-case blocking is O(N * timeout) every interval.
- On rebalance/revocation, `current_offsets` can retain partitions you no longer own, producing stale lag gauges.

**Fix direction (P2):**
- Move watermark polling to a background task.
- Track the current assignment and only compute lag for assigned partitions.
- Export an “lag_update_errors_total” counter if watermark fetch fails persistently.

---

## 2) Worker Priority List (Do These Next)

### P0 (Correctness / paging reliability)
1. **Ensure `websocket_kafka_messages_total{status=*}` is monotonic under scrape**
   - Add a small unit test that gathers `/metrics` twice while incrementing error+success counters and asserts counters never decrease.
   - If unit testing is too awkward, add a structured note in docs and treat this as a regression guard to revisit.

2. **Make alert rules single-source-of-truth**
   - Wire `scripts/check_dashstream_alert_rules_sync.sh` into the default verification path (whatever “verify” script the repo uses for docs/infra).
   - Minimum: document that every alert change must update both files (or the script must pass).

### P1 (Operational correctness / responder UX)
3. **Update exporter + websocket docs to mention TLS/SASL and group IDs**
   - `crates/dashflow-prometheus-exporter/README.md` should list `KAFKA_GROUP_ID` and the TLS/SASL env vars.
   - Confirm websocket server docs (and `docs/CONFIGURATION.md`) are consistent.

4. **Fix CLI alert-name drift**
   - `crates/dashflow-cli/src/commands/status.rs` should explain `HighMessageProcessingErrorRate` and `KafkaConsumerLagHigh/Critical`.
   - Keep backward-compatible aliases for old names.

### P2 (Scale and performance)
5. **Move consumer lag polling off the hot path**
   - Background task + assignment-aware lag computation.

6. **DLQ durability mode**
   - Today: DLQ send happens asynchronously; offsets can be committed even if DLQ send fails (forensic loss).
   - Add a config knob like `DLQ_MODE=best_effort|fail_closed`:
     - `best_effort`: current behavior (don’t stall stream)
     - `fail_closed`: don’t store/commit offset until DLQ send succeeds (with backoff)

---

## 3) Verification Commands (Worker Must Run)

```bash
# Alert rules must match
./scripts/check_dashstream_alert_rules_sync.sh

# promtool validation (docker)
./scripts/check_dashstream_alert_rules_sync.sh --promtool

# Build the Kafka-facing binaries
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
```

---

## 4) “Look For More Similar Bugs” Heuristics

1. **Never export a Prometheus Counter derived from multiple atomics**
   - Derived values belong in PromQL, or must be stored as their own monotonic counters.

2. **Any config duplicated in two places is a future incident**
   - Prefer single source-of-truth or enforce sync with a check script.

3. **Anything that does network I/O inside a consume loop is a potential lag generator**
   - Move it to a bounded background task with explicit error metrics.
