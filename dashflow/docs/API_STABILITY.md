# API Stability Policy

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Version:** 1.11.3
**Date:** 2025-12-22
**Status:** Active

---

## Purpose

This document defines DashFlow's API stability guarantees and deprecation process. Following these policies ensures:
- Predictable upgrade paths for applications
- Clear communication about API changes
- Minimal disruption to production systems

---

## Semantic Versioning

DashFlow follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH (e.g., 1.11.3)
```

### Version Semantics

| Version Type | Breaking Changes | New Features | Bug Fixes | Deprecations |
|--------------|------------------|--------------|-----------|--------------|
| **MAJOR (2.0)** | ✅ Allowed | ✅ Allowed | ✅ Allowed | ⚠️ May remove deprecated APIs |
| **MINOR (1.N)** | ❌ Not allowed | ✅ Allowed | ✅ Allowed | ✅ Allowed |
| **PATCH (1.N.P)** | ❌ Not allowed | ❌ Not allowed | ✅ Allowed | ❌ Not allowed |

### Breaking Changes

**Breaking changes require MAJOR version bump.**

Examples of breaking changes:
- Removing public API without deprecation period
- Changing function signatures
- Removing trait methods
- Changing error types
- Incompatible serialization format changes

**NOT considered breaking:**
- Adding new methods to traits with default implementations
- Adding new optional parameters (with defaults)
- Deprecating APIs (with compatibility shims)
- Internal implementation changes (private APIs)

---

## Stable API Guarantees

### What We Guarantee (v1.x Series)

These APIs are guaranteed stable across all v1.x releases:

#### DashFlow APIs

**StateGraph<S>** (Core Construction API):
```rust
// Construction
pub fn new() -> Self
pub fn builder() -> Self

// Node Management
pub fn add_node(&mut self, name: impl Into<String>, node: impl Node<S>) -> &mut Self
pub fn add_node_from_fn<F>(&mut self, name: impl Into<String>, func: F) -> &mut Self

// Edge Management
pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self
pub fn add_conditional_edges<F>(...) -> &mut Self
pub fn add_parallel_edges(&mut self, from: impl Into<String>, to: Vec<String>) -> &mut Self

// Configuration
pub fn set_entry_point(&mut self, node: impl Into<String>) -> &mut Self

// Compilation
pub fn compile(self) -> Result<CompiledGraph<S>>

// Utilities
pub fn validate(&self) -> Vec<String>
pub fn to_mermaid(&self) -> String
```

**CompiledGraph<S>** (Execution API):
```rust
// Execution
pub fn stream(&self, initial_state: S) -> impl Stream<Item = Result<StreamEvent<S>>>

// Configuration Builders
pub fn with_name(self, name: impl Into<String>) -> Self
pub fn with_callback<C>(self, callback: C) -> Self
pub fn with_checkpointer<C>(self, checkpointer: C) -> Self
pub fn with_thread_id(self, thread_id: impl Into<ThreadId>) -> Self
pub fn with_scheduler(self, scheduler: WorkStealingScheduler<S>) -> Self
pub fn with_timeout(self, timeout: Duration) -> Self
pub fn with_node_timeout(self, timeout: Duration) -> Self

// Introspection
pub fn metrics(&self) -> ExecutionMetrics
pub fn entry_point(&self) -> &str
pub fn node_count(&self) -> usize
pub fn edge_count(&self) -> usize
```

#### Core Traits

**GraphState** (User State Definition):
```rust
pub trait GraphState: Clone + Send + Sync + 'static {
    fn merge(&mut self, other: Self);
}
```

**Node** (Node Implementation):
```rust
pub trait Node<S: GraphState>: Send + Sync {
    fn execute(&self, state: S) -> BoxFuture<'static, Result<S>>;
}
```

**Checkpointer** (State Persistence):
```rust
pub trait Checkpointer<S: GraphState>: Send + Sync {
    fn save(&self, thread_id: &ThreadId, state: &S) -> BoxFuture<'static, Result<()>>;
    fn load(&self, thread_id: &ThreadId) -> BoxFuture<'static, Result<Option<S>>>;
}
```

### What We Don't Guarantee

**No stability guarantee for:**
- Internal/private modules (not in public API)
- APIs marked `#[doc(hidden)]`
- APIs marked `unstable` in documentation
- Debug implementations (format may change)
- Exact error messages (wording may improve)

---

## Deprecation Process

### Standard Deprecation Timeline

When changing public APIs, we follow this process:

#### Phase 1: Deprecate (v1.N)
- Add new API
- Mark old API `#[deprecated]`
- Provide compatibility shim (old API delegates to new)
- Update documentation
- Add migration guide

#### Phase 2: Maintain (v1.N → v1.N+K)
- Keep deprecated API functional
- Minimum retention: **2 MINOR versions** or **6 months** (whichever longer)
- Continue showing deprecation warnings

#### Phase 3: Remove (v2.0 earliest)
- Remove deprecated API in next MAJOR version
- Document in breaking changes section
- Provide automated migration tool if possible

### Example: add_conditional_edge → add_conditional_edges

**v1.0:** Only `add_conditional_edge` (singular)
```rust
graph.add_conditional_edge(...)
```

**v1.1:** Deprecate old, add new
```rust
// Old API (deprecated, still works)
#[deprecated(since = "1.1.0", note = "Use `add_conditional_edges` (plural) instead")]
pub fn add_conditional_edge<F>(...) -> &mut Self {
    self.add_conditional_edges(...)  // Shim to new API
}

// New API
pub fn add_conditional_edges<F>(...) -> &mut Self {
    // Implementation
}
```

**v1.1 - v1.11:** Both APIs work
- Apps using old API: compile with warnings, still functional
- Apps using new API: no warnings
- Migration at app's convenience

**v2.0:** Remove old API
- Only `add_conditional_edges` available
- Apps must update before v2.0 upgrade
- Migration guide provided

---

## Stability Tiers

We classify APIs into stability tiers:

### Tier 1: Stable ✅
**Guarantee:** Never breaks across v1.x
**Examples:** StateGraph core methods, CompiledGraph, GraphState trait
**Migration:** Only in MAJOR versions (v2.0+)

### Tier 2: Evolving ⚠️
**Guarantee:** May change with deprecation period
**Examples:** Advanced features, optimization APIs
**Migration:** Via deprecation process (shims provided)
**Notice:** Minimum 2 MINOR versions warning

### Tier 3: Experimental 🧪
**Guarantee:** May change without deprecation
**Examples:** Features marked `#[doc = "experimental"]`
**Migration:** Follow release notes
**Notice:** Clearly marked in documentation

### How to Identify Tiers

```rust
// Tier 1: Stable (no markers)
pub fn add_node(&mut self, ...) -> &mut Self

// Tier 2: Evolving (deprecated marker)
#[deprecated(since = "1.6.0", note = "Use new_api instead")]
pub fn old_api(&mut self, ...) -> &mut Self

// Tier 3: Experimental (doc marker)
#[doc = "⚠️ Experimental: API may change without notice"]
pub fn experimental_feature(&mut self, ...) -> &mut Self
```

---

## Migration Guides

### Where to Find Migration Information

1. **CHANGELOG.md** - Summary of changes per version
2. **docs/MIGRATION_v{OLD}_to_v{NEW}.md** - Detailed migration guides
3. **Deprecation warnings** - Compiler output with guidance
4. **Release notes** - Breaking changes highlighted

### Current Migration Guides

- [Comprehensive Migration Guide](MIGRATION_GUIDE.md) - **All deprecated APIs with migration paths**
- [v1.0 to v1.6](MIGRATION_v1.0_to_v1.6.md) - Covers singular → plural API change (legacy)

---

## Compatibility Testing

### How We Verify Compatibility

**Automated Tests:**
```rust
// crates/dashflow/tests/v1_0_compatibility.rs
#[test]
fn v1_0_api_works_in_v1_11() {
    // Test that v1.0 code compiles and runs correctly
    let mut graph = StateGraph::new();
    #[allow(deprecated)]
    graph.add_conditional_edge(...);  // v1.0 API
    // Verify execution matches v1.0 behavior
}
```

**Manual Verification:**
- Example programs using old APIs (in `examples/`)
- Compilation checks (warnings OK, errors not OK)
- Behavioral equivalence tests

---

## Version Support Policy

### Active Support

| Version | Status | Support Until | Security Fixes | Bug Fixes |
|---------|--------|---------------|----------------|-----------|
| **v1.11.x** | Active | Current | ✅ Yes | ✅ Yes |
| **v1.10.x** | Maintenance | v1.12.0 release | ✅ Yes | ⚠️ Critical only |
| **v1.9.x** | Unsupported | Ended | ❌ No | ❌ No |

**Policy:**
- Latest MINOR version: Full support
- Previous MINOR version: Security + critical bug fixes only
- Older versions: Unsupported (please upgrade)

---

## API Design Principles

### Designing for Stability

**1. Prefer Extensibility Over Perfection**
```rust
// Good: Can add fields without breaking
pub struct ConfigBuilder { /* fields private */ }

impl ConfigBuilder {
    pub fn with_option(mut self, opt: Opt) -> Self { /* ... */ }
    // Can add new with_* methods without breaking
}

// Bad: Adding parameters breaks callers
pub fn configure(opt1: T1, opt2: T2) -> Config { /* ... */ }
```

**2. Use Builder Pattern for Complex APIs**
```rust
// Extensible: new options don't break existing code
let graph = StateGraph::builder()
    .add_node("start", start_fn)
    .add_edge("start", "end")
    .compile()?;
```

**3. Traits with Default Implementations**
```rust
pub trait MyTrait {
    fn required_method(&self) -> String;

    // Adding new methods with defaults doesn't break implementors
    fn optional_method(&self) -> bool {
        true  // Default implementation
    }
}
```

**4. Non-Exhaustive Enums**
```rust
#[non_exhaustive]
pub enum Event {
    Started,
    Completed,
    // Can add variants without breaking match statements
}
```

---

## Reporting API Issues

### If You Encounter Breaking Changes

**Please report if:**
- API changed without deprecation warning
- Deprecated API removed before policy timeline
- Compatibility shim doesn't work correctly
- Migration guide missing or incorrect

**How to report:**
1. Open issue: https://github.com/dropbox/dTOOL/dashflow/issues
2. Label: `api-stability`
3. Include:
   - Old version and new version
   - Code that broke
   - Expected behavior vs actual behavior

---

## Future Considerations

### When We Might Break Compatibility

**MAJOR version (v2.0) allowed to break IF:**
1. Security vulnerability requires API change
2. Fundamental design flaw discovered
3. Rust edition change requires adaptation
4. Ecosystem shift (e.g., tokio 2.0 → 3.0)

**Process:**
1. Announce breaking changes 6+ months in advance
2. Provide automated migration tooling
3. Document all changes comprehensively
4. Offer support during transition

---

## Appendix: Historical API Changes

### v1.0 → v1.1 Changes (Graph API)

| Old API | New API | Version | Status | Notes |
|---------|---------|---------|--------|-------|
| `add_conditional_edge` | `add_conditional_edges` | v1.1 | Deprecated | Plural form, shim provided |
| `add_parallel_edge` | `add_parallel_edges` | v1.1 | Deprecated | Plural form, shim provided |

### v1.0.1 Changes (Chat Model Construction)

| Old API | New API | Crates | Status |
|---------|---------|--------|--------|
| `ChatOpenAI::new()` | `build_chat_model(&config)` | dashflow-openai | Deprecated |
| `ChatAzureOpenAI::new()` | `build_chat_model(&config)` | dashflow-azure-openai | Deprecated |
| `ChatFireworks::new()` | `build_chat_model(&config)` | dashflow-fireworks | Deprecated |
| `ChatXAI::new()` | `build_chat_model(&config)` | dashflow-xai | Deprecated |
| `ChatPerplexity::new()` | `build_chat_model(&config)` | dashflow-perplexity | Deprecated |

### v1.1.0 Changes (Streaming Codec)

| Old API | New API | Status | Notes |
|---------|---------|--------|-------|
| `decode_message_with_decompression()` | `decode_message_strict()` | Deprecated | Security: legacy lacks header validation |
| `decode_message_with_decompression_and_limit()` | `decode_message_strict()` | Deprecated | Security: legacy lacks header validation |

### v1.9.0 Changes (Tool Binding)

| Old API | New API | Status | Notes |
|---------|---------|--------|-------|
| Provider-specific `with_tools()` | `ChatModelToolBindingExt::bind_tools()` | Deprecated | Unified, type-safe tool binding |

### v1.11.0 Changes (Kafka Consumer)

| Old API | New API | Status | Notes |
|---------|---------|--------|-------|
| `ConsumerConfig.group_id` | (removed) | Deprecated | rskafka uses partition-based consumption |
| `ConsumerConfig.session_timeout_ms` | (removed) | Deprecated | rskafka uses partition-based consumption |

### v1.11.3 Changes (Cost & Trace Modules)

| Old Module | New Module | Status |
|------------|------------|--------|
| `dashflow::optimize::cost_monitoring::*` | `dashflow_observability::cost::*` | Deprecated |
| `dashflow::optimize::trace_types::TraceEntry` | `ExecutionTrace::to_trace_entries()` | Deprecated |
| `dashflow::optimize::trace::TraceCollector` | `ExecutionTrace` + `ExecutionTraceBuilder` | Deprecated |

### External API Deprecations

| Crate | Status | Notes |
|-------|--------|-------|
| `dashflow-zapier` | Deprecated v1.0.0 (removed) | Zapier NLA API sunset 2023-11-17 |

**Full migration details:** See [MIGRATION_GUIDE.md](MIGRATION_GUIDE.md)

**Tested:** See `crates/dashflow/examples/v1_0_legacy_api.rs`

---

**Document History:**
- 2025-12-22: Updated with all deprecations through v1.11.3
  - Added comprehensive deprecation tables
  - Linked to new MIGRATION_GUIDE.md
- 2025-11-10: Initial version
  - Created as part of Phase 2 (API Stability) completion
  - Documents existing deprecation practices
  - Establishes formal stability policy

**Maintainers:** DashFlow Core Team
**Review Cycle:** Updated with each MAJOR or MINOR release
