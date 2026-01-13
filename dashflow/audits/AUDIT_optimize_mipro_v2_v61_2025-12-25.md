# v61 Skeptical Audit: optimize/optimizers/mipro_v2.rs

**Date:** 2025-12-25
**Worker:** #1728
**File:** `crates/dashflow/src/optimize/optimizers/mipro_v2.rs`
**Lines:** 1240
**Status:** COMPLETE

## Summary

MIPROv2 is a multi-stage instruction, prompt, and demo optimizer that jointly optimizes instructions and few-shot demonstrations. Implementation uses random search instead of Optuna (baseline). Clean architecture with good config validation defined (but not called). Found four P4 issues.

## Issues Found

### P4 Issues

| ID | Priority | Category | Description | Location |
|----|----------|----------|-------------|----------|
| M-885 | P4 | Inconsistency | validate() method defined but never called; inconsistent with zeroshot support | `mipro_v2.rs:184-239` |
| M-886 | P4 | Misleading | Non-random valset sampling despite comment saying "sample" | `mipro_v2.rs:503-505` |
| M-887 | P4 | Incomplete | best_demos computed but discarded; optimized signature lacks demos | `mipro_v2.rs:711` |
| M-888 | P4 | Dead Code | _rng parameter unused in resolve_hyperparameters | `mipro_v2.rs:496` |

**M-885 Details:**
- `MIPROv2Config::validate()` at lines 184-239 has comprehensive validation
- Rejects `max_bootstrapped_demos == 0` and `max_labeled_demos == 0`
- But lines 511-512 explicitly support zeroshot mode (both == 0)
- validate() is never called in build() or compile()

**M-886 Details:**
- Line 504: Comment says "Sample without replacement (simplified - just take first val_size)"
- Code: `&valset[..val_size]` - takes first N elements, not random sample
- _rng parameter at line 496 was probably intended for this sampling

**M-887 Details:**
- optimize_prompt_parameters() finds best_demos at line 691
- Line 711: `let _ = best_demos; // Suppress unused warning`
- Optimized signature returned without the best few-shot examples
- Caller cannot use the demos that MIPROv2 discovered

**M-888 Details:**
- `_rng: &mut StdRng` at line 496 is never used
- Likely intended for random valset sampling (see M-886)

## Code Quality

- **Config Validation:** Comprehensive validate() method (but uncalled)
- **Test Coverage:** ~30% by line (lines 862-1239), good integration tests
- **Error Handling:** Proper error messages for config violations
- **Documentation:** Excellent module-level docs with references
- **Mock Testing:** Uses MockChatModel for LLM-free testing

## Verification

- Code compiles without warnings
- Tests pass
- No deprecated API without #[allow(deprecated)]

## Recommendations

1. **M-885 (P4):** Either call validate() in build() or align validation with zeroshot support
2. **M-886 (P4):** Implement actual random sampling using the _rng parameter
3. **M-887 (P4):** Return demos alongside optimized signature or store in signature metadata
4. **M-888 (P4):** Use _rng for random sampling or remove parameter

## Summary Statistics

| Priority | Count |
|----------|-------|
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |
| P4 | 4 |
| **Total** | **4** |
