# Architecture Decision Records (ADRs)

**Last Updated:** 2025-12-22 (Worker #1414 - Initial ADR system creation)

This directory contains Architecture Decision Records (ADRs) for DashFlow. ADRs document significant architectural decisions made during the project's development.

## What is an ADR?

An Architecture Decision Record is a short document that captures an important architectural decision made along with its context and consequences. ADRs help:

- **Newcomers** understand why things are the way they are
- **Future maintainers** avoid repeating past mistakes
- **Current team** make informed decisions by reviewing past context

## ADR Index

### Accepted

| ADR | Title | Date | Summary |
|-----|-------|------|---------|
| [0001](0001-single-telemetry-system.md) | Single Telemetry System | 2025-12-22 | All execution telemetry flows through ExecutionTrace |
| [0002](0002-streaming-is-optional-transport.md) | Streaming is Optional Transport | 2025-12-22 | Local analysis never requires external infrastructure |
| [0003](0003-single-introspection-api.md) | Single Introspection API | 2025-12-22 | One introspection entry point for platform discovery |
| [0004](0004-rust-only-implementation.md) | Rust-Only Implementation | 2025-12-22 | No Python runtime dependency in production |
| [0005](0005-non-exhaustive-public-enums.md) | Non-Exhaustive Public Enums | 2025-12-22 | Public enums use #[non_exhaustive] for semver safety |

### Proposed
<!-- ADRs in discussion go here -->

### Deprecated/Superseded
<!-- Old ADRs that are no longer valid go here -->

## How to Propose a New ADR

1. **Copy the template:**
   ```bash
   cp docs/adr/0000-template.md docs/adr/NNNN-short-title.md
   ```

2. **Fill out the sections:**
   - Context: What problem are you solving?
   - Decision: What are you proposing?
   - Consequences: What are the trade-offs?
   - Alternatives: What else did you consider?

3. **Submit for review:**
   - Set status to `proposed`
   - Create a PR or include in your worker commit
   - Request review from relevant stakeholders

4. **After acceptance:**
   - Change status to `accepted`
   - Add to the index table above

## ADR Naming Convention

```
NNNN-short-hyphenated-title.md
```

- `NNNN`: Four-digit sequential number (0001, 0002, etc.)
- `short-hyphenated-title`: Descriptive name using hyphens

## Key Decisions Summary

### Pure Rust Architecture
DashFlow is implemented entirely in Rust with no Python runtime dependency. This enables:
- Predictable performance with no GIL contention
- Single binary deployment with static linking
- Memory safety guarantees at compile time

### Single Source of Truth Principle
Each concern has exactly one canonical implementation:
- **Telemetry:** `ExecutionTrace` in `introspection/trace.rs`
- **Module Discovery:** Unified Introspection API
- **Configuration:** Workspace inheritance in `Cargo.toml`

### Optional Infrastructure
Core functionality never requires external infrastructure:
- Kafka/Redis/PostgreSQL are for distributed scenarios
- Local development works with in-memory defaults
- Feature flags enable/disable infrastructure dependencies

## Related Documents

- [`DESIGN_INVARIANTS.md`](../../DESIGN_INVARIANTS.md) - Architectural laws and invariants
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) - System architecture overview
- [`BEST_PRACTICES.md`](../BEST_PRACTICES.md) - Development best practices
