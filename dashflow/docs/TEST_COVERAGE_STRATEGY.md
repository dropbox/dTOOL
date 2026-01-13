# Test Coverage Strategy for DashFlow

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Version:** 1.11
**Author:** Andrew Yates © 2026
**Purpose:** Rigorous, auditable test coverage metrics for production quality assurance

---

## Executive Summary

This document establishes **skeptical, rigorous test coverage standards** using Rust best-practice tools to ensure framework reliability. When bugs occur, we can **audit coverage metrics** to identify gaps.

**Key Principle:** Coverage metrics must be **multi-dimensional** - line coverage alone is insufficient for production frameworks.

---

## Coverage Dimensions

### 1. Line Coverage (via `cargo-tarpaulin`)

**Tool:** `cargo-tarpaulin`
**Target:** ≥85% for core crates, ≥70% overall

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --out Html --output-dir coverage/
```

**Interpretation:**
- **≥90%:** Excellent - critical core functionality
- **≥85%:** Good - production crates
- **≥70%:** Acceptable - integration/tooling crates
- **<70%:** Needs improvement

**By Crate Criticality:**
- `dashflow`: ≥90% (foundation)
- `dashflow`: ≥85% (orchestration)
- `dashflow-streaming`: ≥85% (telemetry)
- Provider crates: ≥70% (integration code)
- Tool crates: ≥60% (thin wrappers)

---

### 2. Branch Coverage (via `cargo-tarpaulin --branch`)

**Measures:** Which conditional branches (if/match) are tested

```bash
cargo tarpaulin --workspace --branch --out Html
```

**Target:** ≥80% branch coverage

**Critical for:**
- Error handling paths
- Conditional routing in DashFlow
- State transitions
- Retry logic

---

### 3. Mutation Testing (via `cargo-mutants`)

**Tool:** `cargo-mutants`
**Target:** ≥70% caught mutations for algorithmic code

```bash
cargo install cargo-mutants
cargo mutants --workspace --output mutants.txt
```

**What it tests:** If tests detect code changes

**Targets by Code Type:**
- **Algorithms:** ≥70% caught (text splitters, parsers, diff algorithm)
- **Integration:** ≥40% caught (API wrappers - outcome tests)
- **Orchestration:** ≥60% caught (DashFlow execution)

**Reference:** `docs/TEST_PHILOSOPHY.md` - Different code needs different strategies

---

### 4. Property-Based Testing (via `proptest`)

**Tool:** `proptest`
**Target:** Critical algorithms have property tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn diff_apply_is_identity(state: ArbitraryState) {
        let new_state = mutate_state(&state);
        let diff = diff_states(&state, &new_state)?;
        let reconstructed = apply_diff(&state, &diff)?;
        assert_eq!(reconstructed, new_state);
    }
}
```

**Required for:**
- State diff algorithm (identity property)
- Text splitters (chunk size invariants)
- JSON Patch operations (RFC 6902 compliance)
- Serialization (round-trip property)

---

### 5. Integration Test Coverage

**Tool:** Manual tracking
**Target:** All external dependencies have integration tests

**Categories:**
- Databases (Postgres, Redis, MongoDB, etc.)
- Vector stores (Chroma, Pinecone, Qdrant, etc.)
- LLM providers (OpenAI, Anthropic, etc.)
- Message brokers (Kafka)

**Coverage Metric:**
```
Integration Coverage = (Crates with integration tests) / (Total integration crates)
```

**Target:** 100% (all integration crates have tests)

---

### 6. Example Coverage

**Tool:** Manual tracking
**Target:** All major features have runnable examples

**Coverage Metric:**
```
Example Coverage = (Features with examples) / (Total public features)
```

**Target:** ≥80%

**Quality Standards:**
- Examples must compile
- Examples must run without modification
- Examples must be documented
- Examples must show best practices

---

### 7. Fuzzing (via `cargo-fuzz`)

**Tool:** `cargo-fuzz`
**Target:** Critical parsers and decoders have fuzz tests

```bash
cargo install cargo-fuzz
cargo fuzz run protobuf_decoder
cargo fuzz run json_patch_parser
cargo fuzz run state_diff_apply
```

**Required for:**
- Protobuf decoding (DashStream)
- JSON Patch parsing (diff algorithm)
- State deserialization
- Text splitter edge cases

**Coverage:** Run for ≥1 hour CPU time, 0 crashes

---

## Coverage Reporting Format

### Per-Release Coverage Report

**File:** `coverage/COVERAGE_REPORT_v1.X.X.md`

**Template:**
```markdown
# Test Coverage Report: v1.X.X

**Date:** YYYY-MM-DD
**Commit:** <git hash>

## Summary

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| Line Coverage | 87.3% | ≥85% | ✅ |
| Branch Coverage | 82.1% | ≥80% | ✅ |
| Mutation Score | 68.4% | ≥70% | ⚠️ |
| Integration Coverage | 100% | 100% | ✅ |
| Example Coverage | 83% | ≥80% | ✅ |

## By Crate (Top 20)

| Crate | Line % | Branch % | Mutations | Status |
|-------|--------|----------|-----------|--------|
| dashflow | 91.2% | 85.3% | 72% | ✅ |
| dashflow | 88.7% | 83.1% | 68% | ✅ |
| dashflow-streaming | 86.4% | 81.2% | N/A | ✅ |
| ... | ... | ... | ... | ... |

## Coverage Gaps

### Critical Gaps (Must Fix)
- `dashflow::agents::react`: 45% line coverage (need more tests)
- `dashflow-streaming::diff`: No property tests (add identity tests)

### Minor Gaps (Nice to Have)
- Error handling paths in vector stores (58% branch coverage)

## Action Items

1. Add tests for react agent edge cases
2. Add property tests for diff algorithm
3. Improve error path coverage in vector stores

## Historical Trend

- v1.0.0: 78% line coverage
- v1.1.0: 83% line coverage
- v1.2.0: 87% line coverage  ← 4% improvement
- v1.3.0: Target 90%
```

---

## Tooling Setup

### Install Coverage Tools

```bash
# Core coverage tool
cargo install cargo-tarpaulin

# Mutation testing
cargo install cargo-mutants

# Fuzzing
cargo install cargo-fuzz

# Coverage diff (track improvements)
cargo install cargo-coverage-diff
```

### Configuration Files

**`.tarpaulin.toml`:**
```toml
[config]
command = "test"
clean = false
run-types = ["Tests", "Doctests"]
out = ["Html", "Json"]
output-dir = "coverage/"

# Exclude generated code
exclude-files = [
    "*/proto/*",
    "*/build.rs",
]

# Crate-specific settings
[[coverage-by-crate]]
name = "dashflow"
target-coverage = 90.0

[[coverage-by-crate]]
name = "dashflow"
target-coverage = 85.0
```

**`.cargo/mutants.toml`:**
```toml
# Already exists - tune as needed
minimum_test_timeout = 60
error_on_missing_tests = true

[[package.skip]]
name = "dashflow-*-integration"
reason = "Integration tests are expensive"
```

---

## Continuous Coverage Monitoring

### Pre-Commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Run quick coverage check
cargo tarpaulin --workspace --lib --timeout 300 | tee coverage.txt

# Check if coverage decreased
COVERAGE=$(grep "coverage:" coverage.txt | awk '{print $2}' | sed 's/%//')
if (( $(echo "$COVERAGE < 85" | bc -l) )); then
    echo "❌ Coverage below 85%: $COVERAGE%"
    exit 1
fi
```

### CI/CD Pipeline

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The workflow below is a proposed template for teams using GitHub Actions.

```yaml
# Proposed: .github/workflows/coverage.yml (not yet implemented)
name: Coverage

on: [push, pull_request]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Run coverage
        run: |
          cargo tarpaulin --workspace --out Xml

      - name: Upload to codecov
        uses: codecov/codecov-action@v2

      - name: Check thresholds
        run: |
          if [ $(jq '.line_rate * 100' coverage.json) -lt 85 ]; then
            echo "Coverage below threshold"
            exit 1
          fi
```

---

## Coverage Audit Process

### When Bug is Found

**Step 1: Reproduce**
```bash
# Create failing test
cargo test --package <crate> test_bug_xyz
# Should fail
```

**Step 2: Check Coverage**
```bash
# Generate coverage before fix
cargo tarpaulin --package <crate> --out Html

# Identify uncovered lines
open coverage/index.html
# → Find which lines weren't tested
```

**Step 3: Analyze Gap**
```bash
# Check mutation testing
cargo mutants --package <crate> --file src/<buggy_file>.rs

# Was the bug line mutated and caught?
cat mutants.txt | grep <function_name>
```

**Step 4: Document**
```markdown
# Bug Audit Report

**Bug:** <description>
**File:** src/<file>.rs:<line>
**Coverage:** Line was NOT covered (0 tests)
**Mutation:** Not tested (0 mutations)

**Root Cause:** Gap in test coverage for <error_path>
**Fix:** Added test case for <scenario>
**New Coverage:** 87% → 89%
```

**Step 5: Add Test**
```rust
#[test]
fn test_bug_xyz_regression() {
    // Reproduce the bug scenario
    // Assert correct behavior
}
```

---

## Coverage Quality Standards

### Not Just Quantity - QUALITY Matters

**Bad Coverage (High % but useless):**
```rust
#[test]
fn test_smoke() {
    let _ = MyStruct::new();  // Doesn't test behavior!
}
// 100% line coverage, 0% value
```

**Good Coverage (Tests behavior):**
```rust
#[test]
fn test_error_handling() {
    let result = function_that_should_fail(invalid_input);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExpectedError::InvalidInput);
}
// Tests actual error paths
```

### Code Review Checklist

For each new feature:
- [ ] Unit tests for happy path
- [ ] Unit tests for error paths
- [ ] Property tests if algorithmic
- [ ] Integration test if external dependency
- [ ] Example demonstrating usage
- [ ] Coverage ≥ crate target
- [ ] Mutations ≥ 70% caught (if applicable)

---

## v1.3.0 Coverage Plan

**⏱️ USER DIRECTIVE: Phase 5 should take substantial time - this is EXTENSIVE work**

**Expected Timeline for Phase 5:**
- Coverage measurement: 30-60 minutes runtime
- Mutation testing: 2-4 hours runtime
- Gap analysis: 2-3 commits (careful review)
- Gap filling: 10-15 commits (systematic testing)
- Property testing: 2-3 commits (algorithm correctness)
- Fuzzing: 5-8 commits (robustness validation)
- Final report: 1-2 commits (comprehensive documentation)
- Automation: 3-5 commits (CI/CD setup)

**Total: 20-30 commits of rigorous testing work**

**Do not rush. Quality and thoroughness are paramount.**

---

### Phase 1: Measure Current State (N=921-925)

**Task 1:** Generate baseline coverage report
```bash
cargo tarpaulin --workspace --out Html Json --output-dir coverage/v1.2.0/
```

**Task 2:** Run mutation testing on all crates
```bash
cargo mutants --workspace --output coverage/v1.2.0/mutations.txt
```

**Task 3:** Generate coverage report
- Use template above
- Identify gaps
- Set v1.3.0 targets

**Task 4:** Commit baseline
```
# 925: v1.2.0 Test Coverage Baseline Report

Measured current state:
- Line coverage: 87.3%
- Branch coverage: 82.1%
- Mutation score: 68.4%
- Integration coverage: 100%
- Example coverage: 83%

Identified 15 critical gaps for v1.3.0.
```

---

### Phase 2: v1.3.0 Feature Development (N=926-960)

**Features (proposed):**
1. **S3 Checkpointer** - AWS S3 state persistence
2. **Graph Composition** - Subgraphs and reusable workflows
3. **Advanced Agent Patterns** - Plan & Execute, Reflection agents
4. **Distributed Execution** - Multi-machine graph execution
5. **WebAssembly Support** - Browser/edge deployment

**For EACH feature:**
- Write tests FIRST (TDD)
- Target ≥90% line coverage for new code
- Add property tests for algorithms
- Add integration tests
- Create working example

---

### Phase 3: Coverage Audit & Gap Filling (N=961-980)

**Task 1: Measure v1.3.0 coverage**
```bash
cargo tarpaulin --workspace --out Html Json --output-dir coverage/v1.3.0/
```

**Task 2: Compare to v1.2.0 baseline**
```bash
cargo-coverage-diff coverage/v1.2.0/tarpaulin.json coverage/v1.3.0/tarpaulin.json
```

**Task 3: Fill identified gaps**
- Add tests for uncovered lines
- Focus on error paths
- Test edge cases
- Add property tests

**Task 4: Re-run mutation testing**
```bash
cargo mutants --workspace --output coverage/v1.3.0/mutations.txt
```

**Task 5: Final coverage report**
```
# 980: v1.3.0 Test Coverage Complete - 90%+ Line Coverage Achieved

Coverage improvements:
- Line coverage: 87.3% → 92.1% (+4.8%)
- Branch coverage: 82.1% → 88.3% (+6.2%)
- Mutation score: 68.4% → 74.2% (+5.8%)

Added 180 tests to fill gaps.
All critical paths covered.
```

---

### Phase 4: Coverage Maintenance (Ongoing)

**Every N commits:**
- N mod 20: Run full coverage report
- N mod 100: Deep coverage audit

**Pre-release:**
- Full tarpaulin run
- Full mutation testing
- Coverage report in release notes

---

## Bug-Driven Coverage Improvements

### When Bug Found in Production

**Immediate Actions:**
1. Create failing test reproducing bug
2. Check coverage of buggy code
3. Run mutations on buggy function
4. Document gap in coverage audit
5. Fix bug + add comprehensive tests
6. Verify coverage increase

**Template for Bug Analysis:**
```markdown
## Bug Coverage Audit: [Bug Description]

**Date:** YYYY-MM-DD
**Commit:** <hash where bug found>

### Bug Details
- **File:** src/module/file.rs:123
- **Function:** `buggy_function()`
- **Symptom:** Crash when input is null

### Coverage Analysis
**Line Coverage:**
- Buggy function: 45% (8/18 lines)
- Line 123 (crash site): NOT COVERED ❌

**Branch Coverage:**
- Null check branch: NOT TESTED ❌

**Mutation Testing:**
- Function mutations: 3/8 caught (37%) ⚠️
- Null check mutation: NOT CAUGHT ❌

### Root Cause
- Missing test for null input path
- Error handling branch never exercised
- Assumed input always valid

### Fix Applied
- Added test: `test_null_input_error()`
- Added test: `test_empty_input_error()`
- Added property test: All inputs must validate

### Coverage After Fix
- Line coverage: 45% → 94%
- Branch coverage: 60% → 100%
- Mutations: 37% → 75%

### Lessons Learned
- Always test error paths
- Add null/empty input tests
- Property tests catch edge cases

### Prevention
- Added to test checklist
- Updated coverage thresholds
- Added pre-commit hook
```

---

## Advanced Coverage Techniques

### 1. Differential Coverage

**Track coverage changes in PRs:**
```bash
# Before PR
cargo tarpaulin --workspace --out Json --output-dir coverage/before/

# After PR
cargo tarpaulin --workspace --out Json --output-dir coverage/after/

# Diff
cargo-coverage-diff coverage/before/tarpaulin.json coverage/after/tarpaulin.json

# Require: New code has ≥90% coverage
```

### 2. Critical Path Coverage

**Identify and prioritize critical paths:**

**Critical Paths:**
- DashFlow execution engine
- State serialization/deserialization
- Checkpoint save/load
- LLM API calls
- Error handling
- Tool execution

**Target:** 95%+ coverage on critical paths

### 3. Concurrency Coverage

**Test concurrent execution:**
```rust
#[test]
fn test_concurrent_checkpoint_access() {
    let checkpointer = Arc::new(PostgresCheckpointer::new());

    // Spawn 100 concurrent tasks
    let handles: Vec<_> = (0..100).map(|i| {
        let cp = checkpointer.clone();
        tokio::spawn(async move {
            cp.save(...).await
        })
    }).collect();

    // All should succeed
    for handle in handles {
        assert!(handle.await.is_ok());
    }
}
```

### 4. Error Path Coverage

**Explicitly test all error conditions:**
```rust
#[test]
fn test_all_error_types() {
    // NetworkError
    assert_eq!(function_with_network_error(), Err(Error::Network));

    // TimeoutError
    assert_eq!(function_with_timeout(), Err(Error::Timeout));

    // InvalidInput
    assert_eq!(function_with_bad_input(), Err(Error::InvalidInput));

    // ... test EVERY error variant
}
```

---

## Coverage Dashboard

### Metrics to Track

**Core Metrics:**
- Overall line coverage %
- Overall branch coverage %
- Mutation score %
- Tests per KLOC (tests per 1,000 lines of code)

**Per-Crate Metrics:**
- Line/branch coverage
- Test count
- Example count
- Last coverage check date

**Trend Metrics:**
- Coverage change over time
- New code coverage (PRs)
- Gap closure rate

### Visualization

**Coverage over time:**
```
Coverage Trend:
100% ┤
 90% ┤                                    ╭──
 80% ┤                          ╭─────────╯
 70% ┤                  ╭───────╯
 60% ┤         ╭────────╯
 50% ┤─────────╯
     └─────────────────────────────────────────
     v1.0   v1.1   v1.2   v1.3   (goal: 95%)
```

---

## Coverage Anti-Patterns (What NOT to Do)

### ❌ Bad: Chasing 100% Coverage
```rust
// Don't write useless tests just for coverage
#[test]
fn test_default() {
    let _ = Thing::default();  // Doesn't test anything!
}
```

### ❌ Bad: Testing Implementation Details
```rust
// Don't test private implementation
#[test]
fn test_internal_cache_size() {
    // This breaks when implementation changes
    assert_eq!(thing.cache.len(), 5);
}
```

### ❌ Bad: Snapshot Testing Everything
```rust
// Don't snapshot test unstable outputs
insta::assert_snapshot!(llm_response);  // LLM output varies!
```

### ✅ Good: Test Behavior and Contracts
```rust
#[test]
fn test_checkpointer_contract() {
    // Test the interface contract
    let cp = PostgresCheckpointer::new();

    // Save → Load should be identity
    cp.save(&state).await?;
    let loaded = cp.load(&thread_id).await?;
    assert_eq!(loaded, state);

    // List should contain saved checkpoint
    let list = cp.list(&thread_id).await?;
    assert!(list.contains(&checkpoint_id));
}
```

---

## Implementation Checklist for v1.3.0

### Phase 1: Baseline (N=921-925)
- [ ] Install tarpaulin, mutants, fuzz
- [ ] Generate v1.2.0 baseline coverage report
- [ ] Create `coverage/` directory structure
- [ ] Document current gaps
- [ ] Set v1.3.0 targets

### Phase 2: Feature Development (N=926-960)
- [ ] TDD for all new features
- [ ] ≥90% coverage for new code
- [ ] Property tests for algorithms
- [ ] Integration tests for external deps
- [ ] Examples for all features

### Phase 3: Gap Filling (N=961-980)
- [ ] Run tarpaulin on full workspace
- [ ] Identify uncovered lines
- [ ] Add tests for gaps
- [ ] Re-run mutations
- [ ] Generate final report
- [ ] Update documentation

### Phase 4: Automation (N=981-985)
- [ ] Add tarpaulin to CI/CD
- [ ] Add coverage badges to README
- [ ] Set up automated reports
- [ ] Create coverage dashboard

---

## Success Criteria for v1.3.0

**Required (must achieve):**
- ✅ Overall line coverage ≥90%
- ✅ Core crates ≥90%
- ✅ Branch coverage ≥85%
- ✅ All integration crates have tests
- ✅ All features have examples
- ✅ Coverage report generated
- ✅ Gaps documented and addressed

**Stretch Goals:**
- 🎯 Mutation score ≥75%
- 🎯 Fuzz testing for 5+ critical functions
- 🎯 Property tests for all algorithms
- 🎯 Coverage dashboard with historical trends

---

## References

- **Tarpaulin:** https://github.com/xd009642/tarpaulin
- **Cargo Mutants:** https://github.com/sourcefrog/cargo-mutants
- **Cargo Fuzz:** https://github.com/rust-fuzz/cargo-fuzz
- **Proptest:** https://github.com/proptest-rs/proptest
- **Test Philosophy:** docs/TEST_PHILOSOPHY.md

---

**Status:** Active
**Next Review:** v1.3.0 release
**Owner:** Engineering Team
