# Design: Unified DashFlow Introspection System

**Created:** 2025-12-12
**Status:** APPROVED FOR IMPLEMENTATION
**Priority:** P0 - Critical (AI made wrong assumption due to incomplete introspection)
**Replaces:** DESIGN_INTROSPECTION.md, DESIGN_SELF_MCP.md, ROADMAP_INTROSPECTION_COMPLETENESS.md

---

## The Problem

An AI manager saw `TODO: Actual distillation` in CLI code and directed a worker to "implement distillation". But `optimize/distillation/` has 100KB of working code. The CLI stub just wasn't wired to the library.

**Root cause:** The platform registry is manually populated and incomplete.

| Registered Modules | Actual Modules |
|--------------------|----------------|
| 7 | 25+ |

The registry knows about `optimize` but not `optimize/distillation`, `optimize/ab_testing`, etc.

---

## The Solution

**Single source of truth:** Source code markers generate everything.

```
Source Code Markers
        │
        ▼
   Build Script
        │
        ├───────────────────┬────────────────────┐
        ▼                   ▼                    ▼
Generated Registry    CLI Help Output    MCP Server Responses
(platform_registry)   (dashflow --help)  (dashflow mcp-server)
```

---

## Marker Format

Every module's `mod.rs` gets markers:

```rust
//! @dashflow-module
//! @name distillation
//! @category optimize
//! @cli dashflow train distill
//! @cli-status stub
//! @status stable
//!
//! Teacher-student model distillation for cost optimization.
//!
//! Uses a teacher model (GPT-4) to generate training data for
//! a cheaper student model (GPT-3.5 or local).

pub mod analysis;
pub mod config;
pub mod distiller;
// ...
```

### Required Markers

| Marker | Description | Example |
|--------|-------------|---------|
| `@dashflow-module` | Marks this as a registrable module | `@dashflow-module` |
| `@category` | Top-level category | `@category optimize` |

### Optional Markers

| Marker | Description | Example |
|--------|-------------|---------|
| `@name` | Override name (defaults to dir name) | `@name distillation` |
| `@cli` | CLI command that uses this | `@cli dashflow train distill` |
| `@cli-status` | CLI wiring status | `@cli-status wired\|stub\|none` |
| `@status` | Module status | `@status stable\|experimental\|deprecated` |
| `@depends` | Dependencies on other modules | `@depends core::language_models` |

---

## CLI Command Markers

In CLI command files:

```rust
// commands/train.rs

/// @dashflow-cli distill
/// @implements optimize::distillation::ThreeWayDistiller
/// @wired false
/// @description Run three-way distillation comparison
async fn run_distill(args: DistillArgs) -> Result<()> {
    // TODO: Wire to library
    println!("TODO: Actual distillation implementation");
}
```

---

## Generated Output

### 1. platform_registry.rs (Auto-Generated Section)

```rust
// ============================================================================
// AUTO-GENERATED FROM @dashflow-module MARKERS - DO NOT EDIT
// Generated: 2025-12-12T10:30:00Z
// ============================================================================

pub static DISCOVERED_MODULES: &[ModuleInfo] = &[
    ModuleInfo {
        name: "distillation",
        category: "optimize",
        path: "crates/dashflow/src/optimize/distillation",
        description: "Teacher-student model distillation for cost optimization",
        cli_command: Some("dashflow train distill"),
        cli_wired: false,
        status: ModuleStatus::Stable,
    },
    ModuleInfo {
        name: "ab_testing",
        category: "optimize",
        path: "crates/dashflow/src/optimize/ab_testing",
        description: "A/B testing for prompt and model comparison",
        cli_command: None,
        cli_wired: false,
        status: ModuleStatus::Stable,
    },
    // ... all modules auto-discovered
];

pub static CLI_MAPPINGS: &[CliMapping] = &[
    CliMapping {
        command: "dashflow train distill",
        module: "optimize::distillation",
        function: "ThreeWayDistiller::distill",
        wired: false,
    },
    // ... all CLI commands
];
```

### 2. MCP Server Responses

```json
// GET dashflow://modules/distillation
{
  "name": "distillation",
  "category": "optimize",
  "path": "crates/dashflow/src/optimize/distillation",
  "description": "Teacher-student model distillation for cost optimization",
  "status": "stable",
  "cli": {
    "command": "dashflow train distill",
    "wired": false,
    "action_needed": "Wire CLI to ThreeWayDistiller::distill()"
  },
  "files": [
    "mod.rs", "analysis.rs", "config.rs", "distiller.rs",
    "metrics.rs", "prompt_optimization_student.rs",
    "teacher.rs", "three_way_distiller.rs"
  ],
  "line_count": 2847,
  "public_apis": [
    "Teacher::new", "Teacher::generate_examples",
    "ThreeWayDistiller::new", "ThreeWayDistiller::distill",
    "DistillationConfig", "DistillationMetrics", "DistillationReport"
  ]
}
```

---

## Implementation Architecture

```
crates/
├── dashflow/
│   ├── build.rs                      <- Parse markers, generate registry
│   └── src/
│       ├── platform_registry.rs      <- Include generated code
│       └── optimize/
│           └── distillation/
│               └── mod.rs            <- @dashflow-module markers
│
├── dashflow-cli/
│   └── src/
│       └── commands/
│           └── train.rs              <- @dashflow-cli markers
│           └── mcp_server.rs         <- NEW: `dashflow mcp-server` command
│
└── dashflow-introspection/           <- NEW: Marker parser crate
    ├── src/
    │   ├── lib.rs
    │   ├── parser.rs                 <- Parse @dashflow-* markers
    │   └── generator.rs              <- Generate Rust code
    └── Cargo.toml
```

---

## Phase 1: Add Markers (No Code Changes Yet)

Add `@dashflow-module` markers to ALL module files. This is documentation that also becomes metadata.

### Modules to Mark

| Category | Modules |
|----------|---------|
| core | language_models, prompts, retrievers, tracers, schema, indexing, document_loaders, document_transformers, structured_query, config_loader |
| optimize | distillation, ab_testing, cost_monitoring, data_collection, multi_objective, optimizers |
| checkpoint | (single module) |
| streaming | (single module - different crate) |
| quality | (single module) |
| network | (single module) |
| func | (single module) |
| colony | (single module) |
| parallel | (single module) |
| scheduler | (single module) |
| self_improvement | (single module) |
| packages | (single module) |

**Estimated:** ~25-30 module marker additions

---

## Phase 2: Build Marker Parser

Create `dashflow-introspection` crate:

```rust
// parser.rs
pub struct ModuleMarkers {
    pub name: Option<String>,
    pub category: String,
    pub cli_command: Option<String>,
    pub cli_status: CliStatus,
    pub status: ModuleStatus,
    pub description: String,
    pub dependencies: Vec<String>,
    pub source_path: PathBuf,
}

pub fn parse_module_markers(source: &str) -> Option<ModuleMarkers> {
    // Look for //! @dashflow-module in doc comments
    // Extract all @key value pairs
    // Parse description from remaining doc comments
}
```

---

## Phase 3: Generate Registry

In `dashflow/build.rs`:

```rust
fn main() {
    // Find all mod.rs files with @dashflow-module
    let modules = dashflow_introspection::discover_modules("src");

    // Generate Rust code
    let generated = dashflow_introspection::generate_registry(&modules);

    // Write to OUT_DIR
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{}/discovered_modules.rs", out_dir), generated).unwrap();

    // Tell cargo to rerun if any mod.rs changes
    println!("cargo:rerun-if-changed=src");
}
```

In `platform_registry.rs`:

```rust
// Include auto-generated modules
include!(concat!(env!("OUT_DIR"), "/discovered_modules.rs"));
```

---

## Phase 4: MCP Server

Add `dashflow mcp-server` command:

```bash
# Add to Claude Code config
{
  "mcpServers": {
    "dashflow": {
      "command": "dashflow",
      "args": ["mcp-server", "--stdio"]
    }
  }
}

# Or run standalone
dashflow mcp-server --port 3100
```

### MCP Tools

```json
{
  "tools": [
    {
      "name": "dashflow_list_modules",
      "description": "List all modules in DashFlow",
      "parameters": {
        "category": { "type": "string", "optional": true }
      }
    },
    {
      "name": "dashflow_get_module",
      "description": "Get detailed info about a module",
      "parameters": {
        "name": { "type": "string", "required": true }
      }
    },
    {
      "name": "dashflow_check_cli_status",
      "description": "Check if a CLI command is wired to its library",
      "parameters": {
        "command": { "type": "string", "required": true }
      }
    },
    {
      "name": "dashflow_find_code",
      "description": "Find where something is implemented",
      "parameters": {
        "query": { "type": "string", "required": true }
      }
    }
  ]
}
```

---

## Phase 5: CI Verification

Add tests that fail if:

1. Module directory exists without `@dashflow-module` marker
2. CLI command exists without `@dashflow-cli` marker
3. `@cli-status wired` but CLI has `TODO` in it
4. Registry module doesn't match actual filesystem

```rust
#[test]
fn test_all_modules_have_markers() {
    let discovered = dashflow_introspection::discover_modules("src");
    let filesystem = find_module_directories("src");

    for dir in filesystem {
        assert!(
            discovered.iter().any(|m| m.source_path == dir),
            "Module at {} has no @dashflow-module marker",
            dir.display()
        );
    }
}

#[test]
fn test_cli_status_accurate() {
    let mappings = CLI_MAPPINGS;

    for mapping in mappings {
        if mapping.wired {
            let source = std::fs::read_to_string(&mapping.source_file).unwrap();
            assert!(
                !source.contains("TODO"),
                "CLI {} marked as wired but contains TODO",
                mapping.command
            );
        }
    }
}
```

---

## Success Criteria

| Metric | Before | After |
|--------|--------|-------|
| Modules in registry | 7 | 25+ |
| Registry accuracy | Manual (drifts) | Auto-generated (always accurate) |
| AI asks "has distillation?" | No answer | "Yes, at optimize/distillation, CLI not wired" |
| AI asks "is CLI wired?" | Must grep code | Query MCP server |
| New module checklist | Manual docs update | Just add marker |

---

## Worker Implementation Order

1. **Add markers to all modules** (~2 hours)
   - Add `@dashflow-module` to every mod.rs
   - No code changes, just documentation

2. **Create dashflow-introspection crate** (~3 hours)
   - Parser for markers
   - Generator for registry code

3. **Integrate with build.rs** (~2 hours)
   - Wire parser to build script
   - Include generated code

4. **Create MCP server command** (~4 hours)
   - Add `dashflow mcp-server` to CLI
   - Implement MCP protocol
   - Serve registry data

5. **Add CI checks** (~1 hour)
   - Test for completeness
   - Test for accuracy

6. **Wire train.rs to distillation** (~1 hour)
   - Replace TODO with actual library call
   - Update marker to `@cli-status wired`

---

## Why This Works

| Problem | Solution |
|---------|----------|
| Manual registry drifts | Auto-generated from markers |
| CLI status unknown | Explicit `@cli-status` marker |
| AI makes assumptions | Query MCP server for truth |
| New modules forgotten | CI fails if marker missing |
| No single source of truth | Markers ARE the source |

---

## Open Questions Resolved

1. **Separate crate vs CLI subcommand?** → CLI subcommand (`dashflow mcp-server`)
2. **Keep patterns database updated?** → Extract from markers automatically
3. **Rustdoc JSON vs source parsing?** → Source parsing (simpler, stable)
4. **Where to store generated registry?** → `OUT_DIR`, included via `include!`

---

## Files Superseded

This design supersedes and consolidates:
- `DESIGN_INTROSPECTION.md` → Marker format and parsing
- `DESIGN_SELF_MCP.md` → MCP server design
- `ROADMAP_INTROSPECTION_COMPLETENESS.md` → Implementation phases

All three documents described parts of the same system. This document unifies them.
