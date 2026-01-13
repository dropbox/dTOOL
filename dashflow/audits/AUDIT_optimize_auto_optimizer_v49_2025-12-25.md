# v49 Skeptical Audit - optimize/auto_optimizer.rs

**Date:** 2025-12-25
**Worker:** #1724
**File:** `crates/dashflow/src/optimize/auto_optimizer.rs`
**Lines:** 1224

## Overview

AutoOptimizer - automatic selection of best optimization algorithm based on data and constraints.

## File Structure

| Lines | Description |
|-------|-------------|
| 1-92 | Module header, documentation, imports, helper functions |
| 73-92 | `sanitize_for_filename_component()` - **NEW** (M-848 fix) |
| 94-148 | `warn_on_unknown_excluded_optimizers()` - **NEW** (M-851 fix) |
| 149-184 | `TaskType` enum (8 variants) with Display impl |
| 186-213 | `ComputeBudget` enum with `max_iterations()` |
| 215-336 | `OptimizationContext` + builder pattern |
| 337-438 | `SelectionResult`, `AlternativeOptimizer`, `OptimizationOutcome`, `OptimizerStats` |
| 440-673 | `AutoOptimizer` struct with async methods |
| 675-825 | `select_optimizer_impl()` - core decision tree |
| 896-945 | `infer_task_type()` - heuristic task detection |
| 960-1004 | Public API functions |
| 1006-1224 | Tests (~218 lines) |

## Key Components

### Decision Tree (lines 675-825)

Research-backed selection logic:
1. Can finetune? → GRPO (RL weight updates)
2. Agent task + 20+ examples? → SIMBA (self-reflective)
3. 50+ examples? → MIPROv2 (best benchmarked)
4. 10-50 examples? → BootstrapFewShot (reliable baseline)
5. 2-10 examples? → BootstrapFewShot (limited data)
6. <2 examples? → "None" (cannot optimize)

### Task Type Inference (lines 896-945)

Heuristic analysis of example patterns:
- Detects code (def, fn, ```)
- Detects math (calculate, solve, math)
- Detects tools/agent
- Detects classification (short output)
- Detects summarization (long output)

### Async Methods

- `record_outcome()`: Persists optimization results to JSON
- `load_outcomes()`: Reads historical outcomes
- `stats_for_optimizer()`: Per-optimizer statistics
- `historical_stats()`: Aggregate statistics

## Issues Found

### P0/P1/P2/P3
None.

### P4 (Minor/Defensive)

**M-848: ~~Filename injection risk in `record_outcome()`~~ ✅ FIXED**
- ~~Location: auto_optimizer.rs:439-442~~
- **Fix:** `sanitize_for_filename_component()` added at lines 73-92
- Sanitization used at line 525 before creating filename
- Strips "..", "/", "\\", and handles empty strings

**M-849: `infer_task_type()` heuristic edge cases**
- Location: auto_optimizer.rs:896-945
- String matching like `contains("def ")` can misclassify
- Example: "How do you define recursion?" would match code pattern
- Expected behavior for heuristic inference
- Recommendation: Document limitations in docstring

**M-850: `historical_stats()` omits `best_task_types`**
- Location: auto_optimizer.rs:619
- Returns empty vec while `stats_for_optimizer()` computes it
- Minor API inconsistency
- Comment says "Simplified" acknowledging this

**M-851: ~~No validation of `excluded_optimizers` names~~ ✅ FIXED**
- ~~Location: auto_optimizer.rs:609-614~~
- **Fix:** `warn_on_unknown_excluded_optimizers()` added at lines 94-148
- Warns on empty names, unknown optimizers, typos, and suggests similar names
- Called from `select_optimizer_impl()` at line 471

## Code Quality

**Positive:**
- No `unsafe` blocks
- No `.unwrap()` or `.expect()` in production paths
- All f64 divisions are guarded (`if filtered.is_empty()` returns early)
- Proper error handling in async I/O
- Good use of `tracing` for observability
- Comprehensive test coverage (14 tests)

**Patterns:**
- Builder pattern for `OptimizationContext`
- Decision tree with confidence scores
- Alternative suggestions for each selection
- Academic citations linked to optimizers

## Test Coverage

14 tests covering:
- Selection with finetuning → GRPO
- Large dataset → MIPROv2
- Medium dataset → BootstrapFewShot
- Small dataset → BootstrapFewShot (lower confidence)
- Agent task → SIMBA
- Insufficient data → "None"
- Exclusion handling
- Task type inference (code, classification)
- Alternatives provided
- Outcome improvement calculation
- Compute budget iterations

## Conclusion

Clean, well-designed module implementing research-backed optimizer selection.
Good documentation with academic citations. No significant issues.

**Summary:**
- P0: 0
- P1: 0
- P2: 0
- P3: 0
- P4: 2 (M-849, M-850)
- FIXED: 2 (M-848, M-851)
