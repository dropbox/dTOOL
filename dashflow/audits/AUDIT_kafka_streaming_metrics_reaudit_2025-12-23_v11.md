# Re-Audit v11: Kafka + Streaming Metrics (DashStream) — 2025-12-23

This extends:
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v10.md`

Re-checked against `main` HEAD (worker commit `#1546`) and then re-checked again after applying the fixes in this audit (see “Fixes Applied in This Iteration”).

Intent: skeptical, correctness-first. Confirm what prior audit findings are *actually* resolved, identify cases where an item was marked “fixed” but reality drifted (code/config/docs), and look for adjacent Kafka/metrics correctness and config gaps.

---

## 0) Executive Summary (What’s True Right Now)

### The good
- The WebSocket lag monitor no longer blocks tokio threads, and the “stale partition cleanup masks lag” hazard from v10 is removed.
- `websocket_kafka_lag_offset_age_seconds` now has real alert coverage in the canonical alert rules.
- The Prometheus exporter now honors `KAFKA_AUTO_OFFSET_RESET`.
- Kafka security env vars (`KAFKA_SECURITY_PROTOCOL`, `KAFKA_SASL_*`, `KAFKA_SSL_*`) are broadly unified via `KafkaSecurityConfig`.

### The bad (real issues found during this re-audit)
- **Deployment drift reintroduced a P0 Kafka correctness bug:** `deploy/kubernetes/overlays/*` scaled the WebSocket server above 1 replica despite M-415’s warning comment, which would cause *partial streams* per client (Kafka partitions spread across pods).
- **“IPv6 support” was only half-done:** a helper existed (`get_broker_address_family()`), but multiple binaries still hard-forced `broker.address.family=v4`, meaning IPv6-only environments would still fail.
- **Alert rules drift existed:** Kubernetes alert rules were missing the “partition stale” alerts present in `monitoring/alert_rules.yml`.
- **Infra errors were measured but not alerted:** `websocket_infrastructure_errors_total` had no alert (M-434 still open).
- **Docs drifted:** metrics docs claimed the offset-age metric triggered automatic cleanup (no longer true) and implied `KAFKA_LAG_STALE_PARTITION_SECS` affects alert thresholds (it doesn’t).

---

## 1) Rigorous Check: Were v10 Issues Actually Resolved?

### ✅ v10 Section 1 (P0): “stale partition cleanup can mask real lag”
**Status now:** fixed in code. The WebSocket lag monitor logs staleness but continues to fetch watermarks and compute lag; it does not delete partitions or zero gauges.

### ✅ v10 Section 2.1: Exporter ignored `KAFKA_AUTO_OFFSET_RESET` (M-432)
**Status now:** fixed in code. Exporter uses `get_auto_offset_reset()` (validated earliest/latest).

### ✅ v10 Section 2.5: Kafka security config unification (M-413/M-474)
**Status now:** materially improved. `KafkaSecurityConfig::create_client_config_checked()` exists, and producer/consumer `from_env()` helpers exist in `dashflow-streaming`.

### ⚠️ v10 Section 2.4: Infra errors measured but not paged (M-434)
**Status at start of this audit:** not fixed.
**Fix applied in this iteration:** added a new alert (see Section 2.2).

### ⚠️ v10 Section 2.2/2.3: CLI env var wiring + tail committing
**Status now:** tail is no longer dangerous-by-default (unique group id + `--commit` flag); however, **only some Kafka CLI commands** use `env="KAFKA_BROKERS"`/`env="KAFKA_TOPIC"`. This remains a configuration footgun.

---

## 2) Fixes Applied in This Iteration (Code + Config + Docs)

### 2.1 M-415 regression: K8s overlays were scaling the WebSocket server (P0)
**Why it’s a real bug:** with a shared `KAFKA_GROUP_ID`, Kafka partitions are distributed across consumer group members; a client connected to one pod sees only the partitions consumed by that pod → partial stream.

**Fix applied:**
- Set WebSocket server replicas back to 1 in:
  - `deploy/kubernetes/overlays/staging/kustomization.yaml`
  - `deploy/kubernetes/overlays/production/kustomization.yaml`
- Removed WebSocket server HPA (kept Quality Monitor HPA):
  - `deploy/kubernetes/overlays/production/hpa.yaml`
- Fixed WebSocket server PDB for single replica:
  - `deploy/kubernetes/overlays/production/pdb.yaml`
- Updated docs to stop recommending scaling the WebSocket server:
  - `deploy/kubernetes/README.md`

**Acceptance criteria:**
- `kustomize build deploy/kubernetes/overlays/production` shows `dashflow-websocket-server` at `replicas: 1` and no HPA targeting it.

### 2.2 M-434: Add infra error alert for `websocket_infrastructure_errors_total`
**Fix applied:**
- Added `KafkaInfraErrorsHigh` alert to:
  - `monitoring/alert_rules.yml`
  - `deploy/kubernetes/base/configs/alert_rules.yml`

**Acceptance criteria:**
- `promtool check rules` (or existing repo drift checks) passes, and the alert appears in the deployed rule set.

### 2.3 M-481 drift: “partition stale” alerts were missing in K8s copy
**Fix applied:**
- Synced `KafkaPartitionStale` + `KafkaPartitionStaleCritical` into:
  - `deploy/kubernetes/base/configs/alert_rules.yml`
- Verified with:
  - `./scripts/check_dashstream_alert_rules_sync.sh`

### 2.4 M-478 completion: stop hard-forcing IPv4 in binaries
**Problem:** `get_broker_address_family()` existed, but:
- `crates/dashflow-observability/src/bin/websocket_server.rs` forced `broker.address.family=v4`
- `crates/dashflow-prometheus-exporter/src/main.rs` forced `broker.address.family=v4`

**Fix applied:**
- Use `KafkaSecurityConfig::create_client_config()` to ensure:
  - TLS/SASL config stays unified
  - `broker.address.family` follows the M-478 auto-detect + `KAFKA_BROKER_ADDRESS_FAMILY` override

### 2.5 Config correctness: `KAFKA_BROKERS` vs `KAFKA_BOOTSTRAP_SERVERS`
**Problem:** some components used `KAFKA_BROKERS`, but `dashflow-streaming` `from_env()` used only `KAFKA_BOOTSTRAP_SERVERS`.

**Fix applied:**
- `ProducerConfig::from_env()` + `ConsumerConfig::from_env()` now accept `KAFKA_BROKERS` first, falling back to `KAFKA_BOOTSTRAP_SERVERS`.

### 2.6 Docs drift fixes
- `monitoring/PROMETHEUS_METRICS.md` no longer claims “automatic stale cleanup”, and clarifies that `KAFKA_LAG_STALE_PARTITION_SECS` affects logging, not alert thresholds.
- `docs/CONFIGURATION.md` now documents `KAFKA_BROKER_ADDRESS_FAMILY` and reflects the current `dashflow-streaming` `from_env()` state.
- `crates/dashflow-prometheus-exporter/README.md` now documents `KAFKA_BROKER_ADDRESS_FAMILY` and `KAFKA_AUTO_OFFSET_RESET`.

---

## 3) Remaining Issues / Next Worker Priority

### P1: CLI Kafka env wiring still inconsistent (operational footgun)
Only `dashflow replay` uses `env="KAFKA_BROKERS"`/`env="KAFKA_TOPIC"`. Other Kafka commands default to `localhost:9092` and require flags, which is easy to forget and leads to “no data” confusion.

**Fix direction:**
- Add clap `env` wiring for `bootstrap_servers` + `topic` across all Kafka CLI commands.
- Keep existing defaults, but make env vars override consistently.

**Acceptance criteria:**
- Running `KAFKA_BROKERS=... KAFKA_TOPIC=... dashflow tail` (and other Kafka commands) uses those values without flags.

### P2: If multi-replica WebSocket server ever becomes a goal, Kafka consumption model must change
Scaling beyond 1 replica requires one of:
- shared backplane that republishes all events to all pods (so any client sees full stream), or
- per-pod unique `KAFKA_GROUP_ID` so each pod consumes the full topic (expensive), plus careful dedupe/metrics semantics.

**Fix direction:** treat this as an explicit architecture project; do not “accidentally” reintroduce scaling via HPA/replica patches.

---

## 4) Skeptical Re-Audit Checklist (for future audits)

When something is marked “fixed”, re-check these *four surfaces*:
1) **code**: runtime behavior (esp. lag, commits, error accounting)
2) **alerts**: canonical vs deployed drift (`check_dashstream_alert_rules_sync.sh`)
3) **k8s overlays**: staging/production patches can silently undo base safety constraints
4) **docs**: metrics + env var docs tend to drift after “fixes”
