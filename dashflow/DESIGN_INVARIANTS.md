# DESIGN_INVARIANTS.md - Architectural Laws for DashFlow

**Last Updated:** 2026-01-02 (Worker #2277 - Fixed stale crate count 109→108)

**REQUIRED READING:** AI workers MUST read this before creating new systems, types, or modules.
Violating these invariants creates technical debt and system fragmentation.

---

## Overview

DashFlow has grown to 108 crates with multiple subsystems. This document defines
architectural invariants that prevent duplication, ensure consistency, and maintain
a coherent system design.

**Before creating ANY new:**
- Data structure for execution/telemetry → Check Invariant 1
- Registry or catalog → Check Invariant 3
- Streaming message type → Check Invariant 2
- Introspection API → Check Invariant 4

---

## Invariant 1: Single Telemetry System

### Canonical Type: `introspection::ExecutionTrace`

**Rule:** ALL execution telemetry flows through `ExecutionTrace`. No parallel types.

**Location:** `crates/dashflow/src/introspection/trace.rs`

**What ExecutionTrace tracks:**
- Node executions with timing (`NodeExecution`)
- State snapshots before/after nodes
- Token usage per node
- Errors and failures
- Total duration and completion status

### Wrong Patterns (DO NOT CREATE):
```rust
// WRONG: Creating parallel trace types
pub struct TraceEntry { ... }        // optimize/trace.rs - DEPRECATED
pub struct ExecutionTracer { ... }   // debug.rs - DEPRECATED
pub struct MyNewTraceType { ... }    // DON'T DO THIS
```

### Right Pattern:
```rust
// RIGHT: Extend ExecutionTrace if needed
impl ExecutionTrace {
    pub fn to_training_examples(&self) -> Vec<Example> { ... }
    pub fn to_dashstream(&self) -> Vec<DashStreamMessage> { ... }
}

// RIGHT: Consume ExecutionTrace
fn analyze(trace: &ExecutionTrace) -> Analysis { ... }
```

### Data Flow:
```
Graph Execution → ExecutionTrace (local, always available)
                        │
                        ├──→ Local consumers (introspection, optimization, debugging)
                        │
                        └──→ Optional: Stream to external listeners
                              └── DashStreamProducer → Kafka/WebSocket
                                                         │
                                      Remote: DashStreamConsumer → ExecutionTrace
```

---

## Invariant 2: Streaming is Optional Transport, Not Primary Source

### Rule: Local analysis NEVER requires external infrastructure

**DashFlow Streaming** (`dashflow-streaming` crate) is for:
- Communicating execution state to external listeners
- Distributed observability across multiple executors
- Real-time dashboards and monitoring

**DashFlow Streaming is NOT for:**
- Local execution analysis
- Running optimizers on local traces
- Debugging local executions

### Wrong Patterns:
```rust
// WRONG: Requiring Kafka for local analysis
let traces = TraceCollector::new("localhost:9092", topic).await?;  // NO!
let analysis = analyze(traces);

// WRONG: Making Kafka mandatory
#[cfg(feature = "dashstream")]  // Feature-gating local functionality
fn collect_traces() { ... }
```

### Right Patterns:
```rust
// RIGHT: Local traces are always available
let trace = compiled.get_execution_trace(thread_id);
let analysis = analyze(&trace);

// RIGHT: Streaming is opt-in for external communication
if let Some(producer) = streaming_producer {
    producer.send(trace.to_dashstream()).await?;
}

// RIGHT: Remote traces are converted to local type
let remote_trace = consumer.receive().await?;
let trace = ExecutionTrace::from_dashstream(remote_trace);
let analysis = analyze(&trace);  // Same code path!
```

---

## Invariant 3: One Canonical Type Per Concern

Before creating a new type, check if one of these already handles your need:

| Concern | Canonical Type | Location |
|---------|---------------|----------|
| Execution telemetry | `ExecutionTrace` | introspection/trace.rs |
| Node execution record | `NodeExecution` | introspection/trace.rs |
| Training examples | `Example` | optimize/example.rs |
| Graph mutations | `GraphMutation` | graph_reconfiguration.rs |
| Mutation types | `MutationType` | graph_reconfiguration.rs |
| Platform capabilities | `PlatformInfo` | platform_introspection.rs |
| App configuration | `GraphManifest` | introspection/graph_manifest.rs |
| Live execution state | `ExecutionState` | live_introspection.rs |
| Execution tracking | `ExecutionTracker` | live_introspection.rs |
| Prompt analysis | `PromptAnalysis` | prompt_evolution.rs |
| Prompt improvements | `PromptEvolution` | prompt_evolution.rs |
| Timeout learning | `TimeoutLearner` | adaptive_timeout.rs |
| Graph events | `GraphEvent` | event.rs |
| Stream messages | `DashStreamMessage` | dashflow-streaming |

### Extension Over Creation

If an existing type is close but not quite right:

1. **Preferred:** Extend the existing type
   ```rust
   // Add a new method to ExecutionTrace
   impl ExecutionTrace {
       pub fn new_capability(&self) -> NewResult { ... }
   }
   ```

2. **If extension isn't possible:** Document why and get approval
   ```rust
   // DESIGN NOTE: Cannot extend ExecutionTrace because [specific reason].
   // This type will be unified with ExecutionTrace in [issue/PR].
   pub struct TemporaryNewType { ... }
   ```

3. **Never:** Silently create a parallel type

---

## Invariant 4: Three-Level Introspection Model

DashFlow uses a three-level introspection architecture. Use the correct level:

### Level 1: Platform Introspection
**What:** DashFlow framework capabilities (shared by ALL apps)
**Use for:** Version info, available features, node types, edge types
**Access:** `compiled.platform_introspection()`
**Location:** `platform_introspection.rs`

### Level 2: App Introspection
**What:** Application-specific configuration (per compiled graph)
**Use for:** Graph structure, nodes, edges, tools, state schema
**Access:** `compiled.introspect()` or `compiled.manifest()`
**Location:** `introspection/` module

### Level 3: Live Introspection
**What:** Runtime execution state (per execution instance)
**Use for:** Active executions, current node, state values, history
**Access:** `compiled.live_executions()` or `tracker.get_execution(id)`
**Location:** `live_introspection.rs`

### Unified Access:
```rust
let unified = compiled.unified_introspection();
// unified.platform - Level 1
// unified.app      - Level 2
// unified.live     - Level 3
```

---

## Invariant 5: Check Before Creating

### Pre-Creation Checklist

Before creating ANY new module, type, or system:

- [ ] Read this document (DESIGN_INVARIANTS.md)
- [ ] Check if existing canonical type handles the need (Invariant 3)
- [ ] Verify streaming isn't being made mandatory (Invariant 2)
- [ ] Confirm using correct introspection level (Invariant 4)
- [ ] Search codebase for similar functionality: `grep -r "similar_term" crates/`

### If You Must Create Something New

1. **Document the gap:** Why doesn't an existing system work?
2. **Plan for unification:** How will this merge with existing systems?
3. **Add to this document:** Update the canonical types table
4. **Notify in commit:** Tag commit with `[NEW-SYSTEM]` for review

---

## System Relationships Map

```
┌─────────────────────────────────────────────────────────────────────┐
│                      DASHFLOW SYSTEM MAP                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  EXECUTION LAYER                                                    │
│  ┌─────────────┐                                                   │
│  │  executor   │──produces──→ ExecutionTrace                       │
│  └─────────────┘                    │                              │
│                                     │                              │
│  ANALYSIS LAYER          ┌─────────┴─────────┐                     │
│                          ▼                   ▼                      │
│              ┌─────────────────┐   ┌─────────────────┐             │
│              │  introspection  │   │    optimize     │             │
│              │  (self-aware)   │   │  (self-improve) │             │
│              └────────┬────────┘   └────────┬────────┘             │
│                       │                     │                       │
│                       ▼                     ▼                       │
│              ┌─────────────────┐   ┌─────────────────┐             │
│              │ GraphMutation   │   │ Model Training  │             │
│              │ PromptEvolution │   │ Example         │             │
│              │ TimeoutLearner  │   │ Optimizer       │             │
│              └─────────────────┘   └─────────────────┘             │
│                                                                     │
│  TRANSPORT LAYER (OPTIONAL - for external communication)           │
│  ┌─────────────────────────────────────────────────────────┐       │
│  │              dashflow-streaming                          │       │
│  │  ExecutionTrace ←→ DashStreamMessage ←→ Kafka/WebSocket  │       │
│  └─────────────────────────────────────────────────────────┘       │
│                                                                     │
│  REGISTRY LAYER (metadata about systems)                           │
│  ┌─────────────────┐   ┌─────────────────┐                        │
│  │ platform_registry│   │  graph_registry │                        │
│  │ (API catalog)    │   │ (graph versions)│                        │
│  └─────────────────┘   └─────────────────┘                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Invariant 6: Full Opt-In by Default, Opt-Out Only

### Rule: All features are ON by default. Users disable what they don't want.

**Principle:** DashFlow is maximally helpful out of the box. No configuration required
to get full capabilities. Users who want less can opt-out of specific features.

### Discovering Automatic Behaviors

**Use introspection to see what's automatic:**
```bash
# Generate current capabilities report
dashflow introspect automatic

# Save to file for reference
dashflow introspect automatic > CURRENT_CAPABILITIES.md
```

**MANDATORY FOR AI WORKERS:** Run `dashflow introspect automatic` before claiming any feature is missing.

### Automatic Telemetry (Environment Variables)

| Feature | Default | Opt-Out Environment Variable |
|---------|---------|------------------------------|
| Trace persistence | **ON** | `DASHFLOW_TRACE=false` |
| PII redaction | **ON** | `DASHFLOW_TRACE_REDACT=false` |
| Live introspection | **ON** | `DASHFLOW_LIVE_INTROSPECTION=false` |
| Event emission | **ALWAYS ON** | (no opt-out) |

### Automatic API Features

| Feature | Default | Opt-Out Method |
|---------|---------|----------------|
| ExecutionTrace collection | **ON** | `.without_tracing()` |
| Local analysis | **ON** | `.without_analysis()` |
| Pattern detection | **ON** | `.without_patterns()` |
| File storage | **ON** | `.without_storage()` |
| Multi-model consensus* | **ON** | `.without_consensus()` |
| Plan generation | **ON** | `.without_plans()` |
| Dasher integration** | **ON** | `.without_dasher()` |

*If API keys present in environment
**Plans require explicit approval before execution

**Wrong:**
```rust
// WRONG: Requiring opt-in for features
let app = graph.compile()?;
app.enable_introspection();  // Should be automatic!
app.with_consensus(providers);  // Should be automatic!
```

**Right:**
```rust
// RIGHT: Everything works automatically
let app = graph.compile()?;
// Full introspection, analysis, consensus, plans - all automatic

// RIGHT: Disable specific features you don't want
let app = graph
    .compile()?
    .without_consensus()   // Don't call external AIs
    .without_storage();    // Don't write to disk
```

**API Key Detection:**
```rust
// If ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_API_KEY are set,
// multi-model consensus runs automatically.
// If not set, that tier is skipped silently (no error).
```

**Dasher Safety:**
```rust
// Dasher generates plans automatically but NEVER executes without approval.
// Each plan requires explicit human approval before implementation.
// This is the only "gate" in the system.
```

---

## Invariant 7: Self-Improvement Through Structured Introspection

### Rule: All AI self-analysis uses the IntrospectionReport system

**Location:** `crates/dashflow/src/self_improvement/` (see `archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md` - COMPLETE)

**What this means:**
- Capability gap analysis → `CapabilityGap` struct
- Deprecation recommendations → `DeprecationRecommendation` struct
- Execution plans → `ExecutionPlan` struct with citations
- Hypotheses → `Hypothesis` with expected evidence and evaluation
- Multi-model consensus → `ConsensusResult` with model reviews

**Wrong Pattern:**
```rust
// WRONG: Ad-hoc self-analysis without structure
println!("I think we need a sentiment tool...");
```

**Right Pattern:**
```rust
// RIGHT: Structured introspection with citations
let gap = CapabilityGap {
    description: "Missing sentiment analysis tool".to_string(),
    evidence: vec![Citation::trace("thread-001"), Citation::trace("thread-017")],
    manifestation: GapManifestation::PromptWorkarounds { patterns: vec!["..."] },
    proposed_solution: "Add SentimentAnalysisTool node".to_string(),
    expected_impact: Impact::High,
    confidence: 0.85,
    category: GapCategory::MissingTool { tool_description: "..." },
};
```

**Storage:** All reports in `.dashflow/introspection/` with git history.

---

## Invariant 8: Serde Rename Convention Guidelines

### Rule: Use appropriate `serde(rename_all)` based on context

**Purpose:** Consistent JSON serialization across the codebase while maintaining API compatibility.

### Convention by Context:

| Context | Convention | Example |
|---------|------------|---------|
| Internal types with multi-word variants | `snake_case` | `ToolUse` → `"tool_use"` |
| Internal types with single-word variants | `lowercase` or `snake_case` | `High` → `"high"` |
| External API compatibility (Gemini, SARIF) | Match external spec | `camelCase`, `SCREAMING_SNAKE_CASE` |
| File format types (audio, image) | `lowercase` | `Jpeg` → `"jpeg"` |

### Key Distinction:

**`lowercase`** transforms: `SomeVariant` → `"somevariant"` (concatenated)

**`snake_case`** transforms: `SomeVariant` → `"some_variant"` (separated)

For single-word variants like `High`, `Low`, `Text`, both produce identical output.
For multi-word variants like `ToolUse`, `RsaPss4096`, they differ significantly.

### Wrong Pattern:
```rust
// WRONG: Multi-word variants with lowercase produce unreadable JSON
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    RsaPss4096,  // → "rsapss4096" (hard to read)
    EcdsaP256,   // → "ecdsap256" (hard to read)
}
```

### Right Pattern:
```rust
// RIGHT: Multi-word variants with snake_case
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    RsaPss4096,  // → "rsa_pss_4096" (readable)
    EcdsaP256,   // → "ecdsa_p256" (readable)
}

// RIGHT: Single-word variants (lowercase is fine)
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,     // → "low"
    Medium,  // → "medium"
    High,    // → "high"
}

// RIGHT: External API compatibility (keep as-is)
#[serde(rename_all = "camelCase")]  // SARIF spec requires camelCase
struct SarifReport { ... }
```

### Backwards Compatibility:

When fixing existing `lowercase` → `snake_case`, add aliases:
```rust
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    #[serde(alias = "rsapss4096")]  // Accept old format
    RsaPss4096,
}
```

---

## Invariant 9: Error Message Format Guidelines

### Rule: All error messages use sentence case with consistent structure

Error messages in DashFlow follow a consistent format for:
- **Grepability**: Easy to find in logs and code
- **Readability**: Clear and professional presentation
- **Consistency**: Uniform style across all crates

### Format Standard:

```
"{Category}: {description}"
```

**Rules:**
1. **Capitalize first word** - `"Storage error: ..."` not `"storage error: ..."`
2. **Use colon + space after category** - `"Network error: timeout"` not `"network error - timeout"`
3. **Keep descriptions lowercase after colon** - `"Invalid config: missing field"` not `"Invalid Config: Missing Field"`
4. **No trailing punctuation** - `"File not found: {path}"` not `"File not found: {path}."`

### Wrong Patterns:
```rust
// WRONG: lowercase start
#[error("package not found: {0}")]  // Bad
#[error("io error: {0}")]           // Bad

// WRONG: inconsistent casing
#[error("NETWORK ERROR: {0}")]      // Bad - all caps
#[error("Network Error: {0}")]      // Bad - title case description
```

### Right Patterns:
```rust
// RIGHT: Sentence case with colon
#[error("Package not found: {0}")]
#[error("IO error: {0}")]
#[error("Network error: {0}")]
#[error("Invalid configuration: {0}")]
#[error("Serialization error: {0}")]

// RIGHT: Validation errors include field name
#[error("Invalid max_size: must be greater than 0, got {0}")]
#[error("Invalid timeout: must be positive, got {0:?}")]

// RIGHT: Not-found errors
#[error("File not found: {}", .0.display())]
#[error("Checkpoint not found: {0}")]
```

### Common Category Patterns:

| Category | Example |
|----------|---------|
| Storage/IO | `"Storage error: ..."`, `"IO error: ..."` |
| Network | `"Network error: ..."`, `"HTTP error: ..."` |
| Validation | `"Invalid {field}: ..."`, `"Validation error: ..."` |
| Not Found | `"{Thing} not found: ..."` |
| Serialization | `"Serialization error: ..."`, `"Deserialization error: ..."` |
| Config | `"Invalid configuration: ..."`, `"Configuration error: ..."` |
| Auth | `"Unauthorized: ..."`, `"Access denied: ..."` |

### Test Updates Required:

When fixing error messages, update any tests that assert on error strings:
```rust
// Update test assertions to match new format
assert!(err.to_string().contains("Storage error"));  // Not "storage error"
assert!(err.to_string().contains("Package not found"));  // Not "package not found"
```

---

## Known Technical Debt (Status)

Previous violations have been addressed with proper deprecation:

| Item | Status | Migration Path |
|------|--------|----------------|
| `optimize::TraceEntry` | ✅ DEPRECATED (v1.12.0) | Use `ExecutionTrace::to_trace_entries()` |
| `debug::ExecutionTrace` | ✅ REMOVED (N=360) | Use `introspection::ExecutionTrace` |
| `TraceCollector` (Kafka-only) | ✅ DEPRECATED (v1.12.0) | Use `ExecutionTrace` + `ExecutionTraceBuilder` locally |
| Multiple Example types | ⏳ Tracked | Unify `Example` and `TrainingExample` (future) |

**Note:** Deprecated types remain for backwards compatibility but will be removed in the next major version.
Legacy code should migrate to the canonical types in the `introspection/` module.

---

## Invariant 10: Configuration Struct Naming Convention

**Purpose:** Distinguish static configuration from per-call options and user preferences.

### The Convention:

| Suffix | Purpose | Example | Characteristics |
|--------|---------|---------|-----------------|
| `*Config` | Static configuration | `DashSwarmConfig`, `KafkaConfig` | From file/env, set once at startup |
| `*Options` | Per-call options | `SearchOptions`, `CompileOptions` | Passed to functions, varies per invocation |
| `*Settings` | User preferences | (Reserved for future use) | Persisted, user-controllable |

### Right Pattern:
```rust
// Config: Static, from environment or configuration file
pub struct KafkaConfig {
    pub brokers: Vec<String>,
    pub timeout_ms: u64,  // Set once at startup
}

// Options: Per-call, passed to functions
pub struct SearchOptions {
    pub limit: Option<u32>,
    pub offset: Option<u32>,  // Varies per search call
}
```

### Wrong Pattern:
```rust
// DON'T: Mix static and per-call concerns
pub struct SearchConfig {  // Should be SearchOptions if per-call
    pub limit: u32,
}
```

### Current Status:
Codebase audit (2025-12-25): 117 `*Config`, 4 `*Options`, 1 `*Settings`

The convention is already largely followed. New code should maintain this pattern.

---

## Invariant 11: `'static` Lifetime Guidelines

### Rule: Use `'static` only when required; document why in non-obvious cases

**Purpose:** Prevent over-constraining APIs with unnecessary `'static` bounds while documenting legitimate uses.

### When `'static` IS Required:

| Use Case | Reason | Example |
|----------|--------|---------|
| `Box<dyn Trait>` | Default trait object lifetime | `Box<dyn ChatModel>` |
| `Arc<dyn Trait>` | Default trait object lifetime | `Arc<dyn Node<S>>` |
| `tokio::spawn(async move { ... })` | Task must own data | Spawned futures |
| Global singletons | `LazyLock`, `OnceCell` | `&'static LlmTelemetrySystem` |
| String literals | Compile-time constants | `const NAME: &'static str` |
| Error trait `source()` | Required by `std::error::Error` | `fn source(&self) -> Option<&(dyn Error + 'static)>` |

### When `'static` Should Be Avoided:

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| `fn foo<T: 'static>(x: T)` without storing T | Over-constrains callers | Remove `'static` if not needed |
| `fn foo(s: &'static str)` for runtime strings | Prevents dynamic strings | Use `&str` or `String` |
| `T: Send + Sync + 'static` when T isn't stored | Prevents borrowed data | Use `T: Send + Sync` only |

### Right Patterns:

```rust
// RIGHT: 'static required because model is stored in Arc
pub fn new<M: ChatModel + 'static>(model: M) -> Self {
    Self { inner: Arc::new(model) }
}

// RIGHT: 'static required for spawning
tokio::spawn(async move {
    handle_request(data).await  // data must be 'static
});

// RIGHT: No 'static because T is only borrowed
fn process<T: Debug>(item: &T) {
    println!("{:?}", item);
}

// RIGHT: Explicit lifetime instead of 'static
fn parse<'a>(input: &'a str) -> Result<&'a str> { ... }
```

### Wrong Patterns:

```rust
// WRONG: 'static not needed if T isn't stored
fn log_item<T: Debug + 'static>(item: &T) {  // Remove 'static
    println!("{:?}", item);
}

// WRONG: Forcing 'static on config strings
pub const DEFAULT_URL: &'static str = "...";  // Fine as const
fn get_url() -> &'static str { ... }  // Returns String or &str instead

// WRONG: Over-constraining generic functions
fn validate<T: Send + Sync + 'static>(item: &T) { ... }  // Remove 'static
```

### Current Codebase Status (Audit 2025-12-29):

- **396** `'static` usages outside test code
- **Categories:**
  - `&'static str` constants: ~120 (legitimate - string literals)
  - Type erasure (`Box/Arc<dyn>`): ~150 (required)
  - Trait implementations: ~80 (mostly required)
  - Task spawning: ~30 (required)
  - Error trait: ~16 (required by std)

**Audit finding:** Most current uses are legitimate. No actionable over-constraints identified.

### Guidelines for New Code:

1. **Before adding `'static`:** Ask "Is this value being stored or sent to a spawned task?"
2. **If storing in `Arc/Box<dyn>`:** `'static` is required - add it
3. **If only borrowing:** Omit `'static`
4. **If unsure:** Try compiling without it - the compiler will tell you if it's needed
5. **Document non-obvious cases:** Add a comment explaining why `'static` is required

---

## Invariant 12: Configuration Struct Construction Pattern

**Purpose:** Minimize boilerplate and leverage Rust's type system for config structs.

### Rule: Use named presets + struct update syntax, NOT builder methods

For configuration structs (types ending in `*Config`), prefer Rust's struct update syntax over `with_*` builder methods.

### Right Pattern (Codex DashFlow style):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexConfig {
    pub default_language: String,
    pub model: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            default_language: "rust".to_string(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 2048,
            temperature: 0.3,
        }
    }
}

impl CodexConfig {
    /// Named preset for common use case
    pub fn for_rust() -> Self {
        Self { default_language: "rust".to_string(), ..Default::default() }
    }

    pub fn for_python() -> Self {
        Self { default_language: "python".to_string(), ..Default::default() }
    }
}

// Usage - compiler checks field names, all fields visible
let config = CodexConfig { max_tokens: 4096, ..CodexConfig::for_rust() };
```

### Wrong Pattern (avoid for config structs):

```rust
impl SomeConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_field1(mut self, v: T) -> Self { self.field1 = v; self }
    pub fn with_field2(mut self, v: T) -> Self { self.field2 = v; self }
    pub fn with_field3(mut self, v: T) -> Self { self.field3 = v; self }
    // ... 10 more methods, each 3 lines
}

// Usage - no compile-time field checking, fields hidden
let config = SomeConfig::new().with_field1(x).with_field2(y);
```

### Why Presets > Builders for Config:

| Criteria | Presets + Struct Update | Builder Methods |
|----------|------------------------|-----------------|
| Boilerplate | None | ~5 lines per field |
| Compile-time field checking | ✅ Yes | ❌ No |
| IDE autocomplete | ✅ Shows all fields | ⚠️ Methods only |
| Rust idiom | ✅ Language feature | ⚠️ Java/OOP pattern |
| Field visibility at call site | ✅ All visible | ❌ Hidden in chain |
| Maintenance cost | ✅ Add field, done | ❌ Add field + method |

### When Builder Pattern IS Appropriate:

- **Private fields:** When struct fields must be encapsulated
- **Validation during construction:** When values need clamping/checking (e.g., `rate.clamp(0.0, 1.0)`)
- **Cross-crate APIs:** When hiding struct internals from external users
- **Complex objects:** When construction has ordering dependencies or produces derived values
- **Non-config types:** `ExecutionTraceBuilder`, `QueryBuilder`, etc.

### Guidelines:

1. **Config structs:** Use `Default` + named presets + struct update syntax
2. **Add presets for common use cases:** `for_production()`, `for_testing()`, `for_<language>()`
3. **Keep fields `pub`:** Enables struct update syntax
4. **Validation:** Add a `fn validate(&self) -> Result<()>` method if needed, call at usage time
5. **Existing `with_*` methods:** Don't remove if already shipped (backwards compat), but don't add new ones

---

## Invariant 13: Documentation Requirements for Public APIs

**Rule:** New public items in the core `dashflow` crate require doc comments.

### Enforcement

The `dashflow` crate has `#![warn(missing_docs)]` enabled at the crate root.
This warns on undocumented public items added directly to `lib.rs`.

### What Requires Documentation

| Item Type | Required | Example |
|-----------|----------|---------|
| Public traits | ✅ Yes | `/// Trait for...` |
| Public structs | ✅ Yes | `/// Configuration for...` |
| Public functions | ✅ Yes | `/// Executes the...` |
| Public enum variants | ✅ Yes | `/// Represents...` |
| Internal modules | ❌ No | `mod internal;` |
| Test modules | ❌ No | `#[cfg(test)] mod tests;` |

### Documentation Guidelines

1. **First line:** Brief summary of what it does (imperative mood)
2. **Examples:** Add `# Examples` section for complex APIs
3. **Panics:** Document panic conditions in `# Panics` section
4. **Errors:** Document error conditions in `# Errors` section
5. **Safety:** For `unsafe` code, document invariants in `# Safety` section

### Right Pattern:

```rust
/// Builds and returns a configured chat model.
///
/// # Arguments
///
/// * `config` - Model configuration parameters
///
/// # Examples
///
/// ```rust,ignore
/// let model = build_chat_model(&config)?;
/// ```
pub fn build_chat_model(config: &ModelConfig) -> Result<Box<dyn ChatModel>> {
    // ...
}
```

### Wrong Pattern:

```rust
// WRONG: No documentation on public API
pub fn build_chat_model(config: &ModelConfig) -> Result<Box<dyn ChatModel>> {
    // ...
}
```

### Scope

- **Core crate:** `#![warn(missing_docs)]` enabled (M-132)
- **External crates:** Should follow same convention (not enforced by lint yet)
- **Existing code:** Document incrementally; prioritize public traits and structs

---

## Adding New Invariants

If you identify a pattern that should be an invariant:

1. Document the problem (what fragmentation occurred)
2. Define the rule (what should be canonical)
3. Add to this document
4. Create a cleanup task if violations exist
5. Commit with `[MANAGER] New Design Invariant: [name]`

---

## Version History

| Date | Change | Author |
|------|--------|--------|
| 2025-12-09 | Initial creation - Telemetry unification invariants | MANAGER |
| 2025-12-12 | Updated technical debt status - deprecated types now tracked | Worker #447 |
| 2025-12-20 | Added Invariant 8: Serde Rename Convention Guidelines | Worker #1287 |
| 2025-12-22 | Added Invariant 9: Error Message Format Guidelines | Worker #1506 |
| 2025-12-25 | Added Invariant 10: Configuration Struct Naming Convention | Worker #1782 |
| 2025-12-29 | Added Invariant 11: `'static` Lifetime Guidelines | Worker #2050 |
| 2025-12-29 | Added Invariant 12: Config Struct Construction Pattern (presets > builders) | MANAGER |
| 2025-12-29 | Added Invariant 13: Documentation Requirements for Public APIs (M-132) | Worker #2120 |
