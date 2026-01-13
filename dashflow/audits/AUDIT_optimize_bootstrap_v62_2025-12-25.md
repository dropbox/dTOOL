# v62 Skeptical Audit: optimize/optimizers/bootstrap.rs

**Date:** 2025-12-25
**Worker:** #1728
**File:** `crates/dashflow/src/optimize/optimizers/bootstrap.rs`
**Lines:** 1121
**Status:** COMPLETE

## Summary

BootstrapFewShot is an optimizer that generates few-shot examples by running the program on training data and collecting successful traces. Good ExecutionTrace integration for telemetry. Found four P4 issues related to hardcoded thresholds and unreliable final_score estimates.

## Issues Found

### P4 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-889 | ~~P4~~ FIXED | Configuration | ~~Hardcoded 0.5 success threshold not configurable~~ Now uses `self.config.success_threshold` | `bootstrap.rs:122,252` (comments reference fix) |
| M-890 | P4 | Accuracy | Estimated final_score uses hardcoded 0.15 improvement | `bootstrap.rs:432` |
| M-891 | P4 | Telemetry | tokens_used always 0 in execution traces | `bootstrap.rs:217,326` |
| M-892 | P4 | API | _valset unused in NodeOptimizer implementation | `bootstrap.rs:483` |

**M-889 Details:** ✅ **FIXED**
- Lines 122 and 252 now have comments: `// M-889: Use configurable success threshold instead of hardcoded 0.5`
- Code now uses `self.config.success_threshold` instead of hardcoded 0.5
- Users can configure what score counts as "successful" via OptimizerConfig

**M-890 Details:**
- Line 432:
  ```rust
  let estimated_improvement = 0.15;
  ```
- Comment acknowledges this is an estimate
- OptimizationResult.final_score is unreliable
- Caller should re-evaluate, but might trust this value

**M-891 Details:**
- `tokens_used: 0` hardcoded in NodeExecution construction
- Lines 217 and 326 both set tokens_used to 0 (with M-891 comments)
- Token usage never tracked even though field exists
- Telemetry for LLM token usage is always wrong

**M-892 Details:**
- NodeOptimizer trait requires `_valset: &[S]` parameter
- Line 483: parameter is unused (prefixed with `_`)
- BootstrapFewShot could use valset for evaluation but doesn't

## Code Quality

- **ExecutionTrace Integration:** Excellent - provides bootstrap_with_traces() and collect_traces_local()
- **Test Coverage:** ~55% by line (lines 503-1121), comprehensive tests
- **Error Handling:** Graceful handling of node execution errors
- **Documentation:** Good method docs explaining trace integration

## Verification

- Code compiles without warnings
- Tests pass
- No deprecated API without #[allow(deprecated)]

## Recommendations

1. **M-889 (P4):** Add `success_threshold: f64` to OptimizerConfig
2. **M-890 (P4):** Either measure real final_score or document estimate prominently
3. **M-891 (P4):** Pass token usage from LLM to traces (requires node interface change)
4. **M-892 (P4):** Use valset for final score evaluation instead of estimating

## Summary Statistics

| Priority | Count |
|----------|-------|
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |
| P4 | 3 |
| FIXED | 1 (M-889) |
| **Total Open** | **3** |
