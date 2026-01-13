# inky

| Director | Status |
|:--------:|:------:|
| TOOL | ACTIVE |

**A Rust port of [Ink](https://github.com/vadimdemedes/ink) - React-like terminal UI framework.**

![Tests](https://img.shields.io/badge/tests-872%20passing-brightgreen)
![Clippy](https://img.shields.io/badge/clippy-zero%20warnings-brightgreen)
![Version](https://img.shields.io/badge/version-0.1.0-blue)

**Author:** Andrew Yates
**Copyright:** 2026 Dropbox, Inc. | **License:** Apache 2.0
**Repo:** https://github.com/dropbox/dTOOL/inky

---

## What is inky?

Inky is a **Rust port of Ink**, the React-like terminal UI framework used by Claude Code, Gemini CLI, and many other modern CLIs. It provides:

- **Flexbox layout** via [Taffy](https://github.com/DioxusLabs/taffy)
- **Component model** similar to Ink's React-like approach
- **Reactive hooks** (`use_signal`, `use_input`, `use_focus`)
- **Declarative macros** (`vbox![]`, `hbox![]`, `text!()`)

### Why Port Ink to Rust?

| Metric | JS Ink | inky |
|--------|--------|------|
| Startup | ~100ms | <5ms |
| Memory (empty) | ~30MB | <1MB |
| Frame time | ~15ms | ~4ms |
| Input latency | ~10ms | <3ms |

The Rust version is **10-20x faster** and uses **30x less memory** while providing the same familiar API patterns.

### Use Case: Porting Claude Code

Claude Code currently uses TypeScript + Ink. Inky enables porting to Rust while preserving the mental model:

```typescript
// TypeScript Ink (current Claude Code)
<Box flexDirection="column" padding={1}>
  <Text color="cyan" bold>Hello, Ink!</Text>
</Box>
```

```rust
// Rust inky (port target)
vbox![
    text!("Hello, inky!" => cyan, bold),
].padding(1)
```

---

## Quick Start

```rust
use inky::prelude::*;

fn main() -> Result<()> {
    App::new()
        .render(|_ctx| {
            vbox![
                text!("Hello, inky!" => cyan, bold),
            ]
        })
        .on_key(|_state, key| matches!(key.code, KeyCode::Char('q')))
        .run()
}
```

---

## Core Features

### Flexbox Layout

CSS-like layout powered by Taffy:

```rust
let ui = BoxNode::new()
    .flex_direction(FlexDirection::Row)
    .justify_content(JustifyContent::SpaceBetween)
    .padding(1)
    .child(TextNode::new("Left"))
    .child(TextNode::new("Right"));
```

### Declarative Macros

Concise UI definitions:

```rust
let ui = vbox![
    text!("Header").bold(),
    hbox![
        text!("Left"),
        Spacer::new(),
        text!("Right").color(Color::Cyan),
    ],
    text!("Footer").dim(),
];
```

### Reactive Hooks

State management similar to React hooks:

```rust
let count = use_signal(0);

// Read
let current = count.get();

// Update (triggers re-render)
count.set(current + 1);

// Or use update for read-modify-write
count.update(|n| *n += 1);
```

### Built-in Components

| Component | Description |
|-----------|-------------|
| `BoxNode` | Flexbox container |
| `TextNode` | Styled text with wrapping |
| `Input` | Text input field |
| `Select` | Selection list |
| `Progress` | Progress bar |
| `Spinner` | Loading indicator |
| `Scroll` | Scrollable viewport |
| `Spacer` | Flexible space filler |
| `Stack` | Z-axis layering |

### Async Support (Tokio)

```rust
use inky::prelude::*;

#[derive(Clone)]
enum Msg {
    StreamChunk(String),
    StreamDone,
}

#[tokio::main]
async fn main() -> Result<()> {
    let (app, handle) = AsyncApp::new()
        .message_type::<Msg>()
        .state(String::new())
        .render(|ctx| text!("{}", ctx.state))
        .on_message(|state, msg| {
            match msg {
                Msg::StreamChunk(chunk) => state.push_str(&chunk),
                Msg::StreamDone => return true,
            }
            false
        })
        .build();

    let h = handle.clone();
    tokio::spawn(async move {
        for chunk in ["Hello", " ", "World", "!"] {
            h.send(Msg::StreamChunk(chunk.into())).ok();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        h.send(Msg::StreamDone).ok();
    });

    app.run_async().await
}
```

Requires the `async` feature:
```toml
inky-tui = { version = "0.1", features = ["async"] }
```

### ratatui Compatibility Layer

For gradual migration from ratatui-based apps:

```rust
use inky::compat::ratatui::{InkyBackend, TerminalBackend};

// Use inky as a ratatui backend
let backend = InkyBackend::new()?;
```

---

## Architecture

```
Component Tree → Virtual DOM → Taffy Layout → Diff → Terminal
     ↓              ↓              ↓           ↓        ↓
  render()      Node tree    Layout tree   LineDiff  ANSI output
```

### Module Structure

```
src/
├── lib.rs              # Public API
├── node.rs             # Node types (Box, Text, Static)
├── style.rs            # Colors, dimensions, styles
├── layout.rs           # Taffy flexbox integration
├── app.rs              # Application runner, event loop
├── diff.rs             # Incremental updates
├── render/             # Buffer, cells, painter
├── hooks/              # Signal, input, focus, interval
├── components/         # Built-in components
├── macros.rs           # vbox![], hbox![], text!()
├── terminal/           # Crossterm backend
└── compat/             # ratatui compatibility
```

---

## Advanced Features (Unstable)

These features are implemented but may change:

### AI Perception APIs

Screen reading for AI agents:

```rust
use inky::perception::Perception;

let perception = Perception::new(&buffer);
let text = perception.as_text();
let tokens = perception.as_tokens();
```

### AI Assistant Components

Components for building AI chat interfaces:

```rust
use inky::components::{ChatView, DiffView, StatusBar, Markdown};
```

### Visualization Components

Data visualization:

```rust
use inky::components::{Heatmap, Sparkline, Plot};
```

---

## Installation

```toml
[dependencies]
inky-tui = "0.1"
```

Optional features:
```toml
[dependencies]
inky-tui = { version = "0.1", features = ["async", "image"] }
```

---

## Comparison with Ink

| Feature | JS Ink | inky |
|---------|--------|------|
| Language | JavaScript | Rust |
| Layout | Yoga (Flexbox) | Taffy (Flexbox) |
| Component model | React | Similar hooks/render |
| Memory | ~30MB | <1MB |
| Startup | ~100ms | <5ms |
| Compile-time checks | No | Yes |

---

## Examples

```bash
# Hello world
cargo run --example hello

# Interactive input
cargo run --example input

# Async streaming
cargo run --example async_stream
```

---

## Related Projects

- [Ink](https://github.com/vadimdemedes/ink) - Original React for CLIs (JavaScript)
- [Ratatui](https://github.com/ratatui/ratatui) - Rust TUI framework
- [Taffy](https://github.com/DioxusLabs/taffy) - Flexbox/Grid layout engine
- [Crossterm](https://github.com/crossterm-rs/crossterm) - Terminal I/O

## License

Apache License 2.0 - see [LICENSE](LICENSE) file.
