# v58 Skeptical Audit: optimize/optimizers/simba.rs

**Date:** 2025-12-25
**Worker:** #1727
**File:** `crates/dashflow/src/optimize/optimizers/simba.rs`
**Lines:** 2660
**Status:** COMPLETE

## Summary

SIMBA (Stochastic Introspective Mini-Batch Ascent) optimizer using Monte Carlo search with LLM introspection. The implementation has clean architecture with proper telemetry integration, but the improvement strategies (AppendADemo, AppendARule) collect demos/rules without applying them to nodes - this is incomplete functionality.

## Issues Found

### P2 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-869 | P2 | Incomplete | AppendADemo/AppendARule strategies collect demos/rules but never apply them to nodes - optimization produces identical candidates | `simba.rs:1058,1223` |

**M-869 Details:**
- Strategies have `_node: &mut N` (unused parameter)
- AppendADemo adds demos to `self.demos` (line 1187)
- AppendARule adds rules to `self.rules` (line 1403)
- Neither modifies the node's prompts or examples
- Strategy objects created fresh each iteration (lines 474, 479) - accumulated demos lost
- Comment at line 449 confirms: "Requires: Access to node's OptimizationState for demo manipulation"
- Impact: All "improved" candidates are clones of source programs with no actual modifications

### P3 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-870 | P3 | Dead Code | `num_threads` field is set but never used | `simba.rs:106-110,197-198` |
| M-871 | P3 | Error UX | Softmax near-zero temperature produces confusing errors | `simba.rs:849` |

**M-870 Details:**
- `num_threads: Option<usize>` field exists
- `with_num_threads()` builder method sets it
- Algorithm runs sequentially with async/await, never uses thread count

**M-871 Details:**
- `(s / temperature).exp()` at line 849 overflows to infinity when temperature → 0
- Error caught at line 884 but message doesn't explain real cause
- User sees "Failed to create weighted distribution" not "temperature too low"

### P4 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-872 | P4 | Defensive | Direct indexing relies on non-empty bucket invariant | `simba.rs:884,1058,1350-1351` |
| M-873 | P4 | UX | Default seed=0 provides reproducibility but may surprise users expecting randomness | `simba.rs:276-280` |
| M-874 | P4 | API | Strategy `apply()` returns true but doesn't modify node - confusing semantics | `simba.rs:1112-1155,1345-1420` |

## Code Quality

- **Telemetry Integration:** Good use of ExecutionTrace and optimizer telemetry
- **Test Coverage:** ~45% by line (lines 1504-2660), comprehensive unit tests
- **Error Handling:** Proper Result returns, graceful handling of metric failures
- **Documentation:** Excellent module-level docs with algorithm explanation

## Verification

- Code compiles without warnings (checked via cargo check)
- Tests pass (checked via cargo test)
- No deprecated API without #[allow(deprecated)]

## Recommendations

1. **M-869 (P2):** The SIMBA improvement strategies are non-functional. Either:
   - Remove the demo/rule collection (misleading)
   - Implement actual node modification via OptimizationState
   - This is the highest priority fix

2. **M-870 (P3):** Remove `num_threads` field or implement parallelization

3. **M-871 (P3):** Add temperature validation in constructor or improve error message

## Summary Statistics

| Priority | Count |
|----------|-------|
| P0 | 0 |
| P1 | 0 |
| P2 | 1 |
| P3 | 2 |
| P4 | 3 |
| **Total** | **6** |
