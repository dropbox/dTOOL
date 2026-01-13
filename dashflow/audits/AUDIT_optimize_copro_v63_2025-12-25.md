# v63 Skeptical Audit: optimize/optimizers/copro.rs

**Date:** 2025-12-25
**Worker:** #1729
**File:** `crates/dashflow/src/optimize/optimizers/copro.rs`
**Lines:** 906
**Status:** COMPLETE - NO SIGNIFICANT ISSUES

## Overview

COPRO (Collaborative Prompt Optimizer) is the original LLM-based prompt optimizer that iteratively generates better instructions and output prefixes using coordinate descent.

## Architecture

1. **Initialization (Depth 0):**
   - Takes current instruction from signature
   - Uses LLM to generate BREADTH-1 instruction variations
   - Adds original instruction as candidate (total = BREADTH)
   - Evaluates all candidates on training set

2. **Iteration (Depth 1 to DEPTH):**
   - Sorts candidates by score (best to worst)
   - Creates "history" of attempts with scores
   - Uses LLM to generate BREADTH new candidates informed by history
   - Evaluates new candidates (deduplicates by instruction + prefix)

3. **Final Selection:**
   - Returns signature with best instruction and prefix

## Findings

### P4 Issues (Minor)

| ID | Category | Description | Location |
|----|----------|-------------|----------|
| M-893 | Dead Code | `track_stats` field is defined but never used | `copro.rs:109,196` |
| M-894 | Resilience | Single evaluation failure aborts all parallel evaluations via `try_join_all` | `copro.rs:295,372` |

### Analysis Details

**M-893: track_stats dead code**
- Line 109: `track_stats: bool` stored in builder
- Line 196: stored in COPRO struct
- Never read or used anywhere in compile() or other methods
- Same pattern as M-883 in copro_v2.rs (both inherit this from the original design)

**M-894: Parallel evaluation failure handling**
- Lines 295, 372: `try_join_all(eval_futures).await?`
- If any single example evaluation fails (LLM error, timeout), the entire optimization fails
- Could use `join_all` with per-example Result handling for resilience
- Design tradeoff: current behavior is fail-fast which may be intentional

## Positive Observations

1. **Good validation:** Builder validates breadth > 1 and requires metric
2. **Proper telemetry:** Uses `record_optimization_start/complete` for metrics
3. **Deduplication:** Avoids re-evaluating duplicate candidates via HashMap
4. **Parallel evaluation:** Candidates evaluated concurrently for performance
5. **Documentation:** Excellent module docs explaining algorithm and references
6. **Test coverage:** ~21% (195 lines of tests out of 906 total)

## Comparison with COPROv2

COPRO (this file) is the simpler original version. COPROv2 adds:
- Confidence-based scoring
- Adaptive temperature
- More sophisticated candidate selection

Both share the same dead `track_stats` pattern.

## Conclusion

No P0/P1/P2/P3 issues found. COPRO is a clean implementation of the prompt optimization algorithm with proper structure and telemetry. The P4 issues are minor (dead code and design tradeoff for error handling).
