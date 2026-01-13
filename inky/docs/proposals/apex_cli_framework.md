# Project Proposal: Apex - AI-First CLI Framework

**Version:** 1.0
**Date:** 2026-01-07
**Author:** WORKER @ inky
**Status:** Draft for Leadership Review

---

## Problem Statement

When AI code generators (Claude Code, Codex) generate terminal UI code, they produce one of two outcomes:

1. **Raw ANSI escape codes**: Works but looks mediocre, inconsistent, inaccessible
2. **Framework code (ratatui, Ink)**: Better but verbose, AI often gets it wrong

Neither outcome produces **excellent** terminal applications. The AI takes the shortest path (fewer tokens), and current frameworks require more tokens than DIY ANSI, so AI often skips them.

### Who Needs This?

- **AI Code Generators**: Claude Code, Codex, Gemini Code
- **End Users**: People using AI-generated terminal applications
- **Developers**: Building tools that AI will extend or modify

---

## Proposed Solution: Apex

**Apex** is a high-level CLI framework where:
1. The framework API is **shorter to use than to avoid**
2. Default output is **beautiful** without extra effort
3. AI generates **consistent, excellent code** every time

### Core Innovation: Semantic Terminal API

```rust
// Apex: ~8 tokens, beautiful output
terminal.error("File not found: config.yaml");

// Raw ANSI: ~50 tokens, mediocre output
print!("\x1b[31m");
print!("Error: ");
print!("\x1b[1m");
print!("{}", msg);
print!("\x1b[0m\n");
```

The Apex version produces a beautiful boxed error with context:

```
┌─ Error ─────────────────────────────────────────────┐
│ ✗ File not found: config.yaml                       │
│                                                     │
│   Searched in:                                      │
│   • ./config.yaml                                   │
│   • ~/.config/myapp/config.yaml                     │
│                                                     │
│   Run 'myapp init' to create a config file.        │
└─────────────────────────────────────────────────────┘
```

### API Surface

**Display (Output):**
```rust
terminal.say(text)             // Plain text
terminal.say(text).red()       // Styled text
terminal.error(text)           // Error (beautiful)
terminal.success(text)         // Success (beautiful)
terminal.warn(text)            // Warning (beautiful)
terminal.info(text)            // Info (beautiful)
```

**Rich Content:**
```rust
terminal.code(text, lang)      // Syntax highlighted code
terminal.diff(a, b)            // Beautiful diff view
terminal.table(data)           // Formatted table
terminal.list(items)           // Bulleted list
terminal.tree(data)            // Tree view
terminal.json(data)            // Pretty JSON
terminal.markdown(text)        // Rendered markdown
```

**Input:**
```rust
terminal.ask(q)                // Free text
terminal.confirm(q)            // Yes/no
terminal.select(options)       // Pick one
terminal.multiselect(options)  // Pick many
terminal.password(q)           // Hidden input
```

**Progress:**
```rust
terminal.progress(ratio)       // Progress bar
terminal.spinner(msg)          // Spinner
terminal.status(msg)           // Status line (replaceable)
```

**Streaming (First-Class):**
```rust
terminal.stream(source).await; // Stream AI output with interruption
```

---

## Design Principles

### Principle 1: Shorter Than DIY

If the framework requires more tokens than raw ANSI codes, AI will skip it.

**The framework MUST be shorter. AI takes the shortest path.**

### Principle 2: One Canonical Way

Multiple ways to do the same thing = inconsistent AI output.

```rust
// ONE way to show an error:
terminal.error(msg);

// NOT five ways:
terminal.print_error(msg);
terminal.show_error(msg);
terminal.display(Error::new(msg));
terminal.emit(Content::Error(msg));
```

### Principle 3: God-Tier Defaults

When AI uses the framework correctly, output must be exceptional.

Same API call → dramatically better output.

### Principle 4: Accept Both Paradigms

Accept both Ink-style (Claude) and ratatui-style (Codex) patterns:

```rust
// Claude-style (Ink/React patterns):
terminal.say(column![
    text!("Success!").green(),
    text!("Operation complete.")
]);

// Codex-style (ratatui patterns):
terminal.say(vec![
    Line::styled("Success!", Color::Green),
    Line::from("Operation complete."),
]);

// Both produce IDENTICAL output.
```

### Principle 5: Accessibility Is Structural

Accessibility comes from semantics, not annotations:

```rust
terminal.error(msg);
// Framework KNOWS it's an error, automatically:
// - Announces to screen readers
// - Sets appropriate ARIA role
// - Provides navigation structure
```

---

## Technical Architecture

### Relationship to Inky

Apex would **build on top of inky**, not replace it:

```
┌─────────────────────────────────────────┐
│              Apex                       │  ← Semantic API
│   terminal.say(), terminal.error()      │
├─────────────────────────────────────────┤
│              Inky                       │  ← Component/Layout
│   BoxNode, TextNode, Taffy Layout       │
├─────────────────────────────────────────┤
│           Crossterm                     │  ← Terminal I/O
└─────────────────────────────────────────┘
```

### Features to Include

From the archived inky ambitious vision:

| Feature | Description | Complexity |
|---------|-------------|------------|
| Semantic Terminal API | `terminal.say()`, `terminal.error()` | Medium |
| God-Tier Defaults | Beautiful boxed errors, code display | Medium |
| Dual-Paradigm Support | Accept Ink + ratatui patterns | Low |
| AI Perception APIs | `as_text()`, `as_tokens()`, `semantic_diff()` | Already done in inky |
| Accessibility | ARIA roles, announcements | Already done in inky |
| MockTerminal | First-class testing support | Medium |
| Streaming | First-class interruption handling | Medium |

### Features to Exclude (Stay in Inky)

| Feature | Rationale |
|---------|-----------|
| Low-level layout | Already in inky |
| Component library | Already in inky |
| GPU rendering | Optional optimization |

---

## Inspiration & Context

### Origin

This vision emerged during inky development. The original design documents are archived at:
- `/docs/archive/2026-01-ambitious-vision/DESIGN_PHILOSOPHY.md`
- `/docs/archive/2026-01-ambitious-vision/ROADMAP.md`

### Related Work

| Framework | Language | What We Learn |
|-----------|----------|---------------|
| [Click](https://click.palletsprojects.com/) | Python | Clean CLI ergonomics |
| [clap](https://github.com/clap-rs/clap) | Rust | Rust CLI patterns |
| [rich](https://github.com/Textualize/rich) | Python | Beautiful defaults |
| [charm/bubbletea](https://github.com/charmbracelet/bubbletea) | Go | Component model |

### The AI Landscape

| Tool | Current TUI | Why They'd Use Apex |
|------|-------------|---------------------|
| Claude Code | Ink (JS) | Native Rust, shorter API |
| Codex CLI | ratatui | Accepts its patterns, better output |
| Gemini CLI | Ink fork | Same benefits as Claude Code |

---

## Scope

### In Scope

- Semantic terminal API (`terminal.say()`, `terminal.error()`, etc.)
- God-tier default rendering
- Dual-paradigm content acceptance
- MockTerminal for testing
- AI perception integration
- Accessibility by default

### Explicitly Out of Scope

- Competing with inky (uses inky as foundation)
- Generic TUI widgets (menus, tabs, modals)
- Configuration options (auto-detect everything)
- Multiple ways to do things
- Backward compatibility

---

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `inky` | Component/layout foundation |
| `crossterm` | Terminal I/O (via inky) |
| `taffy` | Flexbox layout (via inky) |

---

## Beneficiaries

| Project | Benefit |
|---------|---------|
| Claude Code Rust port | Excellent terminal UI with minimal code |
| Any AI-generated CLI | Consistent, beautiful output |
| inky | Clear separation of concerns |

---

## Alternatives Considered

### Alternative 1: Add to Inky

**Rejected.** Inky's mission is to be an Ink port. Adding semantic API bloats it and confuses the purpose.

### Alternative 2: Fork Inky

**Rejected.** Creates maintenance burden and divergence. Better to layer on top.

### Alternative 3: Use Rich (Python)

**Rejected.** We need Rust for performance. Rich shows what's possible, but not in our target language.

### Alternative 4: Don't Build This

**Considered.** But the vision is compelling: if AI generates terminal code and the output is automatically excellent, that's a significant improvement over the status quo.

---

## Estimated Scope

| Phase | Work | Est. Commits |
|-------|------|--------------|
| Phase 1: Core API | `Terminal` struct, say/error/success/warn/info | 5-10 |
| Phase 2: Rich Content | code, diff, table, list, tree, json, markdown | 10-15 |
| Phase 3: Input | ask, confirm, select, multiselect, password | 5-10 |
| Phase 4: Progress | progress, spinner, status | 3-5 |
| Phase 5: Streaming | stream with interruption | 5-8 |
| Phase 6: Testing | MockTerminal, assertions, snapshots | 5-10 |
| Phase 7: Polish | Documentation, examples | 5-10 |

**Total: ~40-70 commits**

---

## Success Metrics

1. **AI Token Efficiency**: `terminal.error(msg)` shorter than DIY ANSI
2. **Output Quality**: Same API call produces 10x better visual output
3. **Dual AI Support**: Both Claude and Codex generate working code first try
4. **Testability**: Any Apex app can be unit tested with MockTerminal
5. **The Wow Test**: People see Apex output and ask "what framework is that?"

---

## Self-Review

### Pass 2: Critique

**Substantial concerns:**

1. **Scope creep risk**: The feature list is ambitious. "God-tier defaults" is vague. Need concrete milestones.

2. **inky dependency**: If Apex depends on inky, and inky changes, Apex breaks. Need clear API boundaries.

3. **"Shorter than DIY" may not be achievable**: Some operations genuinely need more code. Claiming "always shorter" sets unrealistic expectations.

**Other concerns:**

- Testing strategy unclear for "beautiful output"
- No performance targets specified
- Accessibility "by default" is hard to verify

### Pass 3: Responses

1. **Scope creep**: Define Phase 1 as MVP (say/error/success/warn/info only). Ship that before expanding.

2. **inky dependency**: Use inky's stable public API only. Define integration tests at boundary. Version-lock if needed.

3. **"Shorter than DIY"**: Soften to "shorter for common patterns." Accept that edge cases may be longer.

4. **Testing beautiful output**: Snapshot testing with golden files. Human review for initial goldens.

5. **Performance**: Inherit inky's targets (<5ms startup, <3ms latency).

6. **Accessibility verification**: Automated ARIA compliance tests. Manual screen reader testing.

---

## Appendix: Archived Design Documents

The full ambitious vision is preserved at:
- `inky/docs/archive/2026-01-ambitious-vision/DESIGN_PHILOSOPHY.md`
- `inky/docs/archive/2026-01-ambitious-vision/ROADMAP.md`

These provide detailed rationale and additional context for this proposal.
