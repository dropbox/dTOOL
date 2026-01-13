# DashFlow Platform Roadmap: Coding Agent Support

**Date:** 2025-12-06
**Status:** Complete (All Phases Implemented)
**Validated By:** codex_dashflow port (3,300+ tests, production-ready)

---

## Executive Summary

The codex_dashflow port has validated DashFlow as a viable platform for building production coding agents. However, 10 gaps were identified that required custom implementation. This roadmap proposes platform improvements to make DashFlow the best framework for building AI coding assistants.

---

## Phase 1: Core Agent Infrastructure (P0) ✅ COMPLETE

### 1.1 dashflow-context: Context Window Management ✅ COMPLETE

**Priority:** P0 - Critical
**Estimated Effort:** 1 commit
**Status:** Complete (1 commit, #197)

**Features:**
- Token counting (tiktoken-rs for OpenAI, model-specific counters) ✅
- Truncation strategies (drop-oldest, sliding-window, keep-first-and-last) ✅
- Budget tracking with reserved response space ✅
- Model-specific context limits (20+ models supported) ✅

**Implementation:**
- `ContextManager` with builder pattern
- `TruncationStrategy` enum for configurable behavior
- `FitResult` with token counts and metadata
- 12 unit tests + 1 doc test

**API:**
```rust
use dashflow_context::{ContextManager, TruncationStrategy};

let manager = ContextManager::builder()
    .model("gpt-4o")  // Auto-detects 128k limit
    .reserve_tokens(4000)  // For response
    .truncation(TruncationStrategy::DropOldest)
    .build();

let result = manager.fit(&messages);
println!("Token count: {}, Dropped: {}", result.token_count, result.messages_dropped);
```

---

### 1.2 dashflow-git-tool: Git Integration ✅ COMPLETE

**Priority:** P0 - Critical
**Estimated Effort:** 1 commit
**Status:** Complete (1 commit, #197)

**Features:**
- Repository detection (discover from any subdirectory) ✅
- Recent commits (with optional stats) ✅
- Uncommitted changes (staged/unstaged/untracked) ✅
- Branch information + ahead/behind counts ✅
- Diff generation (between refs, working tree) ✅
- GitInfoTool for DashFlow Tool trait integration ✅

**Implementation:**
- `GitTool` for low-level git operations (libgit2)
- `GitContext` for LLM-friendly context collection
- `GitInfoTool` implementing Tool trait
- 13 unit tests + 1 doc test

**API:**
```rust
use dashflow_git_tool::{GitTool, GitContextOptions};

let git = GitTool::discover(working_dir)?;
let context = git.collect_context(GitContextOptions {
    max_commits: 10,
    include_diff: true,
    ..Default::default()
})?;

// For LLM prompts
let prompt_text = context.to_prompt_string();
```

---

### 1.3 Shell Tool Safety Enhancement ✅ COMPLETE

**Priority:** P0 - Critical
**Estimated Effort:** 1 commit
**Status:** Complete (1 commit, #197)

**Features:**
- Command analysis with regex patterns ✅
- Dangerous pattern detection (filesystem, network, system) ✅
- Severity levels (Safe, Unknown, Dangerous, Forbidden) ✅
- Approval callback hooks ✅
- Allowlist/blocklist configuration ✅
- SafeShellTool for secure command execution ✅

**Implementation:**
- `CommandAnalyzer` for pattern-based safety analysis
- `SafetyConfig` with restrictive/permissive presets
- `SafeShellTool` with approval callback support
- 35 unit tests + 10 doc tests

**API:**
```rust
use dashflow_shell_tool::{SafeShellTool, SafetyConfig, Severity};
use std::sync::Arc;

let tool = SafeShellTool::new(SafetyConfig::restrictive())
    .with_approval_callback(Arc::new(|cmd, severity| {
        severity <= Severity::Unknown  // Auto-approve safe commands
    }));

let analysis = tool.analyze("rm -rf /tmp/test");
println!("Severity: {}, Reasons: {:?}", analysis.severity, analysis.reasons);
```

---

## Phase 2: Developer Experience (P1)

### 2.1 Alternative Streaming Backends ✅ COMPLETE

**Priority:** P1 - Important
**Estimated Effort:** 4-6 commits
**Status:** Complete (1 commit, #193)

Add backends beyond Kafka:
- `InMemoryBackend` - For testing ✅
- `FileBackend` - For local dev (JSONL files) ✅
- `SqliteBackend` - For simple persistence ✅

**Implementation:**
- `StreamBackend` trait for backend abstraction
- `StreamProducer` / `StreamConsumer` traits for message handling
- 37 unit tests covering all backends

**Why Important:**
Kafka requirement is a barrier to adoption. Most users just want `dashstream tail`.

---

### 2.2 dashflow-project: Project Context Discovery ✅ COMPLETE

**Priority:** P1 - Important
**Estimated Effort:** 2-3 commits
**Status:** Complete (1 commit, #194)

**Features:**
- Documentation discovery (README, AGENTS.md, CLAUDE.md) ✅
- Language/framework detection ✅
- Build system identification ✅
- Project structure analysis ✅

**Implementation:**
- `ProjectContext` struct with rich project metadata
- `discover_project()` async function for comprehensive scanning
- 14 languages, 23 frameworks, 18 build systems supported
- 17 unit tests

**Why Important:**
codex_dashflow implemented `project_doc.rs` (~200 lines). Coding agents need project context.

---

### 2.3 Tool Derive Macro ✅ COMPLETE

**Priority:** P1 - Important
**Estimated Effort:** 3-4 commits
**Status:** Complete (1 commit, #195)

**Before (verbose):**
```rust
// 50+ lines per tool definition
ToolDefinition {
    name: "read_file".to_string(),
    description: "Read contents of a file".to_string(),
    parameters: serde_json::json!({ ... }),
}
```

**After (with derive macro):**
```rust
#[derive(DashFlowTool)]
#[tool(name = "read_file", description = "Read contents of a file")]
struct ReadFile {
    /// Path to the file to read
    path: String,

    /// Maximum lines to return
    #[arg(default = 1000)]
    max_lines: Option<u32>,
}
```

**Implementation:**
- Added `DashFlowTool` derive macro to dashflow-derive crate
- Auto-generates `Tool` trait implementation from struct definition
- Doc comments become parameter descriptions
- `#[arg(default = ...)]` for optional parameters
- `from_input()` helper for parsing tool input to struct
- JSON schema generation for language model integration

---

## Phase 3: Advanced Features (P2)

### 3.1 StateGraph Debugging ✅ COMPLETE

**Priority:** P2 - Advanced
**Estimated Effort:** 1 commit
**Status:** Complete (1 commit, #196)

**Features:**
- Mermaid diagram export with configurable styling ✅
- Execution tracing with state snapshots ✅
- Edge decision explanations via TracingCallback ✅
- GraphStructure type for flexible Mermaid generation

**Implementation:**
- `debug` module with MermaidExporter, ExecutionTracer, TracingCallback
- MermaidConfig for customizable diagram output
- 8 unit tests

### 3.2 Built-in Approval Flow ✅ COMPLETE

**Priority:** P2 - Advanced
**Estimated Effort:** 1 commit
**Status:** Complete (N=199)

**Features:**
- `ApprovalNode` graph node type implementing Node trait ✅
- Blocking execution until user responds via ApprovalChannel ✅
- Timeout handling with configurable Duration ✅
- `ApprovalRequest` with RiskLevel (Low/Medium/High/Critical) ✅
- `ApprovalResponse` with approve/deny and optional reason ✅
- `AutoApprovalPolicy` for testing (Never/LowRiskOnly/MediumAndBelow/Always) ✅
- `auto_approval_handler()` for automated processing ✅
- 22 unit tests

**API:**
```rust
use dashflow::approval::{ApprovalChannel, ApprovalNode, ApprovalRequest, RiskLevel};

let (channel, mut receiver) = ApprovalChannel::new();

let node = ApprovalNode::new("approve_command", |state: &MyState| {
    ApprovalRequest::new(format!("Execute: {}", state.command))
        .with_risk_level(RiskLevel::High)
        .with_timeout(Duration::from_secs(30))
}).with_channel(channel);

// In approval handler:
if let Some(pending) = receiver.recv().await {
    pending.approve_with_reason("User confirmed");
}
```

### 3.3 SQLite Checkpointer ✅ COMPLETE

**Priority:** P2 - Advanced
**Estimated Effort:** 1 commit
**Status:** Complete (1 commit, #196)

**Features:**
- Lightweight persistence ✅
- Single-file deployment ✅
- WAL mode for concurrent access ✅
- Migration support ✅
- In-memory mode for testing ✅

**Implementation:**
- `SqliteCheckpointer` in checkpoint::sqlite module
- Millisecond-precision timestamps for proper ordering
- 6 unit tests

---

## Validation Strategy

Each phase will be validated against codex_dashflow:

1. **Replace custom code** with new platform features
2. **Verify tests still pass** (currently 3,300+)
3. **Measure code reduction** (target: 50%+ reduction in custom code)
4. **Document integration patterns**

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Custom code in codex_dashflow | ~3,000 lines | <1,500 lines |
| Time to build new coding agent | Days | Hours |
| Streaming setup complexity | Kafka required | File/SQLite option |
| Tool definition lines | 50+ per tool | 10 per tool |

---

## Implementation Status

| Phase | Items | Status | Impact |
|-------|-------|--------|--------|
| Phase 1 | Context, Git, Safety | ✅ COMPLETE | Critical - enables coding agents |
| Phase 2 | Streaming, Project, Macros | ✅ COMPLETE | Important - DX improvement |
| Phase 3 | Debug, Approval, SQLite | ✅ COMPLETE | Nice-to-have - polish |

---

## Conclusion

The codex_dashflow port proves DashFlow can support production coding agents. **All P0, P1, and P2 features are now complete.** DashFlow now provides:

- **Context Management**: Token counting, truncation strategies, budget tracking
- **Git Integration**: Repository detection, commits, diffs, context collection
- **Shell Safety**: Command analysis, severity levels, approval callbacks
- **Streaming Backends**: In-memory, file, SQLite (no Kafka required)
- **Project Discovery**: Language/framework detection, documentation discovery
- **Tool Derive Macro**: Simplified tool definition from structs
- **StateGraph Debugging**: Mermaid export, execution tracing
- **SQLite Checkpointer**: Lightweight persistence
- **Built-in Approval Flow**: ApprovalNode, risk levels, timeout handling, auto-approval policies

**Status**: All phases complete. Coding Agent Support roadmap is finished.
