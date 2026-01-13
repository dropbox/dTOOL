# Inky Roadmap

**Version:** 1.0
**Date:** 2026-01-01
**Status:** Active

---

## Current State

Inky is a mature Rust terminal UI framework with:
- Flexbox layout via Taffy
- Three rendering tiers (ANSI, Retained, GPU)
- Component library (Box, Text, Input, Select, Progress, etc.)
- AI assistant components (Markdown, ChatView, DiffView, StatusBar)
- Async support with Tokio integration
- AI Perception APIs for screen reading

**872 tests passing. Zero clippy warnings.**

---

## Next Phase: AI-First API Surface

The 2026 design philosophy refocuses inky on serving AI code generation (Claude Code, Codex). This roadmap outlines the path to achieving that vision.

### Phase 10: Semantic Terminal API

**Goal:** Create the `terminal.say()` / `terminal.error()` API layer that AIs naturally generate.

| Task | Description | Status |
|------|-------------|--------|
| `Terminal` struct | High-level API wrapper over App | Planned |
| `terminal.say()` | Plain text output with chainable styling | Planned |
| `terminal.error()` | God-tier error display with structure | Planned |
| `terminal.success()` | Success message with checkmark | Planned |
| `terminal.warn()` | Warning message | Planned |
| `terminal.info()` | Info message | Planned |
| `terminal.code()` | Syntax highlighted code block | Planned |
| `terminal.diff()` | Beautiful diff view | Planned |
| `terminal.ask()` | Free text input | Planned |
| `terminal.confirm()` | Yes/no question | Planned |
| `terminal.select()` | Pick one from options | Planned |
| `terminal.stream()` | Stream AI output with interruption | Planned |
| `terminal.progress()` | Progress bar | Planned |
| `terminal.spinner()` | Loading spinner | Planned |

**Deliverable:** AI can generate `terminal.say("Hello").green()` and get god-tier output.

### Phase 11: Dual-Paradigm Support

**Goal:** Accept both Ink-style (Claude) and ratatui-style (Codex) patterns.

| Task | Description | Status |
|------|-------------|--------|
| `Into<Content>` trait | Universal content conversion | Planned |
| `Line`/`Span` types | ratatui-compatible types | Existing (needs refinement) |
| `Box`/`Text` components | Ink-compatible components | Existing |
| Pattern tests | Verify both AI paradigms produce identical output | Planned |

**Deliverable:** Claude-style `column![text!("Hi")]` and Codex-style `vec![Line::from("Hi")]` render identically.

### Phase 12: God-Tier Defaults

**Goal:** Same API call produces dramatically better output than DIY.

| Task | Description | Status |
|------|-------------|--------|
| Error component | Boxed error with context, hints, suggestions | Planned |
| Success component | Checkmark, celebration formatting | Planned |
| Code component | Line numbers, language detection, copy hint | Existing (enhance) |
| Diff component | Side-by-side, color-coded, navigable | Existing (enhance) |
| Progress component | Smooth animation, ETA, cancelable | Existing (enhance) |

**Deliverable:** `terminal.error("File not found")` produces beautiful boxed error.

### Phase 13: MockTerminal and Testing

**Goal:** First-class testing support for AI-generated apps.

| Task | Description | Status |
|------|-------------|--------|
| `MockTerminal` | Capture all output for assertions | Planned |
| Output assertions | `assert_eq!(mock.output(), [...])` | Planned |
| Input simulation | `mock.queue_input("yes")` | Planned |
| Snapshot testing | Visual snapshot comparison | Planned |

**Deliverable:** AI-generated apps are trivially testable.

---

## Completed Phases

### Phase 1-5: Core Foundation (Complete)
- Node types, style system, Taffy layout
- Buffer, cells, ANSI renderer
- Component library (Box, Text, Input, Select, Progress)
- Hooks system (Signal, use_input, use_focus)
- Macros (vbox![], hbox![], text!())

### Phase 6: Capability Detection (Complete)
- Terminal capability detection
- Tier selection and fallback
- Graceful degradation

### Phase 7: GPU Integration (Complete)
- dterm IPC protocol
- GpuBuffer abstraction
- SharedMemory perception

### Phase 8: AI Components (Complete)
- Markdown renderer
- ChatView for conversations
- DiffView for code changes
- StatusBar with states

### Phase 9: Polish (Complete)
- Documentation
- Examples
- Accessibility audit

---

## Success Criteria

The roadmap succeeds when:

1. **AI Token Efficiency**: `terminal.error(msg)` is shorter than DIY ANSI codes
2. **Output Quality**: Same API call produces 10x better visual output than raw terminal
3. **Dual Support**: Both Claude and Codex generate working code first try
4. **Testability**: Any inky app can be unit tested with MockTerminal
5. **The Wow Test**: People see inky output and ask "what framework is that?"

---

## Non-Goals

Things we explicitly won't do:

| Non-Goal | Rationale |
|----------|-----------|
| Generic widgets (menus, tabs) | Only add what AIs need for god-tier apps |
| Configuration options | Auto-detect everything; zero setup |
| Multiple ways to do things | One canonical way per operation |
| Backward compatibility | Move fast; deprecate without remorse |
| Market share | Excellence over popularity |

---

## Reference

| Document | Content |
|----------|---------|
| `/docs/DESIGN_PHILOSOPHY.md` | Core design principles and API surface |
| `/docs/archive/pre-2026-vision/ARCHITECTURE_PLAN.md` | Original technical architecture details |
| `/docs/archive/2026-01-design-exploration/` | Design conversation exploring AI-first philosophy |
| `/AI_TECHNICAL_SPEC.md` | Technical specification for AI agents |
