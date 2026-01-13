# dTerm: The Terminal for AI Agents

**License:** Apache 2.0
**Platforms:** macOS, Windows, Linux, iOS, iPadOS
**Philosophy:** Clean Design = Fast + Secure (proven by seL4)

---

## Table of Contents

1. [Vision](#1-vision)
2. [Architecture Overview](#2-architecture-overview)
3. [Terminal Engine](#3-terminal-engine)
4. [Memory Architecture](#4-memory-architecture)
5. [GPU Rendering](#5-gpu-rendering)
6. [Agent Layer](#6-agent-layer)
7. [Platform Abstraction](#7-platform-abstraction)
8. [Efficiency Design](#8-efficiency-design)
9. [Formal Verification](#9-formal-verification)
10. [Performance Targets](#10-performance-targets)
11. [Competitor Insights](#11-competitor-insights)
12. [Related Documents](#12-related-documents)

---

## 1. Vision

dTerm is the terminal built for the AI agent era. Same power everywhere. Platform-native experience.

**Core insight:** Alacritty proves GPU rendering achieves 10x performance over CPU terminals. But current terminals have architectural limitations dTerm can address:

| Limitation | Current State | dTerm Solution |
|------------|---------------|----------------|
| Memory | Unbounded growth | Tiered storage (hot/warm/cold) |
| Graphics API | OpenGL (deprecated) | wgpu (Metal/Vulkan/DX12/WebGPU) |
| Cell size | 24 bytes | 12 bytes (packed) |
| Search | O(n) full scan | O(1) indexed |
| UI | Generic (winit) | Native per platform |
| Verification | None | TLA+ specs, Kani proofs |
| Mobile | Desktop only | iOS/iPadOS from day one |
| Sessions | Lost on crash | Checkpoint/restore |

**Key philosophy:** Treat the terminal like a database, not a text buffer.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    PLATFORM UI (Native)                          │
│  macOS/SwiftUI • Windows/WinUI • Linux/GTK • iOS/SwiftUI        │
│  Metal        • DX12          • Vulkan     • Metal              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ C FFI (minimal, safe)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DTERM-CORE (Rust, Apache 2.0)                 │
│                    TLA+ Specified • Kani Verified                │
│                                                                  │
│  ├── terminal/    # State machine, parser, grid                 │
│  ├── agent/       # Orchestration, routing, approval            │
│  ├── efficiency/  # Delta compression, offline sync             │
│  └── platform/    # PTY, voice, notifications traits            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    PLATFORM ADAPTERS                             │
│  Unix PTY (macOS/Linux) • ConPTY (Windows) • Remote/SSH (iOS)   │
└─────────────────────────────────────────────────────────────────┘
```

### Crate Structure

```
dterm/
├── dterm-core/              # Pure Rust, no platform deps
│   ├── src/
│   │   ├── terminal/        # State machine, parser, grid
│   │   ├── agent/           # Orchestration, routing, approval
│   │   ├── efficiency/      # Delta, compression, offline
│   │   └── render/          # Abstract render commands
│   ├── tla/                 # TLA+ specifications
│   └── fuzz/                # Fuzz targets
│
├── dterm-pty/               # Platform PTY abstraction
├── dterm-voice/             # STT/TTS abstraction
├── dterm-render/            # wgpu renderer
├── dterm-platform/          # Platform-specific UI
│   ├── macos/               # SwiftUI + AppKit
│   ├── windows/             # WinUI
│   ├── linux/               # GTK
│   └── ios/                 # SwiftUI (touch-first)
└── dterm-protocol/          # Wire protocol for sync
```

---

## 3. Terminal Engine

### 3.1 Packed Cell Structure (12 bytes)

Alacritty uses 24 bytes per cell. We can do better:

```rust
/// dTerm packed cell structure
/// Target: 12 bytes vs Alacritty's 24 bytes
#[repr(C, packed)]
pub struct Cell {
    /// Unicode codepoint (21 bits) + flags (3 bits)
    /// - bit 31: bold
    /// - bit 30: italic
    /// - bit 29: underline
    codepoint_and_flags: u32,     // 4 bytes

    /// Packed color: Named(0x00_INDEX) or RGB(0x01_RRGGBB)
    fg: u32,                      // 4 bytes
    bg: u32,                      // 4 bytes
}

/// Rare attributes stored externally
/// Accessed via HashMap<CellCoord, CellExtra>
pub struct CellExtra {
    hyperlink: Option<Arc<str>>,
    underline_color: Option<u32>,
    zerowidth: SmallVec<[char; 2]>,
}
```

**Memory savings:**

| Buffer Size | Alacritty (24B) | dTerm (12B) | Savings |
|-------------|-----------------|-------------|---------|
| 80x24 (1 screen) | 46 KB | 23 KB | 50% |
| 100K lines | 192 MB | 96 MB | 50% |
| 1M lines | 1.9 GB | 960 MB | 50% |

### 3.2 Grid Storage (Ring Buffer)

Like Alacritty, scrolling is O(1) via display_offset:

```rust
pub struct Grid<T> {
    raw: Storage<T>,           // Row storage (ring buffer)
    columns: usize,            // Terminal width
    lines: usize,              // Visible lines
    display_offset: usize,     // Scroll position (THE KEY!)
    max_scroll_limit: usize,   // Maximum history
}
```

**Memory layout:**
```
┌─────────────────────────┐  <-- max_scroll_limit + lines
│     SCROLLUP REGION     │  ← Scrollback history
├─────────────────────────┤
│     VISIBLE  REGION     │  ← Currently displayed
├─────────────────────────┤  <-- display_offset
│    SCROLLDOWN REGION    │
└─────────────────────────┘
```

Scrolling = change `display_offset`. No data copying.

### 3.3 Parser (SIMD-Accelerated)

Table-driven state machine based on vt100.net reference:

```rust
pub struct Parser {
    state: State,
    params: ArrayVec<u16, 16>,      // CSI parameters (stack allocated)
    intermediates: ArrayVec<u8, 2>,
}

impl Parser {
    /// O(1) state transition via lookup table
    #[inline(always)]
    pub fn advance(&mut self, byte: u8) -> Action {
        let entry = STATE_TABLE[self.state as usize][byte as usize];
        self.state = entry.next_state();
        entry.action()
    }
}
```

**SIMD optimization for ASCII fast path:**

```rust
/// Scan for escape sequences using SIMD (AVX2/NEON)
#[cfg(target_arch = "x86_64")]
pub fn find_escape_simd(input: &[u8]) -> Option<usize> {
    use std::arch::x86_64::*;
    unsafe {
        let escape = _mm256_set1_epi8(0x1B);
        let mut i = 0;
        while i + 32 <= input.len() {
            let chunk = _mm256_loadu_si256(input[i..].as_ptr() as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(chunk, escape);
            let mask = _mm256_movemask_epi8(cmp);
            if mask != 0 {
                return Some(i + mask.trailing_zeros() as usize);
            }
            i += 32;
        }
        input[i..].iter().position(|&b| b == 0x1B).map(|p| i + p)
    }
}
```

**Advantage:** 2-4x faster parsing for ASCII-heavy output.

### 3.4 Damage Tracking

Only redraw changed cells:

```rust
pub struct LineDamageBounds {
    pub line: usize,
    pub left: usize,
    pub right: usize,
}

pub enum TermDamage<'a> {
    Full,                           // Redraw everything
    Partial(TermDamageIterator<'a>), // Redraw only damaged lines
}
```

---

## 4. Memory Architecture

### 4.1 Tiered Storage (Database-Style)

Unlike Alacritty's flat ring buffer, dTerm uses tiered storage:

```
┌─────────────────────────────────────────────────────────────┐
│  HOT TIER (RAM) - Last 1000 lines                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Uncompressed, instant access, ~200KB                │   │
│  └─────────────────────────────────────────────────────┘   │
│                         ↓ Age out                          │
│  WARM TIER (RAM, Compressed) - Last 10K lines              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ LZ4 compressed, ~50KB (10x compression typical)     │   │
│  └─────────────────────────────────────────────────────┘   │
│                         ↓ Age out                          │
│  COLD TIER (Memory-Mapped File) - Unlimited history        │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Zstd compressed pages, lazy load, ~5KB per 1K lines │   │
│  │ 100K lines = ~500KB on disk                         │   │
│  │ 10M lines = ~50MB on disk (vs 2GB in Alacritty)     │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Implementation:**

```rust
pub struct TieredScrollback {
    hot: VecDeque<Line>,           // Recent lines, uncompressed
    warm: Vec<CompressedBlock>,     // LZ4 blocks in RAM
    cold: MmapFile,                 // Zstd pages on disk

    hot_limit: usize,               // e.g., 1000 lines
    warm_limit: usize,              // e.g., 10000 lines
    memory_budget: usize,           // e.g., 100MB total
}

impl TieredScrollback {
    pub fn get_line(&self, idx: usize) -> Line {
        if idx >= self.hot_start() {
            self.hot[idx - self.hot_start()].clone()
        } else if idx >= self.warm_start() {
            self.warm.decompress_line(idx)  // LZ4 fast decompress
        } else {
            self.cold.load_line(idx)  // Lazy mmap + zstd
        }
    }
}
```

**Memory comparison:**

| Lines | Alacritty | dTerm | Improvement |
|-------|-----------|-------|-------------|
| 100K | ~20 MB | ~2 MB | **10x** |
| 1M | ~200 MB | ~20 MB | **10x** |
| 10M | ~2 GB (OOM) | ~200 MB | **∞** |

### 4.2 Memory Budget Enforcement

```rust
pub struct MemoryManager {
    budget: usize,              // e.g., 500MB
    current_usage: AtomicUsize,
}

impl MemoryManager {
    fn handle_pressure(&self, excess: usize) {
        // Priority 1: Compress warm tier
        // Priority 2: Evict cold tier to disk
        // Priority 3: Shrink glyph cache
    }
}
```

### 4.3 Session Checkpoint/Restore

Never lose a multi-day session:

```rust
pub struct SessionCheckpoint {
    /// Checkpoint every 10 seconds or 1000 lines
    pub fn maybe_checkpoint(&mut self, terminal: &Terminal);

    /// Restore from crash
    pub fn restore(&self) -> Result<Terminal, RestoreError>;
}
```

---

## 5. GPU Rendering

### 5.1 wgpu (Not OpenGL)

Alacritty uses OpenGL (deprecated on macOS). dTerm uses wgpu:

| Feature | OpenGL | wgpu |
|---------|--------|------|
| macOS | Deprecated (4.1) | Metal (native) |
| Windows | OK | DX12 (native) |
| Linux | OK | Vulkan (native) |
| Web | WebGL (limited) | WebGPU (modern) |
| iOS/iPadOS | No | Metal (native) |

**Renderer structure:**

```rust
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    glyph_atlas: GlyphAtlas,
    cell_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CellInstance {
    position: [f32; 2],
    uv_offset: [f32; 2],
    uv_size: [f32; 2],
    fg_color: [f32; 4],
    bg_color: [f32; 4],
}
```

### 5.2 Texture Atlas

Like Alacritty, glyphs rasterized once and stored in atlas:

```rust
pub struct GlyphAtlas {
    texture: wgpu::Texture,
    width: u32,              // 1024 pixels
    height: u32,             // 1024 pixels
    row_extent: u32,         // Current X position
    row_baseline: u32,       // Current Y position
}
```

Row-based packing:
```
┌─────┬─────┬─────┬─────┬─────┐
│ A   │ B   │ C   │ D   │ E   │  Row 0
├─────┼─────┼─────┼─────┼─────┤
│ F   │ G   │ H   │ I   │     │  Row 1
├─────┼─────┼─────┴─────┴─────┤
│ J   │ K   │ <- current pos  │  Row 2
└─────┴─────┴─────────────────┘
```

---

## 6. Agent Layer

### 6.1 Interaction Model

```
Human → [natural language] → Agent → [generates commands] → Shell
              ↑                              ↓
              └──── [approval if needed] ────┘
```

### 6.2 Approval Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  Agent wants to: rm -rf node_modules/                       │
│                                                              │
│  [Approve]  [Reject]  [Edit]  [Ask Why]                    │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Multi-Server Routing

```
┌─────────────────────────────────────────────────────────────┐
│  Sessions                                                    │
├─────────────────────────────────────────────────────────────┤
│  🟢 Mac Mini (home)      Agent fixing tests                 │
│  🟢 EC2 prod             Monitoring                         │
│  🟡 Work laptop          Idle                               │
│  📱 Local (iPhone)       Ready                              │
└─────────────────────────────────────────────────────────────┘
```

Agent routes execution to appropriate server based on context.

---

## 7. Platform Abstraction

### 7.1 Native UI Per Platform

Each platform uses native UI frameworks:

| Platform | UI Framework | GPU API |
|----------|--------------|---------|
| macOS | SwiftUI + AppKit | Metal |
| Windows | WinUI 3 | DX12 |
| Linux | GTK4 + libadwaita | Vulkan |
| iOS/iPadOS | SwiftUI | Metal |

### 7.2 Input Modalities

**Keyboard (Desktop):** Full terminal power - Ctrl+C, tab completion, etc.

**Touch (Mobile):**
| Gesture | Action |
|---------|--------|
| Circle region | "Explain this" |
| Swipe left/right | Switch sessions |
| Pinch | Zoom text |
| Long press | Context menu |

**Voice (All):** "Run the tests", "What's the error?", "Stop" → Ctrl+C

### 7.3 Platform Traits

```rust
trait PlatformCapabilities {
    // Core (all platforms)
    fn execute_shell(&self, cmd: &str) -> Result<Output>;
    fn speak(&self, text: &str) -> Result<()>;
    fn listen(&self) -> Result<String>;
    fn notify(&self, msg: &str) -> Result<()>;

    // Desktop
    fn local_docker(&self) -> Option<&dyn Docker>;

    // Mobile
    fn camera(&self) -> Option<&dyn Camera>;
    fn location(&self) -> Option<&dyn Location>;
}
```

---

## 8. Efficiency Design

### 8.1 Power States

```rust
enum PowerState {
    Active,     // Full rendering, live connection
    Background, // No rendering, maintain connection
    Suspended,  // Push notifications only
    Offline,    // Cache only, queue operations
}
```

### 8.2 Delta Protocol

Instead of raw SSH byte stream:

```json
{
  "type": "delta",
  "changes": [
    {"line": 42, "range": [10, 50], "text": "...", "compressed": true}
  ],
  "cursor": [42, 15]
}
```

90% less data for typical use.

### 8.3 Indexed Search (O(1))

For 100K+ line sessions:

```rust
pub struct SearchIndex {
    trigrams: HashMap<[u8; 3], RoaringBitmap>,
    bloom: BloomFilter,
}

impl SearchIndex {
    pub fn search(&self, query: &str) -> impl Iterator<Item = usize> {
        // Check bloom filter first (instant negative)
        if !self.bloom.might_contain(query) {
            return Box::new(std::iter::empty());
        }
        // Use trigram index for candidate lines
        // ...
    }
}
```

**Advantage:** O(1) average vs O(n) full scan. 5ms vs 500ms for 1M lines.

### 8.4 Streaming Parser with Backpressure

```rust
pub struct StreamingParser {
    coalesce_threshold: Duration,    // e.g., 8ms (120Hz)
    drop_threshold: usize,           // e.g., 1MB pending
}

impl StreamingParser {
    pub fn process(&mut self, input: &[u8]) -> ParseResult {
        // Always update terminal state (never lose data)
        // But intelligently decide when to render
    }
}
```

**Advantage:** During `cat huge_file.txt`, dTerm stays responsive.

---

## 9. Formal Verification

### 9.1 TLA+ Specification

```tla
--------------------------- MODULE Grid ---------------------------
EXTENDS Integers, Sequences

VARIABLES rows, cursor, display_offset, history_size

TypeInvariant ==
    /\ cursor.line >= 0 /\ cursor.line < MaxRows
    /\ display_offset <= history_size

CursorVisible ==
    display_offset = 0 => cursor.line < MaxRows
===================================================================
```

### 9.2 Kani Proofs

```rust
#[cfg(kani)]
mod verification {
    #[kani::proof]
    fn cell_pack_unpack_roundtrip() {
        let codepoint: u32 = kani::any();
        kani::assume(codepoint <= 0x10FFFF);

        let cell = Cell::new(codepoint, false, false, Color::default(), Color::default());
        assert_eq!(cell.codepoint(), codepoint);
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn grid_scroll_bounds() {
        let mut grid: Grid<Cell> = Grid::new(24, 80, 10000);
        let scroll_amount: i32 = kani::any();
        kani::assume(scroll_amount.abs() < 100);

        grid.scroll_display(Scroll::Delta(scroll_amount));
        assert!(grid.display_offset() <= grid.history_size());
    }
}
```

### 9.3 Continuous Fuzzing

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut term = Term::new(80, 24, 1000);
    let mut parser = Parser::new();

    for &byte in data {
        if let Some(action) = parser.advance(byte) {
            term.perform(action);
        }
    }

    // Invariants that must hold after any input
    assert!(term.cursor_line() < term.rows());
    assert!(term.cursor_column() < term.columns());
});
```

### 9.4 Verification Matrix

| Component | Method | Tool | When |
|-----------|--------|------|------|
| State machine | Specification | TLA+ | Design time |
| Parser | Bounded model checking | Kani | Before merge |
| Unsafe code | UB detection | MIRI | Every commit |
| Parser | Fuzzing | cargo-fuzz | 24/7 continuous |

---

## 10. Performance Targets

### 10.1 Core Metrics

| Metric | Current Best | dTerm Target | Improvement |
|--------|--------------|--------------|-------------|
| Memory (100K lines) | ~20 MB | ~2 MB | **10x** |
| Memory (1M lines) | ~200 MB | ~20 MB | **10x** |
| Memory (10M lines) | ~2 GB (OOM) | ~200 MB | **∞** |
| Search 1M lines | ~500ms | ~5ms | **100x** |
| Parse throughput | ~400 MB/s | ~800 MB/s | **2x** |
| Input latency | ~5ms | <5ms | Match best |
| 24hr session | Memory grows | Memory capped | **Stability** |
| Crash recovery | Lost | Restored | **∞** |

### 10.2 Mobile Targets

| Metric | Target |
|--------|--------|
| Idle battery drain | <1% per hour |
| Background battery | <0.1% per hour |
| Data per hour (active) | <1 MB |
| Time to connect | <500 ms |

### 10.3 Correctness Targets

| Metric | Target |
|--------|--------|
| Crash rate | **0** per month |
| Parser vulnerabilities | **0** |
| Memory leaks | **0** |

---

## 11. Competitor Insights

### 11.1 What to Adopt

| Source | Pattern | Applicable |
|--------|---------|------------|
| Alacritty | Ring buffer, damage tracking, library separation | Yes |
| Ghostty | Page-based memory, SIMD parsing, native UI | Yes |
| WezTerm | wgpu renderer, sequence numbers | Yes |
| Windows Terminal | RLE attributes, ConPTY reference | Yes |
| Kitty | Graphics protocol, keyboard protocol | Yes |

### 11.2 What to Avoid

| Pattern | Source | Why |
|---------|--------|-----|
| GPL license | Kitty | Limits embedding |
| OpenGL | Alacritty | Deprecated on macOS |
| Monolithic files | All | Hard to navigate |
| No verification | All | dTerm adds TLA+/Kani |

### 11.3 Detailed Analyses

See `research/` directory:
- `alacritty/ANALYSIS.md` - Ring buffer, VTE parser
- `ghostty/ANALYSIS.md` - Page-based memory, SIMD
- `wezterm/ANALYSIS.md` - wgpu, multiplexer
- `kitty/ANALYSIS.md` - Graphics protocol, threading
- `rio/ANALYSIS.md` - wgpu, damage tracking
- `contour/ANALYSIS.md` - VT compliance
- `windows-terminal/ANALYSIS.md` - ConPTY, RLE
- `terminal-app/README.md` - macOS Terminal.app (reverse engineered)

---

## 12. Related Documents

### Source Documents (Consolidated Here)

These documents contain additional detail and are referenced by this design:

| Document | Content |
|----------|---------|
| `ARCHITECTURE.md` | Original vision, agent features, platform capabilities |
| `PERFORMANCE_ADVANTAGES.md` | Detailed tiered memory, SIMD, compression algorithms |
| `research/DTERM_DESIGN.md` | Alacritty source code analysis, packed cell design |

### Competitor Research

| Document | Terminal |
|----------|----------|
| `research/alacritty/ANALYSIS.md` | Ring buffer, VTE parser, damage tracking |
| `research/ghostty/ANALYSIS.md` | Page-based memory, SIMD, native UI |
| `research/wezterm/ANALYSIS.md` | wgpu, sequence numbers, multiplexer |
| `research/kitty/ANALYSIS.md` | Graphics protocol, keyboard protocol |
| `research/rio/ANALYSIS.md` | wgpu, damage tracking |
| `research/contour/ANALYSIS.md` | VT compliance, bulk text |
| `research/windows-terminal/ANALYSIS.md` | ConPTY, RLE attributes |
| `research/terminal-app/README.md` | macOS Terminal.app (reverse engineered) |
| `research/RESEARCH_INDEX.md` | Index of all competitor research |

### Implementation

See `ROADMAP.md` for implementation phases and timeline.

---

## License

Apache 2.0 - Permissive, patent protection, iOS App Store compatible.
