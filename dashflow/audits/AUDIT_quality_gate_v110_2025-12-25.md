# AUDIT: quality_gate.rs (v110)

**File:** `crates/dashflow-streaming/src/quality_gate.rs`
**Lines:** 495
**Date:** 2025-12-25
**Auditor:** Worker #1811

## Summary

| Priority | Count | Status |
|----------|-------|--------|
| P0 | 0 | - |
| P1 | 0 | - |
| P2 | 0 | - |
| P3 | 1 | FIXED |
| P4 | 4 | Noted |

## Issues

### M-1052 (P3): validate_quality() missing timeout — FIXED
**Category:** Liveness/Consistency

**Problem:** `validate_quality()` called `judge.judge_response()` without a timeout, while `execute_with_quality_guarantee()` used a 30s JUDGE_TIMEOUT. If the judge hangs, `validate_quality()` hangs indefinitely.

**Fix:**
1. Extracted `JUDGE_TIMEOUT` to module-level constant for reuse
2. Wrapped `validate_quality()` judge call in `tokio::time::timeout(JUDGE_TIMEOUT, ...)`
3. Returns `JudgeFailed` error on timeout (same pattern as main function)

**Files modified:**
- `crates/dashflow-streaming/src/quality_gate.rs` (lines 59-61, 303-314)

---

### P4 Issues (noted, not actionable)

1. **Quality threshold not validated** (line 116-119)
   - `quality_threshold(threshold: f32)` accepts any f32, including invalid values like -5.0 or 100.0
   - Impact: Low - caller's responsibility, causes confusing but not dangerous behavior

2. **Max retries not capped** (line 122-125)
   - `max_retries(retries: u32)` has no upper bound
   - Impact: Low - caller could set u32::MAX, leading to long waits

3. **Feedback query grows linearly** (line 263-277)
   - Each retry appends ~200 chars of feedback to the query
   - Could hit LLM context limits with high max_retries configs
   - Impact: Low - practical retry counts (3-5) are fine

4. **Verbose logging exposes query content** (line 185)
   - `query = %query` in debug log could expose PII/sensitive data
   - Impact: Low - requires explicit `verbose: true`, debug level only

## Positive Findings

1. **Hard timeouts for execution and judge** - 60s/30s respectively prevents hangs
2. **Epsilon tolerance for float comparison** (line 241) - Prevents 0.8999 vs 0.90 false negatives
3. **Clear error types with `#[non_exhaustive]`** - Forward-compatible API
4. **MockJudge in tests is appropriate** - Tests retry control flow, not LLM quality (per CLAUDE.md mock guidelines)
5. **Builder pattern** - Clean API design

## Verification

```bash
cargo check -p dashflow-streaming  # PASS
```
