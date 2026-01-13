# ADR-0005: Non-Exhaustive Public Enums

**Status:** accepted
**Date:** 2025-12-22
**Author:** DashFlow Architecture Team
**Last Updated:** 2025-12-22 (Worker #1414 - Initial ADR creation)

## Context

DashFlow exposes public enums for error types, status codes, node types, and other domain concepts. As the platform evolves, new variants are frequently added.

Without care, adding a new enum variant is a breaking change:
- Downstream `match` statements become non-exhaustive
- Semver requires major version bump for any addition
- Pressure to "get it right the first time" slows iteration

## Decision

**All public enums that may grow use `#[non_exhaustive]`.**

This attribute tells downstream code they cannot assume the enum is complete, enabling:
- Adding variants in minor/patch releases
- Faster iteration on domain modeling
- Clear signal that the enum will evolve

### Correct Patterns

```rust
// RIGHT: Public enum with non_exhaustive
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum NodeType {
    Llm,
    Tool,
    Transform,
    HumanInLoop,
    // Future: can add more without breaking
}

// RIGHT: Downstream match with wildcard
match node_type {
    NodeType::Llm => handle_llm(),
    NodeType::Tool => handle_tool(),
    _ => handle_unknown(),  // Required with non_exhaustive
}
```

### Anti-Patterns

```rust
// WRONG: Public enum without non_exhaustive (if it may grow)
pub enum ErrorKind {
    Network,
    Parse,
    // Adding Timeout would be breaking change!
}

// WRONG: Exhaustive match on non_exhaustive enum
match node_type {
    NodeType::Llm => ...,
    NodeType::Tool => ...,
    // Missing wildcard - won't compile
}
```

### Exceptions

Some enums are intentionally exhaustive:
- Boolean-like enums (`Enabled`, `Disabled`)
- Protocol-defined enums (external schema)
- Internal enums not exposed in public API

## Consequences

### Positive
- Adding variants is semver-compatible (minor version)
- Faster iteration on domain modeling
- Clear signal of API stability expectations

### Negative
- Downstream code must always have wildcard arms
- Cannot pattern match exhaustively (some type safety lost)
- Slightly more verbose match statements

### Neutral
- Existing enums need migration to add attribute
- IDE autocomplete still shows all variants

## Alternatives Considered

### Alternative 1: Versioned Enums
- `NodeTypeV1`, `NodeTypeV2` for each iteration
- Rejected: Proliferation of types, migration burden

### Alternative 2: String-Based Types
- Use `String` instead of enum
- Rejected: Loses type safety, no autocomplete

### Alternative 3: Accept Breaking Changes
- Major version bump for each addition
- Rejected: Too slow for active development

## Related Documents

- `DESIGN_INVARIANTS.md` Invariant 8
- Rust RFC 2008 (non_exhaustive attribute)
- Worker #1284, #1285, #1286 (added to 46 enums)
