# v51 Skeptical Audit: optimize/propose.rs

**Auditor:** Worker #1725
**Date:** 2025-12-25
**File:** `crates/dashflow/src/optimize/propose.rs`
**Lines:** 1004

## Summary

GroundedProposer generates instruction candidates for MIPROv2-style optimization.
Well-designed module with LLM-based and tip-based instruction generation modes.

**Verdict:** No significant issues (P0/P1/P2/P3). 3 P4 items found.

## Architecture

```
GroundedProposerConfig
├── llm: Option<Arc<dyn ChatModel>>  (LLM for generation)
├── data_aware: bool                  (use dataset summary)
├── tip_aware: bool                   (use prompting tips)
├── view_data_batch_size: usize       (examples for summary)
├── seed: u64                         (UNUSED - see M-856)
└── verbose: bool                     (logging)

GroundedProposer
├── config: GroundedProposerConfig
├── data_summary: Option<String>
│
├── new() -> async constructor, builds dataset summary
├── propose_instructions() -> main API, delegates to:
│   ├── propose_with_llm() -> LLM-based generation
│   └── propose_with_tips() -> tip-based fallback
├── build_proposal_prompt() -> construct LLM prompt
├── parse_proposed_instructions() -> extract from LLM response
└── create_dataset_summary() -> summarize training examples
```

## Code Breakdown

| Section | Lines | % | Description |
|---------|-------|---|-------------|
| Module docs + imports | 1-28 | 3% | Clean preamble |
| TIPS constant | 29-37 | 1% | 6 tip variations |
| Config struct | 39-74 | 4% | Configuration with defaults |
| Proposer struct + docs | 76-94 | 2% | Main struct definition |
| Constructor (new) | 96-124 | 3% | Async init with summary |
| propose_instructions | 126-159 | 3% | Main API entry point |
| propose_with_llm | 161-209 | 5% | LLM-based generation |
| build_proposal_prompt | 211-284 | 7% | Prompt construction |
| parse_proposed_instructions | 286-316 | 3% | Response parsing |
| propose_with_tips | 318-344 | 3% | Tip-based fallback |
| create_dataset_summary | 346-391 | 5% | Dataset summarization |
| **Tests** | 393-1004 | **61%** | Comprehensive test suite |

## Analysis

### Strengths

1. **Clean separation**: LLM vs tip-based modes handled elegantly
2. **Defensive defaults**: Original instruction always included as first candidate
3. **Gap filling**: Automatically fills if LLM returns fewer than requested
4. **Comprehensive tests**: 61% of file is test coverage

### P4 Issues Found

#### M-854: Confusing digit-stripping in parse_proposed_instructions
**File:** `propose.rs:297-301`
**Category:** Code clarity

The parsing logic strips digits in two steps:
```rust
let instruction = if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit()) {
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    // ...
}
```

`strip_prefix` only removes the first digit, then `trim_start_matches` removes remaining digits.
This works but is confusing. Could be simplified to a single `trim_start_matches`.

**Impact:** Code clarity only. Function works correctly.

---

#### M-855: Character length check uses byte count
**File:** `propose.rs:306`
**Category:** Edge case

```rust
if !instruction.is_empty() && instruction.len() > 5 {
```

`instruction.len()` returns byte count, not character count. For ASCII text this is fine,
but multi-byte UTF-8 characters could cause inconsistent filtering.

**Impact:** P4 because expected use is English text. Would need `.chars().count()` for
proper Unicode handling.

---

#### M-856: Unused `seed` field in GroundedProposerConfig
**File:** `propose.rs:57,70`
**Category:** Dead code

```rust
pub seed: u64,       // Line 57 - declaration
seed: 42,            // Line 70 - default value
```

The `seed` field is declared and defaulted but never used in the implementation.
Tip selection in `propose_with_tips` iterates deterministically without randomization.

**Impact:** Unused field suggests incomplete feature. Either remove or implement
randomized tip selection using the seed.

---

## Mock Usage Analysis

The test file includes `MockProposalLLM` (lines 694-732). This is **NOT** a violation
of the mock prohibition because:

1. Tests verify GroundedProposer logic, not ChatModel behavior
2. The mock is transparent about responses (configurable return value)
3. Tests target parsing, prompt building, and gap-filling logic
4. Real LLM calls would be non-deterministic and costly

The CLAUDE.md prohibition targets "mocks that pretend to test real behavior" - this mock
is a legitimate test double for testing the proposer's handling of arbitrary LLM responses.

## Verification

```bash
# Tests pass
cargo test -p dashflow propose -- --nocapture 2>&1 | tail -20
```

## Files Changed

None - audit only.

## Recommendations

1. **M-856 (Low priority):** Either remove `seed` field or implement randomized tip
   selection for diversity
2. Consider adding a method to get current tip selection strategy for debugging
