# WORKER: Your Next Task (Unambiguous)

**Date:** November 11, 2025
**Current:** N=1227
**Your Next:** N=1228

---

## ONE CLEAR DIRECTIVE

**User priority:**
> "Get the complete DashFlow and DashFlow patterns implemented first. Do NOT use workarounds!"

**Translation:**
1. Build core framework features (bind_tools, create_react_agent, etc.)
2. Remove workarounds from apps
3. Use proper Python DashFlow patterns
4. THEN test rigorously

**NOT the reverse.**

---

## Your N=1228: TODO Audit

**Task:** Find all missing core features

**Command:**
```bash
cd ~/dashflow

# Find all TODOs
rg "TODO|FIXME" --type rust crates/ > reports/all-to-rust2/todo_audit_n1228.txt

# Count them
wc -l reports/all-to-rust2/todo_audit_n1228.txt
```

**Categorize each TODO:**

**Category A: STALE (feature exists, TODO not removed)**
- Example: "TODO: with_structured_output" but it exists at N=1199

**Category B: CORE FEATURES (must implement)**
- bind_tools() - Critical
- create_react_agent() - Critical
- tool_choice parameter - Critical
- Streaming - Important
- [Others from Python baseline]

**Category C: INFRASTRUCTURE (defer)**
- LangServe TODOs (separate work)
- WASM executor metrics
- Remote node observability

**Category D: NICE-TO-HAVE (defer)**
- Non-critical enhancements

**Create:** `reports/all-to-rust2/core_features_gap_analysis_n1228.md`

**Structure:**
```markdown
# Core Features Gap Analysis

## Category B: Must Implement (Priority)

### 1. bind_tools() Method
**Python baseline:**
[code example from ~/dashflow]

**Our Rust:** Missing

**Priority:** CRITICAL
**Estimate:** 3 commits

### 2. create_react_agent() Helper
**Python baseline:**
[code example]

**Our Rust:** Missing

**Priority:** CRITICAL
**Estimate:** 2 commits

[List all Category B items]

## Implementation Plan
1. bind_tools() (N=1229-1231)
2. create_react_agent() (N=1232-1233)
3. tool_choice (N=1234)
4. [Others]

Total estimate: 15 commits
```

**Commit:**
```bash
git add reports/all-to-rust2/
git commit -m "# 1228: Core features gap analysis - 12 must-implement features identified

Audited all TODO/FIXME markers in codebase.

Category B (Must Implement): 12 features
- bind_tools() method (CRITICAL)
- create_react_agent() helper (CRITICAL)
- tool_choice parameter (CRITICAL)
- [List others]

Category A (Stale): 8 TODOs to remove
- with_structured_output TODO (implemented N=1199)
- [Others]

Implementation plan: 15 commits (N=1229-1242)

Next: Implement bind_tools()"
```

---

## Your N=1229-1231: Implement bind_tools()

**This is THE core feature for tool calling.**

### N=1229: Check Python Baseline

**Commands:**
```bash
cd ~/dashflow
rg "def bind_tools" libs/dashflow-core/ -A 20 | head -50

# Document exact API signature
# Document behavior
# Document tool schema format
```

**Create design doc:** `crates/dashflow/design/BIND_TOOLS_DESIGN.md`

**Commit:** Design documented

---

### N=1230: Implement bind_tools() in ChatModel

**File:** `crates/dashflow/src/language_models/chat_model.rs`

**Add method:**
```rust
pub trait ChatModel {
    // ... existing methods

    /// Bind tools to this model (Python DashFlow pattern)
    fn bind_tools(self, tools: Vec<Arc<dyn Tool>>) -> ChatModelWithTools
    where Self: Sized + 'static {
        ChatModelWithTools::new(Arc::new(self), tools)
    }
}
```

**Create wrapper:** `crates/dashflow/src/language_models/with_tools.rs`

**Implement ChatModelWithTools:**
- Wraps base model
- Stores tools
- Converts tools to schemas
- Sends schemas to LLM
- Returns responses with tool_calls

**Tests:** 8+ tests

**Commit:** "# 1230: Implement bind_tools() - Core tool calling infrastructure"

---

### N=1231: Update OpenAI to Support bind_tools()

**Ensure ChatOpenAI works with bind_tools:**
- Tool schemas in OpenAI format
- Proper tool_calls parsing
- Works with GPT-4

**Test with real API:**
```bash
export OPENAI_API_KEY=$(grep OPENAI_API_KEY ~/dashflow/.env | cut -d '=' -f 2)
cargo test -p dashflow-openai bind_tools -- --ignored
```

**Commit:** "# 1231: bind_tools() working with OpenAI"

---

## Your N=1232-1233: Implement create_react_agent()

### N=1232: Create ReAct Agent Helper

**File:** `crates/dashflow/src/prebuilt/react.rs`

**Implement:**
```rust
pub fn create_react_agent<M: ChatModel + Clone + 'static>(
    model: M,
    tools: Vec<Arc<dyn Tool>>,
) -> CompiledGraph<MessagesState> {
    // Build graph:
    // - Agent node (calls LLM with tools bound)
    // - Tools node (executes tool_calls)
    // - Conditional edge (continue if tool_calls, else END)
    // - Max iterations (default 15)
}
```

**Match Python API exactly**

**Tests:** 10+ tests

**Commit:** "# 1232: Implement create_react_agent() helper"

---

### N=1233: Verify create_react_agent() with Real Tools

**Test with:**
- Calculator tool
- Search tool
- Multiple tools

**Commit:** "# 1233: create_react_agent() validated with real tools"

---

## Your N=1234-1235: Update Apps (NO WORKAROUNDS)

### N=1234: Update App1 to Use Proper APIs

**Current (workaround):**
```rust
// Manual tool checking (150 lines)
```

**Replace with:**
```rust
// Proper API (30 lines)
let tools = vec![Arc::new(retriever_tool)];
let agent = create_react_agent(
    ChatOpenAI::new().bind_tools(tools.clone()),
    tools
);
```

**Verify:** App works better with proper API

**Commit:** "# 1234: App1 refactored - using bind_tools() and create_react_agent()"

---

### N=1235: Update Apps 2 and 3

**Same refactor for both apps**

**Commit:** "# 1235: Apps 2 and 3 refactored - no more workarounds"

---

## Your N=1236-1237: Enable Standard Tests

**File:** `crates/dashflow-standard-tests/src/chat_model_tests.rs`

**Remove TODOs, implement tests:**
```rust
// Change from:
pub async fn test_bind_tools<T: ChatModel>(model: &T) {
    // TODO: Implement when tool calling infrastructure is ready
}

// To:
pub async fn test_bind_tools<T: ChatModel + Clone>(model: T) {
    let tool = Arc::new(search_tool());
    let llm = model.bind_tools(vec![tool]);
    let result = llm.generate(...).await.unwrap();
    assert!(result.has_tool_calls());
}
```

**Implement all 8 standard tests**

**Run against OpenAI, Anthropic, etc.**

**Commit:** "# 1236-1237: Standard tests enabled - all 8 tests implemented"

---

## Your N=1238-1242: Remaining Core Features (5 commits)

**Based on your N=1228 audit, implement other Category B features**

**Likely includes:**
- Streaming improvements
- Tool error handling
- Any other Python patterns not yet implemented

---

## THEN: Rigorous Testing (N=1243-1260)

**With proper APIs implemented:**
- Apps use bind_tools() and create_react_agent()
- No workarounds
- Testing validates real framework
- 60+ diverse queries
- Multi-turn conversations
- Verify logging

---

## Clear Priority Order

1. ✅ **N=1228:** Audit TODOs (find missing features)
2. ✅ **N=1229-1233:** Implement bind_tools() + create_react_agent() (5 commits)
3. ✅ **N=1234-1235:** Update apps to use proper APIs (2 commits)
4. ✅ **N=1236-1237:** Enable standard tests (2 commits)
5. ✅ **N=1238-1242:** Implement remaining features (5 commits)
6. ✅ **N=1243-1260:** Rigorous testing (18 commits)

**Total: 32 commits**

---

## Is This Clear?

**Yes or No:**
- Build complete Python DashFlow patterns in Rust ✅
- Remove all workarounds from apps ✅
- Use proper APIs (bind_tools, create_react_agent) ✅
- THEN do rigorous testing ✅

**Start with N=1228: TODO audit**

**That's your next task. Simple and clear.**
