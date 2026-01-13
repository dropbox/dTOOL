# Re-Audit v8: Kafka + Streaming Metrics (DashStream) — 2025-12-23

This extends:
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v7.md`
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-22_v6.md`

Re-checked against current `main` HEAD (through worker commit `#1494`).

Intent: be skeptical. Treat “✅ FIXED” claims in docs/roadmap as hypotheses; verify in code + deploy config. Where the repo’s tracking says “complete” but code disagrees, this report explicitly re-opens the item and gives an actionable fix plan.

---

## 0) Executive Summary (What’s actually true at HEAD)

### ✅ True / Verified
- WebSocket server lag polling was moved off the hot consume loop (M-430). It no longer calls `fetch_watermarks()` inline with `consumer.recv()`.
- WebSocket offset storage uses `store_offset_from_message(&msg)` (M-414), so restart consumes from the next message.
- Alert-rule drift guard exists and passes: `./scripts/check_dashstream_alert_rules_sync.sh --promtool`.

### ❌ False / Overstated in repo tracking (must be re-opened)
- **M-413 “Kafka security config unification” is NOT complete**. `KafkaSecurityConfig` exists, but it is only applied in some binaries. Several Kafka clients still ignore it (CLI, admin helpers, quality aggregator, core DashStream producer path). `docs/CONFIGURATION.md`, `WORKER_DIRECTIVE.md`, and `ROADMAP_CURRENT.md` currently claim broader coverage than exists.

### ⚠️ Regressions / New high-risk findings introduced by the “fix”
- The WebSocket lag-monitor background task uses an **unbounded channel** fed **once per message** + performs **blocking** `fetch_watermarks()` calls inside an async task. Under high throughput or Kafka slowness, this can create an unbounded backlog (memory growth) and/or runtime starvation. The hot path is “unblocked”, but the process can still be destabilized.

---

## 1) Status of v7 Findings (Checked Against HEAD)

| v7 item | Status at HEAD | Evidence / why |
|---|---|---|
| CLI applies `KafkaSecurityConfig` | ❌ NOT FIXED | `crates/dashflow-cli/src/commands/*.rs` constructs `ClientConfig::new()` directly and never calls `KafkaSecurityConfig::from_env().apply_to_rdkafka(...)`. |
| Admin helpers apply `KafkaSecurityConfig` | ❌ NOT FIXED | `crates/dashflow-streaming/src/kafka.rs` creates `AdminClient`/`BaseConsumer` via `ClientConfig::new()` without applying `KafkaSecurityConfig`. |
| Exporter supports `KAFKA_AUTO_OFFSET_RESET` | ❌ NOT FIXED | Exporter hardcodes `.set("auto.offset.reset", "earliest")` in `crates/dashflow-prometheus-exporter/src/main.rs`. |
| CLI topic defaults/env wiring consistent | ❌ NOT FIXED | Only `replay` declares `env="KAFKA_BROKERS"`/`env="KAFKA_TOPIC"`. Most commands default `topic="dashstream"`. |
| DLQ durability semantics decision (M-429) | ❌ NOT FIXED | Still best-effort; offsets advance regardless of DLQ send outcome; decision not explicitly “owned” with acceptance criteria. |
| Infra error alert exists | ❌ NOT FIXED | Alert rules comment points to `websocket_infrastructure_errors_total`, but no alert fires on its rate. |

---

## 2) Kafka Client Surface Area (Inventory + Config Reality)

### 2.1 Long-running services (production-ish)
- `crates/dashflow-observability/src/bin/websocket_server.rs` (rdkafka consumer + DLQ producer + lag monitor)
  - ✅ Applies `KafkaSecurityConfig` (consumer, DLQ producer, metadata consumer).
  - ⚠️ Lag monitor implementation hazards (see Section 3).
- `crates/dashflow-prometheus-exporter/src/main.rs` (rdkafka consumer)
  - ✅ Applies `KafkaSecurityConfig`.
  - ❌ Does not honor `KAFKA_AUTO_OFFSET_RESET` despite docs/helm setting it.
- `crates/dashflow-streaming/src/bin/quality_aggregator.rs` (rskafka)
  - ✅ Reads `KAFKA_BROKERS`/`KAFKA_TOPIC`.
  - ❌ Does not read/apply Kafka security env vars (TLS/SASL), despite repo docs describing them globally.

### 2.2 Core telemetry production path (likely the most important producer)
- `crates/dashflow/src/dashstream_callback.rs` uses `dashflow_streaming::producer::DashStreamProducer`
  - ✅ Reads `KAFKA_BROKERS`/`KAFKA_TOPIC`.
  - ❌ Does not expose or load security env vars (TLS/SASL). `ProducerConfig` defaults to `plaintext` unless set programmatically.

### 2.3 Operator tooling
- `crates/dashflow-cli/src/commands/*.rs`
  - ❌ No `KafkaSecurityConfig` application.
  - ❌ Inconsistent env-var wiring (most commands ignore `KAFKA_*` env vars).
  - ⚠️ Defaults (`topic="dashstream"`) are inconsistent with the stack’s typical `dashstream-quality`.

### 2.4 Kafka admin/helpers (used for provisioning + metadata checks)
- `crates/dashflow-streaming/src/kafka.rs`
  - ✅ Defines `KafkaSecurityConfig`.
  - ❌ Does not apply it to the `AdminClient` / metadata `BaseConsumer` it creates.

---

## 3) Streaming Metrics: Correctness + Bug Audit (High Signal)

### 3.1 WebSocket consumer lag (`websocket_kafka_consumer_lag`) still has correctness hazards

Current design (M-430):
- Main consumer sends `(partition, offset+1)` for every message on an **unbounded channel**.
- Background task periodically calls blocking `fetch_watermarks(topic, partition, 1s)` for each tracked partition and sets the gauge.

**Bugs / footguns:**
1) **Unbounded memory growth risk**: offset updates are produced per message, but consumption is paused during `fetch_watermarks()` calls. Under load, the channel can grow without bound.
2) **Blocking inside async**: `fetch_watermarks()` is synchronous; calling it inside a tokio task can block runtime worker threads (latency spikes elsewhere).
3) **Rebalance / assignment drift**: tracked partitions are never cleared; if assignment changes, gauges can remain stale/high and page falsely.
4) **No shutdown wiring**: lag monitor task never listens for shutdown; it relies on runtime cancellation (usually fine, but harder to reason about).

**Fix direction (P0/P1):**
- Replace the unbounded “per-message” channel with a “latest offset per partition” structure:
  - Use `DashMap<i32, AtomicI64>` or `RwLock<HashMap<i32, i64>>` updated in the hot loop (O(1)).
  - Lag monitor ticks read a snapshot and compute lag.
  - Optional: throttle updates (only write when offset increases by N or at most once per X ms).
- Move watermark fetching off async runtime threads:
  - Either `tokio::task::spawn_blocking` around the entire tick, or prefetch watermarks via a dedicated thread.
- Make lag measurement assignment-aware:
  - Track current assignment (rebalance callbacks) and clear offsets/gauges for revoked partitions.
  - If assignment APIs aren’t reliably available, add an “offset age” gauge and treat stale offsets as unknown (don’t page on them).

**Add explicit lag-monitor health metrics (P1):**
- `websocket_kafka_lag_poll_failures_total`
- `websocket_kafka_lag_poll_duration_ms` (histogram)
- `websocket_kafka_lag_offset_age_seconds{partition}` (gauge; time since last offset update)

### 3.2 Missing infra paging: infra errors are measured but not operationalized

Alert rules explicitly distinguish:
- `websocket_kafka_messages_total{status="error"}` = message processing errors (decode/format failures)
- `websocket_infrastructure_errors_total` = Kafka/network/client infra

…but there is no alert on infra error rate. Grafana has a panel (`grafana/dashboards/streaming_metrics_dashboard.json`), but no paging.

**Fix direction (P2, but operationally important):**
- Add alert `KafkaInfraErrorsHigh` based on `sum(rate(websocket_infrastructure_errors_total[5m]))` (and optionally `websocket_kafka_errors_by_type_total`).

### 3.3 Exporter offset reset is misconfigured vs docs

- Helm config sets `KAFKA_AUTO_OFFSET_RESET`.
- Docs claim `KAFKA_AUTO_OFFSET_RESET` exists globally.
- Exporter hardcodes `"earliest"`.

**Fix direction (P1):**
- Add `KAFKA_AUTO_OFFSET_RESET` support to exporter with validation (`earliest|latest`), mirroring websocket-server behavior.

---

## 4) Repo Tracking Drift: Items Marked Complete That Aren’t

### 4.1 M-413 is incorrectly marked “complete” in multiple places

Files that currently over-claim:
- `docs/CONFIGURATION.md` (“wired into dashflow-streaming crate helpers (producer/consumer/admin)”)
- `WORKER_DIRECTIVE.md` (M-413 marked ✅ COMPLETE)
- `ROADMAP_CURRENT.md` (M-413 marked `[x]` with “used consistently across ... producer/consumer/admin”)

**Reality:** `KafkaSecurityConfig` is *defined* in `dashflow-streaming`, but is only *applied* by:
- `crates/dashflow-observability/src/bin/websocket_server.rs`
- `crates/dashflow-prometheus-exporter/src/main.rs`

**Fix direction:** either:
1) Make the code match the docs (preferred): apply security config everywhere it claims, OR
2) Make docs/tracking honest and scoped (stop claiming “unified” until it’s actually unified).

---

## 5) Worker Priority List (Do Next, With Acceptance Criteria)

### P0 (Correctness / “will page or melt”)
1) **Fix WebSocket lag monitor design hazards**
   - Replace unbounded per-message channel; ensure no unbounded buffering.
   - Ensure blocking watermark fetch doesn’t run on async worker threads.
   - Handle assignment drift (no stale partitions).
   - Acceptance: under synthetic high-throughput, memory stays bounded; lag gauge doesn’t page falsely after a rebalance.

2) **Re-open M-413 in tracking + docs**
   - Update `docs/CONFIGURATION.md`, `WORKER_DIRECTIVE.md`, `ROADMAP_CURRENT.md` to reflect reality (until fixed).
   - Acceptance: no repo doc claims M-413 “complete” unless it is verifiably complete.

### P1 (Config correctness / production usability)
3) **Exporter: honor `KAFKA_AUTO_OFFSET_RESET`**
   - Acceptance: setting `KAFKA_AUTO_OFFSET_RESET=latest` changes consumer behavior on first run; documented.

4) **Apply `KafkaSecurityConfig` to remaining rdkafka clients**
   - `dashflow-cli` consumers.
   - `dashflow-streaming/src/kafka.rs` admin + metadata helpers.
   - Acceptance: with `KAFKA_SECURITY_PROTOCOL=sasl_ssl` + CA + SASL creds, all of these can connect without code changes.

5) **Standardize CLI Kafka defaults + env var wiring**
   - All commands accept `env="KAFKA_BROKERS"` + `env="KAFKA_TOPIC"` consistently.
   - Default topic aligns with the stack (pick one and stick to it; document).
   - Avoid surprising commits to shared consumer groups (default to unique group IDs unless explicitly set).

### P2 (Design choices / follow-ups)
6) **DLQ durability semantics (M-429)**
   - Explicitly decide: best-effort vs fail-closed; document and add bounded-retry behavior if fail-closed is chosen.

7) **Expose Kafka security env vars in deploy manifests**
   - Helm/K8s templates should allow passing `KAFKA_SECURITY_PROTOCOL`, `KAFKA_SASL_*`, `KAFKA_SSL_*` (likely via Secret refs).

8) **Docs cleanup: remove stale `DASHSTREAM_*` Kafka env vars**
   - `docs/PRODUCTION_DEPLOYMENT_GUIDE.md` and `crates/dashflow-cli/examples/*.sh` currently use env var names that aren’t read by code.

---

## 6) Verification Commands (Worker Must Run)

```bash
# Alert rules drift + promtool validation
./scripts/check_dashstream_alert_rules_sync.sh
./scripts/check_dashstream_alert_rules_sync.sh --promtool

# Build the Kafka-facing binaries
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-cli
```
