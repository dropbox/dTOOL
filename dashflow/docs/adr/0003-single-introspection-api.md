# ADR-0003: Single Introspection API

**Status:** accepted
**Date:** 2025-12-22
**Author:** DashFlow Architecture Team
**Last Updated:** 2026-01-02 (Worker #2288 - Fixed stale crate count reference)

## Context

DashFlow is a platform with 108 crates providing LLM providers, vector stores, tools, optimizers, and more. Users and AI agents need to discover what's available:
- What LLM providers are configured?
- Which optimizers exist?
- What vector stores can I use?

Early on, multiple discovery mechanisms existed:
- Ad-hoc registry queries
- Hardcoded lists in documentation
- Per-module discovery functions

This led to:
- Inconsistent discovery experiences
- Stale documentation
- Difficulty for AI agents to reason about platform capabilities

## Decision

**One introspection entry point for all platform discovery: the Unified Introspection API.**

Location: `crates/dashflow/src/introspection/`

The API provides:
- Module discovery (`introspect search <keyword>`)
- Capability queries (`introspect show <module>`)
- CLI command listing (`introspect cli`)
- Runtime trace analysis (`introspect ask "..."`)

### Correct Patterns

```rust
// RIGHT: Query introspection for capabilities
use dashflow::introspection::UnifiedIntrospection;

let intro = UnifiedIntrospection::new();
let retrievers = intro.search("retriever")?;
let details = intro.show("optimize::distillation")?;
```

```bash
# RIGHT: CLI introspection
dashflow introspect search retriever
dashflow introspect show optimize::distillation
dashflow introspect cli --stubs-only
```

### Anti-Patterns

```rust
// WRONG: Building parallel discovery
fn find_all_retrievers() -> Vec<String> {
    vec!["chroma", "qdrant", ...]  // Hardcoded list
}

// WRONG: Per-module discovery that bypasses introspection
pub fn list_optimizers() -> Vec<&'static str> { ... }
```

## Consequences

### Positive
- Single source of truth for platform capabilities
- AI agents can programmatically discover features
- Self-linting uses introspection to detect reimplementations
- Documentation stays in sync with code

### Negative
- Introspection module must be kept up-to-date
- Performance overhead for comprehensive queries
- Learning curve for introspection API

### Neutral
- Existing discovery mechanisms need migration to introspection

## Alternatives Considered

### Alternative 1: Static Documentation
- Keep docs/MODULES.md as source of truth
- Rejected: Docs drift from implementation

### Alternative 2: Multiple Registries
- Each subsystem has its own registry
- Rejected: N registries means N places to check

## Related Documents

- `DESIGN_INVARIANTS.md` Invariant 4
- `docs/INTROSPECTION.md`
- `crates/dashflow/src/introspection/`
