# Inky Manifesto: God Tier 2026 Terminal Apps

**Date:** 2026-01-01
**Authors:** Human + AI collaboration
**Purpose:** Define what inky IS and ISN'T

---

## What Inky Is

Inky is a terminal UI framework designed for **one purpose**:

**Enable Claude Code and OpenAI Codex to generate GOD TIER terminal applications.**

We don't care about:
- Being the most popular framework
- Supporting every use case
- Competing with ratatui, tui-rs, Ink, or blessed
- Backward compatibility with 2020 patterns

We care about:
- **Claude Code** generating excellent terminal apps
- **OpenAI Codex** generating excellent terminal apps
- The output being **exceptional**, not "good enough"

---

## The Two Users

### User 1: The AI

Claude Code or Codex. They:
- Generate code fresh each time (no memory)
- Take the shortest path (fewer tokens)
- Won't use abstractions unless forced
- Produce sprawling, poor code without constraints
- Need strong opinions to produce consistent output

**The framework must guide them to excellence.**

### User 2: The Human

Using the terminal app the AI built. They:
- Expect beautiful, responsive UI
- Expect accessibility
- Expect it to work over SSH, in tmux
- Expect 2026 quality, not 2016 quality

**The framework must deliver excellence to them.**

---

## God Tier: What It Means

| Aspect | Mediocre (2016) | God Tier (2026) |
|--------|-----------------|-----------------|
| **Streaming** | Blocks until done | Character by character, instant |
| **Code display** | Monochrome dump | Syntax highlighted, line numbers, copy |
| **Diffs** | `diff` output | Side-by-side, color-coded, navigable |
| **Questions** | `[y/n]` prompt | Beautiful inline, keyboard nav |
| **Progress** | Spinner | Smooth bar, ETA, cancelable |
| **Errors** | Red text | Structured, actionable, pretty |
| **Accessibility** | None | Full screen reader support |
| **Performance** | Janky | 60fps, instant response |

**God tier means the best terminal UI you've ever seen.**

---

## The Design Constraints

### Constraint 1: AI-First API

The API must be what Claude and Codex naturally generate.

```rust
// This is what AI should generate:
terminal.say("Hello");
terminal.error("File not found");
terminal.code(source, "rust");
terminal.confirm("Continue?");

// This is what AI SHOULDN'T have to generate:
let style = StyleBuilder::new()
    .foreground(Color::Rgb(255, 0, 0))
    .modifier(Modifier::Bold)
    .build();
terminal.render(StyledText::new("Hello").with_style(style));
```

### Constraint 2: God Tier Output

When AI uses the framework correctly, output must be exceptional.

```rust
terminal.error("File not found: config.yaml");

// This produces:
// ┌─ Error ─────────────────────────────────────────────┐
// │ ✗ File not found: config.yaml                       │
// │                                                     │
// │   Looked in:                                        │
// │   • ./config.yaml                                   │
// │   • ~/.config/myapp/config.yaml                    │
// │   • /etc/myapp/config.yaml                         │
// │                                                     │
// │   Run 'myapp init' to create a config file.        │
// └─────────────────────────────────────────────────────┘

// Not just:
// Error: File not found: config.yaml
```

### Constraint 3: The Two AI Test

Every feature must work well for BOTH:
- **Claude Code** (tends toward verbose, careful)
- **Codex** (tends toward terse, fast)

If either AI struggles to use a feature correctly, redesign it.

### Constraint 4: No Middle Ground

Either it's god tier or we don't ship it. No "good enough" features.

---

## The Core Principles

### Principle 1: Shorter Than DIY

```rust
// If this is shorter:
print!("\x1b[31mError: {}\x1b[0m\n", msg);

// Than this:
terminal.error(msg);

// We've failed. The framework MUST win on brevity.
```

### Principle 2: Smart by Default

```rust
// AI writes:
terminal.code(source, "rust");

// Framework delivers:
// - Syntax highlighting (detected theme)
// - Line numbers
// - Scrollable if long
// - Copy button (if mouse supported)
// - Fallback to plain text (if basic terminal)
// - Screen reader: "code block, 47 lines, Rust"

// AI didn't ask for any of this. It just happens.
```

### Principle 3: One Way

```rust
// There's ONE way to show an error:
terminal.error(msg);

// Not:
terminal.print_error(msg);
terminal.write_error(msg);
terminal.show_error(msg);
terminal.display(Error::new(msg));
terminal.emit(Content::Error(msg));
```

### Principle 4: Progressive Detail (0-4)

```rust
// Level 0: Just works
terminal.error(msg);

// Level 1: Customize message
terminal.error(msg).hint("Try 'init' to create config");

// Level 2: Full structure
terminal.error(Error::new(msg)
    .hint("Try 'init' to create config")
    .searched(["./config.yaml", "~/.config/myapp"]));

// Level 3: Custom rendering
terminal.error(MyCustomError::new(msg));

// Level 4: Raw
terminal.raw().write(b"\x1b[31mError...");
```

### Principle 5: Streaming Native

```rust
// AI output streams. This is first-class.
terminal.stream(ai_response).await;

// Interruption is normal, not exceptional.
match terminal.stream(ai_response).await {
    Completed(text) => { ... }
    Interrupted(partial) => { ... }
}
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

---

## What We Don't Do

### We don't do: Generic TUI widgets

No buttons, no menus, no tabs, no modals for their own sake.
If Claude or Codex needs it for a god-tier app, we add it.
Otherwise, not our problem.

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

If we can't make it god tier, we don't ship it.

---

## Success Metrics

### Metric 1: AI Generation Quality

When Claude Code generates terminal code using inky:
- Is it consistent across sessions?
- Is it minimal (few tokens)?
- Does it produce god-tier output?

### Metric 2: End User Experience

When a human uses an AI-generated inky app:
- Is it beautiful?
- Is it accessible?
- Is it fast?
- Does it feel 2026, not 2016?

### Metric 3: The Wow Test

When someone sees an inky-powered terminal app:
- Do they say "wow"?
- Do they ask "what framework is that?"
- Do they want to use it?

---

## The Competition

| Framework | Focus | Our Advantage |
|-----------|-------|---------------|
| ratatui | General TUI | We're AI-optimized, god-tier output |
| tui-rs | Low-level TUI | We're high-level, AI-friendly |
| Ink (JS) | React for terminals | We're Rust, native, faster |
| blessed | Node.js TUI | We're Rust, AI-optimized |
| Raw ANSI | Maximum control | We're easier AND better looking |

**We don't compete on features. We compete on excellence.**

---

## The Vision

```
2026: An AI is asked to build a terminal app.

Instead of generating sprawling ANSI code,
it generates:

    use inky::prelude::*;

    terminal.say("Welcome to MyApp").bold();
    let name = terminal.ask("What's your name?");
    terminal.success(format!("Hello, {}!", name));

And the output is BEAUTIFUL.

Screen reader announces everything correctly.
Works over SSH.
60fps smooth.
Looks better than any terminal app you've seen.

The AI didn't try hard.
It just used inky.
Inky made it god tier.
```

**That's the goal. Nothing less.**

---

## The Mantra

> "Claude Code and Codex generate god-tier terminal apps with inky. That's the product. Everything else is noise."
