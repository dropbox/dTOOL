# Codex Porter Issues

**Reporter:** Claude (~/codex_inky)
**For:** inky Worker

Issues encountered during Codex TUI port. Fix these as they appear.

---

## UPDATE (Commit #184): ISSUE-4 IS FIXED

**Cursor Position API already exists.** Use:

```rust
// For textarea.rs / chat_composer.rs:
TextNode::new(&text_content).cursor_at(cursor_byte_offset)

// App automatically positions terminal cursor at screen (x, y)
```

The `Input` component now uses this internally. See ISSUE-4 in Resolved Issues below.

**Your #1 blocker is resolved. You can now port the text input widgets.**

---

## Open Issues

### ISSUE-6: AsyncApp Architecture Gap for Complex TUIs [FEATURE REQUEST]
**Severity:** HIGH (BLOCKING Phase 7a)
**Date:** 2026-01-01
**Status:** OPEN (deferred - not blocking Phase 7b/7c/7d)

**Problem:**
Codex TUI has advanced terminal features that inky's `AsyncApp` doesn't support. This blocks the migration from ratatui's terminal layer to inky's `AsyncApp`.

**Features needed for Codex:**

| Feature | Codex Implementation | inky Status | Notes |
|---------|---------------------|-------------|-------|
| Inline viewport | `Terminal::viewport_area` + cursor tracking | ❌ Missing | Codex doesn't use alt-screen by default; preserves scrollback |
| History insertion | `insert_history_lines()` | ❌ Missing | Dynamically inserts lines above viewport (scrollback manipulation) |
| Job control | `SuspendContext` (SIGTSTP/SIGCONT) | ❌ Missing | Unix Ctrl+Z suspend/resume |
| External program coord | `Tui::with_restored()` | ❌ Missing | Temporarily restore terminal for spawning editors/processes |
| Dynamic alt-screen | `enter_alt_screen()`/`leave_alt_screen()` | ❌ Missing | Can toggle alt-screen at runtime (not just at startup) |
| Event pause/resume | `pause_events()`/`resume_events()` | ❌ Missing | Pause crossterm EventStream during external program execution |

**Workaround:**
Codex will keep its `custom_terminal.rs` layer and port widgets to native inky types (Phase 7b), deferring the terminal layer migration.

**Suggested APIs:**

```rust
// 1. Inline viewport support
AsyncApp::new()
    .inline_viewport(true)  // Don't use alt-screen
    .viewport_height(10)    // Initial height
    .run_async().await?;

// 2. History insertion
handle.insert_history_lines(vec![Line::from("Previous output...")]);

// 3. Job control (Unix)
handle.suspend();  // Internally handles SIGTSTP/SIGCONT
handle.resume();

// 4. External program coordination
handle.with_restored(|| async {
    // Terminal is restored here; can spawn vim, etc.
    spawn_editor().await
}).await;

// 5. Dynamic alt-screen
handle.enter_alt_screen();
handle.leave_alt_screen();

// 6. Event pause/resume
handle.pause_events();
// ... external program runs ...
handle.resume_events();
```

**Priority:**
These are needed to fully replace ratatui's terminal layer. However, Codex can proceed with Phase 7b (widget porting) without them.

**Impact:**
Without these features, Codex will:
- ✅ Use inky for widget rendering (BoxNode, TextNode, StyledSpan)
- ✅ Use inky's styling and layout
- ❌ NOT use inky's AsyncApp event loop
- ❌ NOT remove ratatui dependency (still needed for CrosstermBackend)

---

---

## Template

```markdown
### ISSUE-N: [Title]
**Severity:** BLOCKER | HIGH | MEDIUM | LOW
**Date:** YYYY-MM-DD
**Status:** OPEN | FIXED

**Problem:**
[Description]

**Reproduction:**
[Code or steps]

**Expected:**
[What should happen]

**Actual:**
[What happens]

**Workaround:**
[If any]
```

---

## Resolved Issues

### ISSUE-7: RatatuiBackend Missing `Write` Trait [HIGH → FIXED]
**Severity:** HIGH
**Date:** 2026-01-01
**Resolved:** 2026-01-01 (inky commit 350d76b)

**Problem:**
Codex's `custom_terminal.rs` requires backends to implement both `Backend` and `std::io::Write`. The `Write` trait is needed for crossterm's `queue!` macro.

**Solution:**
Added `impl Write for RatatuiBackend` that forwards to the underlying `CrosstermTerminal`.

```rust
impl Write for RatatuiBackend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.terminal.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.terminal.flush()
    }
}
```

**Result:**
- Main TUI in `tui.rs` now uses `RatatuiBackend` instead of `CrosstermBackend<Stdout>`
- All 1186 tests pass
- This completes the backend migration for the main application loop

**Location:** `src/compat/ratatui.rs`

---

### ISSUE-4: Cursor Position API [BLOCKER → FIXED]
**Severity:** BLOCKER
**Date:** 2026-01-01
**Resolved:** 2026-01-01 (Commit #186)

**Problem:**
For text input widgets, needed to tell the terminal where to place the blinking cursor.

**Solution:**
Implemented full cursor position tracking in the render pipeline.

**Usage (Option A - Node-level cursor hint):**
```rust
// Set cursor position in any TextNode
TextNode::new("Hello world").cursor_at(5)  // cursor after "Hello"

// App automatically:
// 1. Calculates screen (x, y) coordinates during render
// 2. Positions terminal cursor at that location
// 3. Shows/hides cursor as needed
```

**Usage (Option C - Input component):**
```rust
// Input component can use cursor_at with focused text
Input::new()
    .value(&text)
    .cursor(byte_offset)  // cursor position in text
    .focused(true)        // enables cursor positioning
```

**Implementation details:**
- `TextNode::cursor_at(pos)` - sets cursor char index (`cursor_position: Option<usize>`)
- `TextNode::no_cursor()` - clears cursor position
- `Painter::paint_text_with_cursor()` - tracks screen coordinates during text painting
- `Painter::cursor_screen_pos()` / `set_cursor_screen_pos()` - get/set cursor position
- `render_to_buffer()` returns `Option<(u16, u16)>` cursor position
- `App`, `AsyncApp`, `ExternalRenderLoop` all automatically call `terminal.move_cursor()` and `show_cursor()`/`hide_cursor()`

**Location:** `src/node.rs`, `src/render/painter.rs`, `src/render/mod.rs`, `src/app.rs`
**Tests:** 8 cursor positioning tests in `painter.rs`, 3 node tests in `node.rs`

---

### ISSUE-5: StyledSpan Lifetime Parameter [BLOCKER → FIXED]
**Severity:** BLOCKER
**Date:** 2026-01-01
**Resolved:** 2026-01-01 (Commit #73 in codex_inky)

**Problem:**
`StyledSpan` changed from `StyledSpan { text: String }` to `StyledSpan<'a> { text: Cow<'a, str> }`, causing 35 compilation errors in codex_inky.

**Solution:**
Updated codex_inky to use `StyledSpanOwned` (type alias for `StyledSpan<'static>`) everywhere, and used `.into()` for String-to-Cow conversions.

**Key changes in codex_inky:**
- Replace `use inky::prelude::StyledSpan` with `use inky::prelude::StyledSpanOwned`
- Replace `StyledSpan::new()` with `StyledSpanOwned::new()`
- Replace `span.text = string_value` with `span.text = string_value.into()`
- Replace `text.as_str()` with `&*text` or `text.as_ref()`

**Tests:** All 1390 tests pass after the fix.

---

### ISSUE-1: Bridge Code Overhead [CRITICAL → FIXED]
**Severity:** CRITICAL
**Date:** 2026-01-01
**Resolved:** 2026-01-01 (Commit #182)

**Problem:**
Porting Codex from ratatui to inky required 21,926 lines of bridge code due to lack of ratatui-compatible backend.

**Solution:**
Added `RatatuiBackend` implementing ratatui's `Backend` trait.

```rust
use inky::compat::ratatui::RatatuiBackend;
use ratatui::Terminal;

let backend = RatatuiBackend::new()?;
let mut terminal = Terminal::new(backend)?;
```

**Feature:** `compat-ratatui`
**Location:** `src/compat/ratatui.rs`

---

### ISSUE-2: Style Merge API Missing [HIGH → FIXED]
**Severity:** HIGH
**Date:** 2026-01-01
**Resolved:** 2026-01-01 (Commit #182)

**Problem:**
Had 4 duplicate style-merging implementations across adapters.

**Solution:**
Added `TextStyle::merge()` and `TextStyle::patch()`:

```rust
let combined = base_style.merge(&overlay_style);
// Colors: overlay takes precedence if present
// Booleans: OR'd together
```

**Location:** `src/style.rs`
**Tests:** 8 new tests

---

### ISSUE-3: Line-Level Style Wiring [BLOCKER → FIXED]
**Severity:** BLOCKER
**Date:** 2026-01-01
**Resolved:** 2026-01-01 (Commit #183)

**Problem:**
Line-level styling was partially implemented but not wired end-to-end. `paint_spans()` referenced
an undefined `line_cell`, and `render/mod.rs` could not access `TextNode::line_style`.

**Solution:**
Added `TextNode::line_style`, merged it into rendering defaults, and filled line backgrounds
before painting text/spans.

**Location:** `src/node.rs`, `src/render/mod.rs`, `src/render/painter.rs`, `src/components/transform.rs`
**Tests:** 2 new painter tests for line-level backgrounds
