# dTerm: Building a Dramatically Better Terminal

> **Note:** This document has been consolidated into [`docs/architecture/DESIGN.md`](../docs/architecture/DESIGN.md).
> This file contains the detailed Alacritty source analysis. See DESIGN.md for the authoritative design.

## Executive Summary

After reverse engineering Terminal.app and studying Alacritty's source code in depth, we've identified the architectural decisions that make terminals fast (or slow) and the opportunities to exceed the current state of the art.

**The key insight:** Alacritty proves GPU rendering can achieve 10x performance over CPU terminals. But Alacritty made compromises that dTerm can avoid:

| Limitation | Alacritty | dTerm Opportunity |
|------------|-----------|-------------------|
| Graphics API | OpenGL (deprecated on macOS) | wgpu (Metal/Vulkan/DX12/WebGPU) |
| UI | winit (generic, non-native) | Native per platform (SwiftUI, WinUI, GTK) |
| Cell size | 24 bytes | 10-16 bytes (packed) |
| Verification | None | TLA+ specs, Kani proofs |
| Mobile | Desktop only | iOS/iPadOS from day one |
| Parser | Runtime state machine | Compile-time tables + fuzzing |

---

## Part 1: Alacritty Deep Dive

### 1.1 Cell Structure (The Core Data Type)

Every character on screen is a `Cell`. Alacritty uses 24 bytes:

```rust
// alacritty_terminal/src/term/cell.rs:134-140
pub struct Cell {
    pub c: char,                       // 4 bytes - the character
    pub fg: Color,                     // 4 bytes - foreground color
    pub bg: Color,                     // 4 bytes - background color
    pub flags: Flags,                  // 2 bytes - bold, italic, etc.
    pub extra: Option<Arc<CellExtra>>, // 8 bytes - hyperlinks, underline color
    // Padding: 2 bytes
}
// Total: 24 bytes (verified by test at line 301-306)
```

**Smart design decisions:**
1. `Option<Arc<CellExtra>>` keeps rare attributes out of the hot path
2. `Color` is an enum with Named(u8) variant for 256-color mode efficiency
3. `Flags` uses bitflags for O(1) attribute checks

**The Flags bitfield:**
```rust
// alacritty_terminal/src/term/cell.rs:15-36
pub struct Flags: u16 {
    const INVERSE                   = 0b0000_0000_0000_0001;
    const BOLD                      = 0b0000_0000_0000_0010;
    const ITALIC                    = 0b0000_0000_0000_0100;
    const UNDERLINE                 = 0b0000_0000_0000_1000;
    const WRAPLINE                  = 0b0000_0001_0000;      // Line continues
    const WIDE_CHAR                 = 0b0000_0010_0000;      // CJK wide char
    const WIDE_CHAR_SPACER          = 0b0000_0100_0000;      // Placeholder
    const DIM                       = 0b0000_1000_0000;
    const HIDDEN                    = 0b0001_0000_0000;
    const STRIKEOUT                 = 0b0010_0000_0000;
    const DOUBLE_UNDERLINE          = 0b1000_0000_0000;
    const UNDERCURL                 = 0b0001_0000_0000_0000;
    // ... more underline styles
}
```

### 1.2 Grid Storage (Ring Buffer)

The grid is a ring buffer - scrolling just changes an offset:

```rust
// alacritty_terminal/src/grid/mod.rs:108-138
pub struct Grid<T> {
    pub cursor: Cursor<T>,           // Current cursor position + template
    pub saved_cursor: Cursor<T>,     // DECSC saved cursor
    raw: Storage<T>,                 // The actual rows
    columns: usize,                  // Terminal width
    lines: usize,                    // Visible lines
    display_offset: usize,           // Scroll position (THE KEY!)
    max_scroll_limit: usize,         // Maximum history
}
```

**Memory layout (from the source comments):**
```
┌─────────────────────────┐  <-- max_scroll_limit + lines
│      UNINITIALIZED      │
├─────────────────────────┤  <-- raw.inner.len()
│      RESIZE BUFFER      │
├─────────────────────────┤  <-- history_size() + lines
│     SCROLLUP REGION     │  ← Scrollback history
├─────────────────────────┤v lines
│     VISIBLE  REGION     │  ← Currently displayed
├─────────────────────────┤^ <-- display_offset
│    SCROLLDOWN REGION    │
└─────────────────────────┘  <-- zero
```

**Why this matters:**
- Scrolling = change `display_offset` (O(1), no copying)
- New lines = rotate buffer (O(1) amortized)
- Terminal.app uses NSAttributedString with O(n) scrolling

### 1.3 Texture Atlas (GPU Glyph Cache)

Alacritty rasterizes each glyph once, stores in a texture atlas:

```rust
// alacritty/src/renderer/text/atlas.rs:33-61
pub struct Atlas {
    id: GLuint,           // OpenGL texture ID
    width: i32,           // 1024 pixels
    height: i32,          // 1024 pixels
    row_extent: i32,      // Current X position
    row_baseline: i32,    // Current Y position
    row_tallest: i32,     // Tallest glyph in row
    is_gles_context: bool,
}

pub const ATLAS_SIZE: i32 = 1024;
```

**Packing algorithm (row-based):**
```
┌─────┬─────┬─────┬─────┬─────┐
│ A   │ B   │ C   │ D   │ E   │  Row 0
├─────┼─────┼─────┼─────┼─────┤
│ F   │ G   │ H   │ I   │     │  Row 1
├─────┼─────┼─────┴─────┴─────┤
│ J   │ K   │ <- current pos  │  Row 2
└─────┴─────┴─────────────────┘
```

**Key operations:**
```rust
// Insert glyph, create new atlas if full
pub fn load_glyph(
    active_tex: &mut GLuint,
    atlas: &mut Vec<Atlas>,
    current_atlas: &mut usize,
    rasterized: &RasterizedGlyph,
) -> Glyph {
    match atlas[*current_atlas].insert(rasterized, active_tex) {
        Ok(glyph) => glyph,
        Err(AtlasInsertError::Full) => {
            *current_atlas += 1;
            if *current_atlas == atlas.len() {
                atlas.push(Atlas::new(ATLAS_SIZE, is_gles_context));
            }
            Atlas::load_glyph(active_tex, atlas, current_atlas, rasterized)
        },
        // ...
    }
}
```

### 1.4 Damage Tracking

Alacritty only redraws changed cells:

```rust
// alacritty_terminal/src/term/mod.rs:136-174
pub struct LineDamageBounds {
    pub line: usize,      // Which line
    pub left: usize,      // Leftmost damaged column
    pub right: usize,     // Rightmost damaged column
}

impl LineDamageBounds {
    #[inline]
    pub fn expand(&mut self, left: usize, right: usize) {
        self.left = cmp::min(self.left, left);
        self.right = cmp::max(self.right, right);
    }

    #[inline]
    pub fn is_damaged(&self) -> bool {
        self.left <= self.right
    }
}
```

**Damage enum allows full or partial redraw:**
```rust
pub enum TermDamage<'a> {
    Full,                                    // Redraw everything
    Partial(TermDamageIterator<'a>),        // Redraw only damaged lines
}
```

### 1.5 VTE Parser

Alacritty uses the `vte` crate (which Alacritty authors also wrote):

```toml
# alacritty_terminal/Cargo.toml:27
vte = { version = "0.15.0", default-features = false, features = ["std", "ansi"] }
```

The parser is a table-driven state machine based on Paul Williams' research.

**Perform trait (ANSI handler interface):**
```rust
pub trait Perform {
    fn print(&mut self, c: char);                    // Regular character
    fn execute(&mut self, byte: u8);                 // Control char (^C, etc)
    fn csi_dispatch(&mut self, params: &Params, ...); // CSI sequence
    fn esc_dispatch(&mut self, ...);                 // Escape sequence
    fn osc_dispatch(&mut self, params: &[&[u8]], ...); // Operating System Command
}
```

### 1.6 Terminal Modes

```rust
// alacritty_terminal/src/term/mod.rs:53-88
bitflags! {
    pub struct TermMode: u32 {
        const SHOW_CURSOR             = 1;
        const APP_CURSOR              = 1 << 1;   // Application cursor keys
        const APP_KEYPAD              = 1 << 2;   // Application keypad
        const MOUSE_REPORT_CLICK      = 1 << 3;
        const BRACKETED_PASTE         = 1 << 4;   // Security feature
        const SGR_MOUSE               = 1 << 5;
        const MOUSE_MOTION            = 1 << 6;
        const LINE_WRAP               = 1 << 7;   // Auto-wrap
        const ORIGIN                  = 1 << 9;   // Origin mode
        const INSERT                  = 1 << 10;  // Insert mode
        const ALT_SCREEN              = 1 << 12;  // Alternate screen buffer
        const VI                      = 1 << 16;  // Vi mode (Alacritty feature)
        const KITTY_KEYBOARD_PROTOCOL = ...;      // Modern keyboard handling
    }
}
```

---

## Part 2: dTerm Architecture

### 2.1 Packed Cell (10-16 bytes)

We can do better than 24 bytes:

```rust
/// dTerm packed cell structure
/// Target: 10 bytes for common case, 16 with extended attributes
#[repr(C, packed)]
pub struct Cell {
    /// Unicode codepoint (most chars fit in 21 bits)
    /// Top 3 bits: flags for common attributes
    /// - bit 31: bold
    /// - bit 30: italic
    /// - bit 29: underline
    codepoint_and_flags: u32,     // 4 bytes

    /// Packed colors:
    /// - Named color: 0x00_INDEX (index 0-255)
    /// - RGB color: 0x01_RRGGBB (24-bit RGB)
    fg: u32,                      // 4 bytes
    bg: u32,                      // 4 bytes

    // Total: 12 bytes
    // Extra attributes via external HashMap<CellCoord, CellExtra>
}

/// For rare attributes (hyperlinks, underline color, zerowidth chars)
pub struct CellExtra {
    hyperlink: Option<Arc<str>>,
    underline_color: Option<u32>,
    zerowidth: SmallVec<[char; 2]>,
}
```

**Memory savings at scale:**
| Buffer Size | Alacritty (24B) | dTerm (12B) | Savings |
|-------------|-----------------|-------------|---------|
| 80x24 (1 screen) | 46 KB | 23 KB | 50% |
| 100K lines | 192 MB | 96 MB | 50% |
| 1M lines | 1.9 GB | 960 MB | 50% |

### 2.2 wgpu Renderer (Not OpenGL)

Alacritty uses OpenGL which is deprecated on macOS. We use wgpu:

```rust
use wgpu;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,

    glyph_atlas: GlyphAtlas,
    cell_pipeline: wgpu::RenderPipeline,

    // Instance buffer for batched rendering
    instance_buffer: wgpu::Buffer,
    instance_data: Vec<CellInstance>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CellInstance {
    position: [f32; 2],      // Screen position
    uv_offset: [f32; 2],     // Glyph atlas UV
    uv_size: [f32; 2],       // Glyph size in atlas
    fg_color: [f32; 4],      // RGBA foreground
    bg_color: [f32; 4],      // RGBA background
}
```

**Why wgpu:**
| Feature | OpenGL | wgpu |
|---------|--------|------|
| macOS support | Deprecated (4.1) | Metal (native) |
| Windows | OK | DX12 (native) |
| Linux | OK | Vulkan (native) |
| Web | WebGL (limited) | WebGPU (modern) |
| iOS/iPadOS | No | Metal (native) |
| Android | ES 3.0 | Vulkan |

### 2.3 Platform-Native UI

Alacritty uses winit for windowing, which feels non-native everywhere.

**dTerm approach: Native shell, shared core**

```
┌───────────────────────────────────────────────────────────┐
│                    NATIVE UI SHELL                         │
│  macOS: SwiftUI + AppKit                                  │
│  Windows: WinUI 3 + WinRT                                 │
│  Linux: GTK4 + libadwaita                                 │
│  iOS/iPadOS: SwiftUI                                      │
└───────────────────────────────────────────────────────────┘
                              │ C FFI
                              ▼
┌───────────────────────────────────────────────────────────┐
│                    DTERM-CORE (Rust)                       │
│  • Terminal state machine                                 │
│  • Grid storage (ring buffer)                             │
│  • Parser (vte-compatible)                                │
│  • Render content extraction                              │
└───────────────────────────────────────────────────────────┘
```

**Benefits:**
- Native menus, dialogs, settings
- System keyboard shortcuts
- Accessibility (VoiceOver, Narrator, Orca)
- Platform conventions (tabs, splits)
- Notarization/signing for app stores

### 2.4 Formal Verification

**TLA+ Specification for Grid:**

```tla
--------------------------- MODULE Grid ---------------------------
EXTENDS Integers, Sequences

CONSTANTS MaxRows, MaxCols, MaxHistory

VARIABLES
    rows,           \* Sequence of rows
    cursor,         \* {line, column}
    display_offset, \* Scroll position
    history_size    \* Current history

TypeInvariant ==
    /\ Len(rows) <= MaxHistory + MaxRows
    /\ cursor.line >= 0 /\ cursor.line < MaxRows
    /\ cursor.column >= 0 /\ cursor.column < MaxCols
    /\ display_offset <= history_size

ScrollUp ==
    /\ history_size < MaxHistory
    /\ history_size' = history_size + 1
    /\ display_offset' = IF display_offset > 0
                         THEN display_offset + 1
                         ELSE 0
    /\ UNCHANGED <<cursor, rows>>

\* Invariant: cursor always visible when display_offset = 0
CursorVisible ==
    display_offset = 0 => cursor.line < MaxRows
===================================================================
```

**Kani Proofs for Cell Operations:**

```rust
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn cell_pack_unpack_roundtrip() {
        let codepoint: u32 = kani::any();
        kani::assume(codepoint <= 0x10FFFF); // Valid Unicode

        let bold: bool = kani::any();
        let italic: bool = kani::any();

        let cell = Cell::new(codepoint, bold, italic, Color::default(), Color::default());

        assert_eq!(cell.codepoint(), codepoint);
        assert_eq!(cell.is_bold(), bold);
        assert_eq!(cell.is_italic(), italic);
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn grid_scroll_bounds() {
        let mut grid: Grid<Cell> = Grid::new(24, 80, 10000);

        let scroll_amount: i32 = kani::any();
        kani::assume(scroll_amount.abs() < 100);

        grid.scroll_display(Scroll::Delta(scroll_amount));

        // Invariant: display_offset never exceeds history
        assert!(grid.display_offset() <= grid.history_size());
    }
}
```

### 2.5 Parser with Fuzzing

```rust
/// Parser state machine with compile-time tables
pub struct Parser {
    state: State,
    params: ArrayVec<u16, 16>,  // CSI parameters (stack allocated)
    intermediates: ArrayVec<u8, 2>,
    osc_raw: Vec<u8>,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum State {
    Ground = 0,
    Escape = 1,
    EscapeIntermediate = 2,
    CsiEntry = 3,
    CsiParam = 4,
    CsiIntermediate = 5,
    CsiIgnore = 6,
    DcsEntry = 7,
    // ... more states
}

impl Parser {
    /// O(1) state transition via lookup table
    #[inline(always)]
    pub fn advance(&mut self, byte: u8) -> Action {
        // Table generated at compile time
        let entry = STATE_TABLE[self.state as usize][byte as usize];
        self.state = entry.next_state();
        entry.action()
    }
}
```

**Continuous fuzzing:**
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use dterm_core::parser::Parser;
use dterm_core::term::Term;

fuzz_target!(|data: &[u8]| {
    let mut term = Term::new(80, 24, 1000);
    let mut parser = Parser::new();

    for &byte in data {
        if let Some(action) = parser.advance(byte) {
            // This must never panic or corrupt state
            term.perform(action);
        }
    }

    // Invariants that must hold after any input
    assert!(term.cursor_line() < term.rows());
    assert!(term.cursor_column() < term.columns());
});
```

---

## Part 3: Implementation Roadmap

### Phase 1: Core Library (dterm-core)

**Week 1-2: Data Structures**
- [ ] Packed Cell implementation with tests
- [ ] Ring buffer Grid with Kani proofs
- [ ] Basic row operations (insert, delete, scroll)

**Week 3-4: Parser**
- [ ] VTE-compatible state machine
- [ ] Table generation (build.rs)
- [ ] Fuzzing harness
- [ ] ANSI handler trait

**Week 5-6: Terminal State**
- [ ] Term struct with modes
- [ ] Cursor management
- [ ] Scroll regions
- [ ] Alternate screen buffer
- [ ] Damage tracking

### Phase 2: Rendering

**Week 7-8: wgpu Renderer**
- [ ] Basic wgpu setup (device, surface)
- [ ] Glyph rasterization (fontdue or ab_glyph)
- [ ] Texture atlas implementation
- [ ] Cell instance batching

**Week 9-10: Platform Integration**
- [ ] C FFI exports
- [ ] macOS SwiftUI wrapper
- [ ] Basic keyboard/mouse input

### Phase 3: Production Features

**Week 11-12: Essential Features**
- [ ] PTY integration (Unix, ConPTY)
- [ ] Scrollback with search
- [ ] Copy/paste
- [ ] Font loading

**Week 13-14: Polish**
- [ ] Cursor styles
- [ ] Bell (visual/audio)
- [ ] Window title/tab title
- [ ] Performance optimization

### Phase 4: Mobile & Advanced

**Week 15-16: iOS/iPadOS**
- [ ] SwiftUI app shell
- [ ] Touch keyboard integration
- [ ] SSH client integration

---

## Part 4: Comparison Matrix

| Feature | Terminal.app | Alacritty | dTerm Target |
|---------|--------------|-----------|--------------|
| **Performance** ||||
| Rendering | CPU | GPU (OpenGL) | GPU (wgpu) |
| Input latency | 10-20ms | 2-5ms | <5ms |
| Throughput | 50 MB/s | 500 MB/s | 500+ MB/s |
| **Memory** ||||
| Cell size | 32+ bytes | 24 bytes | 12 bytes |
| 1M lines | ~200 MB | ~100 MB | ~50 MB |
| **Platform** ||||
| macOS | Native | winit | Native (SwiftUI) |
| Windows | No | winit | Native (WinUI) |
| Linux | No | winit | Native (GTK4) |
| iOS/iPadOS | No | No | Native (SwiftUI) |
| Web | No | No | WebGPU |
| **Correctness** ||||
| Formal spec | No | No | TLA+ |
| Proof coverage | No | No | Kani |
| Fuzzing | Unknown | Unknown | Continuous |
| **Features** ||||
| Tabs/Splits | Yes | No | Yes |
| Touch Bar | Yes | No | Yes |
| Accessibility | Yes | Limited | Full |
| Agent native | No | No | Yes |

---

## Part 5: Key Files Reference

### Alacritty Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `alacritty_terminal/src/term/cell.rs` | 325 | Cell definition, flags |
| `alacritty_terminal/src/grid/mod.rs` | 657 | Ring buffer grid |
| `alacritty_terminal/src/term/mod.rs` | 1800+ | Terminal state machine |
| `alacritty/src/renderer/text/atlas.rs` | 305 | Texture atlas |

### dTerm Implementation Files (planned)

| File | Purpose |
|------|---------|
| `dterm-core/src/cell.rs` | Packed cell (12 bytes) |
| `dterm-core/src/grid.rs` | Ring buffer with Kani proofs |
| `dterm-core/src/parser.rs` | Table-driven parser |
| `dterm-core/src/term.rs` | Terminal state machine |
| `dterm-core/src/ffi.rs` | C FFI for native apps |
| `dterm-render/src/atlas.rs` | wgpu texture atlas |
| `dterm-render/src/pipeline.rs` | wgpu render pipeline |
| `dterm-macos/Sources/` | SwiftUI app |
| `specs/grid.tla` | TLA+ specification |

---

## Summary

dTerm can be dramatically better than existing terminals by:

1. **Smaller cells** - 12 bytes vs 24 bytes = 50% memory savings
2. **Modern GPU** - wgpu supports Metal/Vulkan/DX12/WebGPU
3. **Native UI** - SwiftUI/WinUI/GTK4 feels right on each platform
4. **Verified** - TLA+ specs and Kani proofs prevent entire bug classes
5. **Fuzzed** - Continuous fuzzing ensures parser handles any input
6. **Mobile** - iOS/iPadOS support from day one

The architectural patterns from Alacritty (ring buffer, texture atlas, damage tracking) are sound. We improve on the implementation with better memory layout, modern graphics APIs, and formal verification.
