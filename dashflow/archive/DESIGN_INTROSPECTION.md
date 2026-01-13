# Design: DashFlow Self-Introspection

**Created:** 2025-12-12
**Status:** DESIGN
**Problem:** AI managers make wrong assumptions about what exists in DashFlow

---

## The Core Problem

An AI saw `TODO: Actual distillation` in CLI code and assumed distillation wasn't implemented. But `optimize/distillation/` has 100KB of working code.

**Root cause:** No single source of truth for "what does DashFlow have?"

---

## What Rust Already Provides

| Tool | What it gives | Limitation |
|------|---------------|------------|
| `cargo doc` | HTML docs from comments | Not machine-readable at runtime |
| `cargo metadata` | Crate structure JSON | No semantic info (descriptions) |
| `rustdoc --output-format json` | JSON docs | Unstable, complex to parse |
| Doc comments (`///`) | Human descriptions | Not queryable at runtime |

**Key insight:** Rust's doc comments ARE the source of truth for descriptions. We shouldn't duplicate them in a registry.

---

## Design Principles

### 1. Single Source of Truth
- Doc comments describe what things do
- Don't duplicate descriptions in a separate registry file
- Generate registry FROM doc comments

### 2. Compile-Time Verification
- If it's in the registry, it must exist (compile error otherwise)
- If module exists, it should be in registry (CI check)

### 3. Zero Maintenance Overhead
- Adding a module shouldn't require updating multiple files
- Registry should auto-discover or use lightweight markers

### 4. AI-Queryable at Runtime
- `PlatformRegistry::get()` returns structured data
- Can answer: "What modules exist?", "What does X do?", "How do I use X?"

---

## Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Source Code                               │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ //! @module distillation                                     │ │
│  │ //! @category optimize                                       │ │
│  │ //! @cli dashflow train distill                              │ │
│  │ //!                                                          │ │
│  │ //! Teacher-student model distillation for cost optimization │ │
│  │                                                              │ │
│  │ pub struct Teacher { ... }                                   │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ build.rs parses
┌─────────────────────────────────────────────────────────────────┐
│                    Generated Registry                            │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ // AUTO-GENERATED - DO NOT EDIT                              │ │
│  │ pub static MODULES: &[ModuleInfo] = &[                       │ │
│  │     ModuleInfo {                                             │ │
│  │         name: "distillation",                                │ │
│  │         category: "optimize",                                │ │
│  │         cli_command: Some("dashflow train distill"),         │ │
│  │         description: "Teacher-student model distillation..." │ │
│  │         path: "crates/dashflow/src/optimize/distillation",   │ │
│  │     },                                                       │ │
│  │ ];                                                           │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ runtime query
┌─────────────────────────────────────────────────────────────────┐
│                      AI Query                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ let registry = PlatformRegistry::get();                      │ │
│  │                                                              │ │
│  │ // "Does DashFlow have distillation?"                        │ │
│  │ registry.has_module("distillation") // true                  │ │
│  │                                                              │ │
│  │ // "What CLI command uses it?"                               │ │
│  │ registry.cli_for_module("distillation")                      │ │
│  │ // Some("dashflow train distill")                            │ │
│  │                                                              │ │
│  │ // "What module implements this CLI?"                        │ │
│  │ registry.module_for_cli("dashflow train distill")            │ │
│  │ // Some("optimize::distillation")                            │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

## Option A: Build Script Generation (Recommended)

### How it works

1. **Marker comments in source:**
```rust
//! @module distillation
//! @category optimize
//! @cli dashflow train distill
//!
//! Teacher-student model distillation for cost optimization.
//! Uses a teacher model (GPT-4) to generate training data for
//! a cheaper student model (GPT-3.5).

pub mod analysis;
pub mod config;
pub mod distiller;
```

2. **Build script parses markers:**
```rust
// build.rs
fn main() {
    let modules = parse_module_markers("crates/dashflow/src");
    generate_registry("src/generated_registry.rs", &modules);
}
```

3. **Generated registry:**
```rust
// generated_registry.rs (AUTO-GENERATED)
pub static REGISTRY: PlatformRegistry = PlatformRegistry {
    modules: &[
        ModuleInfo {
            name: "distillation",
            category: "optimize",
            cli_command: Some("dashflow train distill"),
            description: "Teacher-student model distillation...",
            source_path: "optimize/distillation/mod.rs",
        },
        // ... more modules
    ],
};
```

### Pros
- Single source of truth (doc comments)
- Always accurate (generated at build time)
- No manual registry maintenance
- Lightweight markers (just comments)

### Cons
- Build script complexity
- Custom comment parsing
- Need to define marker format

---

## Option B: Derive Macro

### How it works

```rust
#[derive(Introspectable)]
#[introspect(category = "optimize", cli = "dashflow train distill")]
pub mod distillation {
    //! Teacher-student model distillation...
}
```

### Pros
- Type-safe
- IDE support for attributes
- Standard Rust pattern

### Cons
- Can't use on mod.rs easily (modules aren't items)
- Requires proc-macro crate
- More invasive changes

---

## Option C: Inventory Crate (Runtime Registration)

### How it works

```rust
use inventory;

inventory::submit! {
    ModuleInfo {
        name: "distillation",
        description: include_str!("distillation/README.md"),
        // ...
    }
}

// At runtime:
for module in inventory::iter::<ModuleInfo> {
    println!("{}: {}", module.name, module.description);
}
```

### Pros
- No build script
- Automatic collection at link time
- Well-tested crate

### Cons
- Description separate from doc comments (duplication)
- Requires manual `submit!` for each module
- Runtime cost (though minimal)

---

## Recommended: Option A + Lightweight Fallback

### Implementation Plan

**Phase 1: Lightweight markers (no build script yet)**
```rust
//! @dashflow-module
//! @category optimize
//! @cli dashflow train distill
```

Add markers to all modules. This is documentation that also serves as machine-readable metadata.

**Phase 2: Simple parser**
```rust
// introspection/parser.rs
pub fn parse_module_markers(source: &str) -> Option<ModuleMarkers> {
    // Parse @dashflow-module, @category, @cli from doc comments
}
```

**Phase 3: Registry generation**
```rust
// build.rs or a cargo xtask
fn generate_registry() {
    let modules = walk_crate_and_parse_markers("crates/dashflow/src");
    write_registry_file(&modules);
}
```

**Phase 4: Verification**
```rust
#[test]
fn all_modules_have_markers() {
    // Every mod.rs should have @dashflow-module marker
}

#[test]
fn registry_matches_filesystem() {
    // Generated registry matches actual modules
}
```

---

## Marker Format Specification

```rust
//! @dashflow-module                    <- Required: marks this as a registrable module
//! @name distillation                  <- Optional: defaults to directory name
//! @category optimize                  <- Required: top-level category
//! @cli dashflow train distill         <- Optional: CLI command that uses this
//! @status stable|experimental|deprecated <- Optional: defaults to stable
//!
//! Short description on first line.
//!
//! Longer description follows after blank line.
//! Can include examples, usage notes, etc.
```

---

## CLI Mapping

For CLI commands, use a similar marker in the command file:

```rust
// commands/train.rs

/// @cli-command distill
/// @implements optimize::distillation::ThreeWayDistiller
/// @status wired|stub|unimplemented
async fn run_distill(args: DistillArgs) -> Result<()> {
```

This creates bidirectional mapping:
- Module knows which CLI uses it
- CLI knows which module it should call

---

## Query API

```rust
impl PlatformRegistry {
    /// Get the singleton registry
    pub fn get() -> &'static Self;

    /// Check if a module exists
    pub fn has_module(&self, name: &str) -> bool;

    /// Get module info by name
    pub fn module(&self, name: &str) -> Option<&ModuleInfo>;

    /// Find module by CLI command
    pub fn module_for_cli(&self, command: &str) -> Option<&ModuleInfo>;

    /// List all modules in a category
    pub fn modules_in_category(&self, category: &str) -> Vec<&ModuleInfo>;

    /// Search modules by keyword
    pub fn search(&self, query: &str) -> Vec<&ModuleInfo>;

    /// Export as JSON for AI consumption
    pub fn to_json(&self) -> String;

    /// Check if CLI command is wired to implementation
    pub fn is_cli_wired(&self, command: &str) -> bool;
}
```

---

## Migration Path

### Step 1: Add markers to existing modules (manual, one-time)
- Add `@dashflow-module` to every mod.rs
- Add `@category` for organization
- Add `@cli` where applicable

### Step 2: Create parser
- Parse markers from source files
- No code generation yet, just verification

### Step 3: Generate registry
- Build script or xtask generates registry
- Replace hand-written platform_registry.rs

### Step 4: Add CI checks
- Fail if module exists without marker
- Fail if CLI exists without mapping
- Fail if registry is stale

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Modules with markers | 100% |
| CLI commands mapped | 100% |
| Registry accuracy | 100% (generated) |
| AI query success rate | 100% |

---

## Anti-Patterns to Avoid

### Don't: Duplicate descriptions
```rust
// BAD - description in two places
//! Teacher-student distillation...  <- doc comment
ModuleInfo { description: "Teacher-student distillation..." }  <- registry
```

### Don't: Manual registry maintenance
```rust
// BAD - will drift from reality
// Remember to update this when adding modules!
pub static MODULES: &[&str] = &["distillation", "ab_testing", ...];
```

### Don't: Runtime-only discovery
```rust
// BAD - no compile-time verification
let modules = discover_modules_at_runtime();  // What if it misses one?
```

---

## Open Questions

1. **Where to store generated registry?**
   - `src/generated/` (gitignored)
   - `src/platform_registry.rs` (checked in, regenerated)
   - Inline in build artifacts

2. **How to handle sub-modules?**
   - Register only top-level?
   - Register all public modules?
   - Configurable depth?

3. **Integration with cargo doc?**
   - Can we extract from rustdoc JSON instead of custom parsing?
   - Nightly-only concern?

---

## Next Steps

1. [ ] Review this design
2. [ ] Choose between Option A/B/C
3. [ ] Define marker format precisely
4. [ ] Implement parser (standalone tool first)
5. [ ] Add markers to all modules
6. [ ] Generate registry
7. [ ] Add verification tests
8. [ ] Add CI checks
