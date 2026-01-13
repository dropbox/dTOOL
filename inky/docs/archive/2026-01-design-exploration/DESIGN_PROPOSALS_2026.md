# Three Proposals for a 2026 Terminal UI Architecture

**Date:** 2026-01-01
**Context:** Merging node-tree (React-like) with lines-and-spans (terminal-native)
**Audience:** The inky AI and anyone building the future of terminal UIs

---

## The Problem Space

**React-like (node tree):**
- ✓ Declarative composition
- ✓ Automatic layout
- ✓ Component reuse
- ✗ Thinks in "boxes" when terminals think in "lines"
- ✗ Overhead from tree reconciliation
- ✗ Impedance mismatch with streaming content

**Terminal-native (lines & spans):**
- ✓ Direct mapping to display
- ✓ Natural for text streaming
- ✓ Zero abstraction overhead
- ✗ No composition model
- ✗ Manual layout calculations
- ✗ Doesn't scale to complex UIs

**2026 Reality:**
- Terminals have GPU rendering, true color, inline images, hyperlinks
- AI output streams character-by-character
- Users expect 60fps, instant response
- TUIs are replacing web UIs for developer tools
- The browser DOM model is 30 years old and shows it

---

## Proposal 1: "Lines All The Way Down"

### Core Insight

In a terminal, **everything is lines**. A box with a border? That's just lines with box-drawing characters. A flex layout? That's lines arranged vertically or horizontally. Instead of pretending terminals are browsers, embrace the line.

### The Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   Application State                                         │
│         │                                                   │
│         ▼                                                   │
│   ┌─────────────┐                                          │
│   │  Component  │──render()──▶ LineTree                    │
│   └─────────────┘                                          │
│                                    │                        │
│                                    ▼                        │
│                           ┌──────────────┐                 │
│                           │ Layout Pass  │                 │
│                           │ (line-aware) │                 │
│                           └──────────────┘                 │
│                                    │                        │
│                                    ▼                        │
│                           Vec<RenderedLine>                │
│                                    │                        │
│                                    ▼                        │
│                              Terminal Buffer                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Core Types

```rust
/// A span is styled text. The atom of terminal content.
pub struct Span {
    text: CompactString,  // Stack-allocated for small strings
    style: Style,
}

/// A line is a sequence of spans with optional line-level properties.
pub struct Line {
    spans: SmallVec<[Span; 4]>,  // Most lines have few spans
    props: LineProps,
}

pub struct LineProps {
    style: Style,           // Inherited by spans
    align: Align,           // Left/Center/Right
    truncate: Truncate,     // None/Ellipsis/Fade
    wrap: bool,             // Word-wrap if too wide
}

/// A LineTree is the compositional unit. It's like a React component
/// but produces lines, not DOM nodes.
pub enum LineTree {
    /// Raw lines (leaf node)
    Lines(Vec<Line>),

    /// Vertical stack (default)
    VStack {
        children: Vec<LineTree>,
        gap: u16,
        style: ContainerStyle,
    },

    /// Horizontal split (columns)
    HStack {
        children: Vec<LineTree>,
        widths: Vec<Size>,  // Fixed, Percent, Flex
        gap: u16,
        style: ContainerStyle,
    },

    /// Box with border/padding (wraps content in decorations)
    Boxed {
        child: Box<LineTree>,
        border: Border,
        padding: Edges,
        style: ContainerStyle,
    },

    /// Scrollable region
    Scroll {
        child: Box<LineTree>,
        offset: u16,
        height: Size,
    },

    /// Lazy/virtualized content
    Virtual {
        total_lines: usize,
        visible_range: Range<usize>,
        render_line: Box<dyn Fn(usize) -> Line>,
    },
}
```

### Component Model

```rust
/// Components produce LineTree, not DOM-like nodes
trait Component {
    fn render(&self, ctx: &RenderContext) -> LineTree;
}

/// A simple message component
struct Message {
    author: String,
    content: String,
    timestamp: DateTime,
}

impl Component for Message {
    fn render(&self, ctx: &RenderContext) -> LineTree {
        // Natural line-oriented thinking
        LineTree::Lines(vec![
            line![
                span!("{}", self.author).bold().color(Color::Cyan),
                span!("  {}", self.timestamp.format("%H:%M")).dim(),
            ],
            line![span!("{}", self.content)].wrap(true),
        ])
    }
}

/// A chat view composes messages
struct ChatView {
    messages: Vec<Message>,
    scroll_offset: u16,
}

impl Component for ChatView {
    fn render(&self, ctx: &RenderContext) -> LineTree {
        LineTree::Scroll {
            child: Box::new(LineTree::VStack {
                children: self.messages.iter()
                    .map(|m| m.render(ctx))
                    .collect(),
                gap: 1,
                style: ContainerStyle::default(),
            }),
            offset: self.scroll_offset,
            height: Size::Flex(1),
        }
    }
}
```

### Why This Works for 2026

1. **Streaming-friendly**: Appending to `Vec<Line>` is O(1). No tree diffing.
2. **Cache-friendly**: Lines are contiguous in memory.
3. **Mental model matches output**: What you build is what you see.
4. **Composable**: VStack/HStack/Boxed provide layout without leaving line-land.
5. **Zero impedance mismatch**: The layout pass outputs lines, which are lines.

### The Macros

```rust
// Line construction is natural
let header = line![
    span!("● ").color(Color::Green),
    span!("Ready").bold(),
    span!(" — press "),
    span!("?").inverse(),
    span!(" for help"),
];

// Multi-line content
let content = lines![
    line![span!("First line")],
    line![span!("Second line").italic()],
    line![],  // Empty line
    line![span!("After gap")],
];
```

---

## Proposal 2: "Stream-Native Architecture"

### Core Insight

2026 is the age of LLMs. Everything streams. The UI framework should be **stream-native**, not batch-native. Instead of "render a tree, diff it, apply changes", we should "emit fragments, accumulate them, render incrementally."

### The Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   Data Sources (async streams)                              │
│   ├── LLM response stream                                   │
│   ├── Command output stream                                 │
│   ├── File watcher stream                                   │
│   └── User input stream                                     │
│         │                                                   │
│         ▼                                                   │
│   ┌─────────────────┐                                      │
│   │ Fragment Stream │◀──── Components emit fragments       │
│   └─────────────────┘                                      │
│         │                                                   │
│         ▼                                                   │
│   ┌─────────────────┐                                      │
│   │  Accumulator    │──── Builds display state             │
│   └─────────────────┘                                      │
│         │                                                   │
│         ▼                                                   │
│   ┌─────────────────┐                                      │
│   │ Render Pipeline │──── 60fps batched updates            │
│   └─────────────────┘                                      │
│         │                                                   │
│         ▼                                                   │
│   Terminal (minimal diff)                                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Core Types

```rust
/// The atomic unit of UI emission
pub enum Fragment {
    // Content
    Text(Span),              // Styled text
    LineBreak,               // Explicit newline
    SoftBreak,               // Wrap point hint

    // Structure
    RegionStart {
        id: RegionId,
        layout: RegionLayout,
        style: Style,
    },
    RegionEnd(RegionId),

    // Control
    Clear(RegionId),         // Clear a region's content
    Cursor(CursorPos),       // Set cursor position
    Scroll {                 // Scroll a region
        region: RegionId,
        delta: i16,
    },
}

pub enum RegionLayout {
    /// Flows vertically, wraps text
    Flow { width: Size },

    /// Fixed grid position
    Fixed { x: u16, y: u16, width: u16, height: u16 },

    /// Flex child in parent
    Flex { grow: f32, shrink: f32 },

    /// Horizontal split
    Column { width: Size },
}
```

### Component Model

```rust
/// Components are async streams of fragments
trait Component {
    fn stream(&self) -> impl Stream<Item = Fragment>;
}

/// LLM response component - naturally streaming
struct LlmResponse {
    content: watch::Receiver<String>,
}

impl Component for LlmResponse {
    fn stream(&self) -> impl Stream<Item = Fragment> {
        async_stream::stream! {
            yield Fragment::RegionStart {
                id: "llm-response".into(),
                layout: RegionLayout::Flow { width: Size::Percent(100) },
                style: Style::default(),
            };

            let mut last_len = 0;
            loop {
                self.content.changed().await.ok();
                let text = self.content.borrow();

                // Only emit the new characters
                if text.len() > last_len {
                    let new_text = &text[last_len..];
                    for span in parse_markdown_spans(new_text) {
                        yield Fragment::Text(span);
                    }
                    last_len = text.len();
                }
            }
        }
    }
}

/// Static content is just a one-shot stream
struct Header {
    title: String,
}

impl Component for Header {
    fn stream(&self) -> impl Stream<Item = Fragment> {
        stream::once(async move {
            Fragment::Text(span!("{}", self.title).bold())
        }).chain(stream::once(async { Fragment::LineBreak }))
    }
}
```

### The Accumulator

```rust
/// Accumulates fragments into renderable state
pub struct Accumulator {
    regions: HashMap<RegionId, Region>,
    root_order: Vec<RegionId>,
}

impl Accumulator {
    /// Process a fragment, returns true if display changed
    pub fn process(&mut self, fragment: Fragment) -> bool {
        match fragment {
            Fragment::Text(span) => {
                self.current_region_mut().append_span(span);
                true
            }
            Fragment::LineBreak => {
                self.current_region_mut().newline();
                true
            }
            Fragment::RegionStart { id, layout, style } => {
                self.push_region(id, layout, style);
                false  // Structure change, not content
            }
            Fragment::RegionEnd(id) => {
                self.pop_region(id);
                false
            }
            Fragment::Clear(id) => {
                self.regions.get_mut(&id).map(|r| r.clear());
                true
            }
            // ... etc
        }
    }

    /// Render current state to lines
    pub fn render(&self, width: u16, height: u16) -> Vec<Line> {
        // Layout regions, produce lines
    }
}
```

### Why This Works for 2026

1. **LLM-native**: Streaming text just emits `Fragment::Text`. No tree rebuild.
2. **Incremental by default**: Only new content is processed.
3. **Backpressure-aware**: Async streams naturally handle fast producers.
4. **Region-based updates**: Change one region without touching others.
5. **Time-travel possible**: Store fragment history for undo/replay.

### Usage

```rust
#[tokio::main]
async fn main() {
    let app = App::new()
        .region("header", Header { title: "AI Chat".into() })
        .region("chat", ChatView::new())
        .region("input", InputBox::new())
        .layout(|b| {
            b.vstack([
                b.fixed("header", 1),
                b.flex("chat", 1),
                b.fixed("input", 3),
            ])
        });

    app.run().await
}
```

---

## Proposal 3: "Algebraic UI"

### Core Insight

Both React and terminal TUIs suffer from **implicit** layout rules. CSS is a mess of interacting properties. Flexbox requires understanding "main axis" and "cross axis." What if layout was **algebraic**—simple operations that compose predictably?

### The Algebra

```
UI = Content | UI <-> UI | UI <=> UI | UI @ Style

where:
  Content     = text, spans, lines
  <->         = horizontal composition (side by side)
  <=>         = vertical composition (stacked)
  @ Style     = apply style (border, padding, color)
```

### Core Types

```rust
/// The UI algebra
pub enum UI {
    /// Terminal content (the base case)
    Content(Content),

    /// Horizontal composition: a <-> b
    Horizontal(Box<UI>, Box<UI>, HRule),

    /// Vertical composition: a <=> b
    Vertical(Box<UI>, Box<UI>, VRule),

    /// Styled wrapper: ui @ style
    Styled(Box<UI>, Style),

    /// Empty (identity element)
    Empty,
}

/// How to divide space horizontally
pub enum HRule {
    /// First gets N columns, second gets rest
    LeftFixed(u16),
    /// First gets rest, second gets N columns
    RightFixed(u16),
    /// Split by ratio
    Ratio(u16, u16),
    /// Both get natural width (for inline)
    Natural,
}

/// How to divide space vertically
pub enum VRule {
    TopFixed(u16),
    BottomFixed(u16),
    Ratio(u16, u16),
    Natural,
}

/// Base content types
pub enum Content {
    /// Single line of spans
    Line(Vec<Span>),
    /// Multiple lines
    Lines(Vec<Vec<Span>>),
    /// Text that wraps
    Text(String, TextStyle),
    /// Dynamic/lazy content
    Dynamic(Box<dyn Fn(u16, u16) -> Content>),
}
```

### The Operators

```rust
/// Horizontal composition
impl std::ops::BitOr for UI {
    type Output = UI;
    fn bitor(self, rhs: UI) -> UI {
        UI::Horizontal(Box::new(self), Box::new(rhs), HRule::Natural)
    }
}

/// Vertical composition
impl std::ops::Div for UI {
    type Output = UI;
    fn div(self, rhs: UI) -> UI {
        UI::Vertical(Box::new(self), Box::new(rhs), VRule::Natural)
    }
}

/// Style application
impl std::ops::Rem<Style> for UI {
    type Output = UI;
    fn rem(self, style: Style) -> UI {
        UI::Styled(Box::new(self), style)
    }
}
```

### Building UIs

```rust
fn chat_ui(messages: &[Message], input: &str) -> UI {
    // Header
    let header = text("AI Chat").bold() % border(Border::Bottom);

    // Messages (vertical stack)
    let chat = messages.iter()
        .map(|m| message_ui(m))
        .reduce(|a, b| a / b)
        .unwrap_or(UI::Empty);

    // Input
    let input = text(input) % border(Border::Rounded);

    // Compose: header on top, chat in middle (flex), input at bottom
    header.fixed(1) / chat.flex(1) / input.fixed(3)
}

fn message_ui(msg: &Message) -> UI {
    let header = span(&msg.author).cyan().bold()
               | span("  ")
               | span(&msg.time).dim();
    let body = text(&msg.content).wrap();

    (header / body) % padding(1)
}

// Sidebar example: fixed left, flex right
fn with_sidebar(sidebar: UI, main: UI) -> UI {
    sidebar.fixed_width(30) | main.flex(1)
}
```

### Algebraic Properties

The algebra satisfies useful laws:

```rust
// Identity
ui / Empty == ui
ui | Empty == ui

// Associativity
(a / b) / c == a / (b / c)
(a | b) | c == a | (b | c)

// Style distribution (styles inherit down)
(a / b) % style == (a % style) / (b % style)

// Fixed + Flex = Full
fixed(10) / flex(1) // Uses full height: 10 for first, rest for second
```

### Rendering

```rust
impl UI {
    /// Render to lines given available space
    pub fn render(&self, width: u16, height: u16) -> Vec<Line> {
        match self {
            UI::Content(c) => c.render(width, height),

            UI::Horizontal(left, right, rule) => {
                let (lw, rw) = rule.divide(width);
                let left_lines = left.render(lw, height);
                let right_lines = right.render(rw, height);
                merge_horizontal(left_lines, right_lines, lw)
            }

            UI::Vertical(top, bottom, rule) => {
                let (th, bh) = rule.divide(height);
                let mut lines = top.render(width, th);
                lines.extend(bottom.render(width, bh));
                lines
            }

            UI::Styled(inner, style) => {
                let (iw, ih) = style.inner_size(width, height);
                let inner_lines = inner.render(iw, ih);
                style.wrap_lines(inner_lines, width, height)
            }

            UI::Empty => vec![],
        }
    }
}
```

### Why This Works for 2026

1. **Predictable**: No CSS-like "why is this element here?!" moments.
2. **Composable**: Build complex layouts from simple pieces.
3. **Algebraic laws**: Refactoring preserves behavior.
4. **Efficient**: Direct to lines, no intermediate tree.
5. **Type-safe**: Invalid layouts are compile errors.
6. **Teachable**: The algebra fits in your head.

### Bonus: Time-Varying UI

```rust
/// UI that changes over time (for animations, streaming)
pub enum DynamicUI {
    Static(UI),
    Stream(Pin<Box<dyn Stream<Item = UI>>>),
    Animated {
        from: UI,
        to: UI,
        progress: f32,
        easing: Easing,
    },
}
```

---

## Comparison Matrix

| Aspect | Proposal 1: Lines | Proposal 2: Streams | Proposal 3: Algebra |
|--------|-------------------|--------------------|--------------------|
| **Mental model** | "Everything is lines" | "Everything streams" | "Compose with operators" |
| **Best for** | Traditional TUIs | LLM/streaming apps | Layout-heavy apps |
| **Learning curve** | Low | Medium | Medium |
| **Streaming support** | Good (append lines) | Excellent (native) | Good (with DynamicUI) |
| **Layout complexity** | Medium | Low | High capability, simple API |
| **Performance** | Excellent | Excellent | Excellent |
| **Type safety** | Good | Good | Excellent |
| **Novel factor** | Low (familiar) | Medium | High |

---

## My Recommendation

**Combine elements of all three:**

1. **From Proposal 1**: `Line` and `Span` as first-class types. The `line![]` and `span!()` macros.

2. **From Proposal 2**: Stream-native components for LLM output. The fragment accumulator for incremental updates.

3. **From Proposal 3**: The algebraic operators (`|` for horizontal, `/` for vertical) as ergonomic sugar on top of a LineTree.

```rust
// The best of all worlds:
fn chat_ui(ctx: &Ctx) -> impl Into<LineTree> {
    let header = line![span!("AI Chat").bold()] % border::bottom();

    let messages = ctx.messages.iter()
        .map(|m| message_ui(m))
        .collect::<VStack>();

    let input = streaming_input(&ctx.input_stream);

    header.fixed(1) / messages.scroll().flex(1) / input.fixed(3)
}

// Where streaming_input uses Proposal 2's fragment streams
fn streaming_input(stream: &InputStream) -> impl Component {
    StreamingText::new(stream)
        .on_fragment(|f| /* incremental update */)
}
```

---

## The 2026 Terminal Manifesto

1. **Lines are truth.** The terminal is a grid of characters. Embrace it.

2. **Streams are life.** AI output streams. User input streams. Build for streams.

3. **Composition is power.** Small pieces that combine predictably beat monolithic frameworks.

4. **Performance is UX.** 60fps or bust. Users feel lag.

5. **Types are docs.** If the compiler accepts it, it should render.

6. **The browser was a detour.** We're going back to text, but better.

---

*"The future of UI is not more pixels. It's smarter characters."*

