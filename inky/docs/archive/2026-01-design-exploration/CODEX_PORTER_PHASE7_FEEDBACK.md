# Phase 7 Feedback: Design/API and Feature Improvements for Inky

**Date:** 2026-01-01
**From:** Codex Porter (rigorous audit of 22,842 lines of bridge code)
**Purpose:** Evidence-based recommendations to reduce integration friction

---

## Executive Summary

After porting 80+ widgets and writing **22,842 lines of bridge code**, the core friction isn't missing features—it's **API shape mismatch**. The inky API is designed around a node tree, but TUI apps think in terms of **lines and spans**. This gap forces every widget to:

1. Build `Vec<StyledSpanOwned>` manually
2. Convert to ratatui `Line` for rendering
3. Handle truncation, padding, wrapping manually

The result: 5,465 lines in `inky_widget_integration.rs` alone, consisting mostly of repetitive conversion code.

---

## DESIGN & API IMPROVEMENTS (5+)

### D1: First-Class `Line` Type [HIGH - Eliminates ~3,000 lines of bridge code]

**Evidence:**
```rust
// Current: Porter defined custom type alias
pub type InkyLine = Vec<StyledSpanOwned>;

// 80+ functions follow this pattern:
pub fn render_X_via_inky(...) -> ratatui::Line<'static> {
    let inky_line = inky_X(...);  // Returns Vec<StyledSpanOwned>
    inky_line_to_ratatui(&inky_line)  // Convert manually
}
```

**The Problem:** `TextNode::from_spans()` exists but returns a `Node`, not a line. For line-oriented rendering (chat messages, status bars, lists), we want:
- A line of styled spans
- With optional line-level style (for blockquotes)
- That can be truncated, padded, or wrapped

**Proposed API:**
```rust
// New first-class Line type
pub struct Line {
    spans: Vec<StyledSpan>,
    style: Option<TextStyle>,  // Line-level style (inherited by spans)
}

impl Line {
    pub fn new() -> Self;
    pub fn span(self, text: impl Into<StyledSpan>) -> Self;
    pub fn spans(self, spans: impl IntoIterator<Item = impl Into<StyledSpan>>) -> Self;
    pub fn style(self, style: TextStyle) -> Self;

    // Utilities
    pub fn display_width(&self) -> usize;
    pub fn truncate(&self, max_width: usize, ellipsis: Option<&str>) -> Line;
    pub fn pad(&self, width: usize) -> Line;
}

// Easy conversion
impl From<Line> for TextNode { ... }
impl From<Line> for Node { ... }
```

**Impact:** Would eliminate `inky_line_to_ratatui()`, `inky_span_to_ratatui()`, and the entire `inky_tui_bridge.rs` color conversion layer.

---

### D2: Style Accumulator / Builder [HIGH - Eliminates custom StyleState]

**Evidence:** The porter created their own `StyleState` struct (20 occurrences in `inky_markdown_render.rs`):
```rust
// Porter's custom style state
struct StyleState {
    color: Option<Color>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

impl StyleState {
    fn merge(&self, other: &StyleState) -> StyleState { ... }
    fn apply_to_span(&self, text: String) -> StyledSpanOwned { ... }
}
```

**The Problem:** `TextStyle::merge()` was added, but the pattern needed is:
1. Accumulate style attributes as you parse (markdown, ANSI, etc.)
2. Apply accumulated style to create a span
3. Push span to line, reset or continue accumulating

**Proposed API:**
```rust
// TextStyle should be the accumulator
impl TextStyle {
    // Merge returns new style (already exists)
    pub fn merge(&self, other: &TextStyle) -> TextStyle;

    // Apply to text, creating a span
    pub fn apply(&self, text: impl Into<String>) -> StyledSpan;

    // Or just use Into<StyledSpan>
    impl TextStyle {
        pub fn with_text(self, text: impl Into<String>) -> StyledSpan;
    }
}

// Usage:
let style = TextStyle::new().bold().color(Color::Red);
let span = style.apply("Error:");  // Creates StyledSpan
```

---

### D3: Built-in Line Utilities Module [MEDIUM - Eliminates inky_wrapping.rs]

**Evidence:** Porter wrote 524 lines in `inky_wrapping.rs`:
```rust
// Custom wrapping utilities the porter needed
pub fn inky_word_wrap_line(line: &InkyLine, max_width: usize, opts: InkyRtOptions) -> Vec<InkyLine>
pub fn inky_line_display_width(line: &InkyLine) -> usize
pub fn truncate_inky_line(line: &InkyLine, max_width: usize, ellipsis: Option<&str>) -> InkyLine
pub fn pad_inky_line(line: &InkyLine, width: usize) -> InkyLine
```

**Proposed API:**
```rust
// In inky::text module
pub mod text {
    /// Calculate display width of styled text (handles Unicode, wide chars)
    pub fn display_width(spans: &[StyledSpan]) -> usize;

    /// Word-wrap a line to max width, preserving styles
    pub fn wrap(line: Line, max_width: usize) -> Vec<Line>;

    /// Truncate with optional ellipsis
    pub fn truncate(line: Line, max_width: usize, ellipsis: Option<&str>) -> Line;

    /// Pad to exact width
    pub fn pad(line: Line, width: usize, align: Align) -> Line;
}

// Or as methods on Line (if D1 implemented)
impl Line {
    pub fn wrap(self, max_width: usize) -> Vec<Line>;
    pub fn truncate(self, max_width: usize, ellipsis: Option<&str>) -> Line;
    pub fn pad(self, width: usize) -> Line;
}
```

---

### D4: Simplified Widget Creation [MEDIUM]

**Evidence:** The `Widget` trait requires both `render()` and `measure()`:
```rust
pub trait Widget: Send + Sync {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter);
    fn measure(&self, available_width: u16, available_height: u16) -> (u16, u16);
}
```

**The Problem:** 90% of widgets just want to return styled lines. The current path:
1. Implement Widget trait
2. In measure(), calculate how many lines and their widths
3. In render(), paint each line to the buffer
4. Wrap in CustomNode

**Proposed API:**
```rust
// Simpler widget for line-based content
pub trait SimpleWidget: Send + Sync {
    fn lines(&self, width: u16) -> Vec<Line>;
}

// Automatically implemented Widget
impl<T: SimpleWidget> Widget for T {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        for (i, line) in self.lines(ctx.width).iter().enumerate() {
            painter.draw_line(ctx.x, ctx.y + i as u16, line);
        }
    }

    fn measure(&self, w: u16, _h: u16) -> (u16, u16) {
        let lines = self.lines(w);
        let height = lines.len() as u16;
        let width = lines.iter().map(|l| l.display_width()).max().unwrap_or(0) as u16;
        (width, height)
    }
}
```

---

### D5: Prelude Exports for Common Patterns [LOW]

**Evidence:** Porter imports are verbose:
```rust
use inky::prelude::BoxNode;
use inky::prelude::Color;
use inky::prelude::FlexDirection;
use inky::prelude::StyledSpanOwned;
use inky::prelude::TextNode;
```

**Proposed:** The prelude should include ALL commonly-used items. Current prelude is good but add:
```rust
// Additional prelude exports
pub use crate::text::{Line, wrap, truncate, pad};  // If D3 implemented
pub use crate::style::{TextStyle, BorderStyle};
pub use crate::node::{Node, BoxNode, TextNode, Spacer, CustomNode};
```

---

## FEATURE IMPROVEMENTS (5+)

### F1: Multi-line Text Input Component [HIGH - BLOCKER for textarea.rs port]

**Evidence:** The porter has `inky_textarea.rs` (563 lines) as a wrapper but needs:
- Multi-line text editing with cursor
- Selection (shift+arrow, mouse drag)
- Scroll when content exceeds height
- Line numbers (optional)
- Syntax highlighting hook

**Proposed API:**
```rust
pub struct TextArea {
    // Builder pattern
    pub fn new() -> Self;
    pub fn value(self, text: &str) -> Self;
    pub fn cursor(self, position: usize) -> Self;  // Byte offset
    pub fn selection(self, start: usize, end: usize) -> Self;
    pub fn placeholder(self, text: &str) -> Self;
    pub fn line_numbers(self, show: bool) -> Self;
    pub fn on_change(self, f: impl Fn(&str)) -> Self;
    pub fn on_submit(self, f: impl Fn(&str)) -> Self;  // Ctrl+Enter or configurable
}

// Usage:
TextArea::new()
    .value(&self.input_text)
    .cursor(self.cursor_position)
    .placeholder("Type a message...")
    .on_change(|text| { /* update state */ })
```

---

### F2: Streaming Text Appender [HIGH - For LLM output]

**Evidence:** LLM output streams character by character. Currently must rebuild entire node tree each frame.

**Proposed API:**
```rust
pub struct StreamingText {
    buffer: String,
    parsed_spans: Vec<StyledSpan>,  // Cached parsed ANSI
}

impl StreamingText {
    pub fn new() -> Self;
    pub fn append(&mut self, text: &str);  // Efficiently appends
    pub fn as_node(&self) -> Node;  // Returns TextNode with cached spans
    pub fn clear(&mut self);
}

// For markdown streaming
pub struct StreamingMarkdown {
    raw: String,
    rendered_lines: Vec<Line>,  // Incrementally parsed
}

impl StreamingMarkdown {
    pub fn append(&mut self, text: &str);
    pub fn as_node(&self) -> Node;
}
```

---

### F3: Layout Debugging Mode [MEDIUM]

**Evidence:** From porter feedback (I2, I3):
> "When layout doesn't look right, hard to debug why."
> "Layout failures don't tell you which node caused the problem."

**Proposed API:**
```rust
App::new()
    .debug_layout(true)  // Enables debug mode
    .run()?;

// In debug mode:
// 1. Each node gets a colored border showing its bounds
// 2. Press 'L' to open layout inspector overlay
// 3. Hover over nodes to see:
//    - Node ID and type
//    - Computed x, y, width, height
//    - Style properties (flex_grow, flex_shrink, etc.)
//    - Why this size was chosen
```

---

### F4: Scroll State Persistence [MEDIUM]

**Evidence:** From porter feedback (M2):
> "When re-rendering, scroll position resets unless I manage state externally."

**Proposed API:**
```rust
// Scroll with persistent ID
Scroll::new()
    .id("chat-scroll")  // State persists across re-renders
    .auto_scroll_to_bottom(true)  // For chat UIs
    .on_scroll(|position| { /* optional callback */ })
    .children(messages)

// Programmatic scroll control
let scroll_handle = ScrollHandle::new("chat-scroll");
scroll_handle.scroll_to_bottom();
scroll_handle.scroll_to_line(42);
```

---

### F5: Focus Management API [MEDIUM]

**Evidence:** From porter feedback (M3):
> "How do I programmatically set focus?"
> "How do I know which widget has focus?"

**Proposed API:**
```rust
// In render function, get focus handle
fn render(ctx: &RenderContext) -> Node {
    let input = Input::new()
        .id("user-input")
        .on_focus(|| log::info!("focused"))
        .on_blur(|| log::info!("blurred"));

    // ...
}

// Programmatic control via app handle
app_handle.set_focus("user-input");
let focused_id = app_handle.focused_id();  // Option<String>

// Focus navigation
app_handle.focus_next();  // Tab
app_handle.focus_prev();  // Shift+Tab
```

---

### F6: Table Component [MEDIUM]

**Evidence:** The porter has `inky_pager_views.rs` (676 lines) implementing table-like views manually:
```rust
pub fn inky_resume_picker_columns(width: usize) -> (usize, usize, usize)
pub fn inky_resume_picker_header(updated_width: usize, branch_width: usize, cwd_width: usize) -> InkyLine
pub fn inky_resume_picker_row(...) -> InkyLine
```

**Proposed API:**
```rust
Table::new()
    .columns([
        Column::new("Updated").width(20),
        Column::new("Branch").width(15).align(Align::Left),
        Column::new("Directory").flex(1),  // Fills remaining
    ])
    .header_style(TextStyle::new().bold())
    .row_style(|idx, selected| {
        if selected { TextStyle::new().bg(Color::Blue) }
        else { TextStyle::default() }
    })
    .rows(data.iter().map(|item| [
        item.updated.to_string(),
        item.branch.clone(),
        item.directory.clone(),
    ]))
    .selected(self.selected_index)
```

---

### F7: Incremental ANSI Parser [LOW]

**Evidence:** `TextNode::from_ansi()` exists but parses complete strings. For streaming command output:

**Proposed API:**
```rust
pub struct AnsiParser {
    state: ParserState,  // Tracks escape sequence state
    pending: Vec<StyledSpan>,
}

impl AnsiParser {
    pub fn new() -> Self;
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<StyledSpan>;
    pub fn flush(&mut self) -> Vec<StyledSpan>;  // Handle incomplete sequences
}

// Usage for streaming command output:
let mut parser = AnsiParser::new();
while let Some(chunk) = stream.next().await {
    let spans = parser.feed(&chunk);
    output_buffer.extend(spans);
    app_handle.request_render();
}
```

---

## Summary: Priority Order

| Priority | Item | Impact |
|----------|------|--------|
| 1 | D1: First-Class Line Type | Eliminates ~3,000 lines of bridge code |
| 2 | F1: Multi-line TextArea | Blocker for text input widgets |
| 3 | D2: Style Accumulator | Eliminates custom StyleState everywhere |
| 4 | D3: Line Utilities | Eliminates inky_wrapping.rs |
| 5 | F2: Streaming Text | Required for LLM output |
| 6 | F4: Scroll Persistence | Common need for lists/chat |
| 7 | D4: SimpleWidget trait | Reduces boilerplate for simple widgets |
| 8 | F5: Focus Management | Required for keyboard-driven UIs |
| 9 | F3: Layout Debugging | Dev experience improvement |
| 10 | F6: Table Component | Common UI pattern |

---

**The Core Insight:** Inky is built around a **node tree** (React-like), but TUI apps think in **lines and spans** (terminal-native). Bridging this gap at the API level would eliminate most of the 22,842 lines of bridge code.

-- Codex Porter
