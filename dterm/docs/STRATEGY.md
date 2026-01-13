# dterm Development Strategy

**Date:** 2024-12-28
**Status:** Active

---

## The Plan

### Phase 1: dterm-core → dashterm2

Build `dterm-core` as a Rust library that replaces iTerm2's slow internals. Swap it into dashterm2 (our iTerm2 fork) via C FFI.

**Why this approach:**
- iTerm2 has 20+ years of features, UI polish, and macOS integration
- iTerm2's bottleneck is the core engine (~60 MB/s parse, 16-byte cells, O(n) search)
- We replace the slow parts, keep the good parts
- Proves dterm-core in production before building standalone app

### Phase 2: dterm (Standalone)

Build complete cross-platform terminal using dterm-core:
- macOS: SwiftUI + Metal
- Windows: WinUI + DX12
- Linux: GTK + Vulkan
- iOS/iPadOS: SwiftUI + Metal

**Why build standalone later:**
- dterm-core battle-tested in dashterm2 first
- Time to design agent-native UX properly
- Cross-platform from day 1 (not afterthought)

---

## What dterm-core Replaces in iTerm2

| iTerm2 Component | Problem | dterm-core Replacement |
|------------------|---------|------------------------|
| `VT100Terminal` | ~60 MB/s, Obj-C overhead | Rust parser, ~400 MB/s |
| `screen_char_t` | 16 bytes per cell | 8 bytes per cell |
| `LineBuffer` | Unbounded RAM growth | Tiered storage (hot/warm/cold) |
| Search | O(n) linear scan | O(1) trigram index |
| (none) | No crash recovery | Checkpoint/restore |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 dashterm2 (Objective-C/Swift)               │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐ │
│  │  UI: Tabs, Splits, Preferences, Touch Bar             │ │
│  │  Features: Triggers, Profiles, Tmux, Shell Integration│ │
│  │  Renderer: Metal + CoreText                           │ │
│  └───────────────────────────────────────────────────────┘ │
│                            │                                │
│                            │ C FFI                          │
│                            ▼                                │
│  ┌───────────────────────────────────────────────────────┐ │
│  │                   dterm-core (Rust)                    │ │
│  │                                                        │ │
│  │   Parser ──► Grid ──► Scrollback ──► Search           │ │
│  │      │                    │                            │ │
│  │      └────────────────────┴──► Checkpoints            │ │
│  │                                                        │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## dterm-core Components

### 1. Parser

Table-driven state machine based on vt100.net DEC ANSI parser.

```rust
pub struct Parser {
    state: State,
    params: ArrayVec<[u16; 16]>,
    intermediates: ArrayVec<[u8; 4]>,
}

impl Parser {
    /// Feed bytes, emit actions
    pub fn advance(&mut self, input: &[u8], sink: &mut impl ActionSink);
}

pub enum Action {
    Print(char),
    Execute(u8),
    CsiDispatch { params: &[u16], intermediates: &[u8], final_byte: u8 },
    OscDispatch { params: &[&[u8]] },
    EscDispatch { intermediates: &[u8], final_byte: u8 },
    DcsHook { params: &[u16], intermediates: &[u8], final_byte: u8 },
    DcsPut(u8),
    DcsUnhook,
}
```

**Performance target:** 400+ MB/s (vs iTerm2's ~60 MB/s)

**Key techniques:**
- Compile-time generated transition table
- `memchr` for SIMD escape sequence scanning
- Zero allocation during parse

### 2. Grid

Offset-based pages with style deduplication.

```rust
pub struct Grid {
    pages: PageList,
    cols: u16,
    rows: u16,
    cursor: Cursor,
    styles: StyleTable,
}

/// 8 bytes per cell (vs iTerm2's 16 bytes)
#[repr(C)]
pub struct Cell {
    codepoint: u32,    // Unicode codepoint or grapheme ref
    style_id: u16,     // Index into StyleTable
    flags: CellFlags,  // Wide, grapheme, etc.
}

/// Offset-based page (enables serialization)
pub struct Page {
    data: Box<[u8; PAGE_SIZE]>,
}

pub struct CellRef {
    offset: u32,  // Byte offset into page, NOT a pointer
}
```

**Why offsets:**
- Pages can be `memcpy`'d to disk
- Pages can be mmap'd back without fixup
- Pages can be sent over network
- Enables checkpoints and tiered storage

### 3. Scrollback (Tiered)

Hot/warm/cold storage with memory budget.

```rust
pub struct Scrollback {
    hot: VecDeque<Line>,          // Last ~1000 lines, uncompressed
    warm: Vec<CompressedBlock>,   // LZ4 compressed, in RAM
    cold: MmapFile,               // Zstd compressed, on disk

    hot_limit: usize,
    warm_limit: usize,
    memory_budget: usize,
}

impl Scrollback {
    pub fn push_line(&mut self, line: Line);

    /// Returns line, loading from cold storage if needed
    pub fn get_line(&mut self, idx: usize) -> &Line;

    pub fn line_count(&self) -> usize;
}
```

**Memory targets:**
- 100K lines: ~2 MB (vs iTerm2's ~50 MB)
- 1M lines: ~20 MB (vs iTerm2's ~500 MB)
- 10M lines: ~200 MB (iTerm2 would OOM)

### 4. Search Index

Trigram index for O(1) search.

```rust
pub struct SearchIndex {
    /// "err" -> [line 42, line 100, ...]
    trigrams: HashMap<[u8; 3], RoaringBitmap>,

    /// Fast negative lookup
    bloom: BloomFilter,
}

impl SearchIndex {
    pub fn index_line(&mut self, line_num: usize, text: &str);

    /// Returns matching line numbers
    pub fn search(&self, query: &str) -> impl Iterator<Item = usize>;
}
```

**Performance target:** <10ms for 1M lines (vs iTerm2's ~500ms)

### 5. Checkpoints

Crash recovery via periodic snapshots.

```rust
pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
    last_checkpoint: u64,
}

impl CheckpointManager {
    /// Called periodically (every 10s or 1000 lines)
    pub fn checkpoint(&mut self, grid: &Grid, scrollback: &Scrollback) -> io::Result<()>;

    /// Restore after crash
    pub fn restore(&self) -> io::Result<(Grid, Scrollback)>;
}
```

**Key insight:** Offset-based pages can be written directly to disk. No serialization needed.

### 6. C FFI

Interface for dashterm2.

```rust
// dterm.h

typedef struct dterm_parser dterm_parser_t;
typedef struct dterm_grid dterm_grid_t;
typedef struct dterm_scrollback dterm_scrollback_t;
typedef struct dterm_search dterm_search_t;

// Parser
dterm_parser_t* dterm_parser_new(void);
void dterm_parser_free(dterm_parser_t* parser);
void dterm_parser_feed(
    dterm_parser_t* parser,
    const uint8_t* data,
    size_t len,
    void* context,
    void (*callback)(void* context, dterm_action_t action)
);

// Grid
dterm_grid_t* dterm_grid_new(uint16_t rows, uint16_t cols);
void dterm_grid_free(dterm_grid_t* grid);
dterm_cell_t dterm_grid_get_cell(const dterm_grid_t* grid, uint16_t row, uint16_t col);
void dterm_grid_set_cell(dterm_grid_t* grid, uint16_t row, uint16_t col, dterm_cell_t cell);
void dterm_grid_resize(dterm_grid_t* grid, uint16_t rows, uint16_t cols);

// Scrollback
dterm_scrollback_t* dterm_scrollback_new(size_t memory_budget);
void dterm_scrollback_push(dterm_scrollback_t* sb, const dterm_line_t* line);
const dterm_line_t* dterm_scrollback_get(dterm_scrollback_t* sb, size_t idx);
size_t dterm_scrollback_len(const dterm_scrollback_t* sb);

// Search
dterm_search_t* dterm_search_new(void);
void dterm_search_index_line(dterm_search_t* search, size_t line_num, const char* text);
size_t dterm_search_query(
    const dterm_search_t* search,
    const char* query,
    size_t* results,
    size_t max_results
);

// Checkpoints
int dterm_checkpoint_save(const dterm_grid_t* grid, const dterm_scrollback_t* sb, const char* path);
int dterm_checkpoint_restore(const char* path, dterm_grid_t** grid, dterm_scrollback_t** sb);
```

---

## Crate Structure

```
dterm/
├── CLAUDE.md
├── Cargo.toml                    # Workspace
├── docs/
│   ├── STRATEGY.md               # This file
│   └── architecture/
│       ├── ARCHITECTURE.md       # High-level design
│       └── PERFORMANCE_ADVANTAGES.md  # Technical details
├── research/                     # Competitor analysis
│   ├── RESEARCH_INDEX.md
│   ├── alacritty/
│   ├── ghostty/
│   ├── foot/
│   ├── zellij/
│   └── ...
└── crates/
    └── dterm-core/
        ├── Cargo.toml
        ├── src/
        │   ├── lib.rs            # Public API
        │   ├── parser/
        │   │   ├── mod.rs
        │   │   ├── state.rs      # State enum
        │   │   ├── table.rs      # Transition table (const)
        │   │   └── action.rs     # Action enum
        │   ├── grid/
        │   │   ├── mod.rs
        │   │   ├── page.rs       # Offset-based pages
        │   │   ├── cell.rs       # Cell struct
        │   │   └── style.rs      # Style deduplication
        │   ├── scrollback/
        │   │   ├── mod.rs
        │   │   ├── tier.rs       # Tier trait
        │   │   ├── hot.rs        # RAM tier
        │   │   ├── warm.rs       # Compressed tier
        │   │   └── cold.rs       # Disk tier
        │   ├── search/
        │   │   ├── mod.rs
        │   │   ├── trigram.rs    # Trigram index
        │   │   └── bloom.rs      # Bloom filter
        │   ├── checkpoint/
        │   │   └── mod.rs
        │   └── ffi/
        │       ├── mod.rs
        │       └── types.rs      # C-compatible types
        ├── include/
        │   └── dterm.h           # C header
        ├── build.rs              # Generate header
        └── tests/
            ├── parser_tests.rs
            ├── grid_tests.rs
            └── conformance/      # VT conformance tests
```

---

## Success Metrics

### Phase 1 (dterm-core in dashterm2)

| Metric | iTerm2 Baseline | Target |
|--------|-----------------|--------|
| Parse throughput | ~60 MB/s | 400+ MB/s |
| Memory per cell | 16 bytes | 8 bytes |
| Memory (100K lines) | ~50 MB | ~5 MB |
| Memory (1M lines) | ~500 MB | ~50 MB |
| Search (1M lines) | ~500 ms | <10 ms |
| Crash recovery | None | <1 second |

### Phase 2 (Standalone dterm)

| Metric | Target |
|--------|--------|
| Cross-platform | macOS, Windows, Linux, iOS |
| Binary size | <20 MB |
| Startup time | <100 ms |
| Input latency | <5 ms |
| Agent workflows | Approval UI, audit log |

---

## Implementation Order

### Phase 1a: Parser (Week 1-2)
1. State machine with transition table
2. Action enum matching iTerm2's needs
3. C FFI bindings
4. Wire into dashterm2, benchmark

### Phase 1b: Grid (Week 3-4)
1. Cell struct (8 bytes)
2. Offset-based pages
3. Style deduplication
4. Wire into dashterm2, benchmark

### Phase 1c: Scrollback (Week 5-6)
1. Hot tier (VecDeque)
2. Warm tier (LZ4)
3. Cold tier (disk + zstd)
4. Memory budget enforcement

### Phase 1d: Search + Checkpoints (Week 7-8)
1. Trigram indexer
2. Search query
3. Checkpoint save/restore
4. Integration testing

### Phase 2: Standalone (Future)
- Design agent-native UX
- Build platform shells
- Cross-platform renderer (wgpu)

---

## Why This Strategy Wins

1. **Risk reduction** - Prove core in existing app before building new one
2. **Incremental value** - dashterm2 gets faster immediately
3. **Battle-tested** - Real users find bugs before standalone launch
4. **Focus** - Build the hard part (core) first, UI later
5. **Credibility** - "We made iTerm2 10x faster" is compelling proof

---

## Research Foundation

This strategy is informed by analysis of 12 terminal emulators:

| Terminal | Key Learning |
|----------|--------------|
| Ghostty | Offset-based pages, style dedup |
| foot | Cell-level dirty tracking, latency |
| Zellij | Session persistence, Rust patterns |
| iTerm2 | What to fix (16-byte cells, O(n) search) |
| Alacritty | Parser performance baseline |
| Warp | Block model, AI UX patterns |

See `research/RESEARCH_INDEX.md` for full analysis.

---

## The Thesis

**dterm-core makes dashterm2 the fastest feature-rich terminal on macOS.**

**Then dterm becomes the best terminal everywhere.**
