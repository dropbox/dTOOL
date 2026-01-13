# Instructions for Next AI (N=1390) - Complete LLM-as-Judge Implementation

**Date:** November 13, 2025
**From:** Worker AI N=1389
**To:** Worker AI N=1390
**Context:** Completing MANAGER_INSTRUCTIONS_N1388_LLM_AS_JUDGE.md

---

## Current Status (N=1389 Complete)

**✅ COMPLETED:**
1. Common crate with QualityJudge implementation (examples/apps/common/)
2. document_search tests updated with LLM-as-judge (5/5 tests, 12 conversation turns)
3. Dependencies added to advanced_rag and code_assistant Cargo.toml files

**⏳ REMAINING:**
1. Update advanced_rag/tests/multi_turn_conversations.rs (5 tests, ~12 turns)
2. Update code_assistant/tests/multi_turn_conversations.rs (6 tests, ~19 turns)
3. Run all 16 tests with quality evaluation (40-60 min)
4. Create comprehensive quality evidence report

**Progress:** 5/16 tests complete (31%), infrastructure 100% ready

---

## What You Need to Do

### Task 1: Update advanced_rag Tests (5 tests)

**File:** `examples/apps/advanced_rag/tests/multi_turn_conversations.rs`

**Pattern to apply (same as document_search):**

1. Add import at top:
```rust
use common::QualityJudge;
```

2. In each test, add after agent setup:
```rust
let judge = QualityJudge::new();
```

3. After each `agent.invoke()` call, add evaluation:
```rust
let score = judge
    .judge_response(
        "query text here",
        &answer,
        &["expected", "topics", "here"],
        Some(&context)  // or None for first turn
    )
    .await
    .expect("Failed to judge turn N");

println!(
    "Turn N Quality: {:.2} (Accuracy:{:.2}, Relevance:{:.2}, Completeness:{:.2})",
    score.average(),
    score.accuracy,
    score.relevance,
    score.completeness
);
println!("Reasoning: {}\n", score.reasoning);

assert!(
    score.meets_threshold(0.7),
    "Turn N quality too low: {:.2}. Reasoning: {}",
    score.average(),
    score.reasoning
);
```

4. For multi-turn tests, add final average quality check:
```rust
let avg_quality = (score1.average() + score2.average() + score3.average()) / 3.0;
println!("✓ Test complete. Average quality: {:.2}", avg_quality);
assert!(avg_quality >= 0.75, "Overall quality too low: {:.2}", avg_quality);
```

**Tests to update:**
1. test_route_switching_conversation (3 turns) - Lines 97-178
2. test_progressive_refinement (3 turns) - Lines 180-246
3. test_multi_tool_coordination (1 turn) - Lines 248-289
4. test_context_preserved_across_tools (2 turns) - Lines 291-338
5. test_follow_up_questions (3 turns) - Lines 340-404

**Expected topics for each test:**
- Test 1 Turn 1: ["ownership", "Rust", "memory"]
- Test 1 Turn 2: ["AI", "2024", "developments"]
- Test 1 Turn 3: ["borrowing", "references", "Rust"]
- Test 2 Turn 1: ["Rust", "memory", "ownership", "borrowing"]
- Test 2 Turn 2: ["lifetimes", "annotations", "references"]
- Test 2 Turn 3: ["example", "code", "practical"]
- Test 3 Turn 1: ["ownership", "Rust", "2024", "comparison"]
- Test 4 Turn 1: ["ownership", "Rust"]
- Test 4 Turn 2: ["Rust", "2024", "evolution", "ownership"]
- Test 5 Turn 1: ["ownership", "Rust"]
- Test 5 Turn 2: ["garbage collection", "ownership", "advantages"]
- Test 5 Turn 3: ["borrowing", "references"]

---

### Task 2: Update code_assistant Tests (6 tests)

**File:** `examples/apps/code_assistant/tests/multi_turn_conversations.rs`

Apply the same pattern as above. The file has 6 tests:
1. Test at line 72 (multi-turn)
2. Test at line 145 (multi-turn)
3. Test at line 200 (multi-turn)
4. Test at line 259 (multi-turn)
5. Test at line 313 (multi-turn)
6. Test at line 375 (multi-turn)

Read the test queries and determine appropriate expected_topics for each turn based on the question content.

---

### Task 3: Verify Compilation

After updating both files:
```bash
cargo test --package advanced_rag --test multi_turn_conversations --no-run
cargo test --package code_assistant --test multi_turn_conversations --no-run
cargo test --package document_search --test multi_turn_conversations --no-run
```

All should compile with 0 errors, 0 warnings.

---

### Task 4: Run All 16 Tests

**IMPORTANT:** Ensure OPENAI_API_KEY is set in .env and exported.

```bash
# Run all 16 multi-turn conversation tests with LLM-as-judge
./run_multi_turn_tests.sh

# Or run individually:
cargo test --package document_search --test multi_turn_conversations -- --ignored --nocapture 2>&1 | tee results_ds_n1390.txt

cargo test --package advanced_rag --test multi_turn_conversations -- --ignored --nocapture 2>&1 | tee results_ar_n1390.txt

cargo test --package code_assistant --test multi_turn_conversations -- --ignored --nocapture 2>&1 | tee results_ca_n1390.txt
```

**Expected duration:** 40-60 minutes total (LLM-as-judge doubles LLM calls per turn)
- document_search: ~15-20 min (12 judge calls)
- advanced_rag: ~15-20 min (12 judge calls)
- code_assistant: ~20-25 min (19 judge calls)

---

### Task 5: Create Quality Evidence Report

**File:** `reports/all-to-rust2/llm_as_judge_results_n1390_2025-11-13.md`

**Must include:**

#### 1. Executive Summary
```
Total turns evaluated: 43
Average quality: X.XX
Quality ≥0.7: Y/43 (Z%)
Quality <0.7: N/43 (low quality)
```

#### 2. Per-App Results
```
document_search (12 turns):
- Average quality: X.XX
- Turns ≥0.7: Y/12 (Z%)
- Lowest quality turn: Test X Turn Y (score: X.XX)

advanced_rag (12 turns):
- Average quality: X.XX
- Turns ≥0.7: Y/12 (Z%)
- Lowest quality turn: Test X Turn Y (score: X.XX)

code_assistant (19 turns):
- Average quality: X.XX
- Turns ≥0.7: Y/19 (Z%)
- Lowest quality turn: Test X Turn Y (score: X.XX)
```

#### 3. Quality Distribution
Show histogram/distribution of quality scores across all 43 turns.

#### 4. Low Quality Analysis
For ANY turn scoring <0.7:
- Which test and turn number
- Query, response, expected topics
- Actual scores (accuracy, relevance, completeness)
- LLM reasoning
- Root cause analysis (framework bug vs LLM variability vs test design)
- Fix recommendation

#### 5. Conclusions
- Did we meet success criteria? (≥80% turns ≥0.7, average ≥0.75)
- Framework quality assessment
- Production readiness confirmation
- Any recommended improvements

---

## Success Criteria (from MANAGER instructions)

**Must achieve:**
- ✅ QualityJudge helper implemented and working (DONE N=1389)
- ✅ All 16 tests updated to use LLM-as-judge (PENDING N=1390)
- ✅ All tests run with quality evaluation (PENDING N=1390)
- ✅ Quality scores documented for every turn (43 turns total) (PENDING N=1390)
- ✅ Average quality ≥0.75 across all turns (PENDING N=1390)
- ✅ ≥80% of turns score ≥0.7 on all dimensions (PENDING N=1390)

**Quality thresholds:**
- Pass: Individual dimension ≥0.7
- Good: Average ≥0.8
- Excellent: All dimensions ≥0.9

---

## If Tests Fail Quality Thresholds

**DO NOT lower thresholds!**

**FIX THE FRAMEWORK:**
1. Context not being used? → Check message history passing in dashflow
2. Tool results not integrated? → Check tool message handling in ReAct agent
3. System prompts inadequate? → Improve prompts in agent creation

See MANAGER_INSTRUCTIONS_N1388_LLM_AS_JUDGE.md lines 328-346 for details.

---

## Reference Implementation

**Perfect example:** `examples/apps/document_search/tests/multi_turn_conversations.rs`
- All 5 tests use LLM-as-judge
- Clean error messages with reasoning
- Average quality assertions for multi-turn tests
- Zero warnings, zero errors

**Common crate:** `examples/apps/common/src/quality_judge.rs`
- QualityJudge::new() creates judge with GPT-4o-mini temp=0
- judge_response() evaluates single turn
- judge_conversation() evaluates full conversation (optional, not used in current tests)

---

## Estimated Time

**N=1390 work:**
- Update advanced_rag tests: ~30 min (systematic application of pattern)
- Update code_assistant tests: ~30 min (systematic application of pattern)
- Run all 16 tests: ~60 min (LLM calls, mostly waiting)
- Create evidence report: ~30 min (analyze results, write report)

**Total:** ~2.5 hours AI time (~12 commits at 12 min/commit)

---

## Tips

1. **Use reference:** Copy pattern from document_search tests exactly
2. **Context matters:** For turn 2+, include previous turn in context string
3. **Expected topics:** Match to the query content (what should the answer cover?)
4. **Quality threshold:** Always 0.7 for individual turns, 0.75 for conversation average
5. **Error messages:** Include score.reasoning in assertion failure messages
6. **Test incrementally:** After updating advanced_rag, compile and test before doing code_assistant

---

## Commit Message Template for N=1390

```
# 1390: LLM-as-Judge Implementation - Complete
**Current Plan**: MANAGER_INSTRUCTIONS_N1388_LLM_AS_JUDGE.md
**Checklist**: Cleanup iteration (N=1390, N mod 5 = 0). All 16 tests updated, executed, and validated.

## Changes

**1. Updated advanced_rag tests (5 tests, 12 turns)**
- [List specific changes]

**2. Updated code_assistant tests (6 tests, 19 turns)**
- [List specific changes]

**3. Executed all 16 tests with LLM-as-judge**
- Total: 43 conversation turns evaluated
- Pass rate: X/43 (Y%)
- Average quality: X.XX
- Test duration: X minutes

**4. Created quality evidence report**
- reports/all-to-rust2/llm_as_judge_results_n1390_2025-11-13.md
- Comprehensive analysis of all 43 turns
- [Quality assessment summary]

## New Lessons

[Document any lessons learned from quality analysis]

## Information Expiration

Keyword-based assertions in multi-turn tests are now obsolete. LLM-as-judge provides superior validation.

## Next AI: [Based on results]

If all tests pass quality thresholds:
- Update ROADMAP_CURRENT.md to mark LLM-as-judge work complete
- Consider next feature or wait for user direction

If tests reveal quality issues:
- Fix identified framework bugs
- Re-run tests to validate fixes

## Progress Note

Commit 1390 of ongoing maintenance. LLM-as-Judge implementation complete. All 16 tests validated. Production readiness: [Grade based on results].
```

---

**Good luck, N=1390! The infrastructure is solid. You just need to apply the pattern systematically and validate the results.**

— Worker AI N=1389
