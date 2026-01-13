# Roadmap: Introspection Completeness Audit

**Created:** 2025-12-12
**Priority:** P0 - Critical for AI self-awareness
**Status:** IN PROGRESS
**Goal:** 100% complete and accurate platform introspection

---

## Problem Statement

DashFlow has introspection infrastructure but it's incomplete:
- `platform_registry.rs` exists but doesn't register all modules
- `platform_introspection.rs` exists but doesn't know about all capabilities
- CLI commands exist but aren't mapped to their library implementations

**Result:** AI managers make incorrect assumptions about what exists.

**Example failure:** AI saw `TODO: Actual distillation` in CLI and assumed distillation wasn't implemented, when `optimize/distillation/` has 100KB+ of working code.

---

## Two-Way Audit Process

### Direction 1: Registry → Modules
For each item in the registry, verify:
- [ ] Module actually exists at stated path
- [ ] Description is accurate
- [ ] APIs listed actually exist and work
- [ ] Examples compile and run

### Direction 2: Modules → Registry
For each module in the codebase, verify:
- [ ] Module is registered in platform_registry
- [ ] All public APIs are documented
- [ ] CLI commands (if any) are mapped
- [ ] Dependencies are noted

---

## Phase 1: Module Inventory (N=480)

### Task 1.1: Generate complete module list

```bash
# Top-level modules
find crates/dashflow/src -maxdepth 1 -name "*.rs" -type f | sort

# Top-level directories (multi-file modules)
find crates/dashflow/src -maxdepth 1 -type d | sort

# Optimize submodules
find crates/dashflow/src/optimize -maxdepth 1 -type d | sort

# Core submodules
find crates/dashflow/src/core -maxdepth 1 -type d | sort
```

### Task 1.2: Document each module

For EACH module, create entry:

```markdown
## Module: optimize/distillation

**Path:** crates/dashflow/src/optimize/distillation/
**Files:** 9 files, ~100KB
**Purpose:** Teacher-student model distillation for cost optimization

### Public APIs
- `Teacher::new(llm, signature)` - Create teacher model
- `ThreeWayDistiller::new(teacher, config)` - Compare 3 distillation approaches
- `PromptOptimizationStudent::new(...)` - Few-shot prompt optimization
- `OpenAIFineTuneStudent::new(...)` - OpenAI fine-tuning
- `LocalFineTuneStudent::new(...)` - Local model fine-tuning
- `DistillationMetrics` - Evaluation metrics
- `DistillationReport` - Comparison report

### CLI Commands
- `dashflow train distill` → ThreeWayDistiller::distill()
- `dashflow train finetune` → OpenAIFineTuneStudent::train()

### Status
- [x] Library implementation complete
- [ ] CLI wired up
- [ ] Registered in platform_registry
```

---

## Phase 2: Registry Verification (N=481)

### Task 2.1: Extract current registry entries

```bash
# Get all registered modules
grep -E "\.name\(|name:" crates/dashflow/src/platform_registry.rs
```

### Task 2.2: Verify each entry

For EACH registered item:
1. Does the module exist?
2. Is the description accurate?
3. Do the listed APIs exist?
4. Are there APIs missing from the listing?

### Task 2.3: Fix inaccuracies

Update registry entries that are wrong or incomplete.

---

## Phase 3: Gap Analysis (N=482)

### Task 3.1: Find unregistered modules

```
Modules in codebase: X
Modules in registry: Y
Gap: X - Y = Z unregistered
```

### Task 3.2: Prioritize gaps

| Module | Importance | Has CLI? | Action |
|--------|------------|----------|--------|
| distillation | High | Yes | Register + wire CLI |
| ab_testing | High | No | Register |
| checkpointing | High | Yes | Verify registered |

---

## Phase 4: Complete Registration (N=483-485)

### Task 4.1: Add missing modules

For each unregistered module, add to `platform_registry.rs`:

```rust
ModuleInfo::builder()
    .name("distillation")
    .path("optimize/distillation")
    .description("Teacher-student model distillation for cost optimization")
    .api(ApiInfo::builder()
        .name("Teacher::new")
        .signature("(llm: Arc<dyn ChatModel>, signature: Signature) -> Result<Teacher>")
        .description("Create a teacher model for generating training data")
        .example(r#"
let teacher = Teacher::new(Arc::new(gpt4), signature)?;
let examples = teacher.generate_examples(100).await?;
        "#)
        .build())
    .api(ApiInfo::builder()
        .name("ThreeWayDistiller::new")
        .description("Compare prompt optimization, OpenAI fine-tune, and local fine-tune")
        .build())
    .build()
```

### Task 4.2: Add CLI mappings

```rust
CliMapping {
    command: "dashflow train distill".to_string(),
    module: "optimize::distillation".to_string(),
    function: "ThreeWayDistiller::distill".to_string(),
    implemented: true,
    wired: false,  // CLI doesn't call it yet
}
```

---

## Phase 5: Wire CLI Commands (N=486-488)

For each CLI command that has `TODO` but library exists:

### Task 5.1: train.rs distill command

```rust
// Before:
println!("TODO: Actual distillation implementation");

// After:
use dashflow::optimize::distillation::{Teacher, ThreeWayDistiller, DistillationConfig};

let config = DistillationConfig::from_args(&args)?;
let teacher = Teacher::new(create_llm(&args.teacher_model)?, signature)?;
let distiller = ThreeWayDistiller::new(teacher, config)?;
let result = distiller.distill(&examples).await?;
```

### Task 5.2: Update CLI mapping

```rust
CliMapping {
    command: "dashflow train distill",
    wired: true,  // Now it works!
}
```

---

## Phase 6: Verification Tests (N=489)

### Task 6.1: Registry completeness test

```rust
#[test]
fn test_all_modules_registered() {
    let registry = PlatformRegistry::discover();

    // Every module directory should be registered
    let module_dirs = find_module_directories("crates/dashflow/src");

    for dir in module_dirs {
        let module_name = dir.file_name().unwrap().to_str().unwrap();
        assert!(
            registry.find_module(module_name).is_some(),
            "Module '{}' exists at {} but is not registered",
            module_name, dir.display()
        );
    }
}
```

### Task 6.2: CLI mapping test

```rust
#[test]
fn test_cli_commands_mapped() {
    let registry = PlatformRegistry::discover();

    // Every CLI subcommand should have a mapping
    let cli_commands = ["train distill", "train finetune", "watch", "visualize"];

    for cmd in cli_commands {
        let mapping = registry.find_cli_mapping(cmd);
        assert!(mapping.is_some(), "CLI command '{}' not mapped", cmd);
        assert!(mapping.unwrap().implemented, "CLI '{}' mapped but not implemented", cmd);
    }
}
```

### Task 6.3: API existence test

```rust
#[test]
fn test_registered_apis_exist() {
    let registry = PlatformRegistry::discover();

    for module in &registry.modules {
        for api in &module.apis {
            // Verify the API actually exists (compile-time check via macro)
            verify_api_exists!(&api.name);
        }
    }
}
```

---

## Success Criteria

### Quantitative
- [ ] 100% of modules registered (currently ~60%)
- [ ] 100% of CLI commands mapped
- [ ] 100% of registered APIs verified to exist
- [ ] 0 stale/inaccurate registry entries

### Qualitative
- [ ] AI asking "does DashFlow have X?" gets correct answer
- [ ] AI asking "how do I use X?" gets working example
- [ ] No more "TODO" assumptions - AI checks registry first

---

## Commit Plan

| Commit | Phase | Description |
|--------|-------|-------------|
| #480 | 1 | Module inventory complete |
| #481 | 2 | Registry verification complete |
| #482 | 3 | Gap analysis documented |
| #483 | 4.1 | Core modules registered |
| #484 | 4.1 | Optimize modules registered |
| #485 | 4.2 | CLI mappings added |
| #486 | 5 | train.rs wired to distillation |
| #487 | 5 | Other CLI commands wired |
| #488 | 6 | Verification tests added |
| #489 | - | Mark roadmap COMPLETE |

---

## Files to Modify

- `crates/dashflow/src/platform_registry.rs` - Add modules, APIs, CLI mappings
- `crates/dashflow/src/platform_introspection.rs` - Update if needed
- `crates/dashflow-cli/src/commands/train.rs` - Wire to library
- `crates/dashflow-cli/src/commands/dataset.rs` - Wire to library
- `crates/dashflow/tests/introspection_completeness.rs` - New test file

---

## Maintenance

After this roadmap is complete:

1. **New module checklist:** When adding a new module, MUST:
   - Add to platform_registry.rs
   - Document all public APIs
   - Add CLI mapping if applicable
   - Add to verification test

2. **CI check:** Add CI step that fails if:
   - Module exists but not registered
   - CLI command exists but not mapped
   - Registered API doesn't compile

---

## Notes

This roadmap exists because an AI manager incorrectly assumed distillation wasn't implemented. Complete introspection prevents this class of errors.
