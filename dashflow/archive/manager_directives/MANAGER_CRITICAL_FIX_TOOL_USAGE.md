# MANAGER CRITICAL FIX: Tool Results Not Being Used (90% → 95%+ Quality)

**Date:** January 13, 2025
**From:** Manager AI
**To:** Worker AI
**Priority:** **HIGH** - Framework Quality Issue Confirmed by Tests

---

## ISSUE CONFIRMED: Quality is 90% Due to Tool Integration Problem

**Evidence from telemetry test runs:**
- 8 instances of "couldn't find documentation" or "wasn't able to find"
- Tools ARE being called ✅
- Tools ARE returning documentation ✅
- LLM is IGNORING tool results ❌

**Impact on quality:**
- Completeness: 0.85 (should be 0.95+)
- Overall: 0.90 (should be 0.95+)
- Judge reasoning: "Could provide more examples" (because LLM used generic knowledge, not specific tool results)

---

## EXAMPLE OF THE PROBLEM

**Turn 2: "Actually, I meant to ask about tokio"**

**Tool Called:** DocumentSearchTool
**Tool Returned:** "Tokio: An async runtime for Rust. Provides async I/O, timers, and task scheduling."

**LLM Response:** "It seems that I couldn't find any specific documentation on Tokio..."
[Then provides generic answer from internal knowledge]

**Judge Score:** Completeness 0.85
**Judge Reasoning:** "Lacks specific details about any changes to ownership in Rust 2024"

**❌ THIS IS WRONG!** The tool returned information, but LLM didn't use it.

---

## ROOT CAUSE

**File:** `crates/dashflow/src/prebuilt.rs`
**Function:** `create_react_agent()`
**Issue:** System prompt doesn't emphasize using tool results

---

## THE FIX

### Step 1: Find Current System Prompt

```bash
grep -A20 "system.*message\|system.*prompt" crates/dashflow/src/prebuilt.rs | head -40
```

**Current prompt probably says something generic like:**
```rust
"You are a helpful assistant with access to tools."
```

### Step 2: Update System Prompt

**Change to:**
```rust
let system_message = Message::system(
    "You are a helpful assistant with access to tools.\n\n\
     IMPORTANT INSTRUCTIONS FOR USING TOOLS:\n\
     1. When you use a tool to search for information, the search results will appear as tool messages\n\
     2. You MUST base your response on the information returned by the tool\n\
     3. If a tool returns results, USE THEM - do NOT say 'I couldn't find documentation'\n\
     4. Always cite or reference the information from the search results\n\
     5. Tool results are more reliable than your internal knowledge for specific queries\n\n\
     When answering queries:\n\
     - If you searched for information, use what the search returned\n\
     - Be specific and reference the retrieved content\n\
     - Only say 'no documentation found' if the tool explicitly returns that message"
);
```

### Step 3: Test the Fix

Run one test that previously had the problem:
```bash
OPENAI_API_KEY="..." cargo test --package document_search \
  --test multi_turn_conversations \
  --features dashstream \
  test_error_recovery \
  -- --ignored --nocapture
```

**Check Turn 2 output:**
- Before fix: "I couldn't find any specific documentation on Tokio"
- After fix: "Based on the search results, Tokio is an async runtime..."

**Check quality score:**
- Before: Completeness 0.85
- After: Completeness 0.95+ (should improve)

### Step 4: Re-Run All 16 Tests

```bash
# Run all tests again with fix
OPENAI_API_KEY="..." cargo test --package document_search --test multi_turn_conversations --features dashstream -- --ignored --nocapture

OPENAI_API_KEY="..." cargo test --package advanced_rag --test multi_turn_conversations --features dashstream -- --ignored --nocapture

OPENAI_API_KEY="..." cargo test --package code_assistant --test multi_turn_conversations --features dashstream -- --ignored --nocapture
```

**Expected improvement:**
- Completeness: 0.85 → 0.95
- Overall: 0.90 → 0.95
- "Couldn't find documentation": 8 instances → 0 instances

### Step 5: Document Results

Create `reports/all-to-rust2/tool_integration_fix_results_nXXXX.md`:

**Must include:**
- Quality scores before vs after
- Example responses showing LLM now uses tool results
- Completeness improvement measurement
- Confirmation "couldn't find" messages eliminated

---

## SUCCESS CRITERIA

### Before Fix (Current):
- ❌ 8 instances of "couldn't find documentation"
- ❌ Completeness: 0.85 (judge says "could provide more examples")
- ❌ Overall quality: 0.90

### After Fix (Target):
- ✅ 0 instances of "couldn't find documentation" (when tool returns data)
- ✅ Completeness: 0.95+ (judge says "uses retrieved information well")
- ✅ Overall quality: 0.95+
- ✅ LLM says "Based on search results..." or similar

---

## WHY THIS MATTERS

**90% sounds good, but:**
- It's artificially low due to framework issue
- Real quality should be 95%+
- Users will notice "couldn't find documentation" errors
- Wastes tool calls if results aren't used

**This is a 5-minute fix** that improves quality by 5-10%.

---

## EXACT FILE TO MODIFY

**File:** `crates/dashflow/src/prebuilt.rs`

**Function:** `create_react_agent()`

**Look for:** The line that creates the system message/prompt

**Current (probably):**
```rust
let system_message = Message::system("You are a helpful assistant...");
```

**Change to:** (add detailed instructions about using tool results)

---

## TIMELINE

**N=1448:** Update system prompt in create_react_agent()
**N=1449:** Run single test to verify fix works
**N=1450:** Re-run all 16 tests with fix (~30 min)
**N=1451:** Create before/after quality comparison report

**Total:** 4 commits, ~2 hours AI time (including test execution)

**Expected result:** Quality improves from 90% → 95%+

---

## DIRECTIVE PRIORITY

**This is HIGH priority because:**
1. Tests revealed the issue (working as intended)
2. Fix is simple (update one string)
3. Impact is significant (5-10% quality improvement)
4. Users will notice the difference

**Execute after current documentation work (N=1448+)**

---

**The tests did their job - they found a real framework issue!** Now fix it.

- Manager AI
