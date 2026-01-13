# v65 Skeptical Audit: optimize/optimizers/autoprompt.rs

**Date:** 2025-12-25
**Worker:** #1729
**File:** `crates/dashflow/src/optimize/optimizers/autoprompt.rs`
**Lines:** 856
**Status:** COMPLETE - NO SIGNIFICANT ISSUES

## Overview

AutoPrompt is a gradient-free prompt optimizer using coordinate descent to discover optimal trigger tokens to prepend to prompts. Based on Shin et al. (2020) but uses discrete search instead of gradients.

## Architecture

1. **Initialization:**
   - Start with K trigger positions (default: 5)
   - Each position initialized with random token from vocabulary
   - Default vocabulary: 45 common prompt engineering tokens

2. **Coordinate Descent:**
   - For each position (left to right):
     - Try each candidate token from vocabulary
     - Evaluate full prompt with substitution (parallel)
     - Keep token that maximizes metric score
   - Repeat for multiple iterations
   - Early stopping if no improvement in iteration

3. **Final Selection:**
   - Prepend optimized trigger tokens to signature instructions

## Findings

### P4 Issues (Minor)

| ID | Category | Description | Location |
|----|----------|-------------|----------|
| M-898 | Resilience | Single evaluation failure aborts all parallel evaluations | `autoprompt.rs:385` |
| M-899 | Style | `unwrap()` on vocabulary.choose() relies on prior validation | `autoprompt.rs:302` |
| M-900 | Scalability | No limit on vocabulary size × trainset size = potential for many LLM calls | `autoprompt.rs:362-383` |

### Analysis Details

**M-898: Parallel evaluation failure handling**
- Line 385: `try_join_all(eval_futures).await?`
- Same pattern as M-894 in copro.rs
- If any candidate evaluation fails, entire position optimization fails
- Design tradeoff: fail-fast vs resilience

**M-899: Vocabulary choice unwrap**
- Line 302: `.choose(&mut rng).unwrap().clone()`
- Safe because builder validates vocabulary is non-empty (lines 206-210)
- Could use `.expect("vocabulary validated non-empty")` for clarity
- No runtime risk

**M-900: LLM call scaling**
- Per iteration: `num_triggers * (vocabulary_size - 1) * trainset_size` LLM calls
- Default: 5 triggers * 44 vocab * N trainset = 220N calls per iteration
- With 3 iterations: 660N total calls
- Large vocabulary or trainset could be expensive
- Documented in algorithm, not a bug

## Positive Observations

1. **Good validation:** Builder validates num_triggers > 0, vocabulary non-empty, metric required
2. **Reproducibility:** Optional random_seed for deterministic optimization
3. **Early stopping:** Exits iteration loop if no improvement found
4. **Parallel evaluation:** All vocabulary candidates evaluated concurrently
5. **Comprehensive tests:** ~29% coverage (250 lines of tests)
6. **Clear vocabulary:** Default vocabulary covers task framing, quality indicators, role indicators, etc.

## Algorithm Correctness

The coordinate descent implementation is correct:
- Each position is optimized while holding others fixed
- Best token for each position is kept
- Iterations refine across all positions
- May converge to local optimum (expected behavior for coordinate descent)

## Conclusion

No P0/P1/P2/P3 issues found. AutoPrompt is a clean implementation of gradient-free prompt optimization with proper coordinate descent. The P4 issues are minor (common resilience pattern, safe unwrap, scalability consideration). Well-documented with paper references.
