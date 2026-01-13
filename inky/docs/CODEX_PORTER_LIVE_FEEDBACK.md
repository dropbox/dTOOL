# Live Feedback from Codex Port

**Date:** 2026-01-01
**From:** Codex Porter (~/codex_inky)
**Purpose:** Ongoing feedback as we use inky for real work

This file will be updated as we discover what works, what doesn't, and what's missing.

---

## WHAT WORKS WELL

### W1: Color System
The `Color` enum is clean and complete:
- 16 basic colors with intuitive names
- `Ansi256(u8)` for 256-color
- `Rgb(u8, u8, u8)` for true color
- `Default` for terminal default

No issues here. 1:1 mapping with ratatui was trivial.

### W2: StyledSpan Builder Pattern
```rust
StyledSpan::new("text").bold().color(Color::Cyan).underline()
```
This is ergonomic and readable. Better than ratatui's `Span::styled("text", Style::new().bold().cyan())`.

### W3: TextNode::from_ansi()
ANSI passthrough works. Command output with escape codes renders correctly. This was a potential blocker that didn't materialize.

### W4: Async Integration (AsyncApp)
The `AsyncApp::run_async()` pattern with handle-based updates works for Tokio integration. This was critical for codex.

### W5: VirtualList
List virtualization is implemented and has the right API. Haven't stress-tested with 10K+ items yet.

---

## WHAT NEEDS IMPROVEMENT

### I1: Documentation Gaps [HIGH]

**Problem:** Had to read source code to understand many APIs.

**Missing docs:**
- How to structure a complex app (multiple panes, focus management)
- How to handle resize events
- How to do custom painting (Widget trait usage patterns)
- Performance best practices (when to cache, what to avoid)

**Suggestion:** Add `examples/complex_app.rs` showing a real multi-pane app with:
- Split panes (horizontal/vertical)
- Focus switching between panes
- Resize handling
- Async data loading

### I2: Error Messages [MEDIUM]

**Problem:** When things go wrong, errors are often opaque.

**Example:** Layout failures don't tell you which node caused the problem.

**Suggestion:** Add debug mode that traces layout decisions:
```rust
AsyncApp::new(render).debug_layout(true).run_async(...)?;
```

### I3: No Layout Debugging [MEDIUM]

**Problem:** When layout doesn't look right, hard to debug why.

**Suggestion:** Layout inspector mode that shows:
- Node boundaries (with different colors)
- Computed sizes
- Why a node got a particular size

### I4: TextNode Multi-line Behavior [LOW]

**Question:** When `TextNode::new("line1\nline2\nline3")` is used:
- Does it respect `max_height`?
- Does it handle wrapping correctly per line?
- What happens with mixed wrap + explicit newlines?

Need clearer docs or examples for this.

---

## WHAT'S MISSING (Feature Requests)

### M1: Cursor Position API [HIGH - BLOCKER for text input]

**Problem:** For text input widgets, I need to know where to place the terminal cursor.

**Current workaround:** Track cursor position manually through all layout calculations.

**What I need:**
```rust
// Option 1: Node-level cursor hint
TextNode::new(&text).cursor_at(byte_offset)

// Option 2: Post-layout query
let cursor_pos = app.cursor_position();  // Returns Option<(x, y)>

// Option 3: Input component handles it
Input::new().cursor_position(idx)  // Inky calculates screen position
```

**Priority:** This is the #1 blocker for porting `textarea.rs` and `chat_composer.rs` to native inky.

### M2: Scroll State Persistence [MEDIUM]

**Problem:** When re-rendering, scroll position resets unless I manage state externally.

**What I need:**
```rust
// Scroll remembers position across renders
let scroll = Scroll::new()
    .id("chat-scroll")  // Persistent ID
    .auto_scroll_to_bottom(true)  // For chat UIs
```

### M3: Focus Management [MEDIUM]

**Problem:** Tab/Shift+Tab focus navigation exists but:
- How do I programmatically set focus?
- How do I know which widget has focus?
- How do I handle focus in nested components?

**What I need:**
```rust
// Programmatic focus
app.set_focus("input-field");

// Query focus
let focused = app.focused_id();  // Option<&str>

// Focus events
Input::new().on_focus(|| ...).on_blur(|| ...)
```

### M4: Clipboard Integration [LOW]

**Status:** OSC 52 write-only currently.

**What I need:** Paste support for text input.

### M5: Mouse Drag Selection [LOW]

**Problem:** For text selection in input fields, need:
- Drag start/end events
- Selection range tracking
- Visual selection highlighting

---

## PERFORMANCE OBSERVATIONS

### P1: Haven't Hit Performance Issues Yet

With 80+ widgets routed through the bridge layer, performance is acceptable. But this is with the overhead of ratatui conversion. Native inky should be faster.

### P2: Need Benchmarks Before Removing ratatui [ANSWERED]

Before we delete ratatui, I want to verify:
- Frame render time < 16ms for 60fps
- Memory usage is reasonable
- No GC-like pauses from allocation

**Answer:** Render timing hooks already exist! Use `on_render` with `FrameStats`:
```rust
App::new()
    .on_render(|stats| {
        println!("frame: {:?}, layout: {:?}, paint: {:?}, diff: {:?}",
            stats.frame_time, stats.layout_time, stats.paint_time, stats.diff_time);
    })
    .run()?;

// For AsyncApp:
AsyncApp::new()
    .on_render(|stats| { /* same */ })
    .run_async()?;
```

**FrameStats fields:**
- `frame_time: Duration` - Total frame time
- `layout_time: Duration` - Time in layout calculation
- `paint_time: Duration` - Time painting to buffer
- `diff_time: Duration` - Time diffing buffers
- `output_time: Duration` - Time writing to terminal
- `cells_changed: usize` - Number of cells changed this frame
- `frame_number: u64` - Frame counter

**Benchmarks:** Added in commit #188. Run with `cargo bench`.

### P3: Node Allocation Concern [ANSWERED]

Every frame rebuilds the node tree. For 1000 messages, that's 1000+ allocations per frame.

**Answers:**
1. **SmallVec optimization:** Node children use `SmallVec<[Box<Node>; 8]>` - stack-allocated for ≤8 children per node, avoiding heap allocation in most cases.
2. **Layout caching:** The layout engine caches tree structure hash. If the node tree hasn't changed, layout is skipped entirely (`LayoutEngine::build_if_dirty()`).
3. **Benchmark results:** See `cargo bench` output. The benchmarks in commit #188 measure:
   - `text_nodes/100`, `/1000`, `/10000` - TextNode rendering
   - `box_nodes/1000_nested` - Deep nesting
   - `layout/1000_flat`, `/1000_nested` - Layout calculation
   - `full_frame/chat_ui` - Realistic chat interface

**Actual benchmark results (`cargo bench`):**
- 100 TextNodes: ~23μs full frame
- 1000 TextNodes: ~234μs full frame
- 1000 nested BoxNodes layout: ~176μs build + ~476μs compute
- Chat UI (100 messages): ~475μs full frame
- Cached layout rebuild: ~420ns (nanoseconds!)

**Memory per 1000 nodes:**
- TextNode: ~203 KB
- BoxNode: ~203 KB
- Node enum: ~211 KB

For 60fps you need <16ms per frame. inky achieves **<0.5ms** for realistic chat UIs.

---

## SPECIFIC API FEEDBACK

### A1: BoxNode::child() vs BoxNode::children() [ANSWERED]

Both exist, which is good. But:
```rust
// This is verbose for many children
BoxNode::new()
    .child(node1)
    .child(node2)
    .child(node3)
    .child(node4)

// Would prefer
BoxNode::new().children([node1, node2, node3, node4])
// or
BoxNode::new().children(vec![node1, node2, node3, node4])
```

Does `children()` accept iterators? If so, document it prominently.

**ANSWER:** Yes! `children()` accepts `impl IntoIterator<Item = impl Into<Node>>`. Both syntaxes work:
```rust
BoxNode::new().children([node1, node2, node3, node4])  // arrays
BoxNode::new().children(vec![node1, node2, node3])     // vectors
BoxNode::new().children(nodes.into_iter())             // iterators
```

### A2: Border + Padding Interaction [ANSWERED]

When I have:
```rust
BoxNode::new()
    .border(BorderStyle::Rounded)
    .padding(1)
    .child(content)
```

Is padding inside or outside the border? Document this clearly.

**ANSWER:** Padding is **inside** the border (CSS box model). The doc comment on `Style.padding` says "Inner spacing (inside the border)".

### A3: Percentage Sizing [ANSWERED]

Does inky support percentage-based sizing?
```rust
BoxNode::new().width_percent(50)  // 50% of parent
```

If not, how do I do split panes?

**ANSWER:** Yes! Use `Dimension::Percent(50.0)` or the `percent()` helper:
```rust
use inky::prelude::*;

// Two equal-width panes
hbox![
    BoxNode::new().width(percent(50.0)).child(left_pane),
    BoxNode::new().width(percent(50.0)).child(right_pane),
]
```

The `percent`, `length`, and `auto` helpers are exported in the prelude as of commit #186.

---

## WILL UPDATE AS PORT PROGRESSES

This is a living document. As Phase 7 (full native port) proceeds, I'll add:
- New blockers discovered
- Workarounds found
- Performance measurements
- API friction points

Check back for updates.

---

**Current blocker for native port:** ~~M1 (Cursor Position API)~~ **RESOLVED** in commit #186

-- Codex Porter
