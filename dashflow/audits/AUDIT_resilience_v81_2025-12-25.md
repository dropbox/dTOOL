# Audit Report: self_improvement/resilience.rs (v81)

**Date:** 2025-12-25
**Auditor:** Worker #1759
**File:** `crates/dashflow/src/self_improvement/resilience.rs`
**Lines:** ~1544 (was ~1422 at audit time; +122 lines)
**Commit:** (pending)
**Line refs updated:** 2026-01-01 by Worker #2255

---

## Overview

The `resilience.rs` module provides resilience patterns for the self-improvement system:

1. **Circuit Breaker** (`circuit_breaker` submodule)
   - `CircuitBreaker` - Three-state (Closed/Open/HalfOpen) pattern for protecting external service calls
   - `CircuitBreakerRegistry` - Manages multiple named circuit breakers
   - `CircuitBreakerConfig` - Configuration for thresholds, timeouts
   - Pre-defined configs: `API_CIRCUIT_CONFIG`, `PROMETHEUS_CIRCUIT_CONFIG`, `WEBHOOK_CIRCUIT_CONFIG`

2. **Rate Limiter** (`rate_limiter` submodule)
   - `RateLimiter` - Sliding window rate limiting with burst capacity
   - Exponential backoff on consecutive errors
   - Jitter factor to prevent thundering herd
   - Pre-defined configs: `strict()`, `permissive()`

---

## Issues Found and Fixed

### M-948 (P2): Integer overflow in `max_rate + burst` calculation

**Location:** `resilience.rs` (`try_acquire()` line 1118, `time_until_available()` line 1195) (was 1036, 1089 at audit time)

**Problem:** The `effective_limit = self.config.max_rate + self.config.burst` calculation could overflow if both values are near `u32::MAX`. This would cause incorrect rate limiting behavior.

**Fix:** Changed both occurrences to use `saturating_add`:
```rust
let effective_limit = self.config.max_rate.saturating_add(self.config.burst);
```

---

### M-949 (P4): Unregistered circuit breaker created silently on lock failure

**Location:** `resilience.rs` (`get_or_create()` line 664, `get_or_create_with_config()` line 690) (was 619, 648 at audit time)

**Problem:** When the `RwLock` write lock fails (poisoned), the code silently creates a new circuit breaker that is not added to the registry. This could lead to:
- Multiple independent breakers for the same service name
- Lost metrics and state tracking
- Unpredictable behavior

**Fix:** Added `tracing::warn!` to log when this fallback path is taken, making the issue visible for debugging.

---

### M-950 (P4): SystemTime failure returns 0 without logging

**Location:** `resilience.rs` (`current_time_millis()` line 762) (was 697 at audit time)

**Problem:** If `SystemTime::now()` returns a time before UNIX_EPOCH (e.g., on misconfigured systems), the function silently returns 0. Since `opened_at == 0` means "circuit is closed", this would make all circuits appear closed regardless of actual state.

**Fix:** Changed `unwrap_or(0)` to `unwrap_or_else` with a warning log:
```rust
.unwrap_or_else(|e| {
    tracing::warn!(
        error = %e,
        "SystemTime::now() before UNIX_EPOCH; circuit breaker time will be unreliable"
    );
    0
})
```

---

## Test Results

Ran:

`cargo test -p dashflow --lib resilience`

All 19 resilience tests passed:
- 7 circuit_breaker tests
- 12 rate_limiter tests

---

## Summary

| Priority | Count | Status |
|----------|-------|--------|
| P2 | 1 | FIXED (M-948) |
| P4 | 2 | FIXED (M-949, M-950) |
