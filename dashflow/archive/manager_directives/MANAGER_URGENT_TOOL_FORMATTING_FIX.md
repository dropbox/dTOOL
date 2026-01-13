# MANAGER URGENT: Tool Message Formatting Fix (Completeness 0.0 → 0.95)

**Date:** January 13, 2025
**Priority:** CRITICAL - System Prompt Alone Not Enough

---

## TEST RESULTS: System Prompt Fix Insufficient

**With stricter judge + system prompt fix:**
- LLM STILL says: "I couldn't retrieve any specific documentation"
- Judge correctly scores: Completeness = 0.0 (was 0.85, now properly detected!)
- Test FAILS (correctly) ✅

**This means:** System prompt is being ignored or tool results are not prominent enough

---

## ROOT CAUSE: Tool Message Format Too Subtle

**File:** `crates/dashflow/src/integration.rs:auto_tool_executor()`

**Current tool message format** (probably):
```rust
let tool_message = Message::tool(
    tool_result,  // Just the raw result
    tool_call_id
);
```

**LLM sees:**
```
[Tool message]
content: "Tokio: An async runtime for Rust..."
```

**Problem:** Too subtle! LLM doesn't recognize this as "retrieved documentation"

---

## THE FIX: Make Tool Results OBVIOUS

**Update `auto_tool_executor()` to format results prominently:**

```rust
// After getting tool_result
let formatted_result = format!(
    "=== SEARCH RESULTS ===\n\
     Tool: {}\n\
     Query: {}\n\n\
     Retrieved Information:\n\
     {}\n\
     === END SEARCH RESULTS ===\n\n\
     IMPORTANT: You MUST use this retrieved information in your response.",
    tool_call.name,
    tool_call.args.get("query").and_then(|v| v.as_str()).unwrap_or("[query]"),
    tool_result
);

let tool_message = Message::tool(
    formatted_result,  // Formatted, not raw
    tool_call_id.clone()
);
```

**LLM will now see:**
```
=== SEARCH RESULTS ===
Tool: search_documents
Query: tokio

Retrieved Information:
Tokio: An async runtime for Rust. Provides async I/O, timers, and task scheduling.
=== END SEARCH RESULTS ===

IMPORTANT: You MUST use this retrieved information in your response.
```

**Much harder to ignore!**

---

## IMPLEMENTATION

**File to modify:** `crates/dashflow/src/integration.rs`
**Function:** `auto_tool_executor()`
**Line:** ~440-450 (where tool message is created)

**Find this code:**
```rust
let tool_message = Message::tool(tool_result, tool_call_id.clone());
tool_messages.push(tool_message);
```

**Replace with:**
```rust
// Format tool result to be obvious to LLM
let formatted_result = format!(
    "=== SEARCH RESULTS FROM TOOL '{}' ===\n\n\
     {}\n\n\
     === END RESULTS ===\n\n\
     You MUST use this information in your response.",
    tool_call.name,
    tool_result
);

let tool_message = Message::tool(formatted_result, tool_call_id.clone());
tool_messages.push(tool_message);
```

---

## TEST THE FIX

**Run the failing test:**
```bash
cargo test --package document_search \
  --test multi_turn_conversations \
  --features dashstream \
  test_error_recovery \
  -- --ignored --nocapture
```

**Expected:**
- Turn 2: LLM should now say "Based on the search results, Tokio is..."
- Completeness: 0.0 → 0.95+
- Test: FAILS → PASSES

---

## SUCCESS CRITERIA

**Before fix (current):**
- LLM: "I couldn't retrieve any specific documentation"
- Completeness: 0.0
- Test: FAILS

**After fix (target):**
- LLM: "Based on the search results, Tokio is..."
- Completeness: 0.95+
- Test: PASSES

---

**This is the REAL fix. System prompt + prominent formatting = LLM will use tool results.**

Execute immediately!

- Manager AI
