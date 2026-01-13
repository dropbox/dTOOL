# Test Philosophy and Strategy

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Date:** 2025-11-04
**Status:** Active

---

## Executive Summary

This document establishes testing philosophy for the DashFlow conversion based on mutation testing findings. Key insight: **Different code types require different test strategies**. Chasing universal high mutation scores creates brittle tests and misallocates effort.

**Core principle:** Match test rigor to code criticality and type. Outcome tests for integration code, mechanism tests for algorithms, property tests for invariants.

---

## Test Type Taxonomy

### Type 1: Outcome Tests

**Purpose:** Validate that code produces correct results without constraining implementation.

**Characteristics:**
- Check final state is reasonable
- Allow implementation flexibility
- Use ranges rather than exact values
- Example: `assert!(chunks.len() > 0 && chunks.len() < 100)`

**Mutation Score:** 40-60%

**Catches:**
- Completely broken logic
- Major regressions
- Type errors

**Misses:**
- Subtle operator errors (>, >=)
- Boundary conditions
- Configuration bugs
- Arithmetic precision

**Appropriate for:**
- Integration code (parsers, splitters, formatters)
- UI/display logic
- Middleware/adapters
- Code that may need refactoring

**Example:**
```rust
#[test]
fn test_text_splitter_outcome() {
    let splitter = RecursiveCharacterTextSplitter::new(100, 20);
    let chunks = splitter.split_text("Hello\n\nWorld");

    // Outcome validation: chunks exist and are reasonable
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.len() <= 100));
    assert!(chunks.join("").contains("Hello"));
}
```

**Pros:**
- Tests remain valid through refactoring
- Low maintenance burden
- Clear pass/fail semantics
- Fast to write

**Cons:**
- Lower mutation score (40-60%)
- May miss subtle bugs
- Less precise validation

---

### Type 2: Mechanism Tests

**Purpose:** Validate that code uses specific algorithms, operators, and logic paths.

**Characteristics:**
- Check exact intermediate states
- Assert specific values, not ranges
- Test boundary conditions explicitly
- Example: `assert_eq!(chunks.len(), expected_from_formula)`

**Mutation Score:** 85-95%

**Catches:**
- Operator mutations (>, >=, +, -, &&, ||)
- Boundary condition bugs
- Arithmetic errors
- Configuration bugs
- Logic path errors

**Misses:**
- Equivalent implementations
- Semantically equivalent mutations

**Appropriate for:**
- Core algorithms (sorting, searching, hashing)
- Security-critical code (crypto, auth)
- Performance-critical inner loops
- Mathematical computations
- Data structure implementations

**Example:**
```rust
#[test]
fn test_merge_splits_mechanism() {
    let splitter = RecursiveCharacterTextSplitter::new(10, 2);
    let text = "0123456789ABC";  // Exactly crafted to test boundary
    let chunks = splitter.split_text(text);

    // Mechanism validation: exact counts, exact content, exact boundaries
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "0123456789");
    assert_eq!(chunks[1], "89ABC");
    assert_eq!(chunks[1].len(), 5);
}
```

**Pros:**
- High mutation score (85-95%)
- Catches subtle bugs
- Documents exact behavior
- Validates specific operators

**Cons:**
- Brittle - breaks when implementation changes
- High maintenance cost
- May over-specify implementation
- Can create false test failures

---

### Type 3: Property Tests

**Purpose:** Validate that invariants hold across many randomly generated inputs.

**Characteristics:**
- Check invariants, not specific outputs
- Generate many test cases automatically
- Example: `forall text: chunks.join("") == text` (no data loss)

**Mutation Score:** 60-75%

**Catches:**
- Logic violations
- Invariant breaks
- Edge cases missed by unit tests
- Boundary conditions (via random inputs)

**Misses:**
- Specific operator bugs if invariant still holds
- Implementation details
- Configuration validation

**Appropriate for:**
- Business logic
- State machines
- Parsers/serializers (round-trip properties)
- Data transformations (information preservation)
- Chain compositions

**Example:**
```rust
#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn text_splitter_preserves_content(
            text in "\\PC*",
            chunk_size in 10usize..1000,
            overlap in 0usize..10
        ) {
            let splitter = RecursiveCharacterTextSplitter::new(chunk_size, overlap);
            let chunks = splitter.split_text(&text);

            // Property: no data loss
            let reconstructed = chunks.join("");
            prop_assert!(reconstructed.contains(&text) || text.contains(&reconstructed));

            // Property: all chunks within size limit
            prop_assert!(chunks.iter().all(|c| c.len() <= chunk_size));
        }
    }
}
```

**Pros:**
- Explores edge cases automatically
- Good mutation score (60-75%)
- Documents invariants clearly
- Balanced brittleness vs. coverage

**Cons:**
- Slower test execution
- Can generate flaky tests if properties wrong
- Harder to debug failures
- Requires proptest/quickcheck setup

---

## Mutation Score Guidance by Code Type

| Code Type | Test Strategy | Target Mutation Score | Rationale |
|-----------|---------------|----------------------|-----------|
| **Core algorithms** (sort, hash, crypto) | Mechanism (85%) + Property (15%) | 80-90% | Correctness critical, implementation stable |
| **Integration code** (splitters, parsers) | Outcome (60%) + Property (40%) | 60-75% | Flexibility needed, brittleness costly |
| **Business logic** (chains, workflows) | Property (70%) + Mechanism (30%) | 65-80% | Invariants matter, implementation may change |
| **UI/Display** (fmt::Display, logging) | Outcome (100%) | 40-60% | Low risk, high maintenance cost |
| **Middleware/Adapters** | Outcome (80%) + Mechanism (20%) | 50-70% | Integration glue, flexible implementation |
| **Security code** (auth, validation) | Mechanism (100%) | 85-95% | Bugs are critical, must catch all |

---

## Mutation Testing Results Context

### dashflow-text-splitters (Phase 3A)

**Mutation Score:** 62.3% (91 caught / 55 missed / 6 unviable out of 152 mutants)

**Code Type:** Integration-level text splitting (parsers, HTML, markdown, regex)

**Test Strategy:** Outcome tests (100%)

**Analysis:**
- 62.3% is **APPROPRIATE** for integration code
- Tests correctly validate outcomes (splitting works, content preserved)
- Missed mutants are primarily: configuration getters, boundary conditions, arithmetic operators
- Adding mechanism tests would improve score to 85%+ but create brittle test suite
- Cost/benefit analysis favors accepting 60-75% range

**Gaps (Acceptable):**
- Configuration getter validation (LOW impact: tests validate behavior, not config)
- Boundary condition operators (MEDIUM impact: happy-path testing sufficient for integration)
- Arithmetic precision (LOW impact: outcome correctness more important than calculation method)

**Gaps (Should Fix):**
- None critical for integration-level code

---

### dashflow-chains (Phase 3B - Planned)

**Mutation Score:** TBD (762 mutants expected)

**Code Type:** Business logic (chain composition, workflows)

**Test Strategy:** Property tests (70%) + Unit tests (30%)

**Hypothesis:** Property tests should achieve 70-80% mutation score due to:
1. Testing invariants across many inputs
2. Automatically exploring boundary conditions
3. Catching logic violations

**Expected Results:**
- Mutation score: 70-80%
- Property tests catch: logic bugs, invariant violations, edge cases
- Property tests miss: configuration bugs, specific operator choices
- Comparison to text-splitters validates hypothesis about test type effectiveness

---

## When to Use Each Test Type

### Use Outcome Tests When:
- Code integrates multiple components
- Implementation may need to change
- Behavior matters more than method
- Refactoring is likely
- Examples: parsers, formatters, splitters, middleware

### Use Mechanism Tests When:
- Algorithm correctness is critical
- Implementation is stable
- Bugs have high severity
- Performance is critical
- Examples: sorting, crypto, auth, core data structures

### Use Property Tests When:
- Invariants are clear
- Many edge cases exist
- Round-trip properties apply
- State machines involved
- Examples: serialization, chain logic, transformations

---

## Test Gap Documentation Template

When accepting gaps in mutation coverage, document with:

```rust
// TEST BOUNDARY: This test validates OUTCOME (chunks produced, content preserved)
// but does NOT validate MECHANISM (exact operator choices, boundary conditions).
//
// Acceptable gaps:
// - Boundary operators (>, >=, <, <=): Happy-path testing sufficient
// - Arithmetic precision (+, -, *, /): Outcome correctness matters more
// - Configuration getters: Behavior validation covers this
//
// This is appropriate for integration-level code where implementation flexibility
// is more valuable than operator-level validation. See docs/TEST_PHILOSOPHY.md.
#[test]
fn test_text_splitter_basic_functionality() {
    // Test implementation...
}
```

---

## CI Integration Strategy

### Mutation Testing in CI

**Do NOT run mutation testing on every PR.** Mutation testing is expensive (hours) and provides diminishing returns as regression testing.

**Recommended approach:**

1. **Nightly/Weekly CI job:**
   - Run full mutation testing on core crates
   - Report mutation score trends
   - Alert on regressions >5%

2. **PR mutation testing (selective):**
   - Use `--in-diff` to test only changed code
   - Only on crates flagged as "high criticality"
   - Example: `cargo mutants --package dashflow --in-diff "HEAD~1..HEAD"`

3. **Manual mutation testing:**
   - Before releases
   - After major refactors
   - When adding new core algorithms

### Mutation Score Regression Thresholds

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The configuration below is a proposed template for teams using GitHub Actions.

Set different thresholds per crate type:

```yaml
# .github/mutation-thresholds.yml (proposed template)
dashflow-text-splitters:
  minimum: 60%
  target: 70%
  alert_on_regression: 5%

dashflow-chains:
  minimum: 65%
  target: 75%
  alert_on_regression: 5%

dashflow:
  minimum: 70%
  target: 80%
  alert_on_regression: 3%
```

### When to Add Tests

Add mechanism tests when:
1. Mutation testing reveals critical gap (security, data corruption)
2. Bug was found in production that mutation would have caught
3. Core algorithm added that needs precise validation

Do NOT add mechanism tests when:
1. Mutation score "looks low" but code is integration-level
2. Gaps are in low-severity areas (display, logging)
3. Adding tests would create brittle test suite

---

## Lessons from Phase 3A

### Lesson 1: Mutation Testing Reveals Test Philosophy

Human evaluation ("100% rigorous tests") ≠ Mechanistic validation (62.3% mutation score)

Mutation testing objectively measures what tests actually validate, not what we think they validate.

### Lesson 2: "Mechanistic Intent" ≠ Mechanistic Validation

Tests can be "mechanistic" in spirit (targeting specific logic) but "outcome-focused" in implementation (checking results exist, not exact values).

N=713-714 added 24 "mechanistic" tests but achieved 0% mutation score because assertions were still lenient (`assert!(!chunks.is_empty())` not `assert_eq!(chunks.len(), 3)`).

### Lesson 3: Test Brittleness is a Real Cost

Achieving 85%+ mutation scores requires tests that break when implementation changes, even when behavior remains correct. This maintenance cost must be weighed against benefit.

### Lesson 4: Different Code Needs Different Test Philosophies

- Core algorithms: Need mechanism tests (85%+ score)
- Integration code: Need outcome tests (60-75% score)
- Business logic: Need property tests (70-80% score)

One test philosophy doesn't fit all code types.

### Lesson 5: Context Matters More Than Absolute Score

62.3% is:
- Poor for a sorting algorithm
- Good for integration-level text splitters
- Excellent for UI rendering code

Absolute scores are meaningless without context.

---

## Decision Framework

When deciding test strategy for new code:

```
1. What is the code type?
   → Algorithm/Core → Mechanism tests (target 85%+)
   → Integration     → Outcome tests (target 60-75%)
   → Business Logic  → Property tests (target 70-80%)

2. What is the bug severity?
   → Critical (security, data loss)     → Mechanism tests required
   → High (incorrect behavior)          → Property or mechanism tests
   → Medium (performance, edge cases)   → Outcome or property tests
   → Low (display, logging)             → Outcome tests sufficient

3. How stable is the implementation?
   → Stable (unlikely to refactor)      → Mechanism tests acceptable
   → Unstable (may need changes)        → Outcome tests preferred
   → Unknown                            → Start with outcome, upgrade if needed

4. What is the maintenance cost?
   → High refactor frequency             → Outcome tests
   → Low refactor frequency              → Mechanism tests acceptable
   → Property-heavy code                 → Property tests

5. What are the invariants?
   → Clear invariants (no data loss)     → Property tests
   → Unclear invariants                  → Outcome tests
   → No invariants                       → Mechanism tests
```

---

## Summary

**Key Principle:** Match test rigor to code criticality and type. Not all code deserves 85%+ mutation scores.

**Test Type Guidelines:**
- **Outcome tests (40-60% score):** Integration code, flexible implementations
- **Mechanism tests (85-95% score):** Core algorithms, security-critical code
- **Property tests (60-75% score):** Business logic, invariants

**Mutation Score Targets:**
- Integration code: 60-75%
- Business logic: 65-80%
- Core algorithms: 80-90%

**CI Strategy:** Nightly/weekly full runs, selective PR testing with `--in-diff`, manual testing before releases.

**Documentation:** Use test boundary comments to explain acceptable gaps and rationale.

---

## References

- **cargo-mutants:** https://mutants.rs/

**Document Created:** 2025-11-04
**Author:** N=716
**Status:** Active
