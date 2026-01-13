# v60 Skeptical Audit: optimize/optimizers/copro_v2.rs

**Date:** 2025-12-25
**Worker:** #1728
**File:** `crates/dashflow/src/optimize/optimizers/copro_v2.rs`
**Lines:** 1262
**Status:** COMPLETE

## Summary

COPROv2 is a confidence-based collaborative prompt optimizer that extends COPRO with confidence scoring, filtering, and adaptive temperature. Clean implementation with proper telemetry integration. Found one P3 issue (empty trainset produces NaN) and two P4 issues (dead code, single failure aborts batch).

## Issues Found

### P3 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-882 | P3 | Correctness | Empty trainset produces NaN scores that corrupt optimization silently | `copro_v2.rs:619` |

**M-882 Details:**
- In `evaluate_candidate_with_confidence_static()`, if trainset is empty:
  - `predictions` is empty, `count = 0`
  - If `high_confidence_count == 0`, else branch executes
  - Line 619: `raw_score / count as f64` and `raw_confidence / count as f64`
  - Division by zero produces NaN (not panic in Rust for f64)
  - NaN propagates through optimization without error

### P4 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-883 | P4 | Dead Code | `track_stats` field is set but never used | `copro_v2.rs:245,162-163` |
| M-884 | P4 | Resilience | Single candidate failure aborts all parallel evaluations | `copro_v2.rs:380,473` |

**M-883 Details:**
- `track_stats: bool` field exists at line 245 (with doc comment at 241 noting it's unused)
- Builder method `track_stats()` sets it at lines 162-163
- Field is never read anywhere in the code

**M-884 Details:**
- `try_join_all(eval_futures)` at lines 380 and 473
- If any single LLM call fails, entire batch fails
- Could be more resilient with `join_all` + filtering errors

## Code Quality

- **Telemetry Integration:** Good use of record_candidate_evaluated, record_optimization_start, etc.
- **Test Coverage:** ~22% by line (lines 982-1262), comprehensive unit tests
- **Error Handling:** Proper Result returns for LLM errors
- **Documentation:** Good module-level docs with algorithm explanation
- **Confidence Logic:** Well-designed confidence-weighted scoring system

## Verification

- Code compiles without warnings
- Tests pass
- No deprecated API without #[allow(deprecated)]

## Recommendations

1. **M-882 (P3):** Add early return or validation for empty trainset
2. **M-883 (P4):** Remove `track_stats` field or implement statistics tracking
3. **M-884 (P4):** Consider using `join_all` with error filtering for resilience

## Summary Statistics

| Priority | Count |
|----------|-------|
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 1 |
| P4 | 2 |
| **Total** | **3** |
