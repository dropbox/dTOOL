# 🔴 CRITICAL: 33 Doctest Failures

**Date:** 2025-12-04 13:01
**Severity:** HIGH - Documentation examples don't work
**Impact:** Users copy broken code from docs
**Status:** NEWLY DISCOVERED

---

## 🚨 THE PROBLEM

**33 doctests FAILING:**
```bash
$ cargo test --doc -p dashflow
test result: FAILED. 395 passed; 33 failed; 263 ignored
```

**Root cause:** Import paths in doc examples are outdated after dashflow-core merge

### Example Error:

```rust
/// ```rust
/// use crate::core::messages::Message;  // ❌ FAILS
/// ```
```

**Error:**
```
error[E0433]: failed to resolve: use of undeclared crate or module `crate`
  |
5 | use crate::core::messages::Message;
  |            ^^^^ unresolved import
  |            help: a similar path exists: `dashflow::core`
```

---

## 📊 FAILED DOCTESTS (33 total)

**By file:**
- **agents.rs:** 4 failures (BufferMemory, ConversationBufferWindowMemory, FileCheckpoint, MemoryCheckpoint)
- **caches.rs:** 2 failures (module example, InMemoryCache)
- **output_parsers.rs:** 1 failure (module example)
- **rate_limiters.rs:** 4 failures (module, InMemoryRateLimiter, acquire, try_acquire)
- **tools.rs:** 15 failures (FunctionTool, StructuredTool, 11 builtin tools, sync_structured_tool)
- **utils.rs:** 3 failures (abatch_iterate examples)
- **knn.rs:** 1 failure (module example)
- **best_of_n.rs:** 1 failure (module example)
- **reducer.rs:** 2 failures (AddMessagesReducer, add_messages)

---

## 🔧 THE FIX

**Pattern:** Change doc example imports from `crate::` to `dashflow::`

### Before (BROKEN):

```rust
/// # Example
/// ```rust
/// use crate::core::messages::Message;
/// use crate::core::tools::Tool;
/// ```
```

### After (FIXED):

```rust
/// # Example
/// ```rust
/// use dashflow::core::messages::Message;
/// use dashflow::core::tools::Tool;
/// // Or use re-exports:
/// use dashflow::Message;
/// ```
```

---

## 📋 SYSTEMATIC FIX PLAN

### Step 1: Fix Core Module Imports (20 doctests)

**Files:**
- `crates/dashflow/src/core/agents.rs` (4 doctests)
- `crates/dashflow/src/core/caches.rs` (2 doctests)
- `crates/dashflow/src/core/output_parsers.rs` (1 doctest)
- `crates/dashflow/src/core/rate_limiters.rs` (4 doctests)
- `crates/dashflow/src/core/tools.rs` (15 doctests)
- `crates/dashflow/src/core/utils.rs` (3 doctests)

**Pattern replacement:**
```bash
# In each file:
sed -i '' 's/use crate::core::/use dashflow::core::/g' FILE
# Or better, manual review to use re-exports where possible
```

### Step 2: Fix Optimize Module Imports (2 doctests)

**Files:**
- `crates/dashflow/src/optimize/knn.rs` (1 doctest)
- `crates/dashflow/src/optimize/modules/best_of_n.rs` (1 doctest)

**Fix:**
```rust
// Change:
use crate::optimize::

// To:
use dashflow::optimize::
```

### Step 3: Fix Reducer Imports (2 doctests)

**File:**
- `crates/dashflow/src/reducer.rs` (2 doctests)

**Fix:**
```rust
// Change:
use crate::reducer::

// To:
use dashflow::reducer::
```

### Step 4: Verification

```bash
# Run all doctests
cargo test --doc -p dashflow

# Expected: 0 failed (all 428 should pass or be ignored)
```

---

## ⏱️ TIME ESTIMATE

**Total:** 2-3 hours
- Find and fix 33 doctests: 1.5-2 hours
- Verify all pass: 30 min
- Handle edge cases: 30 min

---

## 🎯 WORKER ASSIGNMENT

**Worker N=98:** Fix all 33 doctest failures

**Priority:** HIGH (users copy code from docs)

**Method:** Systematic replacement of import paths

**Success criteria:**
- [ ] cargo test --doc -p dashflow shows 0 failures
- [ ] All doc examples compile
- [ ] All doc examples run correctly

---

## 📊 IMPACT

### Why This Matters:

**Before fix:**
- User reads documentation
- Copies example code
- Code doesn't compile (import error)
- User frustrated, thinks framework is broken

**After fix:**
- User reads documentation
- Copies example code
- Code compiles and runs immediately
- Good user experience

**This is a QUALITY issue** affecting documentation usability.

---

**NEWLY DISCOVERED BUG. Add to queue. Fix systematically.**
