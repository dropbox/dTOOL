# Audit: dashflow-observability

**Status:** NOT STARTED
**Files:** 8 src + 1 binary (directory) + 2 tests
**Priority:** P2 (Observability)

---

## File Checklist

### Source Files
- [ ] `src/lib.rs` - Module exports
- [ ] `src/config.rs` - Configuration
- [ ] `src/cost.rs` - Cost tracking
- [ ] `src/error.rs` - Error types
- [ ] `src/exporter.rs` - Metrics exporter
- [ ] `src/metrics.rs` - Metrics collection
- [ ] `src/metrics_server.rs` - Metrics server
- [ ] `src/tracer.rs` - Tracer implementation

### Test Files
- [ ] `tests/live_graph_feedback_loop.rs`
- [ ] `tests/otlp_export_m2004.rs`

### Binary Files
- [ ] `src/bin/websocket_server/` (directory - WebSocket streaming server)

---

## Known Issues Found

### Panic Patterns
- `src/cost.rs`: 32 .unwrap()
- `src/metrics.rs`: 16 .unwrap()
- `src/metrics_server.rs`: 9 .unwrap()

---

## Critical Checks

1. **Metrics are accurate** - Correct values collected
2. **Cost tracking complete** - All providers covered
3. **No data loss** - All traces captured
4. **Export works** - Prometheus/OTLP

---

## Test Coverage Gaps

- [ ] Cost calculation accuracy
- [ ] Metrics collection completeness
- [ ] Export format validation
