# dterm-core Gap Roadmap

**Version:** 1.1
**Created:** 2025-12-28
**Total Gaps:** 57 (37 feature + 20 formal verification)

---

## MANDATORY DEVELOPMENT REQUIREMENTS

### Dual Track: Architecture AND Verification

**Both tracks must progress. Neither can be skipped.**

| Track | Focus | Priority |
|-------|-------|----------|
| Architecture | Gap 1-3 (pages, pools, pins) | CRITICAL |
| Verification | Rigorous Kani proofs | CRITICAL |

### Kani Proof Bounds Requirements

**Current problem:** Existing proofs use `rows <= 10, cols <= 10` which proves nothing useful about real terminals.

**REQUIRED for all new Kani proofs:**
```rust
// MINIMUM bounds for meaningful proofs
kani::assume(rows > 0 && rows <= 100);
kani::assume(cols > 0 && cols <= 200);
```

**For proofs that can handle it, use realistic terminal sizes:**
```rust
// PREFERRED bounds (actual terminal sizes)
kani::assume(rows > 0 && rows <= 500);
kani::assume(cols > 0 && cols <= 500);
```

Proofs with bounds `<= 10` are considered **incomplete** and must be upgraded.

### Workflow: Spec → Proof → Implement

For every gap:

```
1. TLA+ SPEC FIRST
   └─> Define state, operations, invariants
   └─> Run TLC, fix counterexamples

2. KANI PROOFS (bounds >= 100x200)
   └─> Write proofs BEFORE implementation
   └─> Proofs will FAIL initially - this is expected

3. IMPLEMENT
   └─> Write minimal code to make proofs pass
   └─> Run `cargo kani` after each change

4. TESTS
   └─> Property tests (proptest)
   └─> Unit tests for edge cases

5. COMMIT
   └─> Reference spec and proof bounds in commit message
```

### Architecture Gap Priority

**Gap 1 (offset-based pages) MUST be done before other feature gaps.**

This blocks: serialization, memory-mapping, network sync.

---

## IMPLEMENTATION HINTS

### Gap 1: Offset-Based Pages

**Existing code to use:**
- `crates/dterm-core/src/grid/page.rs` - Page, Offset, PageSlice, PageStore (294 lines)
- `crates/dterm-core/src/grid/page.rs:19` - `Page` struct (64KB aligned)
- `crates/dterm-core/src/grid/page.rs:47` - `Offset<T>` (type-safe offset, not pointer)
- `crates/dterm-core/src/grid/page.rs:109` - `PageSlice<T>` (slice within page)
- `crates/dterm-core/src/grid/page.rs:170` - `PageStore` (allocator)

**Integration points:**
- `crates/dterm-core/src/grid/mod.rs:106` - `Grid` struct needs `pages: PageStore`
- `crates/dterm-core/src/grid/row.rs:14` - `Row` struct needs `cells: PageSlice<Cell>`
- `crates/dterm-core/src/grid/row.rs:39` - `Row::new()` needs `pages: &mut PageStore` param

**Key pattern from Ghostty:**
```rust
// Row stores offset, not Vec
pub struct Row {
    cells: PageSlice<Cell>,  // NOT Vec<Cell>
    // ...
}

// Grid owns PageStore
pub struct Grid {
    pages: PageStore,
    rows: Vec<Row>,  // Rows reference into pages
    // ...
}
```

**What needs to change:**
1. Remove `#![allow(dead_code)]` from page.rs
2. Grid::new() must initialize PageStore
3. Grid::resize() must handle page reallocation
4. Scrollback eviction must free pages properly
5. Serialization uses offsets (no pointer fixup)

**Kani proofs needed (bounds >= 100x200):**
- `page_store_allocation_safe` - allocations stay within page bounds
- `offset_resolve_safe` - offset resolution produces valid pointers
- `grid_row_access_safe` - row[col] access within allocated PageSlice

### Gap 2: Memory Pooling

**Reference:** Ghostty `src/terminal/PageList.zig`

**Pattern:**
```rust
pub struct MemoryPool {
    free_pages: Vec<Box<Page>>,  // Recycled pages
    free_rows: Vec<Row>,          // Recycled rows
}

impl MemoryPool {
    pub fn preheat(&mut self, page_count: usize, row_count: usize);
    pub fn alloc_page(&mut self) -> Box<Page>;
    pub fn free_page(&mut self, page: Box<Page>);
}
```

### Gap 3: Pin System

**Reference:** Ghostty pins for stable references

**Pattern:**
```rust
pub struct Pin {
    page_id: PageId,
    row_offset: u32,
    generation: u64,  // Detect invalidation
}

impl Pin {
    pub fn resolve(&self, grid: &Grid) -> Option<&Cell>;
    pub fn is_valid(&self, grid: &Grid) -> bool;
}
```

**Use cases:**
- Selection start/end points survive scrollback eviction
- Hyperlink anchors remain valid
- Search match positions stable

---

## REFERENCE TERMINAL SOURCE CODE

All reference terminals are cloned in `research/`. Use these as implementation guides.

### Ghostty (Zig) - Best for Pages/Memory

| Feature | File | Lines | Description |
|---------|------|-------|-------------|
| Page storage | `research/ghostty/src/terminal/page.zig` | 3,131 | Offset-based page with memory layout |
| Page list | `research/ghostty/src/terminal/PageList.zig` | 10,852 | Page allocation, pooling, eviction |
| Terminal | `research/ghostty/src/terminal/Terminal.zig` | 11,677 | Main state machine |
| Screen | `research/ghostty/src/terminal/Screen.zig` | 9,035 | Screen buffer management |
| SIMD parser | `research/ghostty/src/simd/` | - | UTF-8, VT parsing acceleration |

### Alacritty (Rust) - Best for Grid/Rust patterns

| Feature | File | Lines | Description |
|---------|------|-------|-------------|
| Grid | `research/alacritty/alacritty_terminal/src/grid/mod.rs` | 656 | Grid struct, cursor |
| Storage | `research/alacritty/alacritty_terminal/src/grid/storage.rs` | 769 | Ring buffer storage |
| Row | `research/alacritty/alacritty_terminal/src/grid/row.rs` | 293 | Row struct, cells |
| Resize | `research/alacritty/alacritty_terminal/src/grid/resize.rs` | 389 | Resize with reflow |
| Vi mode | `research/alacritty/alacritty_terminal/src/vi_mode.rs` | 893 | Vi keyboard navigation |
| Selection | `research/alacritty/alacritty_terminal/src/selection.rs` | - | Selection state machine |

### Kitty (C/Python) - Best for Protocols

| Feature | File | Description |
|---------|------|-------------|
| Graphics | `research/kitty/kitty/graphics.c` | Kitty graphics protocol |
| Graphics parse | `research/kitty/kitty/parse-graphics-command.h` | Graphics command parsing |
| Key encoding | `research/kitty/kitty/keys.h` | Kitty keyboard protocol |

### WezTerm (Rust) - Best for Features

| Feature | File | Description |
|---------|------|-------------|
| Terminal | `research/wezterm/term/src/terminal.rs` | Terminal state |
| Screen | `research/wezterm/term/src/screen.rs` | Screen buffer |
| BiDi | `research/wezterm/bidi/` | Right-to-left text support |
| SSH | `research/wezterm/wezterm-ssh/` | SSH domain integration |

### foot (C) - Best for Performance

| Feature | File | Description |
|---------|------|-------------|
| Terminal | `research/foot/terminal.c` | Main terminal, coalescing |
| Sixel | `research/foot/sixel.c` | Sixel graphics decoder |
| OSC | `research/foot/osc.c` | OSC sequence handling |
| Render | `research/foot/render.c` | Wayland rendering |

### Contour (C++) - Best for VT Compliance

| Feature | File | Description |
|---------|------|-------------|
| VT sequences | `research/contour/src/vtbackend/` | Comprehensive VT520 support |
| Text shaper | `research/contour/src/text_shaper/` | HarfBuzz text shaping |

### Analysis Documents

Each terminal has an analysis document with architecture overview:
- `research/ghostty_ANALYSIS.md` - Ghostty deep dive
- `research/alacritty_ANALYSIS.md` - Alacritty patterns
- `research/kitty_ANALYSIS.md` - Kitty protocols
- `research/wezterm_ANALYSIS.md` - WezTerm features
- `research/foot_ANALYSIS.md` - foot performance
- `research/contour_ANALYSIS.md` - Contour VT compliance
- `research/windows-terminal_ANALYSIS.md` - Windows Terminal

---

## Status Summary

| Category | Total | Fixed | Remaining |
|----------|-------|-------|-----------|
| Feature Gaps (1-37) | 37 | 30 | 7 |
| Verification Gaps (FV-1 to FV-20) | 20 | 20 | 0 |
| **Total** | **57** | **50** | **7** |

### Fixed Gaps
- Gap 1: Offset-based pages integrated into Grid (iteration 92)
- Gap 2: Memory pooling implemented (iteration 93)
- Gap 3: Pin system for stable references (iteration 93)
- Gap 4: OSC 133 shell integration (iteration 94)
- Gap 31: Block-based output model (iteration 95)
- Gap 5: tmux control mode protocol (iteration 100)
- Gap 7: Kitty keyboard protocol (iteration 96)
- Gap 8: Sixel graphics decoder (iteration 98)
- Gap 9: Kitty graphics protocol (iteration 99)
- Gap 10: Line reflow on resize (iteration 102)
- Gap 6: Triggers system (iteration 104)
- Gap 13: XTWINOPS window manipulation (iteration 103)
- Gap 14: DECRQSS query setting responses (iteration 104)
- Gap 16: C1 control codes (8-bit) (iteration 105)
- Gap 23: Comparative benchmarks (iteration 106)
- Gap 37: VTTEST conformance tests (iteration 107)
- Gap 18: TLA+ Grid counterexamples resolved (iteration 110)
- Gap 19: Parser state machine Kani proofs (iteration 110)
- Gap 20: MIRI in CI for unsafe code (iteration 110)
- Gap 29: Vi mode for keyboard navigation (iteration 111)
- Gap 33: Session resurrection (iteration 112)
- Gap 28: FairMutex for PTY/render coordination (iteration 113)
- Gap 11: BiDi (right-to-left) text support (iteration 114)
- Gap 12: Grapheme cluster shaping (iteration 115)
- Gap 21: Comparative fuzzing corpus (iteration 116)
- Gap 24: Input coalescing (iteration 117)
- FV-1: Kani CI `continue-on-error` removed (iteration 79)
- FV-9: Fuzz time increased to 300s (iteration 79)
- FV-5: TLA+ Grid.tla updated with scroll_top/scroll_bottom variables (iteration 80)
- FV-6: TLA+ Grid.tla updated with DECSTBM operations (iteration 80)
- FV-12: Kani proofs added for ring buffer bounds (iteration 80)
- FV-2: Kani proofs added for Terminal struct (iteration 81)
- FV-7: TLA+ TerminalModes.tla created (iteration 82)
- FV-8: TLA+ Selection.tla created (iteration 83)
- FV-17: TLA+ Scrollback.tla NoLinesLost invariant fixed (iteration 83)
- FV-18: Parser param overflow proofs added (iteration 84)
- FV-19: Hyperlink memory safety proofs added (iteration 85)
- FV-16: AddressSanitizer added to CI for FFI boundary testing (iteration 86)
- FV-13: Damage tracker state machine proofs added (iteration 87)
- FV-10: Differential fuzzing against vte crate added (iteration 88)
- FV-15: Checkpoint serialization proofs added (iteration 89)
- FV-14: Resize content preservation TLA+ proofs added (iteration 90)
- FV-20: Structured proptest strategies for VT sequences added (iteration 91)
- Gap 36: RLE attribute compression (iteration 119)
- Gap 32: Synchronized output validation (iteration 120)
- Gap 35: SSH domain integration (iteration 120)
- Gap 22: TLA+ scrollback tiering spec complete (iteration 121)
- FV-4: PAGE.RS no longer dead code (iteration 121)
- Gap 15: VT520 conformance level tracking (iteration 121)
- FV-3: FFI boundary safety Kani proofs (iteration 122)
- FV-11: Parser TLA+ UTF-8 documented as intentional design (iteration 122)

---

## PHASE 1: CRITICAL ARCHITECTURE GAPS

### Gap 1: OFFSET-BASED PAGES NOT INTEGRATED (from Ghostty)
**Priority:** Critical
**Source:** Ghostty analysis - `src/terminal/page.zig`
**File:** `crates/dterm-core/src/grid/page.rs` is marked `#![allow(dead_code)]`

**Current:**
```rust
pub struct Grid {
    rows: Vec<Row>,  // Simple vector, NOT offset-based pages
}
```

**Should be:**
```rust
pub struct Grid {
    pages: Vec<Page>,  // Offset-based pages for serialization
}
```

**Why Critical:** This is THE KEY innovation from Ghostty that enables:
- Direct serialization to disk without pointer fixup
- Memory-mapping for instant load
- Network sync for remote terminals

**Action:** Integrate page.rs into the actual grid implementation.

---

### Gap 2: MEMORY POOLING NOT IMPLEMENTED (from Ghostty)
**Priority:** High
**Source:** Ghostty analysis - `src/terminal/PageList.zig`

Ghostty pre-heats memory pools for allocation-free hot paths. dterm has no pooling.

**Action:** Add memory pool for cells/rows with preheating:
```rust
pub struct MemoryPool {
    pages: PagePool,
    rows: RowPool,
    cells: CellPool,
}
impl MemoryPool {
    pub fn preheat(&mut self, count: usize) { ... }
}
```

---

### Gap 3: PIN SYSTEM FOR STABLE REFERENCES NOT IMPLEMENTED (from Ghostty)
**Priority:** High
**Source:** Ghostty analysis - Pins remain valid across buffer eviction

**Current:** No pin system - selections/hyperlinks break on scroll.

**Action:** Implement pin system:
```rust
pub struct Pin {
    page_id: PageId,
    row_offset: u32,
    col: u16,
}
impl Pin {
    fn resolve(&self, grid: &Grid) -> Option<CellRef> { ... }
}
```

---

## PHASE 2: SHELL INTEGRATION GAPS

### Gap 4: OSC 133 SHELL INTEGRATION
**Status:** ✅ FIXED (iteration 94)
**Source:** iTerm2, Ghostty shell integration

**Implementation:**
- Added `ShellState` enum for tracking prompt/command/executing states
- Added `CommandMark` struct for tracking command boundaries and exit codes
- Added `ShellEvent` enum for callback notifications
- Implemented OSC 133 A/B/C/D sequence parsing
- Added `shell_state()`, `command_marks()`, `current_mark()` APIs
- Added `set_shell_callback()` for event notifications
- Added `last_successful_command()` and `last_failed_command()` helpers
- Added 14 unit tests covering all OSC 133 functionality
- Command marks preserve working directory from OSC 7

**Files changed:**
- `crates/dterm-core/src/terminal/mod.rs` - All OSC 133 implementation

---

### Gap 5: TMUX CONTROL MODE
**Status:** ✅ FIXED (iteration 100)
**Source:** iTerm2, WezTerm, Ghostty - tmux -CC protocol

**Implementation:**
- Added `crates/dterm-core/src/tmux/mod.rs` - Complete tmux control mode module
- `TmuxControlParser` - Line-based parser for tmux control mode output
- `TmuxNotification` enum - All notification types (Output, SessionChanged, WindowAdd, etc.)
- `TmuxBlockEvent` enum - Events from parser (BlockStart, BlockEnd, BlockData, Notification)
- `TmuxControlMode` - State machine managing control mode lifecycle
- `TmuxEventSink` trait - Callback interface for receiving parsed events
- Layout string parser for window/pane layout trees (horizontal/vertical splits)
- `decode_octal_output()` - Decode tmux's octal-escaped binary output
- Command builders for common tmux operations (send-keys, list-panes, resize-pane, etc.)
- Pause mode support for flow control (tmux 3.2+)

**Protocol coverage:**
- `%begin`/`%end`/`%error` - Command response framing
- `%output`/`%extended-output` - Pane output (with octal decoding)
- `%session-changed`/`%sessions-changed` - Session events
- `%window-add`/`%window-close`/`%window-renamed` - Window lifecycle
- `%window-pane-changed` - Active pane tracking
- `%layout-change` - Layout parsing (recursive splits)
- `%pause`/`%continue` - Flow control (tmux 3.2+)
- `%exit` - Session termination
- `%subscription-changed` - Variable subscriptions
- `%client-detached`/`%client-session-changed` - Client events

**Files changed:**
- `crates/dterm-core/src/tmux/mod.rs` - Main module (~400 lines)
- `crates/dterm-core/src/tmux/parser.rs` - Protocol parser (~800 lines)
- `crates/dterm-core/src/lib.rs` - Added tmux module and prelude exports
- 37 unit tests covering all notification types and parsing

---

### Gap 6: TRIGGERS SYSTEM
**Status:** ✅ FIXED (iteration 104)
**Source:** iTerm2 analysis - regex patterns to actions

**Implementation:**
- Added `crates/dterm-core/src/triggers/mod.rs` - Complete triggers module
- `Trigger` struct with regex pattern, action, and configuration
- `TriggerAction` enum with 12 action types:
  - `Highlight` - Change text colors
  - `Alert` - System notification with title/message
  - `Bell` - Audio bell
  - `Mark` - Navigation mark
  - `SendText` - Send text to terminal
  - `RunCommand` - Execute external command
  - `Capture` - Capture to clipboard/variable/toolbelt
  - `Annotate` - Add annotation to matched region
  - `SetHostname`/`SetDirectory` - Semantic shell integration
  - `DetectPrompt` - Alternative to OSC 133
  - `NotifyAgent`/`RequireApproval` - Agent-specific actions
  - `Custom` - Callback-based custom actions
- `TriggerSet` - Collection of triggers
- `TriggerEvaluator` - Hot/cold path separated evaluator with:
  - Rate limiting (500ms default)
  - Partial line support
  - Idempotent re-evaluation
- `patterns` module with 11 common patterns (URL, email, IP, error keywords, etc.)
- `post_process_match()` for cleaning match boundaries
- 17 unit tests covering all functionality
- Terminal helper methods for trigger evaluation:
  - `row_text()`, `current_line_text()`, `rows_text()`, `all_rows_text()`
  - `iter_rows_text()`, `damaged_rows_text()` for efficient evaluation

**Design notes:**
- Follows iTerm2's hot/cold path separation - triggers run separately from parsing
- Rate limiting prevents CPU thrashing on rapid output
- Partial line triggers for instant feedback
- Post-processing handles URL/path boundary cleanup

**Files changed:**
- `crates/dterm-core/src/triggers/mod.rs` - New triggers module (~700 lines)
- `crates/dterm-core/src/terminal/mod.rs` - Helper methods for text extraction
- `crates/dterm-core/src/lib.rs` - Added triggers module and prelude exports
- `crates/dterm-core/Cargo.toml` - Added regex dependency

---

## PHASE 3: MODERN PROTOCOL GAPS

### Gap 7: KITTY KEYBOARD PROTOCOL NOT IMPLEMENTED (from Kitty)
**Priority:** High
**Source:** Kitty analysis - `kitty/key_encoding.c`

The modern keyboard protocol used by Neovim, Kakoune, etc.

**Features missing:**
- Disambiguated key events
- Key release events
- Modifier state reporting
- Alternate key reporting

**Action:** Implement CSI u keyboard encoding (progressive enhancement).

---

### Gap 8: SIXEL GRAPHICS
**Status:** ✅ FIXED (iteration 98)
**Source:** Contour, foot, Windows Terminal

**Implementation:**
- Added `crates/dterm-core/src/sixel/mod.rs` - Complete Sixel decoder module
- `SixelDecoder` struct with streaming parser state machine
- States: Ground, RasterAttributes, RepeatIntroducer, ColorIntroducer
- `SixelImage` struct for completed images (ARGB pixel buffer)
- Color palette support (1024 colors, VT340 default palette)
- RGB and HLS color definition parsing (with DEC hue rotation)
- Raster attributes parsing for dimension hints
- Repeat introducer (`!n<data>`) for efficient encoding
- Graphics CR (`$`) and NL (`-`) for multi-line images
- Maximum dimension limits (10000x10000) for DoS prevention
- DCS sequence integration in Terminal (DCS q)
- `has_sixel_image()`, `take_sixel_image()`, `peek_sixel_image()` APIs
- `sixel_palette()` for palette access
- Image records cursor position for placement
- 16 unit tests in sixel module + 8 terminal integration tests
- 5 Kani proofs for bounds verification

**Files changed:**
- `crates/dterm-core/src/sixel/mod.rs` - New Sixel module (~800 lines)
- `crates/dterm-core/src/lib.rs` - Added sixel module and prelude exports
- `crates/dterm-core/src/terminal/mod.rs` - DCS handling, Terminal fields
- `crates/dterm-core/src/tests/terminal_integration.rs` - Integration tests

---

### Gap 9: KITTY GRAPHICS PROTOCOL
**Status:** ✅ FIXED (iteration 99)
**Source:** Kitty analysis - `kitty/graphics.c`

**Implementation:**
- Added `crates/dterm-core/src/kitty_graphics/mod.rs` - Complete Kitty graphics module
- `KittyGraphicsCommand` parser - Parses APC graphics commands from key=value format
- `KittyImageStorage` - Stores images by ID with placement management
- `KittyImage` struct for image data with format detection (PNG, RGB, RGBA)
- `KittyPlacement` for image placement within the terminal grid
- Transmission types: Direct, File, Temporary file, Shared memory
- Chunked data transmission with base64 decoding
- Image actions: Transmit, TransmitAndDisplay, Query, Display, Delete
- Delete operations: All, ById, ByNumber, CurrentCell, CursorColumn, AnimationFrames
- Response generation for queries and confirmations
- APC sequence integration in Terminal
- Maximum image size limits (100MB) for DoS prevention
- 15+ unit tests covering parsing, storage, and transmission

**Files changed:**
- `crates/dterm-core/src/kitty_graphics/mod.rs` - New Kitty graphics module (~700 lines)
- `crates/dterm-core/src/lib.rs` - Added kitty_graphics module and prelude exports
- `crates/dterm-core/src/terminal/mod.rs` - APC handling integration

---

### Gap 10: LINE REFLOW ON RESIZE
**Status:** ✅ FIXED (iteration 102)
**Source:** Alacritty, WezTerm, Kitty

**Implementation:**
- Added WRAPPED flag to Row for soft/hard line break tracking
- Added reflow helper methods to Row:
  - `append_cells()` - Append cells to end of row
  - `split_front()` - Remove cells from beginning
  - `split_back()` - Remove cells from end
  - `prepend_cells()` - Insert cells at beginning
  - `extract_cells()` - Get all cells as Vec
  - `is_clear()` - Check if row is empty
- Implemented `resize_with_reflow()` in Grid:
  - `reflow_grow_columns()` - Unwrap soft-wrapped lines when growing
  - `reflow_shrink_columns()` - Wrap long lines when shrinking
  - Cursor position tracking through reflow
  - Round-trip correctness (shrink then grow preserves content)
- Added 8 unit tests covering:
  - Shrink wrapping, grow unwrapping
  - Cursor position preservation
  - Round-trip correctness
  - Hard line break preservation
  - Empty row handling
  - Reflow disable flag

**Files changed:**
- `crates/dterm-core/src/grid/row.rs` - Reflow helper methods
- `crates/dterm-core/src/grid/mod.rs` - Reflow implementation and tests

---

## PHASE 4: INTERNATIONALIZATION GAPS

### Gap 11: BIDI (RIGHT-TO-LEFT) TEXT SUPPORT
**Status:** ✅ FIXED (iteration 114)
**Source:** WezTerm analysis - `bidi/` crate

**Implementation:**
- Added `crates/dterm-core/src/bidi/mod.rs` - Complete BiDi module (~550 lines)
- Uses `unicode-bidi` crate for UBA (Unicode Bidirectional Algorithm) implementation
- `Direction` enum - LTR/RTL direction handling
- `ParagraphDirection` enum - Auto/AutoRtl/Ltr/Rtl hints for paragraph resolution
- `BidiResolver` - Main resolver for terminal text with:
  - `resolve_line()` - Resolve a line of text with BiDi support
  - `resolve_codepoints()` - Resolve from codepoint slice (avoids string allocation)
  - Fast path for pure-LTR text (common case optimization)
  - NSM (non-spacing mark) reordering option for terminal behavior
- `BidiResolution` - Result type with:
  - `visual_runs()` - Runs in visual order for rendering
  - `logical_to_visual()` / `visual_to_logical()` - Position mapping
  - `has_rtl()` / `is_pure_ltr()` - Quick checks
- `BidiRun` - Run of uniform direction with visual_indices() iterator
- `CharBidiClass` enum and `char_bidi_class()` - Character classification
- 13 unit tests covering:
  - Empty text, pure LTR, pure RTL (Hebrew, Arabic)
  - Mixed LTR/RTL text ("Hello שלום World")
  - Numbers in RTL context
  - Explicit BiDi controls (LRE/PDF)
  - NSM handling
  - Forced direction hints
- 5 Kani proofs for formal verification:
  - `direction_opposite_involution` - opposite(opposite(x)) == x
  - `level_direction_consistent` - Level to direction mapping
  - `bidi_run_visual_indices_length` - Iterator produces correct length
  - `bidi_run_visual_indices_contains_all` - All indices present
  - `char_bidi_class_strong_classification` - Strong class identification

**Architecture:**
- Storage: Cells stored in *logical* order (as received from PTY)
- Rendering: At render time, cells reordered to *visual* order
- Per-line: BiDi resolution performed per-line for terminal display

**Files changed:**
- `crates/dterm-core/src/bidi/mod.rs` - New BiDi module
- `crates/dterm-core/src/lib.rs` - Added bidi module and prelude exports
- `crates/dterm-core/Cargo.toml` - Added unicode-bidi dependency

---

### Gap 12: GRAPHEME CLUSTER SHAPING
**Status:** ✅ FIXED (iteration 115)
**Source:** Contour analysis - `text_shaper/`

**Implementation:**
- Added `crates/dterm-core/src/grapheme/mod.rs` - Complete grapheme cluster module (~700 lines)
- Uses `unicode-segmentation` crate for UAX #29 grapheme boundary detection
- Uses `unicode-width` crate for display width calculation (wcwidth equivalent)

**Core types and functions:**
- `Grapheme<'a>` - Information about a single grapheme cluster:
  - `grapheme: &str` - The grapheme string slice
  - `byte_offset: usize` - Position in source string
  - `width: usize` - Display width (0, 1, or 2 cells)
  - `codepoint_count: usize` - Number of Unicode codepoints
  - `is_emoji: bool`, `has_combining: bool` - Classification flags
- `GraphemeInfo` - Aggregate metrics for a string:
  - `grapheme_count`, `display_width`, `codepoint_count`, `byte_count`
  - `has_emoji`, `has_combining`, `has_wide` - Content flags
- `GraphemeSegmenter` - Stateful segmenter for terminal input processing
- `GraphemeCells` - Cell assignment for rendering graphemes

**Key functions:**
- `grapheme_width(s)` - Calculate display width and grapheme metrics
- `grapheme_display_width(g)` - Width of single grapheme (0-2)
- `split_graphemes(s)` - Iterator over graphemes with metadata
- `grapheme_at_byte(s, offset)` - Find grapheme containing byte offset
- `grapheme_at_column(s, col)` - Find grapheme at display column
- `byte_to_column(s, offset)` / `column_to_byte(s, col)` - Position conversion
- `truncate_to_width(s, width)` - Truncate without splitting graphemes
- `pad_to_width(s, width)` - Pad or truncate to exact width
- `assign_cells(s, start_col)` - Map graphemes to terminal cells
- `is_ascii_only(s)` / `ascii_width(s)` - Fast path for ASCII text

**Emoji detection:**
- Comprehensive emoji range detection (Emoticons, Symbols, Flags, ZWJ sequences)
- Regional indicator pairs (flags)
- Skin tone modifiers
- Zero Width Joiner sequences (family emoji)

**Testing:**
- 22 unit tests covering:
  - ASCII, CJK, emoji graphemes
  - Combining characters (e + combining acute)
  - Emoji ZWJ sequences (👨‍👩‍👧‍👦)
  - Regional indicators (🇺🇸)
  - Skin tone modifiers (👋🏽)
  - Mixed text with wide characters
  - Byte/column position conversion
  - Truncation and padding
  - Cell assignment
- 7 Kani proofs for formal verification:
  - `grapheme_display_width_bounded` - Width always 0-2
  - `truncate_preserves_utf8` - Truncation produces valid UTF-8
  - `column_to_byte_valid` - Byte offset in bounds
  - `byte_to_column_monotonic` - Column positions monotonic
  - `segmenter_column_advances` - Segmenter advances correctly
  - `cells_contains_col_correct` - Column containment check
  - `emoji_char_detection` - Emoji detection doesn't panic

**Architecture:**
- Storage: Text stored as-is (no normalization)
- Rendering: Grapheme segmentation at render time
- Fast path: ASCII-only text bypasses Unicode handling

**Files changed:**
- `crates/dterm-core/src/grapheme/mod.rs` - New grapheme module
- `crates/dterm-core/src/lib.rs` - Added grapheme module and prelude exports
- `crates/dterm-core/Cargo.toml` - Added unicode-segmentation dependency

---

## PHASE 5: VT COMPLIANCE GAPS

### Gap 13: WINDOW MANIPULATION (XTWINOPS)
**Status:** ✅ FIXED (iteration 103)
**Source:** Contour analysis - VT520 compliance

**Implementation:**
- Added `WindowOperation` enum for all CSI t operations (27 variants)
- Added `WindowResponse` enum for report responses (6 variants)
- Added `WindowCallback` type for platform-specific window operations
- Added title stack for CSI 22/23 t (push/pop title operations)
- Title stack capped at 10 entries to prevent memory exhaustion

**Window operations implemented:**
- State operations: Iconify (2), DeIconify (1)
- Geometry operations: Move (3), Resize pixels (4), Resize cells (8), Raise (5), Lower (6), Refresh (7)
- Maximize/fullscreen: Maximize (9;1), Restore (9;0), Maximize V (9;2), Maximize H (9;3)
- Fullscreen: Enter (10;1), Exit (10;0), Toggle (10;2)
- Reports: Window state (11), Position (13), Text area size pixels (14), Window size pixels (14;2)
- Reports: Screen size pixels (15), Cell size (16), Text area size cells (18), Screen size cells (19)
- Reports: Icon label (20), Window title (21) - with escape sequence filtering for security
- Title stack: Push title (22), Pop title (23) with icon/window/both options

**Security considerations:**
- Title reports filter escape sequences and control characters to prevent injection attacks
- Title stack has maximum depth limit (10) to prevent DoS

**Files changed:**
- `crates/dterm-core/src/terminal/mod.rs` - All XTWINOPS implementation and 26 unit tests

---

### Gap 14: DECRQSS QUERY SETTING RESPONSES
**Status:** ✅ FIXED (iteration 104)
**Source:** Contour, Windows Terminal

**Implementation:**
- Added `DcsType::Decrqss` for DCS $ q parsing
- `handle_decrqss()` parses the setting mnemonic (Pt)
- Supported queries:
  - `m` - SGR (text attributes): bold, dim, italic, underline, blink, inverse, hidden, strikethrough, fg/bg colors
  - ` q` - DECSCUSR (cursor style): styles 1-6
  - `r` - DECSTBM (scroll margins): top and bottom
  - `"p` - DECSCL (conformance level): reports VT320 level
  - `"q` - DECSCA (character protection): reports protection state
  - `t` - DECSLPP (lines per page): reports terminal height
- Response format: `DCS 1 $ r <payload><Pt> ST` (valid) or `DCS 0 $ r ST` (invalid)
- Uses correct validity byte (1=valid, 0=invalid per actual hardware, not inverted DEC manual)
- 6 unit tests covering all query types

**Files changed:**
- `crates/dterm-core/src/terminal/mod.rs` - DECRQSS implementation and tests

---

### Gap 15: VT520 CONFORMANCE LEVEL TRACKING (from Contour)
**Status:** ✅ FIXED (iteration 121)
**Source:** Contour analysis - `VTType` enum

**Implementation:**
- Added `crates/dterm-core/src/vt_level.rs` - Complete VT conformance level module (~400 lines)
- `VtLevel` enum: VT100, VT220, VT240, VT320, VT330, VT340, VT420, VT510, VT520, VT525
- DA2 (Secondary Device Attributes) parameter encoding/decoding
- DECSCL (Set Conformance Level) parameter encoding/decoding
- Feature support queries: `supports_c1_controls()`, `supports_sixel()`, `supports_mouse()`, `supports_rectangular_ops()`, `supports_pages()`, `supports_sessions()`
- `VtExtension` enum: None, Unknown, XTerm, ITerm2, Kitty
- `DeviceAttributes` flags: COLUMNS_132, PRINTER, SELECTIVE_ERASE, USER_DEFINED_KEYS, NRCS, TECHNICAL_CHARS, ANSI_COLOR, ANSI_TEXT_LOCATOR, SIXEL_GRAPHICS, RECTANGULAR_EDITING, WINDOWING, CAPTURE_SCREEN, COLOR_256, TRUE_COLOR
- `min_vt_level_for_csi()` - Get minimum VT level for CSI sequences
- `min_vt_level_for_esc()` - Get minimum VT level for ESC sequences
- 12 unit tests covering level ordering, DA2/DECSCL roundtrips, feature queries
- 5 Kani proofs for formal verification

**Files changed:**
- `crates/dterm-core/src/vt_level.rs` - New module
- `crates/dterm-core/src/lib.rs` - Added vt_level module and prelude exports

---

### Gap 16: C1 CONTROL CODES (8-BIT)
**Status:** ✅ FIXED (iteration 105)
**Source:** Windows Terminal, ECMA-48

**Implementation:**
- Fixed `advance_fast` parser to properly route C1 codes (0x80-0x9F) to state machine
  - Previously these were incorrectly treated as UTF-8 continuation bytes
- Added C1 execute handlers in Terminal for:
  - 0x84 (IND) - Index (move cursor down, same as ESC D)
  - 0x85 (NEL) - Next Line (CR + LF, same as ESC E)
  - 0x88 (HTS) - Horizontal Tab Set (same as ESC H)
  - 0x8D (RI) - Reverse Index (move cursor up, same as ESC M)
  - 0x8E (SS2) - Single Shift 2 (use G2 for next char, same as ESC N)
  - 0x8F (SS3) - Single Shift 3 (use G3 for next char, same as ESC O)
- Parser state table already supported C1 state transitions:
  - 0x90 (DCS) - Device Control String
  - 0x9B (CSI) - Control Sequence Introducer
  - 0x9C (ST) - String Terminator
  - 0x9D (OSC) - Operating System Command
  - 0x98, 0x9E, 0x9F (SOS, PM, APC) - String sequences
- Added 17 unit tests covering all C1 control codes
- All 8-bit C1 codes now behave identically to their 7-bit ESC equivalents

**Files changed:**
- `crates/dterm-core/src/parser/mod.rs` - Fixed advance_fast C1 handling
- `crates/dterm-core/src/terminal/mod.rs` - Added C1 execute handlers and tests

---

### Gap 17: DRCS (DOWNLOADABLE CHARACTER SETS) (from Windows Terminal)
**Priority:** Low
**Source:** Windows Terminal - `FontBuffer.cpp`

Soft font support.

**Action:** Implement DRCS (soft fonts) protocol.

---

## PHASE 6: FORMAL VERIFICATION GAPS

### Gap 18: TLA+ GRID SPEC COUNTEREXAMPLES
**Status:** ✅ FIXED (iteration 110)
**Source:** Internal audit - `tla/Grid_TTrace_*.bin` files existed

**Finding:** TLA+ model checking previously produced trace files indicating counterexamples.

**Resolution:**
- Trace files removed; no counterexamples remain
- Grid.tla spec fixed in iterations 80, 90 (FV-5, FV-6, FV-14)
- Added scroll region variables and operations (scroll_top, scroll_bottom)
- Added cell content modeling for resize preservation proofs
- TLC model checker runs in CI (`.github/workflows/verify.yml` lines 207-213)
- All invariants pass: TypeInvariant, Safety, content preservation

---

### Gap 19: PARSER STATE MACHINE KANI PROOFS
**Status:** ✅ FIXED (iteration 110)
**Source:** Research comparison - Ghostty/Contour have table-driven parsers

**Implementation:**
- Added 10 Kani proofs for comprehensive state machine verification:
  - `state_transitions_all_valid` - All state transitions from any state produce valid states
  - `state_transitions_sequential_valid` - Multiple bytes maintain valid states
  - `c1_controls_valid_transitions` - C1 control codes (0x80-0x9F) handled correctly
  - `escape_sequence_terminates` - ESC sequences terminate correctly
  - `csi_sequence_terminates` - CSI sequences return to ground after final byte
  - `osc_sequence_terminates` - OSC sequences terminate on BEL/ST
  - `dcs_sequence_terminates` - DCS sequences terminate on ST
  - `cancel_returns_to_ground` - CAN (0x18) abort returns to ground from any state
  - `utf8_continuation_safe` - Orphan continuation bytes don't corrupt state
  - `transition_table_lookup_safe` - Table lookups produce valid action/state pairs

**Coverage:**
- All 14 parser states verified
- All 256 byte values tested against state machine
- State transitions from arbitrary starting states
- Sequence termination proofs for all major sequence types

**Files changed:**
- `crates/dterm-core/src/parser/mod.rs` - Added 10 Kani proofs

---

### Gap 20: MIRI IN CI FOR UNSAFE CODE
**Status:** ✅ FIXED (iteration 110)
**Source:** CLAUDE.md requirement - "Run MIRI in CI"

**Implementation:**
- MIRI job already exists in `.github/workflows/verify.yml` (lines 66-85)
- Runs on every push to main and PR
- Uses nightly Rust with miri component
- Executes `cargo +nightly miri test --package dterm-core -- --skip proptest`
- Skips proptest due to MIRI incompatibility with random generation

**Coverage:**
- All unsafe blocks in dterm-core are verified
- Parser unsafe code (SIMD path)
- Grid unsafe code (pointer operations)
- Checkpoint serialization
- Cell extras HashMap operations

---

### Gap 21: COMPARATIVE FUZZING CORPUS
**Status:** ✅ FIXED (iteration 116)
**Source:** Windows Terminal - OneFuzz integration

**Implementation:**
- Added `crates/dterm-core/fuzz/build_corpus.rs` - Corpus generator script
- Generates 218 structured seed corpus entries covering all vttest categories
- Corpus written to both `corpus/parser/` and `corpus/parser_diff/`

**Categories covered:**
1. Cursor Movements - CUP, CUU, CUD, CUF, CUB, CNL, CPL, CHA, VPA, HPA, HPR, VPR, IND, RI, NEL
2. Screen Features - DECSTBM, ED, EL, SU, SD, DECALN, mode sets
3. Character Sets - G0/G1/G2/G3, LS0, LS1, LS2, LS3, SS2, SS3, box drawing
4. Terminal Reports - DA, DSR, CPR, DECRQSS
5. VT102 Features - ICH, DCH, IL, DL, ECH
6. SGR - All text attributes, 256-color, 24-bit true color
7. Private Modes - DECTCEM, DECSCUSR, DECCKM, alternate screen, mouse, bracketed paste
8. OSC - titles, hyperlinks, OSC 133 shell integration, clipboard
9. DCS - DECRQSS, Sixel, XTGETTCAP
10. C1 Control Codes - 8-bit equivalents (0x84-0x9F)
11. Tab Handling - HT, HTS, TBC, CHT, CBT
12. Reset - RIS, DECSTR
13. Save/Restore - DECSC, DECRC
14. Window Ops - XTWINOPS
15. REP - Repeat character
16. Kitty Keyboard Protocol
17. Real-World Patterns - git status, ls, shell prompts, progress bars
18. Edge Cases - incomplete sequences, overlong params, CAN/SUB interrupts

**Usage:**
```bash
cd crates/dterm-core/fuzz
cargo run --release --bin build_corpus
```

**Files changed:**
- `crates/dterm-core/fuzz/build_corpus.rs` - New corpus generator (~500 lines)
- `crates/dterm-core/fuzz/Cargo.toml` - Added build_corpus binary

---

### Gap 22: TLA+ SPEC FOR SCROLLBACK TIERING INCOMPLETE
**Status:** ✅ FIXED (iteration 121)
**Source:** Internal - `tla/Scrollback.tla`

**Implementation:**
- Added `nextLineId` variable for monotonically increasing line identification
- Updated `WarmBlock` and `ColdPage` records to track `minLineId` and `maxLineId`
- Added helper functions: `WarmMaxLineId`, `WarmMinLineId`, `ColdMaxLineId`, `ColdMinLineId`, `HotMinLineId`, `HotMaxLineId`
- Enhanced tier age ordering invariants with explicit verification:
  - `HotTierNewest` - Hot tier lines are ordered by line ID
  - `WarmBlocksOrdered` - Warm blocks are ordered by age
  - `ColdPagesOrdered` - Cold pages are ordered by age
  - `WarmOlderThanHot` - All warm lines older than all hot lines
  - `ColdOlderThanWarm` - All cold lines older than all warm lines
  - `ColdOlderThanHot` - Transitive ordering verified
- Added transition properties:
  - `LineIdMonotonic` - New lines always get higher IDs
  - `ForwardOnlyTransition` - Lines only move hot→warm→cold
  - `ColdAppendOnly` - Cold tier is append-only
  - `WarmFifoEviction` - FIFO eviction from warm tier
- Added 4 new theorems: `ForwardOnly`, `LineIdAlwaysMonotonic`, `ColdIsAppendOnly`, `WarmIsFifo`
- Updated TLC config to verify all new invariants

**Files changed:**
- `tla/Scrollback.tla` - Enhanced tier transition verification
- `tla/Scrollback.cfg` - Added new invariants to model checker config

---

## PHASE 7: PERFORMANCE GAPS

### Gap 23: COMPARATIVE BENCHMARKS
**Status:** ✅ FIXED (iteration 106)
**Source:** HINT.md benchmark gaps

**Implementation:**
- Added `crates/dterm-core/benches/comparative.rs` - Comprehensive comparative benchmark suite
- Compares dterm-core parser against vte crate (used by Alacritty)
- Test corpus: ASCII, mixed terminal, heavy escapes, UTF-8, vttest-style, realistic output
- Added full terminal processing benchmarks (parser + state machine)

**Benchmark Results (1MB test data, parser only):**

| Workload | dterm-core | vte | Ratio |
|----------|-----------|-----|-------|
| ASCII (best case) | 3.4 GiB/s | 340 MiB/s | **10x faster** |
| Mixed (typical) | 1.85 GiB/s | 330 MiB/s | **5.6x faster** |
| Heavy escapes | 267 MiB/s | 406 MiB/s | 1.5x slower |

**Full Terminal Processing (256KB, 80x24 terminal):**

| Workload | Throughput |
|----------|-----------|
| ASCII | 120 MiB/s |
| Mixed | 105 MiB/s |
| Realistic (git log) | 145 MiB/s |
| Heavy escapes | 87 MiB/s |

**Key findings:**
- dterm's SIMD fast path dominates on ASCII-heavy workloads (10x faster than vte)
- Mixed workloads (typical shell usage) are 5.6x faster
- Heavy escape sequences show vte is 1.5x faster (expected - escape-dense workloads don't benefit from SIMD ASCII scanning)
- Full terminal processing bottlenecks on grid state updates, not parsing
- Realistic throughput with full state machine: 87-145 MiB/s (depends on escape density)

**Files changed:**
- `crates/dterm-core/benches/comparative.rs` - New benchmark file (~770 lines)
- `crates/dterm-core/benches/parser.rs` - Added APC methods to CountingSink
- `crates/dterm-core/Cargo.toml` - Added vte dev-dependency, comparative bench entry

---

### Gap 24: INPUT COALESCING
**Status:** ✅ FIXED (iteration 117)
**Source:** Kitty - `vt-parser.c`, foot - `terminal.c`

**Implementation:**
- Added `crates/dterm-core/src/coalesce/mod.rs` - Complete input coalescing module (~650 lines)
- `InputCoalescer` - Dual-timer coalescing state machine:
  - Lower timer: Reset on every input (0.5ms default) - batches rapid writes
  - Upper timer: Maximum delay (8.3ms default) - ensures bounded latency
  - Buffer threshold: Forces immediate render (16KB default)
  - Max buffer size: Hard limit to prevent DoS (1MB default)
- `CoalesceConfig` - Configurable delays and thresholds:
  - `new()`, `default()` - Standard configuration
  - `low_latency()` - Gaming/interactive (0.25ms/4ms)
  - `high_throughput()` - Build logs (1ms/16.6ms)
  - `for_120hz()`, `for_144hz()` - High refresh rate displays
  - `disabled()` - Bypass coalescing
- `CoalesceAction` - RenderNow or WaitUntil(deadline)
- `CoalesceState` - Idle, Waiting, UpperArmed
- `RenderCallback` trait - Integration with event loops
- Statistics: total_bytes, total_batches, average_batch_size
- 8 Kani proofs for formal verification:
  - `config_upper_less_than_lower_invalid` - Validates config constraints
  - `config_upper_too_large_invalid` - Upper < 1 second check
  - `config_valid_passes` - Valid configs accepted
  - `accumulated_bytes_saturates` - No overflow on accumulation
  - `on_render_resets_state` - State reset correctness
  - `buffer_threshold_triggers_render` - Threshold behavior
  - `state_transitions_valid` - State machine validity
  - `average_batch_size_safe` - Division safety
- 20 unit tests covering all functionality
- TLA+ specification: `tla/Coalesce.tla`

**Design based on:**
- foot: Dual-timer with lower (0.5ms) and upper (8.3ms) bounds using timerfd
- Kitty: input_delay (3ms), repaint_delay (10ms), buffer threshold (16KB)
- WezTerm: Action coalescing with adaptive delay

**Files changed:**
- `crates/dterm-core/src/coalesce/mod.rs` - New coalesce module
- `crates/dterm-core/src/lib.rs` - Added coalesce module and prelude exports
- `tla/Coalesce.tla` - TLA+ specification for state machine

---

### Gap 25: NO FRAME CALLBACK SYNCHRONIZATION (from foot)
**Status:** ✅ FIXED (iteration 130)
**Priority:** Low
**Source:** foot analysis - Wayland frame callbacks

foot syncs rendering to compositor refresh.

**Implementation:**
- Added `crates/dterm-core/src/render/mod.rs` with `FrameSync`
- `FrameSyncMode` supports immediate vs callback rendering
- `FrameAction` signals request/render/no-op actions
- Tests cover callback lifecycle and damage during render

---

### Gap 26: NO WORKER THREAD POOL FOR ROW RENDERING (from foot)
**Priority:** Low
**Source:** foot analysis - `render_worker_thread()`

foot parallelizes row rendering across workers.

**Action:** Add optional parallel row rendering infrastructure.

---

### Gap 27: TRIPLE BUFFERING NOT IMPLEMENTED (from Ghostty)
**Status:** ✅ FIXED (iteration 130)
**Priority:** Low
**Source:** Ghostty - `swap_chain_count = 3`

Ghostty uses triple buffering for smooth rendering.

**Implementation:**
- Added `TripleBuffer<T>` to `crates/dterm-core/src/render/mod.rs`
- Tracks front/middle/back buffers with pending swap flag
- `publish()` and `present()` helpers for render and vsync stages
- Tests cover publish/replace/present behavior

---

## PHASE 8: TERMINAL-SPECIFIC GAPS

### Gap 28: FairMutex FOR PTY/RENDER COORDINATION (from Alacritty)
**Status:** ✅ FIXED (iteration 113)
**Source:** Alacritty analysis - `FairMutex`

**Implementation:**
- Added `crates/dterm-core/src/sync/mod.rs` - Fair synchronization primitives module (~550 lines)
- `FairMutex<T>` - Fair mutex preventing thread starvation:
  - Uses two-lock protocol (next + data) to ensure fairness
  - `lock()` - Fair acquisition that queues behind waiters
  - `lock_unfair()` - Direct acquisition bypassing fairness queue
  - `try_lock()` / `try_lock_unfair()` - Non-blocking variants
  - `lease()` / `try_lease()` - Reserve access without acquiring data lock
  - `lock_with_lease()` - Convert lease to data lock efficiently
  - `get_mut()` / `into_inner()` - Exclusive/consuming access
  - `is_locked()` - Check lock status
- `Lease<'a>` - Reservation type for deferred locking
- `FairRwLock<T>` - Fair read-write lock preventing writer starvation:
  - `read()` - Fair read acquisition (queues behind waiting writers)
  - `write()` - Fair write acquisition
  - `try_read()` / `try_write()` - Non-blocking variants
- 23 unit tests covering:
  - Basic lock/unlock operations
  - Fair and unfair acquisition paths
  - Lease API (acquire, convert, cancel)
  - `try_lock` semantics
  - Multithreaded fairness (10 threads, 1000 increments)
  - RwLock reader/writer concurrency
- 9 Kani proofs for formal verification:
  - `fair_mutex_lock_unlock_safe` - Lock/unlock preserves value
  - `fair_mutex_unfair_lock_consistent` - Unfair lock returns correct value
  - `fair_mutex_try_lock_behavior` - Try lock reports locked status correctly
  - `fair_mutex_get_mut_correct` - Mutable access works correctly
  - `fair_mutex_into_inner_correct` - Consuming access returns value
  - `fair_rwlock_read_correct` - Read returns stored value
  - `fair_rwlock_write_correct` - Write modifies correctly
  - `fair_rwlock_into_inner_correct` - Consuming returns value

**Design based on Alacritty's `sync.rs`:**
- Two-lock fairness protocol from Alacritty
- Extended with Lease API for deferred locking
- Added FairRwLock for read-heavy workloads
- Full Send + Sync implementations for multi-threaded use

**Use case:**
- PTY thread continuously receives output and updates terminal state
- Render thread periodically reads terminal state for rendering
- FairMutex ensures neither thread starves the other

**Files changed:**
- `crates/dterm-core/src/sync/mod.rs` - New sync module
- `crates/dterm-core/src/lib.rs` - Added sync module and prelude exports
- `crates/dterm-core/Cargo.toml` - Added parking_lot dependency
- `Cargo.toml` - Added parking_lot to workspace dependencies

---

### Gap 29: VI MODE
**Status:** ✅ FIXED (iteration 111)
**Source:** Alacritty - `vi_mode.rs` (893 lines), Rio

**Implementation:**
- Added `crates/dterm-core/src/vi_mode/mod.rs` - Complete vi mode module (~750 lines)
- `ViModeCursor` struct for tracking vi mode cursor position (separate from terminal cursor)
- `ViMotion` enum with 21 motion commands:
  - Basic: Up, Down, Left, Right (h/j/k/l)
  - Line: First, Last, FirstOccupied (0/$/ ^)
  - Screen: High, Middle, Low (H/M/L)
  - Word: SemanticLeft/Right (b/w), SemanticLeftEnd/RightEnd (ge/e)
  - WORD: WordLeft/Right (B/W), WordLeftEnd/RightEnd (gE/E)
  - Bracket matching (%)
  - Paragraph: ParagraphUp/Down ({/})
- Left/Right wraps across lines
- Scrollback navigation support
- Line wrap awareness for First/Last motions
- 17 unit tests covering all motion types

**Public API:**
- `ViModeCursor::new(row, col)` - Create cursor at position
- `ViModeCursor::from_terminal(grid)` - Create at terminal cursor
- `ViModeCursor::motion(grid, motion)` - Apply motion
- `ViModeCursor::scroll(grid, lines)` - Scroll by lines (page up/down)
- `ViModeCursor::visible_row(grid)` - Get visible row index
- `ViModeCursor::is_visible(grid)` - Check if in visible area

**Files changed:**
- `crates/dterm-core/src/vi_mode/mod.rs` - New vi mode module
- `crates/dterm-core/src/lib.rs` - Added vi_mode module and prelude exports

---

### Gap 30: DAEMON MODE NOT IMPLEMENTED (from foot)
**Priority:** Low
**Source:** foot analysis - `server.c`

footserver/footclient model for instant window spawn.

**Action:** Design daemon mode for shared font/glyph caches.

---

### Gap 31: BLOCK-BASED OUTPUT MODEL (from Warp)
**Status:** ✅ FIXED (iteration 95)
**Source:** Warp analysis - command/output as atomic blocks

**Why Important for Agents:** Blocks provide natural context boundaries.

**Implementation:**
- Added `BlockState` enum: PromptOnly, EnteringCommand, Executing, Complete
- Added `OutputBlock` struct tracking:
  - Block ID (unique per session)
  - State transitions driven by OSC 133 events
  - Row ranges for prompt/command/output sections
  - Exit code and working directory
- Added block navigation API:
  - `output_blocks()` - completed blocks
  - `current_block()` - in-progress block
  - `all_blocks()` - iterator over all blocks
  - `block_by_id()`, `block_by_index()` - lookup
  - `block_at_row()` - find block containing row
  - `next_block_after_row()`, `previous_block_before_row()` - navigation
  - `last_successful_block()`, `last_failed_block()` - filtering
- OutputBlock helper methods: `prompt_rows()`, `command_rows()`, `output_rows()`, `contains_row()`
- Blocks build on OSC 133 shell integration (Gap 4)
- Added 21 unit tests covering all block functionality

**Files changed:**
- `crates/dterm-core/src/terminal/mod.rs` - All block implementation

---

### Gap 32: SYNCHRONIZED OUTPUT VALIDATION (from WezTerm)
**Status:** ✅ FIXED (iteration 120)
**Source:** WezTerm - DECSET 2026 handling

**Implementation:**
- Added 14 conformance tests for synchronized output mode (2026):
  - `synchronized_output_soft_reset_clears_mode` - DECSTR clears sync mode
  - `synchronized_output_full_reset_clears_mode` - RIS clears sync mode
  - `synchronized_output_idempotent_enable` - Multiple enables are safe
  - `synchronized_output_idempotent_disable` - Multiple disables are safe
  - `synchronized_output_frame_pattern` - Typical enable->draw->disable pattern
  - `synchronized_output_nested_patterns` - Non-nesting semantics
  - `synchronized_output_save_restore_cursor_preserves` - DECSC/DECRC don't affect sync
  - `synchronized_output_alternate_screen_preserves` - Screen switches don't affect sync
  - `synchronized_output_output_continues_normally` - Text output works during sync
  - `synchronized_output_cursor_movement_continues` - Cursor ops work during sync
  - `synchronized_output_query_during_sync` - DECRQM works during sync
  - `synchronized_output_with_combined_mode_sequence` - Mode independence
- Updated DECSTR (soft reset) to clear synchronized output mode
- Based on synchronized rendering spec: https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036

**Files changed:**
- `crates/dterm-core/src/terminal/mod.rs` - Added tests and DECSTR update

---

### Gap 33: SESSION RESURRECTION (from Zellij) ✓ FIXED
**Priority:** Medium
**Source:** Zellij analysis - `session_serialization.rs`

Sessions serialize to KDL and can be restored.

**Action:** Add session serialization/restore capability.

**Implemented:** `crates/dterm-core/src/session/mod.rs`
- `SessionManager` - Save/load/list/delete sessions
- `SessionManifest` - Multi-tab session layout
- `TabManifest` - Tab with multiple panes
- `PaneManifest` - Pane geometry and metadata
- `TerminalState` - Full terminal state serialization:
  - Terminal modes (cursor style, mouse mode, etc.)
  - Current style (colors, flags, protected)
  - Character set state (G0-G3, GL, single shift)
  - Saved cursor states (DECSC/DECRC)
  - Title, icon name, hyperlinks, CWD
  - Kitty keyboard flags
- Binary format with magic bytes and versioning
- Integration with existing checkpoint system for grid/scrollback
- 12 unit tests for roundtrip serialization

---

### Gap 34: WASM PLUGIN SYSTEM (from Zellij)
**Priority:** Low
**Source:** Zellij - `wasm_bridge.rs`

Sandboxed WASM plugins with explicit permissions.

**Action:** Design WASM plugin system for agent extensions.

---

### Gap 35: SSH DOMAIN INTEGRATION (from WezTerm)
**Status:** ✅ FIXED (iteration 120)
**Source:** WezTerm - `wezterm-ssh/`, Domain trait

**Implementation:**
- Added `crates/dterm-core/src/domain/mod.rs` - Complete domain abstraction (~550 lines)
- `Domain` trait - Core abstraction for terminal connection types:
  - `spawn_pane()` - Create new terminal sessions
  - `attach()` / `detach()` - Connection lifecycle
  - `state()` - Connection state tracking
  - `spawnable()` / `detachable()` - Capability queries
- `Pane` trait - Active terminal session interface:
  - `write()` / `read()` - I/O operations
  - `resize()` - Terminal size changes
  - `is_alive()` / `exit_status()` / `kill()` - Process lifecycle
  - `pid()` / `title()` / `cwd()` - Session metadata
- Domain types defined: Local, SSH, WSL, Serial, Mux, Custom
- ID types: `DomainId`, `PaneId` with atomic allocation
- State enum: `DomainState` (Detached, Attached, Connecting, Failed)
- Configuration types:
  - `SpawnConfig` - Process spawn configuration with builder pattern
  - `SshConfig` - SSH connection settings (host, port, auth, keepalive)
  - `WslConfig` - WSL distribution settings
  - `SerialConfig` - Serial port settings (baud, parity, flow control)
- `DomainRegistry` - Multi-domain management with default selection
- `DomainError` - Comprehensive error enum with Display impl
- 9 unit tests covering all configuration builders and registry operations

**Architecture:**
- Trait-based design allows multiple domain implementations
- Registry pattern for managing multiple concurrent domains
- Builder patterns for configuration ergonomics
- Based on WezTerm's `mux/src/domain.rs` architecture

**Files changed:**
- `crates/dterm-core/src/domain/mod.rs` - New domain module
- `crates/dterm-core/src/lib.rs` - Added domain module and prelude exports

---

### Gap 36: RLE ATTRIBUTE COMPRESSION (from Windows Terminal)
**Status:** ✅ FIXED (iteration 119)
**Source:** Windows Terminal - `til/rle.h`

**Implementation:**
- Added `crates/dterm-core/src/rle/mod.rs` - Complete RLE module (~700 lines)
- `Rle<T>` - Run-Length Encoded sequence with O(log n) random access:
  - `push()`, `extend_with()` - Efficient append with auto-merge
  - `get()`, `set()` - Index access and modification
  - `set_range()` - Efficient range updates (key for terminal operations)
  - `resize()`, `truncate()` - Length management
  - Auto-compaction of adjacent runs with same value
  - `iter()`, `iter_runs()` - Both expanded and run-based iteration
- `Run<T>` - Basic run struct with value and length
- `StyleId` - 16-bit style identifier for compact cell representation
- `CompressedStyle` - Compact style representation (fg, bg, flags)
- `StyleRegistry` - Style deduplication registry:
  - ID 0 always maps to default style
  - `get_or_insert()` - Get existing ID or allocate new
  - Generation counter for cache invalidation
- 20 unit tests covering all RLE operations
- 8 Kani proofs for formal verification:
  - `rle_length_consistent` - Sum of runs equals total length
  - `rle_get_valid_index` - Valid indices return values
  - `rle_get_invalid_index` - Out-of-bounds returns None
  - `rle_set_preserves_length` - Set doesn't change length
  - `rle_resize_grow_correct` - Grow adds correct amount
  - `rle_resize_shrink_correct` - Shrink truncates correctly
  - `style_registry_default_is_zero` - Default style ID is 0
  - `style_registry_deduplicates` - Identical styles get same ID

**Design based on:**
- Windows Terminal `til/rle.h` - Run-based compression
- Ghostty style ID indirection pattern
- Auto-compaction for memory efficiency

**Files changed:**
- `crates/dterm-core/src/rle/mod.rs` - New RLE module
- `crates/dterm-core/src/lib.rs` - Added rle module and prelude exports

---

### Gap 37: VTTEST CONFORMANCE
**Status:** ✅ FIXED (iteration 107)
**Source:** Contour analysis - extensive test suite

**Implementation:**
- Added `crates/dterm-core/src/tests/vttest_conformance.rs` - Comprehensive vttest-based test suite
- 40 tests covering vttest menu categories 1-11
- Tests based on vttest terminal conformance program (https://invisible-island.net/vttest/)

**Test Categories and Results:**

| Category | Tests | Status | Notes |
|----------|-------|--------|-------|
| 1. Cursor movements | 5 tests | ✅ PASS | CUP, CUU, CUD, CUF, CUB, CNL, CPL |
| 2. Screen features | 7 tests | ✅ PASS | DECAWM, scroll region, origin mode, DECALN, erase, IRM |
| 3. Character sets | 2 tests | ✅ PASS | DEC Special Graphics, UK charset |
| 4. Double-sized chars | N/A | PARTIAL | DECDHL/DECDWL line size flags only |
| 5. Keyboard | N/A | N/A | Not applicable to terminal core |
| 6. Terminal reports | 3 tests | ✅ PASS | DA, DSR, CPR |
| 7. VT52 mode | N/A | NOT IMPL | VT52 compatibility not implemented |
| 8. VT102 features | 5 tests | ✅ PASS | ICH, DCH, IL, DL, ECH |
| 9. Known bugs | 2 tests | ✅ PASS | Wrap column, tab stops |
| 10. Reset | 2 tests | ✅ PASS | RIS and DECSTR both work |
| 11. Non-VT100 | 5 tests | ✅ PASS | Cursor style, colors, save/restore, alt screen |
| Extra | 9 tests | ✅ PASS | REP, SU/SD, HPR/VPR, CBT, CHT, TBC, C1 codes |

**Update (iteration 108):** DECSTR (CSI ! p) soft reset now implemented.

**Remaining Gaps:**
- DECDHL/DECDWL rendering (double-height/width scaling) not implemented
- VT52 compatibility mode not implemented

**Files changed:**
- `crates/dterm-core/src/tests/vttest_conformance.rs` - New test module (~850 lines)
- `crates/dterm-core/src/tests/mod.rs` - Added vttest_conformance module

---

## FORMAL VERIFICATION GAPS (FV-1 through FV-20)

### FV-1: KANI CI ALLOWS FAILURE
**Status:** ✅ FIXED (iteration 79)
**Location:** `.github/workflows/verify.yml`

Removed `continue-on-error: true` so proofs MUST pass.

---

### FV-2: NO KANI PROOFS FOR TERMINAL STATE MACHINE
**Status:** ✅ FIXED (iteration 81)
**File:** `crates/dterm-core/src/terminal/mod.rs`

Added 8 Kani proofs for Terminal struct covering:
- `terminal_new_valid` - Constructor creates valid state
- `terminal_resize_cursor_bounds` - Cursor stays in bounds after resize
- `terminal_mode_toggle_consistent` - Mode set/reset is consistent
- `terminal_sgr_reset_to_default` - SGR 0 resets to default style
- `terminal_cursor_position_in_bounds` - Cursor always within grid bounds
- `terminal_scroll_region_valid` - Scroll region invariants maintained
- `terminal_palette_color_safe` - All palette indices (0-255) are safe
- `terminal_full_reset_valid` - RIS returns to known good state

---

### FV-3: FFI BOUNDARY SAFETY
**Status:** ✅ FIXED (iteration 122)
**File:** `crates/dterm-core/src/ffi/mod.rs` (90 unsafe extern functions)

**Implementation:**
- Added 12 Kani proofs for FFI boundary safety:
  - `terminal_lifecycle_safe` - Terminal new/free lifecycle correctness
  - `parser_lifecycle_safe` - Parser new/free lifecycle correctness
  - `grid_lifecycle_safe` - Grid new/free lifecycle correctness
  - `terminal_null_checks_safe` - All terminal null checks return safe defaults
  - `grid_null_checks_safe` - All grid null checks return safe defaults
  - `parser_null_checks_safe` - All parser null checks are no-ops
  - `terminal_cursor_bounds_safe` - Cursor always within terminal bounds
  - `grid_set_cursor_bounds_safe` - Cursor clamped to grid bounds after set
  - `dterm_cell_repr_c_safe` - DtermCell FFI struct has stable layout
  - `dterm_action_repr_c_safe` - DtermAction FFI struct has correct bounds
  - `grid_get_cell_null_out_safe` - get_cell handles null out_cell safely
  - `search_lifecycle_safe` - Search new/free lifecycle correctness
  - `search_null_checks_safe` - Search null checks return safe defaults
- 26 null-safety unit tests covering all FFI functions
- AddressSanitizer in CI catches runtime memory issues (FV-16)

**Coverage:**
- Box allocation/deallocation lifecycle verified
- All null pointer checks proven to return safe defaults
- Cursor bounds invariants proven
- #[repr(C)] struct layouts verified

---

### FV-4: PAGE.RS KANI PROOF ON DEAD CODE
**Status:** ✅ FIXED (iteration 121)
**File:** `crates/dterm-core/src/grid/page.rs`

**Finding:** The module was previously marked as dead code but is now fully integrated.

**Resolution:**
- page.rs is actively used by `Grid` and `Row` structures
- `PageStore` manages page allocation for all grid rows
- `PageSlice<Cell>` is used for row cell storage
- The module exports `PageStore`, `PoolStats`, and `PAGE_SIZE` in public API
- 11 Kani proofs verify memory safety:
  - `offset_within_bounds` - Offset arithmetic stays within page
  - `offset_resolve_safe` - Pointer resolution produces valid pointers
  - `page_store_allocation_within_bounds` - Slice allocation fits in page
  - `page_store_allocation_safe` - Cell allocation for terminal rows safe
  - `preheat_stats_consistent` - Pool statistics accurate after preheat
  - `alloc_from_preheated_reduces_free` - Free list properly decremented
  - `reset_preserves_total_pages` - Reset moves pages to free list
  - `shrink_to_fit_releases_free_pages` - Shrink clears free list
  - `allocation_after_reset_uses_free_list` - Reuse after reset works
  - `stats_pages_in_use_bounded` - Statistics invariants hold
- Only `free_page()` method marked dead_code (reserved for future use)

---

### FV-5: TLA+ GRID MISSING SCROLL REGION
**Status:** ✅ FIXED (iteration 80)
**File:** `tla/Grid.tla`

Added `scroll_top` and `scroll_bottom` variables to the TLA+ spec with proper
type invariants and bounds checking.

---

### FV-6: TLA+ MISSING DECSTBM OPERATIONS
**Status:** ✅ FIXED (iteration 80)
**File:** `tla/Grid.tla`

Added scroll region operations:
- `SetScrollRegion(top, bottom)` - set scroll margins
- `ResetScrollRegion` - reset to full screen
- Scroll region-aware `LineFeed` and `ReverseLineFeed`
- Scroll region-aware `WriteCharWithWrap`
- Theorems for scroll region validity

---

### FV-7: NO TLA+ SPEC FOR TERMINAL MODES
**Status:** FIXED (iteration 82)
**File:** `tla/TerminalModes.tla`

Created comprehensive TLA+ specification for terminal modes:
- Models all 13 boolean/enum mode flags from TerminalModes struct
- DECSET/DECRST operations for all private modes
- SM/RM operations for ANSI modes (IRM, LNM)
- Mouse mode state machine (None/Normal/ButtonEvent/AnyEvent)
- Mouse encoding (X10/SGR)
- Cursor style (DECSCUSR 0-6)
- Save/Restore mode state with bounded stack
- FullReset (RIS) and SoftReset (DECSTR) operations
- TypeInvariant and Safety properties
- Theorems for idempotency and mutual exclusivity

---

### FV-8: NO TLA+ SPEC FOR SELECTION
**Status:** FIXED (iteration 83)
**File:** `tla/Selection.tla`

Created comprehensive TLA+ specification for selection state machine:
- Selection states: None, InProgress, Complete
- Selection types: Simple, Block, Semantic, Lines
- Anchor points with side tracking (Left/Right of character)
- Operations: Start, Update, Complete, Clear, Extend
- Scroll handling with selection rotation
- Text change detection that clears overlapping selections
- Semantic and Lines expansion operations
- TypeInvariant and Safety properties
- Theorems for state transitions, anchor handling

---

### FV-9: FUZZ SMOKE TEST ONLY 30 SECONDS
**Status:** ✅ FIXED (iteration 79)
**Location:** `.github/workflows/verify.yml`

Increased to 300 seconds.

---

### FV-10: NO DIFFERENTIAL FUZZING
**Status:** ✅ FIXED (iteration 88)
**File:** `crates/dterm-core/fuzz/fuzz_targets/parser_diff.rs`

Added differential fuzzing against the `vte` crate (industry-standard Rust VT parser).

**Implementation:**
- Created `parser_diff` fuzz target that runs both parsers on the same input
- Normalizes action types for comparison (handles API differences)
- Compares: Print, Execute, CSI, ESC, OSC, DCS actions
- Added to CI as `fuzz-diff` job running 300 seconds
- Discovery mode initially (logs differences without failing)

**Files changed:**
- `crates/dterm-core/fuzz/fuzz_targets/parser_diff.rs` - New differential fuzz target
- `crates/dterm-core/fuzz/Cargo.toml` - Added vte dependency
- `.github/workflows/verify.yml` - Added differential fuzz CI job

**Normalization for API differences:**
- vte `Params` → `Vec<u16>` (flatten subparameters)
- vte `char` final byte → `u8`
- Ignore vte's `ignore` flag and `bell_terminated` fields

---

### FV-11: PARSER TLA+ UTF-8 HANDLING
**Status:** ✅ DOCUMENTED (iteration 122)
**File:** `tla/Parser.tla`

**Resolution:** UTF-8 handling is an INTENTIONAL DESIGN CHOICE not modeled in TLA+.

**Rationale documented in Parser.tla:**
1. VT parsers operate on byte streams, not Unicode codepoints
2. UTF-8 is a layer ABOVE the escape sequence parser
3. In implementation, `advance_fast()` identifies UTF-8 lead bytes (0xC2-0xF4)
   and decodes multi-byte sequences before the Print action
4. The parser spec models the state machine; UTF-8 is data encoding
5. Kani proofs verify UTF-8 decoding safety in `parser/mod.rs`

**Design precedent:**
- This separation follows the VT100.net reference parser design
- Matches real terminal emulators (Ghostty, Alacritty, Kitty, etc.)
- UTF-8 Kani proofs: `utf8_continuation_safe`, `transition_table_lookup_safe`

---

### FV-12: NO KANI PROOF FOR RING BUFFER
**Status:** ✅ FIXED (iteration 80)
**File:** `crates/dterm-core/src/grid/mod.rs`

Added three Kani proofs for ring buffer safety:
- `ring_buffer_index_within_bounds` - proves row_index always returns valid index
- `ring_head_within_bounds_after_scroll` - proves ring_head stays valid after scrolls
- `display_offset_bounded` - proves display_offset never exceeds scrollback

---

### FV-13: DAMAGE TRACKER STATE MACHINE UNVERIFIED
**Status:** ✅ FIXED (iteration 87)
**File:** `crates/dterm-core/src/grid/damage.rs`

Added 14 Kani proofs verifying the Damage state machine:
- `damage_new_creates_partial` - Constructor creates Partial state
- `damage_mark_full_transitions_to_full` - mark_full() transitions to Full
- `damage_reset_transitions_full_to_partial` - reset() returns to Partial
- `damage_mark_row_preserves_partial` - mark_row doesn't change to Full
- `damage_mark_rows_preserves_partial` - mark_rows doesn't change to Full
- `damage_mark_cell_preserves_partial` - mark_cell doesn't change to Full
- `damage_full_operations_idempotent` - Operations on Full are no-ops
- `damage_full_all_rows_damaged` - Full reports all rows damaged
- `damage_full_row_bounds_full_width` - Full returns (0, cols) bounds
- `damage_state_machine_cycle` - Full Partial→Full→Partial cycle
- `damage_partial_unmarked_not_damaged` - Unmarked rows not damaged
- `damage_partial_marked_is_damaged` - Marked rows are damaged

The proofs verify:
- State transitions (Partial ↔ Full) are correct
- Operations preserve expected state (mark_* doesn't escalate to Full)
- Full state behavior (all rows damaged, idempotent operations)
- Partial state selectivity (only marked rows damaged)

---

### FV-14: NO PROOF RESIZE PRESERVES CONTENT
**Status:** ✅ FIXED (iteration 90)
**File:** `tla/Grid.tla`

Added cell content modeling and resize content preservation proofs:

**Cell Content Modeling:**
- Added `cells` variable: function from (row, col) to unique cell ID
- Added `nextCellId` variable: counter for generating unique cell IDs
- Updated all operations to properly handle cell state

**Content Preservation Properties:**
- `ResizePreservesContent` - Cells within new bounds keep their content
- `ResizeNewCellsEmpty` - New cells from grid expansion are initialized to 0
- `ResizeCellIdsValid` - All cell values in new grid are valid

**Operations Updated:**
- WriteChar/WriteCharWithWrap now assign unique cell IDs
- LineFeed/ReverseLineFeed properly shift cell content during scroll
- Erase operations properly clear cell content
- Resize preserves cells within intersection of old/new dimensions

The spec now proves that resize never corrupts content - cells within the new
bounds retain their original content, and only cells in truncated columns are lost
(which is standard terminal behavior).

---

### FV-15: NO KANI PROOF FOR CHECKPOINT SERIALIZATION
**Status:** ✅ FIXED (iteration 89)
**Files:** `crates/dterm-core/src/checkpoint/`

Added 15 Kani proofs verifying checkpoint serialization safety:

**Format proofs (format.rs):**
- `header_roundtrip_preserves_fields` - Header to_bytes/from_bytes roundtrip preserves all fields
- `invalid_magic_rejected` - Non-"DTCK" magic bytes are rejected
- `valid_magic_valid_version_accepted` - Valid magic + V1 version is accepted
- `unknown_version_rejected` - Unknown version numbers are rejected
- `version_conversion_bijective` - as_u32/from_u32 are inverses for known versions
- `header_flags_set_contains_consistent` - Flag set/contains operations are consistent
- `header_flags_bits_roundtrip` - from_bits/bits roundtrip preserves value

**Checkpoint proofs (mod.rs):**
- `crc32_deterministic` - CRC32 produces same output for same input
- `crc32_empty_input` - CRC32 of empty input is well-defined (0)
- `crc32_detects_single_byte_change` - CRC32 detects single-byte corruption
- `cell_serialization_packing` - Codepoint (21 bits) and flags (11 bits) pack/unpack correctly
- `cell_serialization_size` - Cell serialization is exactly 12 bytes
- `grid_header_minimum_size` - Grid deserialize rejects data < 24 bytes
- `row_header_minimum_size` - Row deserialize rejects data < 5 bytes
- `scrollback_header_minimum_size` - Scrollback deserialize rejects data < 40 bytes

The proofs verify:
- Format version compatibility (known versions accepted, unknown rejected)
- Corruption detection (invalid magic rejected, CRC32 detects changes)
- Serialization roundtrip (header fields preserved, cell data packed correctly)
- Input validation (minimum size requirements enforced)

---

### FV-16: MIRI DOESN'T RUN ON FFI
**Status:** ✅ FIXED (iteration 86)
**Limitation:** MIRI cannot verify FFI functions by nature.

Added AddressSanitizer (ASan) CI job to `.github/workflows/verify.yml`:
- Uses nightly Rust with `-Z sanitizer=address`
- Detects memory leaks (`detect_leaks=1`)
- Detects stack-use-after-return (`detect_stack_use_after_return=1`)
- Runs all dterm-core tests including FFI boundary tests
- Uses `-Z build-std` to build sanitized standard library

This complements MIRI by catching memory safety issues in FFI code that MIRI
cannot analyze (extern "C" functions, pointer arithmetic, etc.).

---

### FV-17: SCROLLBACK LINES LOST INVARIANT VIOLATED
**Status:** FIXED (iteration 83)
**File:** `tla/Scrollback.tla`

Fixed the NoLinesLost invariant that was violated by ClearScrollback and TruncateToLast:
- Added `linesRemoved` variable to track explicitly removed lines
- Updated invariant: `NoLinesLost == lineCount = linesAdded - linesRemoved`
- ClearScrollback now increments linesRemoved by lineCount before clearing
- TruncateToLast now calculates and tracks lines removed by truncation
- TypeInvariant ensures `linesRemoved <= linesAdded`
- All operations properly maintain UNCHANGED for linesRemoved when not modifying it

---

### FV-18: PARSER PARAM OVERFLOW NOT PROVEN
**Status:** ✅ FIXED (iteration 84)
**File:** `crates/dterm-core/src/parser/mod.rs`

Added 4 Kani proofs verifying parameter accumulation safety:
- `param_accumulation_saturates` - Digit accumulation uses saturating_mul/saturating_add
- `param_finalize_bounded` - Finalized params correctly clamped to u16::MAX
- `param_many_digits_safe` - 10+ digits don't cause overflow (exceeds u32::MAX)
- `param_semicolon_safe` - Semicolon handling correctly resets state

The implementation uses `saturating_mul(10).saturating_add(digit)` which prevents
overflow by saturating at u32::MAX, then clamping to u16::MAX on finalize.

---

### FV-19: HYPERLINK MEMORY SAFETY UNVERIFIED
**Status:** ✅ FIXED (iteration 85)
**Files:** `crates/dterm-core/src/grid/extra.rs`

Added 8 Kani proofs verifying hyperlink memory safety:
- `hyperlink_roundtrip` - Set/get preserves Arc identity
- `hyperlink_arc_clone_safe` - Arc reference counting is correct
- `hyperlink_has_data_consistent` - has_data tracks hyperlink presence
- `hyperlink_extras_cleanup` - Empty extras are removed from HashMap
- `hyperlink_clear_row_safe` - clear_row removes hyperlinks in row
- `hyperlink_clear_range_safe` - clear_range removes hyperlinks in range
- `hyperlink_shift_down_safe` - Hyperlinks move with rows on scroll down
- `hyperlink_shift_up_safe` - Hyperlinks move with rows on scroll up

The proofs verify that:
- `Arc<str>` ownership is correctly maintained across clone/drop cycles
- CellExtras HashMap properly removes entries when cleared
- Row shift operations correctly move or delete hyperlinks

---

### FV-20: PROPTEST DOESN'T COVER STRUCTURED SEQUENCES
**Status:** ✅ FIXED (iteration 91)
**File:** `crates/dterm-core/src/tests/proptest.rs`

Added comprehensive proptest strategies for structured VT sequences:

**Strategies Added:**
- `csi_sequence()` - Generates valid CSI sequences (ESC [ params final)
- `osc_sequence()` - Generates valid OSC sequences (ESC ] command ; data ST)
- `dcs_sequence()` - Generates valid DCS sequences (ESC P params final data ST)
- `esc_sequence()` - Generates simple escape sequences
- `sgr_sequence()` - Generates SGR (text attribute) sequences
- `mixed_terminal_input()` - Interleaves text with various sequence types

**Property Tests Added (11 new tests):**
- `csi_sequence_parsed` - CSI sequences correctly dispatched
- `osc_sequence_parsed` - OSC sequences correctly dispatched
- `dcs_sequence_parsed` - DCS hook/unhook pairs matched
- `esc_sequence_parsed` - ESC sequences don't crash
- `sgr_sequence_parsed` - SGR produces CSI with 'm' final
- `mixed_input_never_crashes` - Mixed input survives all combinations
- `structured_csi_params_bounded` - Params stay within bounds
- `text_preserved_with_sequences` - Text interleaved with sequences preserved
- `rapid_csi_sequences_stable` - Rapid sequences don't corrupt state
- `osc_hyperlink_parses` - OSC 8 hyperlink format works
- `heterogeneous_sequences` - Multiple sequence types in one stream

**Coverage Improvements:**
- Tests now exercise the parser with well-formed sequences
- Random bytes still tested via existing `parser_state_consistent`
- Structured sequences ensure semantic correctness, not just crash-freedom

---

## EXECUTION ORDER

### Phase 0: Verification Infrastructure (DONE)
1. ✅ Remove `continue-on-error` from Kani CI (FV-1)
2. ✅ Increase fuzz time to 300s (FV-9)

### Phase 1: Architecture (HIGHEST PRIORITY)
3. Integrate offset-based pages into Grid (Gap 1)
4. Add memory pooling (Gap 2)
5. Add pin system (Gap 3)

### Phase 2: TLA+ Spec Completeness
6. Add scroll region to Grid.tla (FV-5, FV-6)
7. Create TerminalModes.tla (FV-7)
8. Create Selection.tla (FV-8)
9. Fix Scrollback.tla NoLinesLost (FV-17)

### Phase 3: Kani Proof Coverage
10. Add Terminal Kani proofs (FV-2) - CRITICAL
11. Add ring buffer proof (FV-12)
12. Add checkpoint proof (FV-15)
13. Add param overflow proof (FV-18)
14. Add hyperlink proof (FV-19)

### Phase 4: Shell Integration
15. Implement OSC 133 (Gap 4)
16. Block-based output model (Gap 31)
17. ~~Session resurrection (Gap 33)~~ ✓ DONE

### Phase 5: Modern Protocols
18. Kitty keyboard protocol (Gap 7)
19. Kitty graphics protocol (Gap 9)
20. Sixel graphics (Gap 8)
21. Line reflow (Gap 10)

### Phase 6: Advanced Verification
22. Differential fuzzing against vte (FV-10)
23. Structured proptest strategies (FV-20)
24. AddressSanitizer in CI (FV-16)

### Phase 7: VT Compliance
25. XTWINOPS (Gap 13)
26. DECRQSS (Gap 14)
27. C1 controls (Gap 16)
28. VTTEST conformance (Gap 37)

### Phase 8: Performance
29. Input coalescing (Gap 24)
30. Comparative benchmarks (Gap 23)
31. RLE compression (Gap 36)

### Phase 9: Features
32. BiDi support (Gap 11)
33. Vi mode (Gap 29)
34. Triggers (Gap 6)
35-57. Remaining gaps

---

## SUCCESS CRITERIA

The codebase is complete when:

### Verification (MUST PASS)
1. All tests pass
2. Zero clippy warnings
3. Kani CI blocks on failure
4. All 55+ Kani proofs pass
5. Terminal has Kani proofs (FV-2)
6. TLA+ specs match code
7. MIRI passes on all code
8. AddressSanitizer passes
9. Differential fuzzing shows parity

### Architecture (MUST IMPLEMENT)
10. Grid uses offset-based pages (Gap 1)
11. Memory pooling implemented (Gap 2)
12. Pin system for stable refs (Gap 3)

### Features (MUST IMPLEMENT)
13. OSC 133 shell integration works (Gap 4)
14. Kitty keyboard protocol supported (Gap 7)
15. Sixel graphics decode to images (Gap 8)
16. Line reflow on resize (Gap 10)

### Validation (MUST DOCUMENT)
17. Comparative benchmarks prove throughput claims (Gap 23)
18. VTTEST basic conformance documented (Gap 37)
19. All 57 gaps addressed
