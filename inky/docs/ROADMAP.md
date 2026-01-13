# Inky Roadmap

**Version:** 2.0
**Date:** 2026-01-07
**Status:** Active

---

## Mission

**Port Ink (React for CLIs) to Rust** for use in porting Claude Code and similar applications.

---

## Current State (Complete)

Inky is a mature Rust port of Ink with:

| Feature | Status |
|---------|--------|
| Core node types (Box, Text, Static) | Complete |
| Flexbox layout via Taffy | Complete |
| SimpleLayout fast path (40-67x speedup) | Complete |
| Buffer rendering with diff | Complete |
| Crossterm backend | Complete |
| Component library (Input, Select, Progress, Spinner, Scroll) | Complete |
| Hooks (signal, input, focus, interval, mouse) | Complete |
| Macros (vbox![], hbox![], text!(), style!{}) | Complete |
| Async support (tokio integration) | Complete |
| ratatui compatibility layer | Complete |
| 872 tests passing | Complete |
| Zero clippy warnings | Complete |

---

## Next Phase: Production Hardening

Focus on stability and real-world usage for Claude Code port.

### Phase 10: Testing Infrastructure

| Task | Description | Status |
|------|-------------|--------|
| `MockTerminal` | Test harness for terminal apps | Planned |
| Output assertions | `assert_eq!(mock.output(), [...])` | Planned |
| Input simulation | `mock.queue_input("yes")` | Planned |
| Snapshot testing | Visual snapshot comparison | Planned |

### Phase 11: Performance Validation

| Task | Description | Status |
|------|-------------|--------|
| Benchmark suite | Standardized performance tests | Partial |
| Memory profiling | Verify <2MB for typical apps | Planned |
| Latency profiling | Verify <3ms input latency | Planned |

### Phase 12: Documentation

| Task | Description | Status |
|------|-------------|--------|
| API docs | Complete rustdoc coverage | Partial |
| Migration guide | Ink → inky patterns | Planned |
| Example suite | Real-world usage examples | Partial |

---

## Experimental Features (Unstable)

These features exist but are not part of the core Ink port mission:

| Feature | Description | Stability |
|---------|-------------|-----------|
| AI Perception APIs | `Perception::as_text()`, `as_tokens()` | Unstable |
| AI Components | ChatView, DiffView, StatusBar, Markdown | Unstable |
| Visualization | Heatmap, Sparkline, Plot | Unstable |
| Accessibility | ARIA roles, announcements | Unstable |
| Animation | Animation system | Unstable |
| Elm architecture | Model/Update/View pattern | Unstable |
| GPU rendering | Tier 3 dterm integration | Unstable |

These may be extracted to a separate project focused on AI-first CLI development.

---

## Non-Goals

| Non-Goal | Rationale |
|----------|-----------|
| Compete with ratatui | Different use case; provide compat layer instead |
| Semantic terminal API | Out of scope for Ink port |
| "God-tier defaults" | Out of scope; basic Ink parity is goal |
| GPU rendering as default | Optional, not core |

---

## Success Criteria

1. **Ink Parity**: Can port Claude Code's Ink usage to inky
2. **Performance**: 10x faster than JS Ink
3. **Memory**: <2MB for typical apps
4. **Stability**: No regressions, 90%+ test coverage
5. **Documentation**: Complete migration guide from Ink

---

## Reference

| Document | Content |
|----------|---------|
| `/docs/archive/2026-01-ambitious-vision/` | Archived ambitious AI-first terminal vision |
| `/docs/archive/pre-2026-vision/` | Original technical architecture |
