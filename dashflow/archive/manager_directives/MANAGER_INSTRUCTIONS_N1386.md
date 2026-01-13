# MANAGER Instructions for Worker N=1388+

**Date:** January 13, 2025
**From:** Manager AI
**To:** Worker AI (N=1388+)
**Priority:** CRITICAL - Add LLM-as-Judge to Integration Tests

## PROBLEM: Current Tests Are Insufficient

Worker N=1387 ran the tests, but they only use **basic keyword assertions**:
```rust
assert!(answer.contains("tokio"))  // ❌ Not good enough!
```

**User requirement:** Tests must use **LLM-as-judge** to evaluate response quality!

---

## Your Task: Add LLM-as-Judge Quality Evaluation

The **16 multi-turn conversation tests exist and run**, but they need **LLM-as-judge evaluation** added.

**Current tests have:**

```
examples/apps/document_search/tests/multi_turn_conversations.rs (5 tests)
examples/apps/advanced_rag/tests/multi_turn_conversations.rs (5 tests)
examples/apps/code_assistant/tests/multi_turn_conversations.rs (6 tests)
```

**These tests already exist and compile successfully.** Your job is to:

1. ✅ Commit the test files
2. ✅ Run ALL 16 tests with real LLM calls
3. ✅ Capture and document actual outputs
4. ✅ Prove they work

---

## Step 1: Commit Test Files (N=1386)

```bash
git add examples/apps/*/tests/multi_turn_conversations.rs
git add examples/apps/advanced_rag/Cargo.toml examples/apps/code_assistant/Cargo.toml
git add Cargo.lock
git add reports/all-to-rust2/integration_test_*

# Commit message:
# 1386: Multi-Turn Conversation Test Suites - 16 Integration Tests Created
```

Include in commit message:
- 16 tests across 3 apps
- 43 conversational turns
- 10 conversation patterns
- Ready for execution

---

## Step 2: Run Tests with Real LLM (N=1387)

**Prerequisites:**
```bash
export OPENAI_API_KEY="sk-..."  # User must provide
```

**Execute ALL 16 tests:**
```bash
# Document Search (5 tests)
cargo test --package document_search --test multi_turn_conversations -- --ignored --nocapture 2>&1 | tee results_document_search.txt

# Advanced RAG (5 tests)
cargo test --package advanced_rag --test multi_turn_conversations -- --ignored --nocapture 2>&1 | tee results_advanced_rag.txt

# Code Assistant (6 tests)
cargo test --package code_assistant --test multi_turn_conversations -- --ignored --nocapture 2>&1 | tee results_code_assistant.txt
```

**Expected time:** 20-30 minutes total

---

## Step 3: Create Evidence Report (N=1387)

Create: `reports/all-to-rust2/multi_turn_test_results_n1387_YYYY-MM-DD.md`

**Must include:**

### Test Execution Summary
```
Total tests: 16
Passed: X
Failed: Y
Pass rate: Z%
```

### Per-App Results
```
Document Search: X/5 passed
Advanced RAG: X/5 passed
Code Assistant: X/6 passed
```

### Sample Outputs (At Least 3 Complete Conversations)

**Example format:**
```
### Test: DS-MT-1 (Progressive Depth Conversation)

Turn 1: "Tell me about tokio"
Agent Response:
[ACTUAL LLM RESPONSE - COPY VERBATIM FROM OUTPUT]

Turn 2: "How do I spawn tasks with it?"
Agent Response:
[ACTUAL LLM RESPONSE - COPY VERBATIM FROM OUTPUT]

Turn 3: "What about channels for communication?"
Agent Response:
[ACTUAL LLM RESPONSE - COPY VERBATIM FROM OUTPUT]

Result: ✅ PASSED
Context preserved: ✅ Yes (tokio mentioned across all turns)
Tool called: ✅ Yes (3 times)
```

### Failure Analysis (if any)

For any failed tests:
- Exact error message
- Which assertion failed
- Why it failed
- How to fix (framework issue vs test issue)

### Performance Metrics

- Average time per test
- Total execution time
- LLM API latency

---

## Step 4: Fix Any Framework Issues (N=1388+ if needed)

If tests reveal framework problems:
1. Document the issue clearly
2. Fix in framework (dashflow-core, dashflow)
3. Re-run tests to verify fix
4. Commit fix separately

**Examples of framework issues to watch for:**
- Context not preserved across turns
- Tool calling failures
- State management bugs
- Error handling gaps

**Do NOT fix test code unless it has bugs** - if framework behaves incorrectly, fix the framework!

---

## Success Criteria

### Must Achieve:
- ✅ All 16 tests run successfully
- ✅ Evidence report shows ACTUAL LLM outputs (not descriptions)
- ✅ Context preservation verified in outputs
- ✅ Tool calls logged and visible
- ✅ Pass rate ≥80% (13+/16 tests)

### Ideal Achievement:
- 🎯 100% pass rate (16/16 tests)
- 🎯 All conversation patterns work correctly
- 🎯 Zero framework bugs found
- 🎯 Rich output examples in evidence report

---

## Important Notes

### About the Tests

**These tests use MOCKS:**
- DocumentSearchTool: Mock Rust async/tokio knowledge base
- VectorStoreSearchTool: Mock Rust ownership/borrowing docs
- WebSearchTool: Mock current events
- CodeExecutionTool: Mock code execution

**This is intentional** (disk space constraint). Tests validate:
- ✅ Graph execution flow
- ✅ LLM reasoning and context
- ✅ Tool calling patterns
- ✅ Multi-turn conversation state

**Not validated** (deferred to machine with more storage):
- ⏸️ Real vector similarity search
- ⏸️ Embedding quality
- ⏸️ Large-scale retrieval

### About Test Failures

**If a test fails:**

1. **Check if it's a real failure:**
   - Does the LLM response make sense but fail assertion?
   - Is assertion too strict?

2. **Determine root cause:**
   - LLM didn't understand query? → Test design issue
   - Context not preserved? → Framework bug (FIX FRAMEWORK)
   - Tool not called? → Framework bug (FIX FRAMEWORK)
   - Empty response? → Framework bug or LLM issue

3. **Fix appropriately:**
   - Framework bug → Fix in dashflow-core/dashflow
   - Test too strict → Relax assertion
   - LLM variability → Run test 2-3 times

### Test Flags

All tests marked with:
```rust
#[tokio::test]
#[ignore]  // Requires OPENAI_API_KEY
```

Must use `-- --ignored` to run them!

---

## Documentation References

**Test Matrix:** `reports/all-to-rust2/integration_test_suite_matrix_2025-01-13.md`
- Complete grid of 16 tests
- Conversation patterns
- Tool coverage
- Execution commands

**Evidence Report:** `reports/all-to-rust2/integration_test_evidence_2025-01-13.md`
- Verification commands
- Compilation proof
- Expected outputs
- Quality metrics

---

## Manager's Expectations

**I expect to see:**

1. ✅ Commit of test files (N=1386)
2. ✅ Test execution results with REAL outputs (N=1387)
3. ✅ Evidence that conversations work correctly
4. ✅ Actual LLM responses (not "The test passed")
5. ✅ Framework fixes if issues found (N=1388+)

**Do NOT:**
- ❌ Just commit without running tests
- ❌ Report "tests passed" without showing actual outputs
- ❌ Skip tests that fail - investigate and fix
- ❌ Fix test assertions if framework is broken - fix the framework!

---

## Timeline

**N=1386:** Commit test files (~10 minutes)
**N=1387:** Run all 16 tests, create evidence report (~30-40 minutes including LLM calls)
**N=1388+:** Fix any framework issues discovered (if needed)

**Expected completion:** N=1387 or N=1388 (depends on if issues found)

---

## This is High-Value Work

These tests validate the core value proposition:
- Multi-turn conversations work correctly
- Context is preserved across turns
- Tools are called appropriately
- Agents reason correctly with conversation history

**This is exactly what users need to trust the system.**

Execute thoroughly and document results rigorously.

---

**Good luck!**
- Manager AI
