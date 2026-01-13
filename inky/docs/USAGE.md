# inky Usage Guide

This guide covers inky's Phase 8 components for building AI assistant interfaces like OpenAI Codex CLI.

## Quick Start

```rust
use inky::prelude::*;
use inky::components::{ChatView, ChatMessage, MessageRole, DiffView, DiffLine, StatusBar, StatusState, Markdown};
```

## Components Overview

| Component | Purpose | Key Features |
|-----------|---------|--------------|
| `Markdown` | Render markdown text | Headings, bold, italic, code blocks, lists, links |
| `ChatView` | Conversation history | Role-aware styling, markdown for assistant, scrolling |
| `DiffView` | Code diff display | Add/delete/context lines, line numbers, summaries |
| `StatusBar` | Status indicator | Animated spinner, state colors, custom messages |
| `StreamingText` | Streaming text output | Thread-safe append, ANSI passthrough |
| `Clickable` | Mouse click handler | Click, hover, focus-on-click support |
| `Draggable` | Drag source | Drag events, data transfer |
| `DropZone` | Drop target | Accept/reject drops, enter/leave events |

---

## Markdown Component

Renders markdown text to styled terminal output.

### Basic Usage

```rust
use inky::components::Markdown;

let md = Markdown::new("# Hello World\n\nThis is **bold** and `code`.");
let node = md.to_node();
```

### Supported Features

- **Headings** - `# H1` through `###### H6` with distinct styles
- **Bold** - `**text**` renders in bold
- **Italic** - `*text*` renders in italic
- **Strikethrough** - `~~text~~` renders with strikethrough
- **Inline code** - `` `code` `` renders in cyan
- **Code blocks** - Triple backticks with language indicator
- **Lists** - Unordered (`- item`) and ordered (`1. item`)
- **Links** - `[text](url)` shows text with URL in parentheses
- **Blockquotes** - `> text` with green border marker

### Code Themes

```rust
use inky::components::{Markdown, CodeTheme};

// Dark theme (default) - cyan code
let md = Markdown::new("```rust\nfn main() {}\n```")
    .code_theme(CodeTheme::Dark);

// Light theme - dark gray code
let md = Markdown::new("```rust\nfn main() {}\n```")
    .code_theme(CodeTheme::Light);
```

---

## ChatView Component

Displays conversation history with role-aware styling.

### Basic Usage

```rust
use inky::components::{ChatView, ChatMessage, MessageRole};

let view = ChatView::new()
    .message(ChatMessage::new(MessageRole::User, "Hello"))
    .message(ChatMessage::new(MessageRole::Assistant, "**Hi** there!"));

let node = view.to_node();
```

### Message Roles

| Role | Label | Color | Content Rendering |
|------|-------|-------|-------------------|
| `User` | "You" | Cyan | Plain text |
| `Assistant` | "Assistant" | Green | Markdown |
| `System` | "System" | Gray (dim) | Plain text (dimmed) |

### Adding Multiple Messages

```rust
let messages = vec![
    ChatMessage::new(MessageRole::User, "Fix the bug"),
    ChatMessage::new(MessageRole::Assistant, "I'll fix it by..."),
    ChatMessage::new(MessageRole::User, "Thanks!"),
];

let view = ChatView::new().messages(messages);
```

### Timestamps

```rust
let msg = ChatMessage::new(MessageRole::User, "Hello")
    .timestamp("10:15 AM");

let view = ChatView::new()
    .show_timestamps(true)
    .message(msg);
```

### Scrolling

```rust
// Show only 5 messages, starting from offset 2
let view = ChatView::new()
    .messages(messages)
    .max_visible(5)
    .scroll_offset(2);
```

### Message Grouping

ChatView automatically adds spacing between messages from different roles, creating visual groups for user/assistant exchanges.

---

## DiffView Component

Displays code diffs with syntax highlighting.

### Basic Usage

```rust
use inky::components::{DiffView, DiffLine};

let view = DiffView::new()
    .file_path("src/main.rs")
    .line(DiffLine::context(1, "fn main() {"))
    .line(DiffLine::delete(2, "    println!(\"old\");"))
    .line(DiffLine::add(2, "    println!(\"new\");"))
    .line(DiffLine::context(3, "}"));

let node = view.to_node();
```

### Line Types

| Type | Factory | Symbol | Color |
|------|---------|--------|-------|
| Added | `DiffLine::add(line_num, content)` | `+` | Green |
| Deleted | `DiffLine::delete(line_num, content)` | `-` | Red |
| Context | `DiffLine::context(line_num, content)` | ` ` | Dim |
| Separator | `DiffLine::hunk_separator()` | `⋮` | Dim |

### Adding Multiple Lines

```rust
let lines = vec![
    DiffLine::context(1, "fn main() {"),
    DiffLine::add(2, "    let x = 5;"),
    DiffLine::context(3, "}"),
];

let view = DiffView::new().lines(lines);
```

### Hunk Separators

Use hunk separators to indicate non-contiguous sections:

```rust
let view = DiffView::new()
    .line(DiffLine::context(10, "// First section"))
    .line(DiffLine::add(11, "// Added here"))
    .line(DiffLine::hunk_separator())
    .line(DiffLine::context(50, "// Later section"))
    .line(DiffLine::delete(51, "// Removed here"));
```

### Display Options

```rust
let view = DiffView::new()
    .file_path("src/lib.rs")
    .show_line_numbers(true)   // Show line number gutter (default: true)
    .show_summary(true)        // Show +N/-M summary (default: true)
    .line(DiffLine::add(1, "new"));
```

### Output Format

```
src/main.rs (+1 -1)
 1  fn main() {
 2 -    println!("old");
 2 +    println!("new");
 3  }
```

---

## StatusBar Component

Displays operational status with animated spinners.

### Basic Usage

```rust
use inky::components::{StatusBar, StatusState};

let status = StatusBar::new()
    .state(StatusState::Thinking)
    .message("Processing request...");

let node = status.to_node();
```

### Status States

| State | Color | Indicator | Default Label |
|-------|-------|-----------|---------------|
| `Idle` | Green | `●` | "Ready" |
| `Thinking` | Yellow | Spinner | "Thinking" |
| `Executing` | Blue | Spinner | "Executing" |
| `Error` | Red | `✗` | "Error" |

### Animation

For `Thinking` and `Executing` states, call `tick()` to animate the spinner:

```rust
let mut status = StatusBar::new()
    .state(StatusState::Thinking);

// In your render/update loop:
status.tick();
let node = status.to_node();
```

### Custom Spinner Style

```rust
use inky::components::{StatusBar, StatusState, SpinnerStyle};

let status = StatusBar::new()
    .state(StatusState::Executing)
    .spinner_style(SpinnerStyle::Circle);
```

Available spinner styles: `Dots` (default), `Line`, `Circle`, `Box`, `Arrow`.

### Inspecting State

```rust
let status = StatusBar::new()
    .state(StatusState::Thinking)
    .message("Custom");

assert!(status.current_state().is_active());
assert_eq!(status.current_message(), "Custom");
```

---

## StreamingText Component

Efficient token-by-token rendering for streaming output.

### Basic Usage

```rust
use inky::components::StreamingText;

let stream = StreamingText::new();
let handle = stream.handle();

handle.append("Hello ");
handle.append("world!");

let node = stream.to_node();
```

### ANSI Passthrough

ANSI parsing is enabled by default for streaming output:

```rust
use inky::components::StreamingText;

let stream = StreamingText::new().parse_ansi(true);
stream.append("\x1b[32mOK\x1b[0m");
```

For static text, use `TextNode::from_ansi`:

```rust
use inky::prelude::*;

let text = TextNode::from_ansi("\x1b[31mError:\x1b[0m file not found");
```

---

## Mouse Interaction Components

inky provides components for building interactive mouse-driven interfaces.

### Clickable Component

Wraps any node to make it respond to mouse clicks.

```rust
use inky::prelude::*;
use inky::components::Clickable;

let button = Clickable::new(TextNode::new("Click me!"))
    .on_click(|event| {
        println!("Clicked at ({}, {})", event.local_x, event.local_y);
    });
```

#### Hover State

Track hover state for visual feedback:

```rust
let button = Clickable::new(TextNode::new("Hover me!"))
    .hover_background(Color::Blue)
    .on_hover(|| println!("Mouse entered!"))
    .on_unhover(|| println!("Mouse left!"));
```

#### Focus on Click

Integrate with focus management for keyboard navigation:

```rust
use inky::hooks::{use_focus, FocusHandle};

let focus = use_focus();
let button = Clickable::new(TextNode::new("Click to focus"))
    .focus_on_click(focus.clone())
    .on_click(|_| println!("Clicked!"));
```

### Draggable Component

Makes any node draggable for drag-and-drop interactions.

```rust
use inky::prelude::*;
use inky::components::Draggable;

let item = Draggable::new(TextNode::new("Drag me!"))
    .on_drag_start(|event| {
        println!("Started dragging from ({}, {})", event.start_x, event.start_y);
        true // Allow drag to start
    })
    .on_drag(|event| {
        println!("Dragging... delta: ({}, {})", event.delta_x, event.delta_y);
    })
    .on_drag_end(|event| {
        println!("Drag ended at ({}, {})", event.current_x, event.current_y);
    });
```

#### Drag Data

Attach data to draggables for transfer to drop zones:

```rust
use std::sync::Arc;

let item = Draggable::new(TextNode::new("Item 1"))
    .drag_data(Arc::new("item-1".to_string()));
```

### DropZone Component

Creates areas that accept dropped items.

```rust
use inky::prelude::*;
use inky::components::DropZone;

let zone = DropZone::new(TextNode::new("Drop here"))
    .on_drop(|event| {
        println!("Received drop at ({}, {})", event.drop_x, event.drop_y);
    })
    .on_drag_enter(|| {
        println!("Draggable entered zone");
    })
    .on_drag_leave(|| {
        println!("Draggable left zone");
    });
```

#### Accepting Drops

Selectively accept or reject drops:

```rust
let zone = DropZone::new(TextNode::new("Drop files here"))
    .accept_drop(|event| {
        // Check the drag data to decide if we accept
        event.data.is_some()
    })
    .on_drop(|event| {
        println!("Accepted drop!");
    });
```

### Mouse Interaction API Summary

| Component | Method | Description |
|-----------|--------|-------------|
| `Clickable` | `.on_click(fn)` | Handle click events |
| | `.on_hover(fn)` | Handle mouse enter |
| | `.on_unhover(fn)` | Handle mouse leave |
| | `.hover_background(Color)` | Set hover background color |
| | `.focus_on_click(FocusHandle)` | Auto-focus when clicked |
| `Draggable` | `.on_drag_start(fn)` | Handle drag start (return bool) |
| | `.on_drag(fn)` | Handle drag movement |
| | `.on_drag_end(fn)` | Handle drag completion |
| | `.drag_data(Arc<T>)` | Attach transfer data |
| `DropZone` | `.on_drop(fn)` | Handle drops |
| | `.on_drag_enter(fn)` | Handle drag enter |
| | `.on_drag_leave(fn)` | Handle drag leave |
| | `.accept_drop(fn)` | Filter acceptable drops |

---

## Composing Components

### Full AI Assistant Layout

Here's how to compose components into a complete interface (from `codex_tui` example):

```rust
use inky::prelude::*;
use inky::components::{
    ChatMessage, ChatView, DiffLine, DiffView,
    MessageRole, StatusBar, StatusState,
};

fn build_chat(messages: &[ChatMessage], height: u16) -> Node {
    let view = ChatView::new()
        .messages(messages.iter().cloned())
        .show_timestamps(false);

    BoxNode::new()
        .flex_grow(1.0)
        .height(height)
        .padding_xy(1.0, 0.0)
        .flex_direction(FlexDirection::Column)
        .child(view.to_node())
        .into()
}

fn build_diff() -> Node {
    let diff = DiffView::new()
        .file_path("src/main.rs")
        .line(DiffLine::context(1, "fn main() {"))
        .line(DiffLine::delete(2, "    if x == null {"))
        .line(DiffLine::add(2, "    if let Some(x) = x {"))
        .line(DiffLine::context(3, "        process(x);"))
        .line(DiffLine::context(4, "    }"))
        .line(DiffLine::context(5, "}"));

    BoxNode::new()
        .padding_xy(1.0, 1.0)
        .flex_direction(FlexDirection::Column)
        .child(diff.to_node())
        .into()
}

fn build_status_bar(state: StatusState, message: &str, frame: usize, width: u16) -> Node {
    let mut status = StatusBar::new().state(state).message(message);

    // Advance spinner animation
    for _ in 0..frame {
        status.tick();
    }

    BoxNode::new()
        .width(width)
        .height(1)
        .padding_xy(1.0, 0.0)
        .child(status.to_node())
        .into()
}
```

### Complete App Structure

```rust
fn main() -> Result<()> {
    App::new()
        .alt_screen(true)
        .render(|ctx| {
            let width = ctx.width();
            let height = ctx.height();

            BoxNode::new()
                .width(width)
                .height(height)
                .flex_direction(FlexDirection::Column)
                .child(build_header(width))
                .child(build_chat(&messages, chat_height))
                .child(build_diff())
                .child(build_status_bar(StatusState::Idle, "Ready", 0, width))
                .child(build_input(&input_text, width))
                .into()
        })
        .on_key(|state, key| {
            match key.code {
                KeyCode::Esc => true,  // Quit
                _ => false,
            }
        })
        .run()
}
```

---

## Running the Demo

```bash
cargo run --example codex_tui
```

Controls:
- **Type**: Enter text in the input field
- **Enter**: Submit message (cycles through demo states)
- **Ctrl+D**: Toggle diff view
- **Esc** or **Ctrl+Q**: Quit

---

## Async App Loop (Tokio)

Enable the `async` feature to use `AsyncApp`:

```toml
inky = { version = "0.1", features = ["async"] }
```

```rust
use inky::app::AsyncApp;
use inky::prelude::*;

#[derive(Clone, Default)]
struct AppState {
    text: String,
}

#[derive(Clone)]
enum Msg {
    Append(String),
    Quit,
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = AsyncApp::new()
        .state(AppState::default())
        .message_type::<Msg>()
        .render(|ctx| TextNode::new(&ctx.state.text).into())
        .on_message(|state, msg| {
            match msg {
                Msg::Append(text) => state.text.push_str(&text),
                Msg::Quit => return true,
            }
            false
        });

    let handle = app.async_handle();
    tokio::spawn(async move {
        handle.send(Msg::Append("Hello".into()));
        handle.render();
    });

    app.run_async().await
}
```

---

## Custom Widgets

Implement `Widget` when you need custom rendering logic:

```rust
use inky::node::{CustomNode, Widget, WidgetContext};
use inky::prelude::{Color, Node};
use inky::render::{Cell, Painter};

struct Gauge {
    value: f32,
}

impl Widget for Gauge {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        let filled = (ctx.width as f32 * self.value) as u16;
        for x in 0..filled.min(ctx.width) {
            let cell = Cell::new('=').with_fg(Color::Green);
            painter.buffer_mut().set(ctx.x + x, ctx.y, cell);
        }
    }

    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        (available_width, 1)
    }
}

let node: Node = CustomNode::new(Gauge { value: 0.7 }).into();
```

---

## API Summary

### Markdown

| Method | Description |
|--------|-------------|
| `Markdown::new(content)` | Create with markdown content |
| `.code_theme(CodeTheme)` | Set code block theme |
| `.to_node()` | Convert to Node for rendering |

### ChatView

| Method | Description |
|--------|-------------|
| `ChatView::new()` | Create empty chat view |
| `.message(ChatMessage)` | Add single message |
| `.messages(impl IntoIterator)` | Add multiple messages |
| `.show_timestamps(bool)` | Toggle timestamp display |
| `.max_visible(usize)` | Limit visible messages |
| `.scroll_offset(usize)` | Set scroll position |
| `.to_node()` | Convert to Node |

### ChatMessage

| Method | Description |
|--------|-------------|
| `ChatMessage::new(role, content)` | Create message |
| `.timestamp(str)` | Add timestamp |

### DiffView

| Method | Description |
|--------|-------------|
| `DiffView::new()` | Create empty diff view |
| `.file_path(str)` | Set file path header |
| `.line(DiffLine)` | Add single line |
| `.lines(impl IntoIterator)` | Add multiple lines |
| `.show_line_numbers(bool)` | Toggle line numbers |
| `.show_summary(bool)` | Toggle +N/-M summary |
| `.to_node()` | Convert to Node |

### DiffLine

| Method | Description |
|--------|-------------|
| `DiffLine::add(line_num, content)` | Create added line |
| `DiffLine::delete(line_num, content)` | Create deleted line |
| `DiffLine::context(line_num, content)` | Create context line |
| `DiffLine::hunk_separator()` | Create separator |

### StatusBar

| Method | Description |
|--------|-------------|
| `StatusBar::new()` | Create with Idle state |
| `.state(StatusState)` | Set status state |
| `.message(str)` | Set custom message |
| `.spinner_style(SpinnerStyle)` | Set spinner style |
| `.tick()` | Advance spinner animation |
| `.current_state()` | Get current state |
| `.current_message()` | Get display message |
| `.to_node()` | Convert to Node |

### StreamingText

| Method | Description |
|--------|-------------|
| `StreamingText::new()` | Create empty stream |
| `.with_content(str)` | Create with initial content |
| `.handle()` | Get thread-safe append handle |
| `.append(str)` | Append from the same thread |
| `.parse_ansi(bool)` | Enable or disable ANSI parsing |
| `.to_node()` | Convert to Node |

### StatusState

| Method | Description |
|--------|-------------|
| `.is_active()` | Returns true for Thinking/Executing |
| `.color()` | Get state color |
| `.label()` | Get default label |
| `.indicator()` | Get status symbol |

---

## Integration with claude_code_rs

These components are designed for seamless integration with `claude_code_rs`. Key features:

1. **Type-safe APIs** - Builder patterns prevent invalid states
2. **From<T> for Node** - All components convert directly to nodes
3. **Default implementations** - Sensible defaults reduce boilerplate
4. **No panics** - Components handle edge cases gracefully
5. **Minimal allocations** - Designed for real-time rendering

Example integration pattern:

```rust
use inky::prelude::*;
use inky::components::{ChatView, ChatMessage, MessageRole, StatusBar, StatusState};

// Your state management
struct AppState {
    messages: Vec<ChatMessage>,
    status: StatusState,
}

// Render function
fn render(state: &AppState) -> Node {
    vbox![
        ChatView::new()
            .messages(state.messages.iter().cloned())
            .to_node(),
        StatusBar::new()
            .state(state.status)
            .to_node(),
    ]
}
```
