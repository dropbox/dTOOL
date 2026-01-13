# Re-Audit v10: Kafka + Streaming Metrics (DashStream) — 2025-12-23

This extends:
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v9.md`

Re-checked against current `main` HEAD (through worker commit `#1509`).

Intent: skeptical, correctness-first. Confirm which v9 findings were actually fixed, then look for second-order bugs introduced by the fixes and for adjacent config/observability gaps that remain.

---

## 0) What Changed Since v9 (Verified)

### ✅ M-431: Lag monitor design hazards were addressed (bounded + non-blocking)
The WebSocket server lag monitor no longer:
- pushes per-message offsets into an unbounded channel
- runs blocking `fetch_watermarks()` inside tokio worker threads

It now:
- stores offsets in `Arc<std::sync::RwLock<HashMap<i32, (i64, Instant)>>>`
- polls lag from a dedicated `std::thread`

### ✅ M-437: Lag-monitor health metrics now exist
New metrics were added:
- `websocket_kafka_lag_poll_failures_total`
- `websocket_kafka_lag_poll_duration_seconds{status="success"|"error"}`
- `websocket_kafka_lag_offset_age_seconds{partition="..."}`

This materially improves debuggability of “lag gauge stopped updating” scenarios.

---

## 1) New High-Risk Finding: Stale partition cleanup can *mask* real lag (P0)

The new lag monitor uses `KAFKA_LAG_STALE_PARTITION_SECS` (default 60s) to identify partitions with no offset updates, then:
- **removes** them from tracking
- **sets** their lag gauge and offset-age gauge to `0`

This is intended to address rebalance/assignment drift, but it has a severe false-negative failure mode:

### Failure mode: stuck consumer becomes “lag=0”
If the consumer is stalled (bug, deadlock, backpressure, decode loop, etc.), offsets stop updating. Producers may continue producing, so **lag is increasing**, but after 60s:
- the partition is removed
- `websocket_kafka_consumer_lag` is set to `0`
- `websocket_kafka_lag_offset_age_seconds` is set to `0`

Result: both lag and staleness signals disappear, and the existing lag alerts (`max(websocket_kafka_consumer_lag) > ...`) won’t page.

This is strictly worse than the pre-#1509 behavior: we traded false positives (stale partitions) for potentially silent outages (false negatives).

### Fix direction (P0)
Do not “zero out” lag for stale partitions unless you can prove the partition is no longer assigned.

Options (choose one):
1) **Assignment-aware tracking (preferred)**:
   - Maintain an `assigned_partitions` set from the *main consumer* (poll `consumer.assignment()` periodically or via a rebalance callback).
   - Lag monitor only computes/cleans partitions not in assignment.
2) **Keep stale partitions, don’t delete series**:
   - Do not remove from `partition_offsets`.
   - Keep `websocket_kafka_lag_offset_age_seconds` increasing.
   - Add an alert: `max(websocket_kafka_lag_offset_age_seconds) > N` (e.g., 120s).
   - Optionally set lag to `-1` for “unknown” rather than `0` when stale.
3) **Conditional cleanup**:
   - Only remove “stale” partitions if lag is confirmed to be 0 **and** no Kafka infra errors recently, otherwise keep and alert.

### Acceptance criteria
- In a controlled test where the consumer stops advancing offsets while producers continue, lag/staleness metrics must remain non-zero and page.
- In a controlled rebalance, lag metrics for revoked partitions must not remain stuck high indefinitely.

---

## 2) Re-validated Open Issues (Still Not Fixed)

### 2.1 M-432: Exporter still ignores `KAFKA_AUTO_OFFSET_RESET`
`crates/dashflow-prometheus-exporter/src/main.rs` still hardcodes:
`auto.offset.reset=earliest`.

This is a real production footgun: on a new group id with retained topics, exporter can do huge backfills unintentionally.

**Fix:** add `KAFKA_AUTO_OFFSET_RESET` support with validation (`earliest|latest`), mirroring websocket-server.

### 2.2 M-433: CLI env var wiring + defaults still inconsistent
Only `replay` uses clap `env="KAFKA_BROKERS"`/`env="KAFKA_TOPIC"`. Other Kafka commands default to:
- brokers: `localhost:9092`
- topic: `dashstream`

**Impact:** “no data” confusion, inconsistent with stack default `dashstream-quality`.

**Fix:** wire env vars consistently across all Kafka CLI commands and standardize the default topic.

### 2.3 CLI tail still commits offsets by default (debug tooling mutates state)
`dashflow tail` uses:
- fixed `group.id = dashflow-cli-tail`
- `enable.auto.commit = true`

**Impact:** debug sessions can interfere with each other and mutate committed offsets in ways operators don’t expect.

**Fix:** default to `enable.auto.commit=false` + unique group id, add explicit `--commit`/`--group-id`.

### 2.4 M-434: Infra errors are measured but not paged on
`websocket_infrastructure_errors_total` exists and is graphed, but there is no alert on its rate.

**Fix:** add `KafkaInfraErrorsHigh` alert and keep the alert rules synced via `./scripts/check_dashstream_alert_rules_sync.sh --promtool`.

### 2.5 M-413 (still partial): “secure Kafka works everywhere” is not end-to-end
rdkafka surfaces are mostly covered via `KafkaSecurityConfig::create_client_config()`, but:
- `dashflow-streaming` producer path has no `ProducerConfig::from_env()`
- rskafka consumer path has no `ConsumerConfig::from_env()`
- `create_client_config()` does not enforce `validate()` at call sites

**Fix:** add `from_env()` helpers for producer+rskafka consumer and add a “checked config builder” that calls validate.

---

## 3) Updated Worker Priority List (Post-#1509)

### P0
1) Fix stale cleanup masking real lag (Section 1).

### P1
2) Exporter: honor `KAFKA_AUTO_OFFSET_RESET` (M-432).
3) CLI: wire `KAFKA_BROKERS`/`KAFKA_TOPIC` env vars consistently + standardize defaults (M-433).
4) CLI: make `tail` non-mutating by default (no auto commit; unique group id).
5) Finish M-413 end-to-end: `ProducerConfig::from_env()` + rskafka `ConsumerConfig::from_env()` + enforce validation.

### P2
6) Add infra error alert (M-434).
7) Wire Kafka security env vars into Helm/K8s via values+Secrets (M-435).
8) Clean up stale env var names in docs + CLI scripts (M-436).

---

## 4) Verification Commands (Worker Must Run)

```bash
./scripts/check_dashstream_alert_rules_sync.sh
./scripts/check_dashstream_alert_rules_sync.sh --promtool

CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-observability --bin websocket_server --features websocket-server
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-prometheus-exporter
CARGO_TARGET_DIR=target_check_kafka_audit cargo check -p dashflow-cli
```
