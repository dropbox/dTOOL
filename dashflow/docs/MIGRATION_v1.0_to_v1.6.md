# Migration Guide: v1.0 → v1.6

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Context:** User reported difficulty upgrading app from v1.0 to v1.6
**Purpose:** Step-by-step migration guide for this specific upgrade path

---

## Breaking Changes (v1.0 → v1.6)

### 1. DashFlow API Changes (v1.1.0)

**COMPATIBILITY SHIMS AVAILABLE (v1.6+):** Your v1.0 code works without changes!

**v1.0:**
```rust
graph.add_conditional_edge(from, condition, routes);
graph.add_parallel_edge(from, targets);
```

**v1.6 (Recommended):**
```rust
graph.add_conditional_edges(from, condition, routes);  // Note: edgeS (plural)
graph.add_parallel_edges(from, targets);               // Note: edgeS (plural)
```

**Migration Options:**

**Option 1: No changes required (v1.0 code works immediately)**
```rust
// v1.0 code still works in v1.6 (with deprecation warnings)
graph.add_conditional_edge(from, condition, routes);  // ⚠️  Deprecated but functional
graph.add_parallel_edge(from, targets);                // ⚠️  Deprecated but functional
```
- Your v1.0 code compiles and runs in v1.6
- Deprecation warnings guide you to new API
- Migrate at your convenience

**Option 2: Update to recommended API (removes warnings)**
```bash
# Find and replace (optional, removes warnings)
rg "add_conditional_edge\(" --files-with-matches | xargs sed -i 's/add_conditional_edge(/add_conditional_edges(/g'
rg "add_parallel_edge\(" --files-with-matches | xargs sed -i 's/add_parallel_edge(/add_parallel_edges(/g'
```

**Why the change?**
- Consistency with Python DashFlow API
- Plural names indicate multiple edges can be added
- More intuitive for new users

---

### 2. Checkpointing API Changes (v1.2.0+)

**NEW:** Checkpointers now support compression and retention

**v1.0:**
```rust
let checkpointer = MemoryCheckpointer::new();
```

**v1.6 (backward compatible, but new features):**
```rust
// Still works:
let checkpointer = MemoryCheckpointer::new();

// New capabilities:
let checkpointer = PostgresCheckpointer::new(url)
    .with_compression(CompressionType::Zstd)
    .with_retention_policy(policy);
```

**Migration:** No changes required (backward compatible)

---

### 3. Error Type Changes

**If you matched on specific error variants:**

**v1.0:**
```rust
match error {
    DashFlowError::InvalidInput(msg) => // ...
}
```

**v1.6:**
Check if error variants changed. Use `is_*` methods:
```rust
if error.is_authentication() {
    // Handle auth error
}
```

---

### 4. New Features You Can Adopt

**Optional upgrades (non-breaking):**

**Metrics (v1.2.0):**
```rust
let app = graph.compile()?;
let result = app.invoke(state).await?;
let metrics = app.metrics();  // NEW
println!("{}", metrics.to_string_pretty());
```

**Mermaid Diagrams (v1.2.0):**
```rust
let diagram = graph.to_mermaid();  // NEW
std::fs::write("workflow.mmd", diagram)?;
```

**Advanced Agents (v1.3.0):**
```rust
// NEW patterns available
let agent = PlanAndExecuteAgent::new(llm);
let agent = ReflectionAgent::new(actor, critic);
```

---

## Step-by-Step Migration

### Step 1: Update Dependencies

```toml
# Cargo.toml

# Before:
[dependencies]
dashflow = "1.0"
dashflow-openai = "1.0"

# After:
[dependencies]
dashflow = "1.6"
dashflow-openai = "1.6"
dashflow = "1.6"  # If using DashFlow
```

### Step 2: Fix Compilation Errors

```bash
cargo build 2>&1 | tee compile_errors.txt

# Read errors, fix one by one
```

**Common errors:**
- Method renamed (add_conditional_edge → add_conditional_edges)
- Trait bounds changed (add Send + Sync if needed)
- Import paths changed (check error messages)

### Step 3: Update DashFlow Calls

**If using DashFlow:**
```bash
# Automated fixes
sed -i 's/add_conditional_edge(/add_conditional_edges(/g' src/**/*.rs
sed -i 's/add_parallel_edge(/add_parallel_edges(/g' src/**/*.rs
```

### Step 4: Test Thoroughly

```bash
cargo test --workspace
cargo clippy --workspace
cargo build --release
```

### Step 5: Update Error Handling (if needed)

**If matching on error types:**
```rust
// Check if specific error variants you use still exist
// Use is_* methods for forward compatibility:
if error.is_authentication() { }  // Better than matching
```

---

## If Migration is Still Hard

**You likely have tight coupling. Apply adapter pattern:**

### Current (Tightly Coupled):
```rust
// Your app directly uses framework types
struct MyApp {
    llm: ChatOpenAI,  // Coupled to OpenAI!
    graph: StateGraph<MyState>,  // Coupled to DashFlow!
}
```

**Problem:** Every framework change breaks your app

### Refactor (Loosely Coupled):

```rust
// Step 1: Define YOUR interface
trait MyLLM {
    async fn ask(&self, question: &str) -> Result<String>;
}

// Step 2: Adapt framework to YOUR interface
struct DashFlowLLM {
    inner: Box<dyn ChatModel>,  // Framework type hidden
}

impl MyLLM for DashFlowLLM {
    async fn ask(&self, question: &str) -> Result<String> {
        // Adapt framework API to yours
    }
}

// Step 3: Use YOUR interface
struct MyApp {
    llm: Box<dyn MyLLM>,  // YOUR trait!
}
```

**Now:** Framework updates only affect DashFlowLLM adapter, not MyApp!

---

## API Stability Commitment

**Going forward (v1.6+), we commit to:**

### Stable APIs (Won't Break)

**Core traits:**
- `ChatModel` - Will remain stable
- `Runnable` - Will remain stable
- `VectorStore` - Will remain stable
- `Checkpointer` - Will remain stable

**If changes needed:** Deprecated + new version, both work

### May Change (Use With Caution)

- Provider-specific methods
- Internal APIs (non-pub)
- Experimental features (documented as such)

### Clear Deprecation Path

**Example:**
```rust
// v1.6
#[deprecated(since = "1.6.0", note = "Use new_method() instead")]
pub fn old_method(&self) { }

pub fn new_method(&self) { }  // Recommended
```

**Both work in v1.6, old removed in v2.0**

---

## Backwards Compatibility Testing

**We should add to CI:**

```rust
// tests/backwards_compat/
// Test that v1.0-style code still works

#[test]
fn test_v1_0_style_still_works() {
    // Code that worked in v1.0
    let graph = StateGraph::new();
    // Should still work in v1.6
}
```

---

## Quick Reference: What Changed

| Version | Breaking Changes | Migration Effort |
|---------|------------------|------------------|
| **v1.0 → v1.1** | DashFlow method names (add_*_edge → add_*_edges) | Low (find/replace) |
| **v1.1 → v1.2** | None (additive only) | None |
| **v1.2 → v1.3** | None (additive only) | None |
| **v1.3 → v1.4** | None (additive only) | None |
| **v1.4 → v1.5** | None (additive only) | None |
| **v1.5 → v1.6** | None (additive only) | None |

**Key insight:** Only v1.0 → v1.1 had breaking changes!

---

## Recommended: Refactor for Future Upgrades

**Even if v1.6 works now, prepare for future:**

### 1. Add Adapter Layer (30 min)

```rust
// src/adapters/llm.rs
pub trait AppLLM {
    async fn generate(&self, prompt: &str) -> Result<String>;
}

// src/adapters/dashflow_llm.rs
pub struct DashFlowLLMAdapter {
    inner: Box<dyn ChatModel>,
}

impl AppLLM for DashFlowLLMAdapter {
    async fn generate(&self, prompt: &str) -> Result<String> {
        // Wrap framework
    }
}
```

### 2. Extract Domain Logic (1-2 hours)

```rust
// src/domain/
// Move business logic here
// No framework imports
```

### 3. Configuration (30 min)

```toml
# config.toml
[llm]
provider = "openai"
model = "gpt-4o-mini"
```

```rust
// Load from config, not hardcode
let llm = config.create_llm();
```

**Total refactor time:** 2-3 hours
**Future upgrades:** 30 minutes instead of "too hard"

---

## Specific Help for Your App

**What specifically was hard about v1.0 → v1.6 upgrade?**

Common issues:
1. **Method renames:** add_conditional_edge → add_conditional_edges (fixable with sed)
2. **Trait bounds:** Need to add Send + Sync (add where S: Send + Sync)
3. **Import paths:** Module reorganization (update use statements)
4. **Error handling:** Error types changed (use is_* methods)

**We can create automated migration tool if needed!**

---

## Going Forward: Stability Promise

**v1.6+ commits to:**

✅ **Semantic versioning:**
- PATCH (1.6.0 → 1.6.1): Bug fixes only, safe
- MINOR (1.6 → 1.7): New features, backward compatible
- MAJOR (1.x → 2.0): Breaking changes, migration guide provided

✅ **Deprecation policy:**
- Deprecate first (with warning)
- Keep deprecated for at least one MINOR version
- Document replacement
- Remove in next MAJOR

✅ **Migration guides:**
- Provided for every MAJOR version
- Automated tools where possible
- Example migrations

✅ **API stability tiers:**
- Tier 1 (Stable): Core traits, won't break
- Tier 2 (Evolving): Provider APIs, may change with deprecation
- Tier 3 (Experimental): Marked clearly, may change

---

**Your feedback is valuable - it identified a real pain point!**

**Author:** Andrew Yates © 2026
