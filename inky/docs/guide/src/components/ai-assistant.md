# AI Assistant Components

These components are designed for building AI assistant interfaces like OpenAI Codex CLI.

## Available Components

| Component | Purpose |
|-----------|---------|
| `Markdown` | Render markdown with headings, bold, italic, code blocks, lists |
| `ChatView` | Conversation history with role-aware styling |
| `DiffView` | Code diffs with add/delete/context lines |
| `StatusBar` | Animated status indicator with spinners |
| `StreamingText` | Thread-safe streaming text with ANSI support |

## StreamingText

Efficient token-by-token rendering for streaming LLM output:

```rust,no_run
use inky::components::StreamingText;

let stream = StreamingText::new();
let handle = stream.handle();

// Append from any thread
handle.append("Hello ");
handle.append("world!");

let node = stream.to_node();
```

### ANSI Passthrough

ANSI parsing is enabled by default:

```rust,no_run
use inky::components::StreamingText;

let stream = StreamingText::new().parse_ansi(true);
stream.append("\x1b[32mOK\x1b[0m");  // Green "OK"
```

For static text, use `TextNode::from_ansi`:

```rust,no_run
use inky::prelude::*;

let text = TextNode::from_ansi("\x1b[31mError:\x1b[0m file not found");
```

## ChatView

Display conversation history with role-aware styling:

```rust,no_run
use inky::components::{ChatView, ChatMessage, MessageRole};

let view = ChatView::new()
    .message(ChatMessage::new(MessageRole::User, "Hello"))
    .message(ChatMessage::new(MessageRole::Assistant, "**Hi** there!"));

let node = view.to_node();
```

Message roles: `User` (cyan), `Assistant` (green, markdown), `System` (dim).

## StatusBar

Animated status indicator:

```rust,no_run
use inky::components::{StatusBar, StatusState};

let mut status = StatusBar::new()
    .state(StatusState::Thinking)
    .message("Processing...");

// Call tick() in your render loop to animate
status.tick();
let node = status.to_node();
```

States: `Idle` (green), `Thinking` (yellow, spinner), `Executing` (blue, spinner), `Error` (red).

## DiffView

Code diff display:

```rust,no_run
use inky::components::{DiffView, DiffLine};

let view = DiffView::new()
    .file_path("src/main.rs")
    .line(DiffLine::context(1, "fn main() {"))
    .line(DiffLine::delete(2, "    old_code();"))
    .line(DiffLine::add(2, "    new_code();"))
    .line(DiffLine::context(3, "}"));

let node = view.to_node();
```

See [docs/USAGE.md](../../../USAGE.md) for complete API reference.
