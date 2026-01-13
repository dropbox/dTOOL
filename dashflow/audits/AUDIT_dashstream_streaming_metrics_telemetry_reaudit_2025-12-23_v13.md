# Re-Audit v13: DashStream Streaming Telemetry (Metrics/Measurement/Export) — 2025-12-23

This extends:
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v12.md`

Re-checked against `main` HEAD through worker commit `#1562`.

Intent: skeptical, correctness-first. This audit focuses on **DashStream telemetry** (metrics + measurement + export surfaces), not just Kafka correctness. The key question is: *are the metrics we think we have actually visible to Prometheus, and do their names/labels/units mean what alerts/docs assume?*

---

## 0) Mental Model: Where DashStream Telemetry Comes From (and Where It Must Surface)

DashStream observability is only “real” if it survives the full chain:

1) **Emit** in-process counters/histograms (DashStreamCallback, producer/consumer, websocket-server, exporter)
2) **Register** into the *same* Prometheus registry that is scraped
3) **Expose** via a `/metrics` endpoint
4) **Document** consistently (names, labels, buckets, semantics)
5) **Alert** on metrics that truly exist in the scraped output

In this repo today, step (2) is the major systemic failure.

---

## 1) Critical Finding: Prometheus Registry Split = Silent Missing Metrics

### 1.1 What I verified in code (registry ownership)

**DashStreamCallback** registers its global drop metric into the *Prometheus default registry*:
- `crates/dashflow/src/dashstream_callback/mod.rs` → `prometheus::default_registry().register(...)`
  - Metric: `dashstream_telemetry_dropped_total`

**dashflow-streaming library** registers most “library metrics” into the *default registry*:
- `crates/dashflow-streaming/src/metrics_utils.rs` → `prometheus::default_registry().register(...)`
  - Examples: `dashstream_messages_sent_total`, `dashstream_messages_received_total`, `dashstream_rate_limit_exceeded_total`, `dashstream_dlq_*`, etc.
- `crates/dashflow-streaming/src/metrics_monitor.rs` reads from the *default registry* via `prometheus::gather()`

**But the production-ish binaries tend to scrape from a *custom registry***:
- **WebSocket server** uses a custom registry:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs` → `let prometheus_registry = Registry::new();` and `/metrics` uses `state.prometheus_registry.gather()`
- **Prometheus exporter** uses a custom registry:
  - `crates/dashflow-prometheus-exporter/src/main.rs` → `let registry = Arc::new(Registry::new());` and `/metrics` uses `registry.gather()`
- **DashFlow “generic metrics server”** uses `dashflow_observability::metrics::MetricsRegistry`, which also wraps a custom registry:
  - `crates/dashflow-observability/src/metrics_server.rs` → `MetricsRegistry::global().export()` which gathers its internal `Registry::new()`

### 1.2 Impact (this is a correctness failure, not just “style”)

This is the highest-risk DashStream telemetry gap right now:

- **A metric can be “implemented” and even unit-tested (via `prometheus::gather()`), but never appear in production scrape output.**
- The repo includes explicit operator guidance to alert on `dashstream_telemetry_dropped_total` (DashStreamCallback), but there is no guarantee it is exported anywhere.

Concretely, in the default docker-compose stack (`docker-compose.dashstream.yml`):
- `/metrics` on **websocket-server** does **not** include default registry metrics (only its custom registry).
- `/metrics` on **prometheus-exporter** does **not** include default registry metrics (only its custom registry).
- “quality-monitor” is actually the `quality_aggregator` binary and exposes **only `/health`**, no `/metrics`.

Result: important DashStream producer-side metrics (and drop/backpressure telemetry) are extremely likely to be **invisible** to Prometheus.

### 1.3 “Similar errors” pattern (this will keep happening)

Anywhere we:
- create metrics in libraries using `prometheus::default_registry()`, AND
- export via a handler that gathers from a custom `Registry::new()`,

we are building a **false sense of observability**.

---

## 2) Metric Contract Drift: Same Name, Different Meaning (Labels/Buckets/Scope)

This is the second systemic failure: even when metrics are exported, several names are reused across components with incompatible “contracts”.

### 2.1 Histogram bucket contract violation (dangerous for dashboards/alerts)

Metric: `dashstream_redis_operation_latency_ms` (HistogramVec)

Defined in two places with *different buckets*:
- `crates/dashflow-streaming/src/rate_limiter.rs` buckets include sub-millisecond (0.1, 0.5, … 500ms)
- `crates/dashflow-observability/src/bin/websocket_server/` buckets start at 1ms and use a different set up to 1000ms

Why this matters:
- Prometheus histograms are only aggregatable if bucket boundaries match across targets.
- Queries like `histogram_quantile(0.95, sum(rate(dashstream_redis_operation_latency_ms_bucket[5m])) by (le))` become misleading if half your fleet uses different `le` values.

### 2.2 Counter label contract mismatch

Metric: `dashstream_redis_connection_errors_total`
- In `rate_limiter.rs` it is a `CounterVec` with `operation` label.
- In the websocket server (`crates/dashflow-observability/src/bin/websocket_server/`) it is a plain `IntCounter` (no labels) and likely refers to replay-buffer Redis, not rate-limiting Redis.

Alerts/docs currently treat this as a generic Redis error signal (which is ambiguous).

### 2.3 DLQ metrics are documented as labeled, but implemented as unlabeled

`monitoring/PROMETHEUS_METRICS.md` claims the library emits:
- `dashstream_dlq_sends_total` as CounterVec with `error_type`
- `dashstream_dlq_send_failures_total` as CounterVec with `reason`

But `crates/dashflow-streaming/src/dlq.rs` implements these as plain `Counter` (no labels).

This mismatch also leaks into alert rule commentary (it mentions aggregating across labels that don’t exist).

---

## 3) Measurement Correctness: “Derived” Loss Metrics Are Not Sound As Written

`crates/dashflow-streaming/src/metrics_monitor.rs` computes `dashstream_message_loss_rate` by:
- reading `dashstream_messages_sent_total` and `dashstream_messages_received_total` from `prometheus::gather()` (default registry),
- then computing `(sent + send_failures - received) / (sent + send_failures)`.

Why this is not correct in real deployments:
- “sent” and “received” counters live in **different processes** (producer vs consumer).
- A single process-local registry cannot observe both sides, so the computed “loss” rate is at best a partial local statistic and at worst meaningless.

The only place this computation is valid is a monolithic binary that both produces and consumes and exports the default registry — which is not the DashStream architecture.

---

## 4) Config/Docs Consistency Gaps That Affect Telemetry Outcomes

### 4.1 M-435 still open (Kafka TLS/SASL env vars not wired in deploy manifests)
`KafkaSecurityConfig` supports secure env vars (`KAFKA_SECURITY_PROTOCOL`, `KAFKA_SASL_*`, `KAFKA_SSL_*`), but Helm/K8s don’t provide consistent wiring via values/secrets.

Outcome: operators may think they have “secure Kafka” support, but production manifests may silently omit required env vars.

### 4.2 M-436 still open (stale DASHSTREAM_* env vars in docs/examples)
Some docs reference env vars not read by the active code paths. This causes silent misconfiguration (telemetry simply not flowing, or flowing to wrong topics).

### 4.3 Helm chart appears incomplete for the exporter
`deploy/helm/dashflow/values.yaml` has `prometheusExporter` settings, but `deploy/helm/dashflow/templates/` contains no `prometheus-exporter` Deployment/Service template.

Outcome: a Helm-based deployment may ship **without** the exporter that provides most `dashstream_*` Prometheus metrics.

---

## 5) Worker Priority Fix List (What To Do Next, With Acceptance Criteria)

This extends the v12 P1 list (M-642..M-645). Those remain the top Kafka/metrics correctness items for the websocket-server.

### P1 — Must Fix (Telemetry correctness / alert validity)

#### M-646: Unify Prometheus registry usage for DashStream telemetry (metrics must actually be scraped)
**Problem:** DashStreamCallback + dashflow-streaming library metrics register into the default registry, while `/metrics` endpoints often gather from custom registries. This silently drops metrics from Prometheus.

**Fix direction (recommended, incremental):**
1) Pick a single “scraped registry contract” per binary.
2) If a binary exports a custom registry, it must also export default-registry metrics *without duplicating metric families*.
   - Implement a deduping merge at export time (by metric family name) and log when a collision occurs.
   - Then progressively migrate library metrics off `default_registry()` into the binary’s explicit registry to eliminate collisions entirely.

**Targets (highest value first):**
- `crates/dashflow-observability/src/metrics_server.rs` (DashFlow apps) — this is where DashStreamCallback metrics should become visible.
- Any binaries that use dashflow-streaming producer/consumer and expose `/metrics`.

**Acceptance:**
- Running a DashFlow process that uses `DashStreamCallback` and the metrics server:
  - `curl http://localhost:<port>/metrics | rg dashstream_telemetry_dropped_total` returns a metric line (after forcing at least one drop in a test).
- At least one producer-side counter from dashflow-streaming (`dashstream_messages_sent_total`) becomes visible on a scraped endpoint in a realistic setup.
- Scrape output contains **no duplicate metric family names** (Prometheus parser must accept it).

#### M-647: Enforce metric “contract” uniqueness (no shared name with different buckets/labels)
**Problem:** Some metrics share names across components but have incompatible buckets/labels (notably Redis histograms).

**Fix direction:**
- Either:
  1) Centralize definitions (names + labels + buckets) in one crate/module and reuse everywhere, OR
  2) Rename metrics to be component-scoped when semantics differ:
     - Example: `dashstream_rate_limiter_redis_operation_latency_ms` vs `websocket_replay_redis_operation_latency_ms`

**Acceptance:**
- `dashstream_redis_operation_latency_ms` is either:
  - defined once with consistent buckets everywhere it is emitted, OR
  - replaced by component-specific names so no cross-target bucket mismatch exists.
- `dashstream_redis_connection_errors_total` semantics are unambiguous (rate limiting vs replay buffer).

#### v12 P1 carry-over (still open; affects measurement correctness)
- **M-642** assignment-aware lag tracking (avoid false stale alerts on revoked partitions)
- **M-643** infra error alert tiering (avoid paging on single transient error)
- **M-644** clock-skew hardening for `websocket_e2e_latency_ms` (no negative/outlier corruption; stage label accurate)
- **M-645** reconcile `websocket_kafka_messages_total` vs `websocket_old_data_decode_errors_total` semantics

### P2 — High Value Cleanup (Docs/alerts must reflect reality)

#### M-648: Fix DLQ metric docs vs implementation (and decide if library DLQ should be labeled)
**Problem:** Docs/alerts suggest labeled DLQ metrics; implementation is unlabeled Counters.

**Fix direction:**
- Either update docs + alert commentary to match unlabeled Counters, OR
- Upgrade library DLQ metrics to CounterVec with bounded label sets (`error_type`, `reason`) and update the docs accordingly.

**Acceptance:**
- `monitoring/PROMETHEUS_METRICS.md` matches the emitted metric types/labels.
- Alert rule comments do not refer to nonexistent labels.

#### M-649: Deprecate or redesign `dashstream_message_loss_rate`
**Problem:** process-local loss computation is misleading in a distributed system.

**Fix direction:**
- Remove it (and any docs that imply it is end-to-end), OR
- Move loss detection to a topology-aware component (e.g., exporter-derived metrics using Kafka offsets/consumer lag + producer counts), with explicit semantics.

#### M-650: Fix Helm deployment completeness for Prometheus exporter (if Helm is supported)
**Problem:** Helm values exist but templates for exporter deployment/service appear missing.

**Acceptance:**
- Helm install produces a `prometheus-exporter` Deployment/Service when enabled, and Prometheus can scrape it.

---

## 6) Quick “Skeptical” Worker Verification Checklist

After any telemetry/metrics changes:
- Verify scrape output contains expected metrics:
  - `curl <service>/metrics | rg '^(dashstream_|websocket_)' | head`
- Verify no duplicate metric families (Prometheus text parser rejects duplicates):
  - `curl <service>/metrics | promtool check metrics` (or equivalent local parser)
- Verify alerts reference metrics that actually exist on the scraped targets:
  - `rg -n \"dashstream_|websocket_\" monitoring/alert_rules.yml` and then `curl`-grep those names on the relevant service `/metrics`.
