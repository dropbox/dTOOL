# v46 Skeptical Audit - optimize/telemetry.rs

**Date:** 2025-12-25
**Auditor:** Worker #1722
**File:** `crates/dashflow/src/optimize/telemetry.rs` (526 lines)
**Test Coverage:** ~30% (6 tests for ~160 lines of test code)

## Module Overview

The optimize/telemetry module provides Prometheus metrics for the DashOptimize system:

1. **OptimizerMetrics struct** (lines 45-77)
   - Execution metrics: runs_total, duration_seconds, iterations_total, candidates_total
   - Score metrics: initial_score, final_score, improvement
   - Error metrics: errors_total
   - Active tracking: active_optimizations
   - Demo/rule metrics: demos_added_total, rules_generated_total

2. **Initialization** (lines 79-226)
   - `new()` - creates metrics and registers with provided registry
   - `initialize()` - global singleton initialization via OnceLock
   - `global()` - get global instance, initializing if necessary

3. **Convenience Functions** (lines 228-357)
   - `record_optimization_start()` - increment run counter and active gauge
   - `record_optimization_complete()` - record all completion metrics
   - `record_error()` - record error and decrement active count
   - `record_demos_added()`, `record_rules_generated()` - demo/rule counts
   - `record_iteration()`, `record_candidate_evaluated()` - progress tracking

## Safety Analysis

**Global singleton pattern (lines 211-218):**
- Uses OnceLock for thread-safe single initialization
- Graceful fallback: if registration fails, creates dummy registry
- Fallback logged at WARN level
- `global()` calls `initialize()` before `get()` - cannot panic

**Line 224 `expect()` usage:**
- Safe: `initialize()` is called immediately before
- OnceLock guarantees METRICS is set after `get_or_init()`
- Pattern: `initialize(); METRICS.get().expect(...)` is correct

## Issues Found

### No P0/P1/P2/P3 Issues

The module is well-designed with:
- Clean API for recording metrics
- Proper singleton pattern with OnceLock
- Graceful fallback on registration failure

### P4 Issues (Minor Design)

| ID | Priority | Category | Description | File(s) |
|----|----------|----------|-------------|---------|
| M-839 | P4 | Validation | `improvement` gauge accepts any f64; caller could pass NaN/inf scores | `telemetry.rs:284` |
| M-840 | P4 | Precision | u64 to f64 cast could lose precision for values > 2^53 | `telemetry.rs:268,272,329,338` |
| M-841 | P4 | Race | `record_error` check-then-act on active count could miscount in concurrent errors | `telemetry.rs:311-320` |

### Issue Details

**M-839: Score validation**
- Impact: If caller passes NaN scores, improvement gauge becomes NaN
- Likelihood: Low - callers typically pass valid 0.0-1.0 scores
- Fix: Could clamp scores to 0.0-1.0 or check for is_finite()

**M-840: u64 to f64 precision loss**
- Impact: Iteration/candidate counts > 2^53 (~9 quadrillion) would lose precision
- Likelihood: Extremely low - unrealistic optimization run sizes
- Fix: None needed - design tradeoff for Prometheus Counter compatibility

**M-841: Record error race**
- Impact: If two concurrent errors race, active count might be wrong
- Likelihood: Low - concurrent errors on same optimizer are rare
- Impact: Metrics only, not correctness-critical
- Fix: Could use atomic compare-exchange, but not worth complexity

## Positive Observations

1. **Clean API**: Convenience functions make metric recording simple
2. **Proper singleton**: OnceLock is the correct pattern for global metrics
3. **Graceful fallback**: Failed registration doesn't panic, just warns
4. **Good documentation**: Module docs explain all metrics
5. **Reasonable test coverage**: Tests cover all metric types

## Test Summary

```
test result: ok. 6 passed; 0 failed; 0 ignored
```

Tests cover:
- Metric initialization and registration
- Duration histogram observations
- Score gauges (initial, final, improvement)
- Error tracking with labels
- Active optimization tracking
- Demo and rule counters

## Conclusion

**v46 Audit Status: COMPLETE - NO SIGNIFICANT ISSUES**

The optimize/telemetry module is well-designed with:
- Clean Prometheus metrics API
- Thread-safe global singleton pattern
- Graceful error handling
- Good test coverage for a metrics module

The P4 items are minor design considerations that don't affect correctness.
