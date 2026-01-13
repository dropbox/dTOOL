# v59 Skeptical Audit: optimize/optimizers/grpo.rs

**Date:** 2025-12-25
**Worker:** #1727
**File:** `crates/dashflow/src/optimize/optimizers/grpo.rs`
**Lines:** 1380
**Status:** COMPLETE

## Summary

GRPO (Group Relative Policy Optimization) is an RL-based optimizer that uses DashStream traces to compute rewards and train models via ChatModel::reinforce(). Clean implementation with proper config validation, but has a potential correctness issue in thread_id/example pairing and drops trailing examples from normalization.

## Issues Found

### P2 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-875 | P2 | Correctness | Thread ID / example pairing relies on index alignment - misalignment silently corrupts training data | `grpo.rs:435` |

**M-875 Details:**
- `let example = &examples[i % examples.len()]` assumes thread_ids and examples correspond by position
- If `thread_ids.len() != examples.len()`, modulo causes wrong pairings
- Caller must ensure correct alignment - fragile API contract
- Could silently produce training data with wrong example/trace pairs

### P3 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-876 | P3 | Correctness | Integer division drops trailing examples from normalization | `grpo.rs:638` |
| M-877 | P3 | Validation | No validation trainset.len() >= num_examples_per_step | `grpo.rs:582-584` |
| M-878 | P3 | Silent Failure | Empty step_data continues silently - masks collection failures | `grpo.rs:933-936` |

**M-876 Details:**
- `let num_groups = examples.len() / group_size`
- If 5 examples with group_size=4 → `num_groups=1`
- Loop `for group_idx in 0..1` only processes first 4 examples
- 5th example never normalized, keeps raw reward

**M-877 Details:**
- Line 582-584 uses modulo to select examples
- If trainset.len() < num_examples_per_step, examples duplicated
- Not necessarily wrong but may produce unexpected results

**M-878 Details:**
- When no traces collected for a step, logs warning and continues
- Could complete optimization with 0 or few training examples
- May mask Kafka/DashStream connectivity issues

### P4 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-879 | P4 | Reproducibility | HashMap iteration order non-deterministic in format_prompt_from_inputs | `grpo.rs:1007` |
| M-880 | P4 | Statistics | Population variance instead of sample variance (divide by N not N-1) | `grpo.rs:657-662` |
| M-881 | P4 | Performance | Unnecessary clone of all_training_data before reinforce | `grpo.rs:961` |

**M-879 Details:**
- `for (key, value) in inputs` - HashMap has no order guarantee
- Prompt format varies between runs
- May affect reproducibility if LLM sensitive to field order

**M-880 Details:**
- Uses N instead of N-1 for variance calculation
- With small groups (4 rollouts), underestimates variance
- Minor for RL contexts but technically incorrect for sample statistics

## Code Quality

- **Config Validation:** Comprehensive validate() method with suggestions
- **Error Types:** Custom GRPOError enum with clear categories
- **Telemetry:** Proper use of optimizer telemetry (record_iteration, etc.)
- **Test Coverage:** ~24% by line (lines 1046-1380)
- **Deprecated API:** Uses TraceCollector/TraceEntry, documented and #[allow(deprecated)]

## Verification

- Code compiles without warnings
- Tests pass (test_normalize_rewards_by_group, etc.)
- Config validation catches invalid configurations

## Recommendations

1. **M-875 (P2):** Add assertion or validation that thread_ids.len() == examples.len()

2. **M-876 (P3):** Fix normalization to handle partial groups:
   ```rust
   let num_groups = (examples.len() + group_size - 1) / group_size; // Round up
   ```

3. **M-878 (P3):** Consider failing or requiring minimum data per step

## Summary Statistics

| Priority | Count |
|----------|-------|
| P0 | 0 |
| P1 | 0 |
| P2 | 1 |
| P3 | 3 |
| P4 | 3 |
| **Total** | **7** |
