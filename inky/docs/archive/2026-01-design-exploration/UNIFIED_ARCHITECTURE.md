# Unified Architecture: True Union of Node-Tree and Lines-and-Spans

**Date:** 2026-01-01
**Question:** Does the Adaptive Hybrid architecture support the union of both frameworks?

---

## The Test: What "Union" Means

A true union would mean:

| Capability | Node-Tree | Lines-and-Spans | Union Must Support |
|------------|-----------|-----------------|-------------------|
| **Composition** | `Box { children }` | Manual line building | Mix freely |
| **Layout** | Flexbox auto-layout | Manual positioning | Both in same tree |
| **Streaming** | Rebuild tree each frame | Append to buffer | Incremental updates |
| **Mental model** | "Components return nodes" | "Functions return lines" | Either, interchangeably |
| **Type signature** | `fn() -> Node` | `fn() -> Vec<Line>` | **Same return type** |

The last row is the key: if a "line-oriented" function and a "node-oriented" function have different return types, they don't compose seamlessly. There's a seam.

---

## Current State: Almost Unified, But There's a Seam

The Adaptive Hybrid proposal has:

```rust
pub enum Node {
    Line(Line),           // ← Line-oriented
    Lines(Vec<Line>),     // ← Line-oriented
    Text(Text),           // ← Line-oriented
    Box { ... },          // ← Node-oriented
    Image { ... },        // ← Graphics-oriented
    Adaptive { ... },     // ← Meta
}
```

**The seam:** `Line` is inside `Node`, but they're different types. You can't just return a `Line` where a `Node` is expected without wrapping.

```rust
// Node-oriented component
fn header() -> Node {
    Node::Box {
        children: vec![
            Node::Text(Text::new("Title").bold()),
        ],
        style: BoxStyle::row(),
    }
}

// Line-oriented function
fn status_line() -> Line {
    line![
        span!("●").color(Color::Green),
        span!(" Ready"),
    ]
}

// To compose them... you need conversion
fn app() -> Node {
    Node::Box {
        children: vec![
            header(),
            Node::Line(status_line()),  // ← Manual wrapping!
        ],
        style: BoxStyle::column(),
    }
}
```

**This is not a true union.** There's friction at the boundary.

---

## True Union: Everything is a Node, Lines are Just Simple Nodes

### The Insight

What if `Line` wasn't a *variant* of `Node`, but rather:
- A `Line` **is** a `Node` (via `Into<Node>`)
- A `Vec<Line>` **is** a `Node` (via `Into<Node>`)
- A `Span` **is** a `Node` (via `Into<Node>`)
- A `String` **is** a `Node` (via `Into<Node>`)

The node tree isn't a *different thing* from lines—it's a **generalization** that includes lines as a special case.

### The Architecture

```rust
/// The universal UI primitive.
/// Everything that can be rendered implements Into<Node>.
pub enum Node {
    // === Atomic content ===

    /// Single styled text span (leaf)
    Span(Span),

    // === Composition ===

    /// Sequence of nodes (horizontal by default, like spans in a line)
    Sequence {
        children: Vec<Node>,
        direction: Direction,  // Horizontal (inline) or Vertical (block)
        style: Style,
    },

    // === Graphics ===

    /// Pixel content with text fallback
    Graphic {
        content: GraphicContent,
        fallback: Box<Node>,
    },

    // === Streaming ===

    /// Content that updates incrementally
    Stream {
        id: StreamId,
        current: Box<Node>,
    },
}

#[derive(Default)]
pub enum Direction {
    #[default]
    Horizontal,  // Like spans in a line
    Vertical,    // Like lines in a block
}
```

### The Key: Generous `Into<Node>` Implementations

```rust
// A span is a node
impl From<Span> for Node {
    fn from(span: Span) -> Node {
        Node::Span(span)
    }
}

// A string is a node (unstyled span)
impl From<&str> for Node {
    fn from(s: &str) -> Node {
        Node::Span(Span::new(s))
    }
}

impl From<String> for Node {
    fn from(s: String) -> Node {
        Node::Span(Span::new(s))
    }
}

// A Line is a horizontal sequence of spans
pub struct Line(pub Vec<Span>);

impl From<Line> for Node {
    fn from(line: Line) -> Node {
        Node::Sequence {
            children: line.0.into_iter().map(Node::Span).collect(),
            direction: Direction::Horizontal,
            style: Style::default(),
        }
    }
}

// Vec<Line> is a vertical sequence of lines
impl From<Vec<Line>> for Node {
    fn from(lines: Vec<Line>) -> Node {
        Node::Sequence {
            children: lines.into_iter().map(Node::from).collect(),
            direction: Direction::Vertical,
            style: Style::default(),
        }
    }
}

// A Box is just a styled sequence
pub struct Box {
    children: Vec<Node>,
    style: BoxStyle,
}

impl From<Box> for Node {
    fn from(b: Box) -> Node {
        Node::Sequence {
            children: b.children,
            direction: b.style.direction,
            style: b.style.into(),
        }
    }
}
```

### Now Everything Composes Seamlessly

```rust
// Line-oriented function - returns impl Into<Node>
fn status_line() -> Line {
    line![
        span!("●").green(),
        span!(" Ready"),
    ]
}

// Node-oriented function - returns impl Into<Node>
fn header() -> impl Into<Node> {
    hbox![
        "Title".bold(),
        Spacer,
        status_line(),  // ← Line used directly, no wrapping!
    ]
}

// String literals work too
fn simple() -> impl Into<Node> {
    vbox![
        "First line",           // &str → Node
        "Second line".bold(),   // Span → Node
        status_line(),          // Line → Node
        header(),               // impl Into<Node> → Node
    ]
}
```

**No seams.** Everything that can be rendered is `Into<Node>`.

---

## The Unified Mental Model

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│                    Everything is a Node                     │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                                                     │   │
│  │   Span ──────┐                                      │   │
│  │              │                                      │   │
│  │   &str ──────┼──▶ Node::Span                       │   │
│  │              │                                      │   │
│  │   String ────┘                                      │   │
│  │                                                     │   │
│  │   Line ──────────▶ Node::Sequence { Horizontal }   │   │
│  │                                                     │   │
│  │   Vec<Line> ─────▶ Node::Sequence { Vertical }     │   │
│  │                                                     │   │
│  │   HBox ──────────▶ Node::Sequence { Horizontal }   │   │
│  │                                                     │   │
│  │   VBox ──────────▶ Node::Sequence { Vertical }     │   │
│  │                                                     │   │
│  │   Image ─────────▶ Node::Graphic                   │   │
│  │                                                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│   A Line IS a horizontal sequence of spans.                 │
│   A VBox IS a vertical sequence of children.                │
│   They're the same thing with different defaults.           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Streaming: Also Unified

The streaming model from Proposal 2 fits naturally:

```rust
/// Streaming content - updates incrementally
pub struct StreamingText {
    id: StreamId,
    buffer: String,
}

impl StreamingText {
    pub fn append(&mut self, text: &str) {
        self.buffer.push_str(text);
    }
}

// StreamingText is a Node
impl From<&StreamingText> for Node {
    fn from(s: &StreamingText) -> Node {
        Node::Stream {
            id: s.id,
            current: Box::new(Node::Span(Span::new(&s.buffer))),
        }
    }
}
```

Usage:
```rust
fn chat_message(msg: &Message) -> impl Into<Node> {
    vbox![
        line![span!(&msg.author).bold(), span!(" "), span!(&msg.time).dim()],
        &msg.content,  // Could be StreamingText or String - both work!
    ]
}
```

---

## Graphics: Also Unified

```rust
pub struct Image { ... }

impl From<Image> for Node {
    fn from(img: Image) -> Node {
        Node::Graphic {
            content: GraphicContent::Image(img),
            fallback: Box::new(img.fallback_node()),
        }
    }
}

// Use it anywhere
fn dashboard() -> impl Into<Node> {
    vbox![
        "System Status".bold(),
        Image::from_file("chart.png"),  // Graphics node
        status_line(),                   // Line node
        "All systems operational",       // String node
    ]
}
```

---

## The Final API

```rust
// Everything works everywhere

// Strings
let a: Node = "hello".into();

// Styled spans
let b: Node = span!("hello").bold().into();

// Lines (horizontal sequences)
let c: Node = line![span!("hello").bold(), " ", "world"].into();

// Vertical stacks
let d: Node = vbox!["line 1", "line 2", "line 3"].into();

// Nested composition
let e: Node = vbox![
    hbox!["Left", Spacer, "Right"],
    "Content",
    hbox![
        Image::new(chart_data),
        vbox!["Label 1", "Label 2"],
    ],
].into();

// Functions returning impl Into<Node> compose seamlessly
fn widget_a() -> impl Into<Node> { "A" }
fn widget_b() -> impl Into<Node> { line![span!("B").bold()] }
fn widget_c() -> impl Into<Node> { vbox!["C1", "C2"] }

let app: Node = vbox![
    widget_a(),
    widget_b(),
    widget_c(),
].into();
```

---

## Comparison: Before and After

### Before (Seam Between Paradigms)

```rust
// Different return types create friction
fn lines_fn() -> Vec<Line> { ... }
fn node_fn() -> Node { ... }

// Must manually convert
let app = Node::VBox {
    children: vec![
        node_fn(),
        Node::Lines(lines_fn()),  // ← Explicit conversion
    ]
};
```

### After (True Union)

```rust
// Same return type: impl Into<Node>
fn lines_fn() -> impl Into<Node> { vec![line!["a"], line!["b"]] }
fn node_fn() -> impl Into<Node> { vbox!["c", "d"] }

// Just compose
let app = vbox![
    node_fn(),
    lines_fn(),  // ← No conversion needed
];
```

---

## Answer: Does This Support the Union?

**Yes.** With `Into<Node>` as the universal trait:

| Paradigm | How It's Supported |
|----------|-------------------|
| **Node-tree (React-like)** | `vbox![]`, `hbox![]`, component functions |
| **Lines-and-spans** | `line![]`, `span!()`, `Vec<Line>` |
| **Graphics** | `Image`, `Canvas`, with auto-fallback |
| **Streaming** | `StreamingText` implements `Into<Node>` |

The key insight: **A line IS a node.** It's a horizontal sequence of spans. There's no conversion needed because they're the same thing at different levels of abstraction.

```
Line = Node::Sequence { direction: Horizontal, children: spans }
VBox = Node::Sequence { direction: Vertical, children: nodes }
```

**They're isomorphic.** The "line" mental model and the "node tree" mental model are just two views of the same underlying structure.

---

## Implementation Checklist

To achieve true union in inky:

1. [ ] Make `Node` the universal type
2. [ ] Implement `From<Span>`, `From<&str>`, `From<String>` for `Node`
3. [ ] Define `Line` as `struct Line(Vec<Span>)` with `Into<Node>`
4. [ ] Define `impl From<Vec<Line>> for Node`
5. [ ] Make `vbox![]` and `hbox![]` accept `impl Into<Node>`
6. [ ] Make `line![]` return something that implements `Into<Node>`
7. [ ] Ensure all components return `impl Into<Node>`

Then both paradigms work seamlessly, because they're the same paradigm.
