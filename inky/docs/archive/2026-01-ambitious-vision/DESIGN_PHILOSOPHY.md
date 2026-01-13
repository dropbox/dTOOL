# Inky Design Philosophy

**Version:** 1.0
**Date:** 2026-01-01
**Status:** Authoritative

---

## Mission

**Enable Claude Code and OpenAI Codex to generate god-tier terminal applications.**

We don't optimize for:
- Popularity or market share
- Supporting every use case
- Competing with ratatui, Ink, or blessed
- Backward compatibility with legacy patterns

We optimize for:
- Claude Code generating excellent terminal apps
- OpenAI Codex generating excellent terminal apps
- End users experiencing exceptional quality

---

## The Two Users

### User 1: The AI (Developer)

Claude Code or Codex generates the code. They:
- Generate code fresh each time (no memory between sessions)
- Optimize for fewer tokens (shorter = cheaper = more likely)
- Won't use abstractions unless they're obviously beneficial
- Produce sprawling, poor code without strong constraints
- Need one canonical way to do each thing

**The framework must guide AI to excellent code.**

### User 2: The Human (End User)

Uses the terminal app the AI built. They:
- Expect beautiful, responsive UI
- Expect accessibility (screen readers, keyboard navigation)
- Expect it to work everywhere (SSH, tmux, basic terminals)
- Expect 2026 quality, not 2016 quality

**The framework must deliver excellence to them.**

---

## Core Principles

### Principle 1: Shorter Than DIY

If the framework requires more tokens than raw ANSI codes, AI will skip it.

```rust
// DIY: ~50 tokens
print!("\x1b[31m");
print!("Error: ");
print!("\x1b[1m");
print!("{}", msg);
print!("\x1b[0m\n");

// Inky: ~8 tokens
terminal.error(msg);
```

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

**One verb per action. AI generates the same code every time.**

### Principle 3: Smart Defaults, Progressive Depth

Level 0 just works. Levels 1-4 add control without changing the API shape.

```rust
// Level 0: Just works
terminal.error(msg);

// Level 1: Add context
terminal.error(msg).hint("Run 'init' to create config");

// Level 2: Full structure
terminal.error(Error::new(msg)
    .hint("Run 'init' to create config")
    .searched(["./config.yaml", "~/.config/myapp"]));

// Level 3: Custom rendering
terminal.error(MyCustomError::new(msg));

// Level 4: Raw access
terminal.raw().write(b"\x1b[31mError...");
```

**Same API shape at all levels. No "level jumping" required.**

### Principle 4: God-Tier Output

When AI uses the framework correctly, output must be exceptional.

```rust
terminal.error("File not found: config.yaml");

// Produces:
// ┌─ Error ─────────────────────────────────────────────┐
// │ ✗ File not found: config.yaml                       │
// │                                                     │
// │   Searched in:                                      │
// │   • ./config.yaml                                   │
// │   • ~/.config/myapp/config.yaml                     │
// │                                                     │
// │   Run 'myapp init' to create a config file.        │
// └─────────────────────────────────────────────────────┘

// NOT just:
// Error: File not found: config.yaml
```

**Same API call. Dramatically better output.**

### Principle 5: Accept Both Paradigms

Claude Code (trained on Ink/React) and Codex (trained on ratatui) generate different patterns. Accept both.

```rust
// Claude-style (Ink/React patterns):
terminal.say(
    column![
        text!("Success!").green(),
        text!("Operation complete.")
    ]
);

// Codex-style (ratatui patterns):
terminal.say(vec![
    Line::styled("Success!", Color::Green),
    Line::from("Operation complete."),
]);

// Both produce IDENTICAL output.
```

**The framework doesn't care how you describe UI. It cares that UI is god-tier.**

### Principle 6: Streaming Native

AI output streams character by character. This is first-class, not exceptional.

```rust
// Streaming is the default
terminal.stream(ai_response).await;

// Interruption is normal
match terminal.stream(ai_response).await {
    Completed(text) => { /* AI finished */ }
    Interrupted(partial) => { /* User interrupted */ }
}
```

### Principle 7: Accessibility Is Structural

Accessibility comes from semantics, not annotations.

```rust
terminal.error(msg);
// Framework KNOWS it's an error, automatically:
// - Announces to screen readers
// - Sets appropriate ARIA role
// - Provides navigation structure

terminal.code(source, "rust");
// Framework KNOWS it's code, automatically:
// - Announces "code block, 47 lines, Rust"
// - Provides line-by-line navigation
// - Enables copy without line numbers
```

---

## The API Surface

### Display (Output)

```rust
terminal.say(text)             // Plain text
terminal.say(text).red()       // Styled text
terminal.error(text)           // Error (beautiful)
terminal.success(text)         // Success (beautiful)
terminal.warn(text)            // Warning (beautiful)
terminal.info(text)            // Info (beautiful)
```

### Rich Content

```rust
terminal.code(text, lang)      // Syntax highlighted code
terminal.diff(a, b)            // Beautiful diff view
terminal.table(data)           // Formatted table
terminal.list(items)           // Bulleted list
terminal.tree(data)            // Tree view
terminal.json(data)            // Pretty JSON
terminal.markdown(text)        // Rendered markdown
```

### Input

```rust
terminal.ask(q)                // Free text
terminal.confirm(q)            // Yes/no
terminal.select(options)       // Pick one
terminal.multiselect(options)  // Pick many
terminal.password(q)           // Hidden input
```

### Progress

```rust
terminal.progress(ratio)       // Progress bar
terminal.spinner(msg)          // Spinner
terminal.status(msg)           // Status line (replaceable)
```

### Streaming

```rust
terminal.stream(source)        // Stream content
terminal.append(content)       // Append to output
terminal.replace(region, content) // Replace region
```

### Modifiers (Chainable)

```rust
.red() .green() .blue() .yellow() .cyan() .magenta()
.bold() .dim() .italic() .underline()
```

---

## What We Don't Do

### We don't do: Generic TUI widgets

No buttons, menus, tabs, or modals for their own sake. If Claude or Codex needs it for a god-tier app, we add it. Otherwise, not our problem.

### We don't do: Configuration

```rust
// NO:
let terminal = Terminal::builder()
    .color_mode(TrueColor)
    .encoding(Utf8)
    .theme(Theme::Monokai)
    .build()?;

// YES:
let terminal = Terminal::new();  // Figures it out
```

### We don't do: Multiple ways

Every operation has ONE canonical way. AI generates the same code every time.

### We don't do: Mediocre

If we can't make it god-tier, we don't ship it.

---

## Strong Opinions

| Opinion | Rationale |
|---------|-----------|
| Append-only output with named regions | Most AI output appends; status indicators need replacement |
| Turn-based with first-class interruption | Real conversations have interruption; it's normal, not exceptional |
| Single-agent default, multi-agent via composition | Most apps need one agent; multi-agent is composition, not new API |
| Content types are traits, built-ins are canonical | Common types built-in; custom types implement `Content` trait |
| Testing is first-class | `MockTerminal` from day one, not an afterthought |
| Semantic accessibility | Structure determines accessibility, not annotations |

---

## Success Metrics

### Metric 1: AI Generation Quality

When Claude Code or Codex generates terminal code using inky:
- Is it consistent across sessions?
- Is it minimal (few tokens)?
- Does it produce god-tier output?

### Metric 2: End User Experience

When a human uses an AI-generated inky app:
- Is it beautiful?
- Is it accessible?
- Is it responsive?
- Does it feel 2026, not 2016?

### Metric 3: The Wow Test

When someone sees an inky-powered terminal app:
- Do they say "wow"?
- Do they ask "what framework is that?"
- Do they want to use it?

---

## The Competition

| Framework | Focus | Inky Advantage |
|-----------|-------|----------------|
| ratatui | General Rust TUI | AI-optimized, god-tier output |
| Ink | React for terminals | Native Rust, streaming-first |
| blessed | Node.js TUI | Native Rust, AI-optimized |
| Raw ANSI | Maximum control | Easier AND better looking |

**We don't compete on features. We compete on excellence.**

---

## The Mantra

> "Claude Code and Codex generate god-tier terminal apps with inky. That's the product. Everything else is noise."

---

## Technical Foundation

Inky's technical architecture includes:

### Rendering Tiers

| Tier | Output | Latency | Use Case |
|------|--------|---------|----------|
| **1: ANSI** | Escape codes to stdout | 8-16ms | Any terminal, portable CLIs |
| **2: Retained** | Node tree → diff → minimal updates | 4-8ms | Interactive apps, TUIs |
| **3: GPU Direct** | Zero-copy to GPU cell buffer | <1ms | Real-time visualization, 120 FPS |

### Core Technologies

- **Layout**: [Taffy](https://github.com/DioxusLabs/taffy) flexbox/grid engine
- **I/O**: [Crossterm](https://github.com/crossterm-rs/crossterm) terminal abstraction
- **Cells**: 8-byte GPU-compatible cell structure
- **Perception**: AI agent APIs for screen reading (`as_text()`, `as_tokens()`, `semantic_diff()`)

### Performance Targets

| Metric | JS Ink | inky (Tier 2) | inky (Tier 3) |
|--------|--------|---------------|---------------|
| Startup | ~100ms | <5ms | <5ms |
| Memory | ~30MB | <1MB | <1MB |
| Frame (full redraw) | ~15ms | ~4ms | <1ms |
| Input latency | ~10ms | <3ms | <1ms |

---

## Reference

### Archives

| Archive | Content |
|---------|---------|
| `/docs/archive/2026-01-design-exploration/` | Design conversation exploring AI-first philosophy, two paradigms, pit of success |
| `/docs/archive/pre-2026-vision/` | Original technical vision, architecture plan, rendering tiers, API details |

### Related Documents

- `/docs/ROADMAP.md` - Implementation roadmap
- `/AI_TECHNICAL_SPEC.md` - Technical specification for AI agents
- `/README.md` - Project overview and quick start
