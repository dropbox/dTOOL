# Audit Report: self_improvement/observability.rs (v79)

**Date:** 2025-12-25
**Auditor:** Worker #1757
**File:** `crates/dashflow/src/self_improvement/observability.rs`
**Lines:** ~2928 (was ~2273 at audit time; +655 lines from code additions)
**Commit:** #1757
**Line refs updated:** 2026-01-01 by Worker #2255

---

## Overview

The `observability.rs` module consolidates observability functionality for the self-improvement system:

1. **Alerts Module** (~500 lines)
   - `AlertSeverity` enum (Info, Warning, Error, Critical)
   - `Alert` struct with metadata, context, and builder pattern
   - `AlertHandler` trait with implementations:
     - `ConsoleAlertHandler` - prints to stdout with optional colors
     - `FileAlertHandler` - appends to file (JSON or text format)
     - `WebhookAlertHandler` - POSTs JSON to URL with auth support
   - `AlertDispatcher` - multi-handler dispatch with deduplication

2. **Events Module** (~750 lines)
   - `EventType` enum (23 event types covering plans, hypotheses, analysis, etc.)
   - `EventData` enum for structured event payloads
   - `Event` struct with typed constructors
   - `EventBus` with subscribe/publish and history
   - Global event bus singleton via `OnceLock`

3. **Logging Module** (~300 lines)
   - `Component` enum for log context (13 components)
   - Logging macros (`log_si_error!`, `log_si_warn!`, etc.)
   - Operation logging helpers
   - Debug mode support via `DASHFLOW_DEBUG` env var

---

## Issues Found and Fixed

### M-941 (P3): `WebhookAlertHandler::with_timeout()` has no effect

**Location:** `observability.rs:747-752` (was 716-719 at audit time)

**Problem:** The `with_timeout()` method updated the `timeout_secs` field but the HTTP client was already constructed in `new()`. The field was marked `#[allow(dead_code)]` but users calling `with_timeout()` would expect the timeout to change.

**Fix:** Rebuild the HTTP client in `with_timeout()`:
```rust
pub fn with_timeout(mut self, seconds: u64) -> Self {
    self.timeout_secs = seconds;
    // M-941 fix: Rebuild the client to apply the new timeout
    self.client = create_webhook_client(seconds);
    self
}
```

---

### M-942 (P4): Division by zero risk in `Alert::from_trigger()`

**Location:** `observability.rs:171-183` (was 189 at audit time; now from_trigger starts at 171, division at 179-180)

**Problem:** When creating an alert from a `SlowNode` trigger, the slowdown ratio was calculated as:
```rust
*duration_ms as f64 / *threshold_ms as f64
```
If `threshold_ms` was 0, this would produce `f64::NAN` or `f64::INFINITY` depending on `duration_ms`.

**Fix:** Guard against zero threshold and use saturating multiplication:
```rust
let slowdown_ratio = if *threshold_ms > 0 {
    *duration_ms as f64 / *threshold_ms as f64
} else {
    f64::INFINITY // Threshold of 0 means any duration is infinitely slow
};
// Also use saturating_mul for severity check
if *duration_ms > threshold_ms.saturating_mul(3) { ... }
```

Added test `test_alert_from_slow_node_zero_threshold` to verify no panic.

---

### M-943 (P4): EventBus history uses inefficient O(n) eviction

**Location:** `observability.rs:1845-1951` (was 1575-1689 at audit time; struct at 1845, impl at 1873)

**Problem:** The `EventBus` stored history in a `Vec<Event>` and used `history.remove(0)` for eviction when exceeding `max_history`. `Vec::remove(0)` is O(n) because it shifts all elements.

**Fix:** Changed to `VecDeque<Event>`:
- Import: `use std::collections::{HashMap, VecDeque};`
- Field: `history: RwLock<VecDeque<Event>>`
- Construction: `RwLock::new(VecDeque::new())`
- Operations: `push_back()` and `pop_front()` (both O(1))

---

### M-944 (P4): Potential u64→i64 overflow in dedup window calculation

**Location:** `observability.rs:897-929` (was 870, 885 at audit time; is_duplicate at 897-913, mark_dispatched at 914-929)

**Problem:** The dedup window code cast `dedup_window_secs: u64` to `i64` and multiplied by 2:
```rust
age.num_seconds() < self.dedup_window_secs as i64
let cutoff = Utc::now() - chrono::Duration::seconds(self.dedup_window_secs as i64 * 2);
```
For extreme values (unlikely but possible), this could overflow.

**Fix:** Use safe conversion with saturation:
```rust
// In is_duplicate:
let window_secs = i64::try_from(self.dedup_window_secs).unwrap_or(i64::MAX);

// In mark_dispatched:
let window_secs = i64::try_from(self.dedup_window_secs).unwrap_or(i64::MAX / 2);
let cutoff_secs = window_secs.saturating_mul(2);
```

---

## Code Quality Assessment

### Strengths

1. **Well-structured module organization** - Clear separation of alerts, events, and logging
2. **Comprehensive builder pattern** - All major types support fluent configuration
3. **Poison-safe RwLock handling** - Uses `unwrap_or_else(|e| e.into_inner())` consistently
4. **Good test coverage** - 45 tests covering handler implementations, event bus, and logging
5. **Proper async handling** - Alert handlers are async with `async_trait`

### Patterns Used

- Builder pattern for `Alert`, `ConsoleAlertHandler`, `WebhookAlertHandler`, etc.
- Trait objects for `AlertHandler`
- Singleton pattern via `OnceLock` for global event bus
- Observer pattern for event pub/sub

### No Significant Issues Found

The following were reviewed and found acceptable:
- File I/O in `FileAlertHandler` blocks async (acceptable for low-frequency alerts)
- No graceful shutdown in `run_alerts_daemon_cli` (acceptable for CLI tool)
- Global event bus state (acceptable for singleton observability)

---

## Test Results

```
running 45 tests
test self_improvement::observability::alerts::tests::test_alert_creation ... ok
test self_improvement::observability::alerts::tests::test_alert_from_slow_node_trigger ... ok
test self_improvement::observability::alerts::tests::test_alert_from_slow_node_zero_threshold ... ok
test self_improvement::observability::alerts::tests::test_alert_severity_ordering ... ok
test self_improvement::observability::alerts::tests::test_console_handler ... ok
test self_improvement::observability::alerts::tests::test_file_handler ... ok
test self_improvement::observability::alerts::tests::test_dispatcher ... ok
test self_improvement::observability::events::tests::test_event_creation ... ok
test self_improvement::observability::events::tests::test_event_bus_subscribe_publish ... ok
test self_improvement::observability::events::tests::test_event_type_display ... ok
test self_improvement::observability::logging::tests::test_component_as_str ... ok
test self_improvement::observability::logging::tests::test_component_display ... ok
test self_improvement::observability::logging::tests::test_create_span ... ok
test self_improvement::observability::logging::tests::test_debug_env_var_name ... ok
[... all 45 pass]
```

---

## Summary

| Priority | Count | Status |
|----------|-------|--------|
| P3 | 1 | ✅ FIXED (M-941) |
| P4 | 3 | ✅ FIXED (M-942, M-943, M-944) |

**Verdict:** Module is well-implemented. Four minor robustness/performance issues fixed.

---

## Recommendations for Future Work

1. Consider adding a `ChannelAlertHandler` for async message queues
2. The event bus history could be persisted to disk for restart recovery
3. Consider adding rate limiting to `AlertDispatcher` (beyond deduplication)
