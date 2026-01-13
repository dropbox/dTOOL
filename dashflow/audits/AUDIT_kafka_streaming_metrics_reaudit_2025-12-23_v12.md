# Re-Audit v12: Kafka + Streaming Metrics (DashStream) — 2025-12-23

This extends:
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v11.md`

Re-checked against `main` HEAD through worker commit `#1560`.

Intent: skeptical, correctness-first. Go beyond “the fix exists” and check whether Kafka behavior + metrics/alerts/docs/config remain consistent across:
- runtime behavior (code)
- alert rules (canonical + deployed copies)
- deployment manifests (k8s base + overlays + Helm)
- docs/runbooks/examples

---

## 0) What Changed Since v11 (Verified)

### Confirmed still-fixed (from v11 / #1548)
- **M-415**: WebSocket server stays single-replica in staging/production overlays; websocket HPA removed; websocket PDB fixed. (If this ever regresses again, it’s a P0 data correctness failure.)
- **M-434**: `KafkaInfraErrorsHigh` alert now exists (canonical + K8s copy).
- **M-478**: WebSocket server + Prometheus exporter no longer hard-force `broker.address.family=v4`; they use `KafkaSecurityConfig::create_client_config()` and honor `KAFKA_BROKER_ADDRESS_FAMILY`.
- **M-481**: `KafkaPartitionStale*` alerts are present in the deployed K8s rule copy; drift check exists.

### Newly confirmed since v11
- **M-631**: CLI Kafka env wiring is now present (`KAFKA_BROKERS`, `KAFKA_TOPIC`) in `tail`, `inspect`, `watch`, `export` (worker commit `#1552`).
- **M-618**: Kafka session timeout constants were centralized and reused by the exporter (worker commit `#1559`).

---

## 1) New/Deeper High-Risk Findings (Not Fully Solved Yet)

These are correctness/operability issues that can silently break streaming observability or page operators incorrectly.

### 1.1 Lag monitor is still *assignment-unaware* (false positives during rebalance / partition revocation)

Current behavior in `crates/dashflow-observability/src/bin/websocket_server.rs`:
- The lag monitor tracks partitions seen historically (via “offset updates map”).
- M-481 removed stale “cleanup” (good: avoids masking real lag).
- But there is still **no proof that a “stale” partition is currently assigned** to this consumer.

Failure modes:
- After a rebalance/revocation, a partition may stop seeing offset updates because it’s no longer assigned.
- The lag monitor will continue to:
  - compute `offset_age` for that partition (growing forever)
  - fetch watermarks for that partition
  - potentially alarm via `KafkaPartitionStale*` even though the consumer is healthy and just no longer assigned.

This is now the mirror-image of the old bug: previously we could mask real lag (false negatives); now we can create noisy pages (false positives).

**Fix direction (preferred): assignment-aware lag tracking**
1) Add a rebalance callback (rdkafka consumer context) or poll `consumer.assignment()` periodically.
2) Maintain a shared `assigned_partitions` set (Arc/RwLock).
3) Lag monitor should only:
   - compute lag/age for partitions that are currently assigned
   - garbage-collect partitions that are no longer assigned (this is safe and not “masking lag”).

**Acceptance criteria**
- In a forced rebalance where a partition is revoked, stale alerts should NOT fire due to revoked partitions.
- In a forced stall where a partition remains assigned but offsets stop advancing, stale alerts MUST fire.

> Suggested new issue: **M-642 (P1)** “Lag monitor lacks assignment awareness; stale alerts can fire for revoked partitions.”

---

### 1.2 `KafkaInfraErrorsHigh` alert is probably too sensitive (noise risk)

Current rule (canonical + k8s copy):
- `sum(increase(websocket_infrastructure_errors_total[5m])) > 0` for 2m

Risk:
- If rdkafka reports occasional transient errors (rebalance, broker reconnect, DNS blip), this will page on a single error in 5 minutes.
- The websocket server already has `websocket_kafka_errors_by_type_total{error_type=...}`; we are not using it for targeted alerting.

**Fix direction**
- Split into tiers:
  - `KafkaInfraErrorsDetected` (severity=warning): `> 0` in 5m (for awareness)
  - `KafkaInfraErrorsHigh` (severity=high): `>= N` in 5m (e.g. 5)
  - `KafkaInfraErrorsCritical` (severity=critical): sustained rate or very high increase
- Or alert on specific error types (DNS/broker_down) using `websocket_kafka_errors_by_type_total`.

**Acceptance criteria**
- A single reconnect does not page critical.
- Sustained infra errors page within minutes.

> Suggested new issue: **M-643 (P1)** “Infra error alert is too sensitive; needs tiering or error-type targeting.”

---

### 1.3 E2E latency metric is mislabeled and can be corrupted by clock skew (potential negative observations)

Current implementation:
- `websocket_e2e_latency_ms` is computed as `Utc::now().timestamp_micros() - header.timestamp_us`
- It is recorded under label `stage="kafka_to_websocket"`

Problems:
- This measures **producer-clock → consumer-clock delta**, not Kafka-only latency and not end-to-end-to-browser latency.
- If producer and consumer clocks are skewed, `latency_us` can be negative or arbitrarily large.
  - Negative histogram observations are meaningless and may break assumptions/dashboards.
  - Huge values blow out `_sum` and quantiles.

**Fix direction**
- Rename the stage label and help string to match reality (e.g. `producer_to_consumer`).
- Guard the observation:
  - if `latency_us < 0`, increment a `websocket_clock_skew_events_total` counter and skip (or clamp to 0).
  - if `latency_ms` exceeds a sanity cap (e.g. 60s), treat as skew/outlier and do not observe into the histogram.
- Optionally compute from Kafka message timestamp (`msg.timestamp()`) instead of producer header timestamp to reduce clock issues (still imperfect).

**Acceptance criteria**
- Metric never records negative values.
- Obvious skew/outliers are visible via a separate counter and do not corrupt p99.

> Suggested new issue: **M-644 (P1)** “E2E latency metric can record negative/outlier values due to clock skew; label/description inaccurate.”

---

### 1.4 `websocket_kafka_messages_total` does not count “old data decode errors” (metric semantics mismatch)

Observed behavior:
- WebSocket server increments an internal “messages received” counter on every Kafka message (including ones that later become “old data decode errors”).
- Prometheus metric `websocket_kafka_messages_total{status="success"|"error"}` is exported from separate success/error atomics, and **does not include** `websocket_old_data_decode_errors_total`.

This creates two correctness problems:
1) The metric name/help implies “total Kafka messages processed”, but it’s actually “new-data decode outcomes only”.
2) Health endpoints, logs, and Prometheus can disagree about “how many Kafka messages we processed”.

**Fix direction**
Choose one explicit semantics and make it consistent:
- Option A (recommended): add `status="old_data_error"` to `websocket_kafka_messages_total` and update alerts to use denom `status=~"success|error"` only.
- Option B: keep current split but rename/clarify help strings and add a separate `websocket_kafka_messages_received_total`.

**Acceptance criteria**
- It is possible to reconcile “messages received” vs “messages successfully decoded” vs “old-data skipped” from Prometheus alone.

> Suggested new issue: **M-645 (P1)** “Kafka message totals exclude old-data decode errors; metrics/health disagree.”

---

## 2) Configuration + Docs Drift That Still Matter

### 2.1 M-435 still open: Helm/K8s do not expose Kafka security env vars
`KafkaSecurityConfig` supports `KAFKA_SECURITY_PROTOCOL`, `KAFKA_SASL_*`, `KAFKA_SSL_*`, but:
- Helm chart configmap/secret templates do not provide a way to set them via values.
- K8s base manifests only set `KAFKA_BROKERS` and `KAFKA_TOPIC` (security must be injected manually).

**Fix direction**
- Add optional values/secret entries for all `KafkaSecurityConfig` env vars in Helm and document them.
- Provide a safe pattern: TLS/SASL values sourced from a Secret, not ConfigMap.

### 2.2 M-436 still open: Production docs reference stale DASHSTREAM_* env vars and outdated code snippets
Example: `docs/PRODUCTION_DEPLOYMENT_GUIDE.md` uses `DASHSTREAM_KAFKA_BROKERS` / `DASHSTREAM_TOPIC_PREFIX`, which are not read by the current codepath (canonical is `KAFKA_BROKERS`, `KAFKA_TOPIC`, plus `KafkaSecurityConfig` env vars).

**Fix direction**
- Replace with the env vars actually used by `DashStreamCallback`, websocket-server, exporter, and CLI.
- If aliases exist (`DASHSTREAM_TOPIC`), document them as legacy and prefer `KAFKA_TOPIC`.

---

## 3) Worker Priority List (Concrete Fix Plan)

### P0 (do immediately if you see regression)
1) **Guardrail: WebSocket server must remain single replica** (M-415). Add a CI/static check if practical (grep overlays for replicas>1 or HPA target).

### P1 (correctness + paging quality)
1) **M-642**: Assignment-aware lag monitoring (rebalance callback + assigned partition set).
2) **M-643**: Infra error alert tiering / error-type targeting; avoid paging on a single transient error.
3) **M-644**: Fix E2E latency metric semantics + clock-skew hardening (no negative/outlier corruption).
4) **M-645**: Make Kafka message totals semantically consistent (old-data handling).

### P2 (configuration surface area / operator UX)
1) **M-435**: Expose Kafka TLS/SASL env vars in Helm/K8s templates (with Secret wiring).
2) **M-436**: Fix production docs to use canonical env vars and correct code snippets.
3) **Topic naming cleanup**: decide whether `dashstream-quality` is the unified stream topic; if so, rename/docs to avoid implying “quality-only”.

---

## 4) Suggested “Skeptical” Verification Commands (Worker Checklist)

These are cheap sanity checks that catch drift:
- `./scripts/check_dashstream_alert_rules_sync.sh`
- `rg -n \"replicas\\s*:\\s*[2-9]|minReplicas:\\s*[2-9]\" deploy/kubernetes/overlays -S` (ensure websocket-server is not scaled)
- `rg -n \"KafkaInfraErrorsHigh|KafkaPartitionStale\" monitoring/alert_rules.yml deploy/kubernetes/base/configs/alert_rules.yml`
- For the metric semantics issues: run websocket-server locally and compare:
  - `websocket_kafka_messages_total` vs `websocket_old_data_decode_errors_total` vs health JSON counts.
