# Vision: Terminal-Native Editor for the AI Age

**Date:** 2026-01-01
**Context:** Design exploration for inky + dterm + DashTerm2 convergence

---

## The Core Insight: Everything is a Buffer

Like Emacs, but terminal-native. Every view is navigable, searchable text:

| Buffer Type | Behavior |
|-------------|----------|
| File buffer | Traditional editing with syntax highlighting |
| Shell buffer | Live PTY output, cursor can move anywhere, yank any text, re-execute any command |
| AI buffer | Streaming LLM responses with rendered markdown inline |
| Image buffer | Sixel/Kitty images with text annotations |
| Help buffer | Documentation rendered as markdown |

The terminal output isn't ephemeral—it's a document you inhabit.

---

## Chat Apps Are Terminals (But Don't Know It)

Modern chat interfaces (Claude, ChatGPT, iMessage) are structurally identical to terminals:

| Terminal Concept | Chat Equivalent |
|-----------------|-----------------|
| Scrollback | Message history |
| Command output | Received messages |
| Command input | Message composer |
| ANSI styling | Markdown/rich text |
| Sixel/Kitty images | Inline images |
| Shell integration (OSC 133) | Read receipts, typing indicators |
| Ctrl-R search | Message search |

But chat apps don't go far with this insight. They're read-only feeds with text boxes.

---

## What Claude Code Does That Chat Apps Don't

Claude Code (terminal CLI) is smarter than chat apps:

| Capability | Chat Apps | Claude Code |
|------------|-----------|-------------|
| Input while AI working | Blocked | Always available |
| See work happening | Spinner | Tool calls visible |
| Interrupt | Maybe "Stop" button | Escape, immediate |
| Redirect mid-thought | Wait, then new message | Just type |
| Queue follow-ups | No | Type ahead |
| Multiple parallel tasks | No | Background agents |
| Input history | No | Up arrow |

**Chat apps impose turn-taking because it's easy to implement, not because it's right.**

---

## What's Missing: Output as Interactive Document

Current pain points with terminal output:

1. **Can't select semantically** — "that code block" vs "lines 42-67"
2. **Can't reply to specific parts** — have to quote manually
3. **Can't fold/expand** — verbose output stays verbose
4. **Can't pin/bookmark** — important info scrolls away
5. **Can't annotate** — no way to add notes
6. **Screenshot as coping mechanism** — users don't trust the interface

### Required Capabilities

#### 1. Semantic Selection
```
┌─ Option A ──────────────────────[1]─┐
│ Use a HashMap for O(1) lookup       │   ← tap [1] to select whole block
│ ```rust                             │
│ let map: HashMap<K, V> = ...        │
│ ```                                 │
└─────────────────────────────────────┘

You: "let's go with [1] but use FxHashMap instead"
     ↑ semantic reference, not copy-paste
```

#### 2. Inline Replies / Threading
```
Claude: The issue is in the parser.

│ You: [inline] are you sure?          ← threaded reply
│
│ Claude: Let me double-check...
│ Actually, you're right, it's the lexer.

Here's the fix:                        ← continues main thread
```

#### 3. Fold/Expand
```
▶ Background (tap to expand)           ← collapsed, already read
▼ The Fix                              ← expanded, relevant
  [detailed content here]
▶ Alternative approaches (collapsed)   ← optional detail
```

#### 4. Pin/Bookmark
```
📌 Pinned (always visible)
┌─────────────────────────────────────┐
│ "The key insight is that the AST    │
│  must be traversed post-order"      │
│                      — 23 min ago   │
└─────────────────────────────────────┘
```

#### 5. Annotations
```
│ You: [inline] are you sure?
│ [Your annotation]: "tried this, works
│ but slow on large files. revisit."
```

#### 6. Persistence + Search ("Feel Safe")
```
Search: "ast traversal"

Results from this session:
 • 14:23 "traverse post-order" [jump]
 • 14:45 code block with traverse()

Results from past sessions:
 • Dec 28: "AST visitor pattern" [open]

Everything saved. Nothing lost. No screenshots needed.
```

---

## Native Markdown Rendering

Markdown rendering is an **overlay on editable text**, not a separate view:

```
Rendered view:                    Source (underneath):
─────────────                     ─────────────────────
█ Getting Started                 # Getting Started

Clone the repo:                   Clone the repo:

┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓   ```bash
┃ $ git clone git@...         ┃   $ git clone git@...
┃ $ cd inky && cargo run      ┃   $ cd inky && cargo run
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛   ```

[Rendered Kitty image]            ![arch](./docs/arch.png)
```

**Key behavior:** When you tap in rendered mode, cursor maps to source position. You see pretty, edit real.

---

## Shell Buffer: The Navigable Terminal

Unlike traditional terminals where output scrolls away:

```
┌─ *shell* ────────────────────────────────────────────────────────┐
│ ┌─ $ ls -la ─────────────────────[1]─┐  ← Command block 1       │
│ │ drwxr-xr-x  5 user  160 Dec 31 .   │    (OSC 133 detected)    │
│ │ -rw-r--r--  1 user 2341 Dec 31 app │                          │
│ └────────────────────────────────────┘                          │
│                                                                  │
│ ┌─ $ cargo test ─────────────────[2]─┐  ← Command block 2       │
│ │ test buffer::test_write ... ok     │    tap = jump to test    │
│ │ test layout::test_flex ... FAILED  │    tap FAILED = details  │
│ │ ✗ exit 1                           │                          │
│ └────────────────────────────────────┘                          │
│                                                                  │
│ $ █                                      ← input line            │
└──────────────────────────────────────────────────────────────────┘

Actions:
- Tap any command → re-run it
- Long-press output → select, copy, pipe
- Swipe command left → delete from history
- Search → trigram index, <10ms on 1M lines
```

---

## iPhone Design

Small screen forces clarity: **one buffer, full attention, fluid switching**.

### Main View
```
┌─────────────────────────────────┐
│ ◀ Buffers    README.md    ⋯    │  ← tap "Buffers" = drawer
├─────────────────────────────────┤
│                                 │
│  ┌─────────────────────────────┐│
│  │ # Getting Started           ││  ← rendered heading
│  └─────────────────────────────┘│
│                                 │
│  Clone the repo and run:        │
│                                 │
│  ╭─ shell ─────────────────────╮│
│  │ $ git clone git@...         ││  ← tap = copy command
│  │ $ cd inky && cargo run      ││
│  ╰─────────────────────────────╯│
│                                 │
├─────────────────────────────────┤
│ ▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░ │  ← scroll position
├─────────────────────────────────┤
│  [Raw]  [Copy]  [Run]  [AI ✨] │  ← context actions
└─────────────────────────────────┘
```

### Gesture Language

| Gesture | Action |
|---------|--------|
| Tap | Place cursor / select block |
| Tap code block | Copy command / run |
| Tap link | Navigate |
| Long press | Select word, context menu |
| Swipe right | Back to previous buffer |
| Swipe left | Forward buffer |
| Swipe up from bottom | Command palette |
| Two-finger tap | Toggle edit/view mode |
| Pinch | Zoom text |

### AI Integration
```
┌─────────────────────────────────┐
│ Claude                     [⋯]  │
├─────────────────────────────────┤
│ Claude:                         │
│ Looking at the parser...        │
│ [Reading src/parser.rs]         │  ← live, streaming
│                                 │
│ ┌─────────────────────────────┐ │
│ │ Queued:                     │ │
│ │ • "check lexer too"         │ │  ← your queued inputs
│ │ • "grep for token"          │ │
│ └─────────────────────────────┘ │
├─────────────────────────────────┤
│ also check if...            [+] │  ← always available
└─────────────────────────────────┘
```

---

## Architecture: Layered Design

### The Problem

dterm-core currently provides both **logic** and **rendering**:
- VT100 parser, scrollback, search (logic)
- Cell grid, glyph atlas, Metal shaders (rendering)

The cell-based rendering is terminal-specific. iPhone needs native rendering.

### The Solution: Separate Model from View

```
┌─────────────────────────────────────────────────────────────┐
│                     What to render                          │
│                                                             │
│  "Show message with code block, user can fold/pin/reply"    │
│                                                             │
│  This is SHARED. Same document model. Same interactions.    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      How to render                          │
│                                                             │
│  Terminal: 8-byte cells, monospace, box drawing, ANSI       │
│  iPhone: Core Text, variable width, native selection        │
│  Web: DOM nodes, CSS, Canvas                                │
│                                                             │
│  This is DIFFERENT per platform. That's fine.               │
└─────────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Terminal | iPhone | Shareable? |
|-----------|----------|--------|------------|
| VT100 parser | Yes | Maybe (shell output) | ✅ |
| Terminal modes | Yes | No | ❌ |
| Scrollback storage | Yes | Yes (message history) | ✅ |
| Trigram search | Yes | Yes | ✅ |
| Image handling | Yes | Yes | ✅ |
| Cell grid | Yes | No | ❌ |
| Glyph atlas | Yes | Maybe | 🟡 |
| Box drawing | Yes | No (use native) | ❌ |
| Metal shaders | Yes | Could share | 🟡 |

### Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Applications                             │
│                                                                 │
│   DashTerm2          inky-chat (iPhone)        inky-ed         │
│   (terminal)         (native chat)             (editor)        │
└───────┬────────────────────┬───────────────────────┬────────────┘
        │                    │                       │
        ▼                    ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Rendering Backends                           │
│                                                                 │
│   dterm-cells           UIKit/SwiftUI            inky-buffer   │
│   (8-byte grid)         (native views)           (terminal)    │
│   + Metal shaders       + Core Text                            │
└───────┬────────────────────┬───────────────────────┬────────────┘
        │                    │                       │
        └────────────────────┼───────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      dterm-core (shared)                        │
│                                                                 │
│  ┌──────────────┐  ┌────────────────┐  ┌──────────────────┐   │
│  │   Parsing    │  │    Storage     │  │     Media        │   │
│  │              │  │                │  │                  │   │
│  │ • VT100      │  │ • Rope/buffer  │  │ • Image decode   │   │
│  │ • Markdown   │  │ • Tiered store │  │ • Sixel/Kitty    │   │
│  │ • ANSI       │  │ • Compression  │  │ • Caching        │   │
│  │              │  │ • Trigram idx  │  │                  │   │
│  └──────────────┘  └────────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Canvas Trait (Apps Have Full Control)

```rust
/// App provides its own rendering implementation
trait Canvas {
    fn draw_text(&mut self, x: f32, y: f32, text: &str, style: &TextStyle);
    fn draw_image(&mut self, x: f32, y: f32, image: &Image);
    fn draw_rect(&mut self, rect: Rect, fill: Color, stroke: Option<Stroke>);
    fn measure_text(&self, text: &str, style: &TextStyle) -> Size;
}

/// Terminal: renders to cell grid
struct CellCanvas { cells: Grid<Cell>, glyph_atlas: GlyphAtlas }

/// iPhone: renders to Core Graphics / UIKit
struct NativeCanvas { context: CGContext }
```

---

## Data Model

```rust
/// A conversation is a tree, not a flat list
struct Conversation {
    id: ConversationId,
    root: MessageNode,
    pinned: Vec<PinnedItem>,
    annotations: HashMap<BlockId, Vec<Annotation>>,
    fold_state: HashMap<BlockId, FoldState>,
}

/// Messages can have inline replies (threading)
struct MessageNode {
    id: MessageId,
    role: Role,
    blocks: Vec<Block>,          // Semantic blocks within message
    replies: Vec<MessageNode>,   // Inline threaded replies
    timestamp: Instant,
}

/// Semantic blocks within a message
enum Block {
    Paragraph { id: BlockId, text: String },
    CodeBlock { id: BlockId, language: String, code: String },
    List { id: BlockId, items: Vec<String> },
    Image { id: BlockId, data: ImageData },
}

/// Pinned items for "feeling safe"
struct PinnedItem {
    block_id: BlockId,
    pinned_at: Instant,
    note: Option<String>,
}

/// User annotations
struct Annotation {
    id: AnnotationId,
    position: TextPosition,
    text: String,
    created_at: Instant,
}
```

---

## Design Language: Related but Platform-Native

**Shared across both platforms:**
- Semantic structure (messages, blocks, code, pins, folds)
- Interaction model (reply to block, pin, collapse, search)
- Data model (conversation tree, annotations)
- Keyboard shortcuts (where applicable)

**Different per platform:**

| Concept | Terminal | iPhone |
|---------|----------|--------|
| Fold/expand | `▶`/`▼` ASCII | Native disclosure chevron |
| Code block | Box drawing border | Rounded rect with shadow |
| Selection | Block cursor, highlight | Native handles + loupe |
| Pin | `📌` emoji or `[*]` | Native pin icon, haptic |
| Context menu | Popup list | Native action sheet |
| Navigation | vim keys, Tab | Swipe, tap, gestures |
| Text rendering | Monospace grid | Proportional, Core Text |

The **mental model** is the same. The **visual language** is platform-native.

---

## Summary

**The philosophy:**
- Emacs got it right: everything is navigable text
- Terminals got it wrong: output is ephemeral
- Chat apps are terminals that don't know it
- Claude Code is smarter (async I/O) but output is still a stream
- The fix: output as interactive document (fold, pin, reply, search)
- Architecture: shared document model, platform-native rendering

**The result:**
- README renders beautifully with inline diagrams
- Shell history is a buffer you navigate and re-run from
- AI conversations are buffers you search and copy from
- Works on iPhone with touch gestures
- No screenshots needed—the interface is trustworthy

---

## Related Projects

- **dterm** (`~/dterm/`) - GPU-accelerated terminal core
- **DashTerm2** (`~/dashterm2/`) - macOS terminal app (iTerm2 fork + dterm)
- **inky** (`~/inky/`) - Rust TUI library with React-like components
