# v64 Skeptical Audit: optimize/optimizers/better_together.rs

**Date:** 2025-12-25
**Worker:** #1729
**File:** `crates/dashflow/src/optimize/optimizers/better_together.rs`
**Lines:** 1018
**Status:** COMPLETE - DESIGN LIMITATION DOCUMENTED

## Overview

BetterTogether is a meta-optimizer that composes multiple optimization strategies using three composition modes:
1. **Sequential:** Execute optimizers one after another (A → B → C)
2. **Parallel:** Run all independently, pick best by final score
3. **Ensemble:** Run all independently, combine results via weighted averaging

## Architecture

- Defines `NodeOptimizer<S>` trait for composable optimizers
- `CompositionStrategy` enum for the three modes
- `PipelineStage` stores intermediate results
- Generic over `GraphState` trait

## Findings

### P3 Issues (Design Limitation - Documented)

| ID | Category | Description | Location |
|----|----------|-------------|----------|
| M-895 | Design | "Parallel" strategy is sequential execution with shared mutable node state | `better_together.rs:403-407,424-426` |

### P4 Issues (Minor)

| ID | Category | Description | Location |
|----|----------|-------------|----------|
| M-896 | Semantics | Ensemble result's `initial_score` comes from best optimizer, not actual initial | `better_together.rs:571-572` |
| M-897 | Semantics | Ensemble `converged` requires ALL optimizers to converge | `better_together.rs:579` |

### Analysis Details

**M-895: "Parallel" strategy isn't actually parallel**

The code has explicit documentation about this limitation (lines 403-407):
```rust
// Since we can't clone trait objects easily, we'll use a different approach:
// Run all optimizers sequentially but track their results, then pick the best.
// This is "parallel" in the logical sense (independent optimizations), not concurrent.
```

However, there's a correctness concern: each optimizer mutates the node in place (lines 424-426), so:
- Optimizer 1 runs on original node, mutates it
- Optimizer 2 runs on already-mutated node (not original!)
- Comparing final scores is comparing different starting points

This is a design limitation, not a bug, because:
1. It's documented in the code
2. Trait objects can't be easily cloned
3. The alternative would require serialization/deserialization

**M-896: Ensemble initial_score mismatch**
- Line 571-572: `initial_score: best_result.initial_score`
- This takes initial_score from the best-performing optimizer
- But the ensemble represents a weighted combination, so the "true" initial is ambiguous
- Minor semantic inconsistency

**M-897: Ensemble converged logic**
- Line 579: `converged: self.pipeline_stages.iter().all(|s| s.result.converged)`
- If ANY optimizer fails to converge, ensemble reports not converged
- Could reasonably use `any()` or best optimizer's status instead
- Design choice, not a bug

## Positive Observations

1. **Clean trait design:** `NodeOptimizer<S>` is well-designed for composition
2. **Good telemetry:** Uses iteration and completion metrics
3. **Proper error handling:** Validates empty optimizer list, wraps stage errors
4. **Comprehensive tests:** ~42% coverage with tests for all three strategies
5. **Clear documentation:** Module docs explain each strategy with examples

## Conclusion

No P0/P1/P2 issues found. One P3 design limitation (documented) where "Parallel" strategy shares mutable state across optimizers. The P4 issues are minor semantic inconsistencies in ensemble result reporting. BetterTogether is well-designed for its purpose with explicit documentation of limitations.
