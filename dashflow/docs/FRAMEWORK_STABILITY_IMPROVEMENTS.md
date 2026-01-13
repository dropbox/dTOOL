# Framework Improvements for Upgrade Safety

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

> **Status:** PROPOSAL - This document describes proposed improvements that have NOT been implemented.
> The crate references (dashflow-facade, dashflow-compat) are proposals, not existing crates.

**Problem:** Apps that upgrade from v1.0 → v1.6 face breaking changes
**Goal:** Make framework easier to build on without tight coupling
**Principle:** Apps should upgrade with zero code changes (patch), minimal changes (minor)

---

## Current Issues (Based on v1.0 → v1.6 Pain)

### Issue 1: Direct API Surface Too Large

**Problem:**
```rust
// App depends on many framework methods
graph.add_node_from_fn(...)
graph.add_conditional_edges(...)
graph.set_entry_point(...)
graph.compile()
// 20+ methods × API changes = upgrade nightmare
```

**If ANY method changes, apps break**

---

### Issue 2: No Compatibility Shims

**Problem:**
```rust
// v1.0 → v1.1 broke this:
graph.add_conditional_edge(...)  // Removed!

// No shim provided, apps must change immediately
```

---

### Issue 3: Framework Types Leak

**Problem:**
```rust
// App forced to use framework types
pub struct MyApp {
    graph: CompiledGraph<MyState>,  // Framework type in app!
}
// When CompiledGraph API changes, MyApp breaks
```

---

## Proposed Framework Improvements

### Improvement 1: Stable Facade Layer

**Add to framework:**
```rust
// crates/dashflow-facade/src/lib.rs

/// Stable facade over DashFlow - guaranteed backward compatibility
pub struct StableGraph<S> {
    inner: StateGraph<S>,
}

impl<S: GraphState> StableGraph<S> {
    pub fn new() -> Self {
        Self { inner: StateGraph::new() }
    }

    /// Stable method - will never change signature
    pub fn add_node(&mut self, name: impl Into<String>, node: impl FnOnce(S) -> BoxFuture<Result<S>>) {
        // Adapt to whatever current StateGraph API is
        self.inner.add_node_from_fn(name, node);
    }

    /// Stable method - v1.0 API compatibility
    #[deprecated(since = "1.1.0", note = "Use add_edges (plural) - this shim provided for v1.0 compatibility")]
    pub fn add_conditional_edge(&mut self, ...) {
        // Shim to new API
        self.inner.add_conditional_edges(...)
    }

    /// Stable method - v1.1+ API
    pub fn add_edges(&mut self, ...) {
        self.inner.add_conditional_edges(...)
    }
}
```

**Benefits:**
- Apps use StableGraph, not StateGraph directly
- Facade provides compatibility shims
- Framework can evolve underneath
- Apps keep working

---

### Improvement 2: Builder Pattern (Breaking → Non-Breaking)

**Current (Fragile):**
```rust
// This breaks if parameters change
let app = graph.compile()?;
```

**Improved (Extensible):**
```rust
// Add parameters without breaking
let app = GraphBuilder::new(graph)
    .with_timeout(duration)         // Optional (v1.2)
    .with_checkpointer(cp)           // Optional (v1.3)
    .with_callback(cb)               // Optional (v1.4)
    .with_new_feature(x)             // Future - doesn't break existing
    .build()?;
```

**Implementation:**
```rust
// crates/dashflow/src/builder.rs

pub struct GraphBuilder<S> {
    graph: StateGraph<S>,
    checkpointer: Option<Box<dyn Checkpointer<S>>>,
    callbacks: Vec<Box<dyn EventCallback<S>>>,
    timeout: Option<Duration>,
    // New fields don't break existing code
}

impl<S: GraphState> GraphBuilder<S> {
    pub fn new(graph: StateGraph<S>) -> Self {
        Self {
            graph,
            checkpointer: None,
            callbacks: Vec::new(),
            timeout: None,
        }
    }

    pub fn with_checkpointer(mut self, cp: impl Checkpointer<S> + 'static) -> Self {
        self.checkpointer = Some(Box::new(cp));
        self
    }

    pub fn build(self) -> Result<CompiledGraph<S>> {
        // Construct CompiledGraph with all options
    }
}
```

**Benefits:**
- Adding options doesn't break existing code
- Clear, readable API
- Self-documenting
- Testable in isolation

---

### Improvement 3: Version Compatibility Module

**Add to framework:**
```rust
// crates/dashflow-compat/src/lib.rs

/// Compatibility module - maintains old APIs
pub mod v1_0 {
    pub use super::v1_0_compat::*;
}

pub mod v1_0_compat {
    use dashflow::StateGraph;

    /// v1.0-compatible API
    pub trait StateGraphV1_0<S> {
        fn add_conditional_edge(&mut self, ...);  // Old name
    }

    impl<S> StateGraphV1_0<S> for StateGraph<S> {
        fn add_conditional_edge(&mut self, ...) {
            // Shim to new API
            self.add_conditional_edges(...)
        }
    }
}
```

**Usage in app:**
```rust
// App staying on v1.0 API
use dashflow_compat::v1_0::*;

// Code keeps working with new framework!
graph.add_conditional_edge(...)  // Shimmed to new API
```

---

### Improvement 4: Re-exports for Stability

**Problem:**
```rust
// App imports get stale
use dashflow::something::internal::Type;  // Breaks easily
```

**Solution:**
```rust
// crates/dashflow/src/stable_api.rs

/// Re-export stable types at top level
pub mod stable {
    // These NEVER move, even if internal restructure
    pub use crate::messages::{Message, BaseMessage};
    pub use crate::language_models::ChatModel;
    pub use crate::runnable::Runnable;
    // etc.
}
```

**Usage:**
```rust
// App imports from stable module
use dashflow::core::stable::*;
// Never breaks, even if internals reorganize
```

---

### Improvement 5: Explicit Deprecation + Migration

**When changing API:**
```rust
// v1.6 (deprecate old)
#[deprecated(
    since = "1.6.0",
    note = "Use add_conditional_edges() instead. See docs/MIGRATION_GUIDE.md"
)]
pub fn add_conditional_edge(&mut self, ...) {
    // Still works! Shim to new API
    self.add_conditional_edges(...)
}

pub fn add_conditional_edges(&mut self, ...) {
    // New API
}
```

**Benefits:**
- Apps get warning, not error
- Still works (shim provided)
- Link to migration guide
- Time to migrate before removal

---

### Improvement 6: Feature Flags for New Features

**Instead of adding to main API:**
```toml
[features]
default = ["core"]
core = []
experimental = []  # New features here first
metrics = []       # Optional features
tracing = []
```

**Usage:**
```rust
#[cfg(feature = "metrics")]
pub fn metrics(&self) -> ExecutionMetrics {
    // Only if feature enabled
}
```

**Benefits:**
- New features don't increase surface area by default
- Apps opt-in to complexity
- Easier to maintain compatibility

---

### Improvement 7: Sealed Traits for Stability

**Problem:**
```rust
pub trait GraphState: Clone + Serialize + DeserializeOwned {
    // Apps implement this
}

// Later we want to add method:
fn new_required_method(&self);  // BREAKS ALL APPS!
```

**Solution:**
```rust
pub trait GraphState: Clone + Serialize + DeserializeOwned + private::Sealed {
    // Apps can't implement directly

    // Framework provides blanket impl:
    // impl<T: Clone + Serialize + DeserializeOwned> GraphState for T {}
}

mod private {
    pub trait Sealed {}
    impl<T: Clone + Serialize + DeserializeOwned> Sealed for T {}
}
```

**Benefits:**
- Can add methods without breaking apps
- Clear which traits are "implement" vs "use"

---

### Improvement 8: Extension Traits (Non-Breaking)

**Instead of modifying core trait:**
```rust
// BAD: Adding to core trait breaks apps
pub trait GraphState {
    fn new_method(&self);  // BREAKING
}

// GOOD: Extension trait (optional)
pub trait GraphStateExt: GraphState {
    fn new_method(&self) {
        // Default implementation
    }
}

impl<T: GraphState> GraphStateExt for T {}
```

**Usage:**
```rust
// App doesn't need to change
use dashflow::GraphStateExt;  // Just import extension

// New methods available
state.new_method();
```

---

### Improvement 9: Type Aliases for Simplification

**Instead of exposing complex types:**
```rust
// BAD: Complex type in public API
pub fn create() -> Arc<dyn Fn(State) -> Pin<Box<dyn Future<Output = Result<State>> + Send>> + Send + Sync> {
    // Apps depend on this monster
}

// GOOD: Hide complexity
pub type NodeFunction<S> = Arc<dyn Fn(S) -> BoxFuture<Result<S>> + Send + Sync>;

pub fn create() -> NodeFunction<State> {
    // Simple, stable type alias
}
```

**Benefits:**
- Can change internals without breaking apps
- Clearer API
- Type alias can stay stable while implementation evolves

---

### Improvement 10: Versioned Modules

**Structure framework with version-specific modules:**
```rust
// crates/dashflow/src/lib.rs

pub mod v1 {
    //! Stable v1 API - will work throughout v1.x lifetime
    pub use crate::stable_v1::*;
}

pub mod v2 {
    //! v2 API (when ready)
    pub use crate::stable_v2::*;
}

// Default export is latest
pub use v1::*;
```

**Usage:**
```rust
// App pins to specific version
use dashflow::core::v1::*;

// Even if framework is v1.7, app uses v1 API
// Compatibility guaranteed
```

---

## Concrete Action Items for Framework

### High Priority (Prevent Future Pain)

1. **Create dashflow-facade crate** (backward compatibility layer)
2. **Add GraphBuilder** (extensible pattern)
3. **Audit public API** (reduce surface area)
4. **Add stability tiers** (mark what's stable vs evolving)
5. **Deprecation process** (shims + warnings + removal timeline)

### Medium Priority

6. **Create dashflow-compat** (old API shims)
7. **Extension traits** (add features without breaking)
8. **Type aliases** (hide complexity)
9. **Feature flags** (optional features)

### Low Priority (Nice to Have)

10. **Versioned modules** (explicit version pinning)
11. **Sealed traits** (prevent external impl)
12. **Migration automation tool** (upgrade assistant)

---

## Implementation Plan

### Phase 1: Audit Current API (1 week)

**Tasks:**
1. List all public types/traits/functions
2. Mark tier (Stable / Evolving / Experimental)
3. Identify tight coupling points
4. Document what MUST remain stable

**Deliverable:** API_STABILITY.md

---

### Phase 2: Add Compatibility Layer (2 weeks)

**Tasks:**
1. Create dashflow-facade crate
2. Add v1.0 compatibility shims
3. Add GraphBuilder pattern
4. Test with old app code

**Deliverable:** dashflow-facade v1.6.1

---

### Phase 3: Refactor Public API (3 weeks)

**Tasks:**
1. Move complex types behind aliases
2. Add extension traits
3. Reduce public surface
4. Add deprecation warnings

**Deliverable:** Cleaner, more stable API

---

### Phase 4: Documentation (1 week)

**Tasks:**
1. Write upgrade guides for each version
2. Document stability commitments
3. Create app architecture examples
4. Add migration tooling

**Deliverable:** Complete upgrade documentation

---

## Measuring Success

**Framework upgrade safety measured by:**

| Metric | Target |
|--------|--------|
| **API surface area** | Minimize |
| **Compatibility shims** | v1.0 code works in v1.6 |
| **Breaking changes** | 0 in MINOR versions |
| **Migration time** | <1 hour for apps using facade |
| **App coupling** | Apps use traits, not structs |
| **Deprecation lead time** | ≥1 MINOR version |

---

## Example: Better Graph API

### Current (Coupling-Prone):
```rust
// App tightly coupled to StateGraph
let mut graph = StateGraph::new();
graph.add_node_from_fn("x", |s| Box::pin(async move { Ok(s) }));
graph.add_conditional_edges(...)
graph.set_entry_point("x");
let app = graph.compile()?;
```

### Improved (Decoupled):
```rust
// Option A: Builder pattern (less coupling)
let app = GraphBuilder::new()
    .node("x", |s| async move { Ok(s) })  // Simpler signature
    .conditional("x", condition, routes)
    .start_at("x")
    .build()?;

// Option B: Declarative macro (zero coupling to types)
let app = graph! {
    start: "research",
    nodes: {
        research => research_fn,
        write => write_fn,
    },
    edges: {
        research -> write,
        write ->? quality_check => {
            "good" -> END,
            "bad" -> research,
        }
    }
};

// Option C: Configuration-based (zero code coupling)
let app = GraphBuilder::from_yaml("workflow.yaml")?;
```

**Benefits:**
- Less direct coupling to StateGraph
- APIs can change underneath
- Simpler for users

---

## Recommendations (Priority Order)

### 1. Create Stable Trait Boundaries (HIGH)

**Define once, never change:**
```rust
/// Stable trait - will never have breaking changes in v1.x
#[stable(since = "1.6.0")]
pub trait ChatModel: Send + Sync {
    async fn generate(&self, messages: &[BaseMessage]) -> Result<ChatResult>;
    // That's it. Simple, stable.
}

/// Extension for optional features (non-breaking to add)
pub trait ChatModelExt: ChatModel {
    async fn generate_with_timeout(&self, messages: &[BaseMessage], timeout: Duration) -> Result<ChatResult> {
        // Default implementation
    }
}
```

**Rule:** Core traits NEVER change, only extend

---

### 2. Builder Pattern Everywhere (HIGH)

**Replace:**
```rust
fn new(a, b, c, d, e)  // Adding 'f' breaks everyone
```

**With:**
```rust
Builder::new()
    .with_a(a)
    .with_b(b)
    .with_f(f)  // Adding this breaks nobody!
    .build()
```

**Apply to:**
- Graph construction
- Provider initialization
- Agent creation
- Checkpointer setup

---

### 3. Facade Crate (MEDIUM)

**Create:** `dashflow-app-facade`

**Purpose:** Stable API designed for apps (not framework internals)

```rust
// Apps import from facade
use dashflow_app_facade::prelude::*;

// Facade provides:
// - Simplified APIs
// - Stability guarantees
// - Compatibility shims
// - Clear upgrade path
```

---

### 4. Deprecation Process (HIGH)

**Mandatory process for ANY API change:**

```rust
// Step 1: Add new API (v1.6)
pub fn new_api() { }

// Step 2: Deprecate old (v1.6)
#[deprecated(since = "1.6.0", note = "Use new_api()")]
pub fn old_api() {
    self.new_api()  // Shim!
}

// Step 3: Keep both for 1 MINOR version (v1.6, v1.7)

// Step 4: Remove old in MAJOR (v2.0)
// pub fn old_api() { }  // Deleted

// Step 5: Migration guide provided
```

---

### 5. API Stability Markers (MEDIUM)

**Mark every public item:**
```rust
#[stable(since = "1.0.0")]  // Never breaks
pub trait ChatModel { }

#[evolving(since = "1.6.0")]  // May change with deprecation
pub struct GraphTemplate { }

#[experimental(since = "1.6.0")]  // May change without warning
#[cfg(feature = "experimental")]
pub struct NewFeature { }
```

**Custom attribute macros:**
```rust
#[proc_macro_attribute]
pub fn stable(attr, item) -> TokenStream {
    // Generate docs, warnings, etc.
}
```

---

### 6. Upgrade Assistant Tool (MEDIUM)

**Create:** `dashflow-upgrade` CLI tool

```bash
# Analyze app for upgrade issues
dashflow-upgrade check --from 1.0 --to 1.6

# Output:
# ⚠️ 3 breaking changes found:
#   - src/main.rs:45 - add_conditional_edge renamed to add_conditional_edges
#   - src/workflow.rs:123 - Error::InvalidInput moved to Error::Validation
#   - src/config.rs:67 - with_model signature changed
#
# Run 'dashflow-upgrade fix' to auto-fix

# Auto-fix what's possible
dashflow-upgrade fix --from 1.0 --to 1.6

# Apply fixes
# Manual review needed for 1 change
```

---

### 7. Example: App Template (HIGH)

**Provide:** `examples/app-template/`

**Structure:**
```
app-template/
├── src/
│   ├── domain/       # Your logic (no framework imports)
│   ├── adapters/     # Framework adapters (isolation layer)
│   ├── config.rs     # Configuration
│   └── main.rs       # Minimal entry point
├── config/
│   ├── dev.toml
│   └── prod.toml
└── tests/
    ├── unit_tests.rs     # With mocks (no framework)
    └── integration_tests.rs  # With framework (ignored)
```

**Documentation:**
```
# How to use this template:
1. Copy template
2. Add your domain logic to domain/
3. Configure adapters/ for your needs
4. Framework is isolated - easy to upgrade
```

---

## Breaking Change Checklist

**Before making ANY breaking change:**

- [ ] Can it be avoided with deprecation?
- [ ] Can it be backward compatible?
- [ ] Shim provided for old API?
- [ ] Migration guide written?
- [ ] Upgrade tool updated?
- [ ] Announced in advance?
- [ ] Only in MAJOR version?

**If all "no" → Don't make the change**

---

## Framework Design Principles

### Principle 1: Additive, Not Subtractive

**Good:**
```rust
// v1.6: Add new optional method
impl GraphBuilder {
    pub fn with_new_feature(self, x: X) -> Self {
        // New - doesn't break existing code
    }
}
```

**Bad:**
```rust
// v1.6: Change required method
trait ChatModel {
    fn generate(&self, extra_param: Y);  // BREAKS EVERYONE
}
```

### Principle 2: Hide Implementation, Expose Traits

**Good:**
```rust
pub trait ChatModel { }  // Expose trait
// Implementation hidden in crate
```

**Bad:**
```rust
pub struct ChatOpenAI {
    pub client: Client,  // Implementation exposed!
    pub settings: Settings,
}
// Changing internals breaks apps
```

### Principle 3: Stability by Default

**New features:**
```rust
#[cfg(feature = "experimental")]  // Opt-in
pub mod new_experimental_feature { }

// Graduate to stable after 1-2 versions
#[cfg(not(feature = "experimental"))]  // Default
pub mod graduated_stable_feature { }
```

---

## Testing for Compatibility

**Add to CI:**
```rust
// tests/compatibility/v1_0_apps.rs

#[test]
fn v1_0_code_still_works() {
    // Actual v1.0 app code
    let mut graph = StateGraph::new();
    graph.add_conditional_edge(...)  // Old API

    // Should work with shim
    assert!(graph.compile().is_ok());
}
```

**Run on every commit:** Ensure v1.0 code still works

---

## Summary: Framework Stability Roadmap

**Immediate (v1.6.1):**
1. Add compatibility shims for v1.0 → v1.1 breaking change
2. Create app template with proper separation
3. Document stability commitments

**Short-term (v1.7.0):**
4. Add GraphBuilder pattern
5. Create dashflow-facade crate
6. Mark API stability tiers

**Long-term (v2.0.0):**
7. Version-pinned modules
8. Upgrade assistant tool
9. Sealed traits where appropriate

**Goal:** Apps written today work with v1.99 with zero changes

---

**Your upgrade pain → Framework improvements → Easier for everyone**

**Author:** Andrew Yates © 2026
