# dterm Technical Design

> **Note:** This document has been consolidated into [`DESIGN.md`](./DESIGN.md).
> This file contains additional implementation details. See DESIGN.md for the authoritative design.

**Date:** 2025-12-30
**Status:** Design Notes
**Goal:** Best-in-class terminal for AI agents, huge sessions, and continuous operation

---

## Executive Summary

dterm combines the best architectural patterns from Ghostty, Alacritty, Terminal.app, and others while adding innovations for AI-native workflows and unlimited session duration.

**Core insight:** Treat the terminal like a database, not a text buffer.

**Key differentiators:**
1. Tiered storage (hot/warm/cold) with disk backing
2. Offset-based pages enabling serialization/sync
3. Indexed search (O(1) vs O(n))
4. Formal verification (TLA+, Kani)
5. Agent-native approval workflows

---

## Memory Architecture

### Tiered Storage Model

```
dterm Memory Model:
+-------------------------------------------------------------+
|  HOT TIER (RAM) - Last 1000 lines                           |
|  +-------------------------------------------------------+  |
|  | Uncompressed pages, instant access, ~500KB            |  |
|  +-------------------------------------------------------+  |
|                         | Age out                           |
|  WARM TIER (RAM, Compressed) - Last 10K lines               |
|  +-------------------------------------------------------+  |
|  | LZ4 compressed pages, ~50KB (10x compression)         |  |
|  +-------------------------------------------------------+  |
|                         | Age out                           |
|  COLD TIER (Memory-Mapped File) - Unlimited history         |
|  +-------------------------------------------------------+  |
|  | Zstd compressed pages, lazy load                      |  |
|  | 100K lines = ~500KB on disk                           |  |
|  | 10M lines = ~50MB on disk (vs 2GB in Alacritty)       |  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+
```

### Offset-Based Page Design (from Ghostty)

**Critical for:** Serialization, disk backing, network sync, crash recovery.

```rust
const PAGE_SIZE: usize = 64 * 1024;  // 64KB pages

pub struct Page {
    /// Single contiguous allocation - can be memcpy'd, mmap'd, sent over network
    data: Box<[u8; PAGE_SIZE]>,
}

/// Offset into page, NOT a pointer
/// Enables: copy without fixup, serialize, mmap, network send
#[derive(Copy, Clone)]
pub struct Offset<T> {
    byte_offset: u32,
    _marker: PhantomData<T>,
}

impl<T> Offset<T> {
    pub fn get<'a>(&self, page: &'a Page) -> &'a T {
        unsafe {
            &*(page.data.as_ptr().add(self.byte_offset as usize) as *const T)
        }
    }
}

pub struct PageLayout {
    rows: Offset<[Row; ROWS_PER_PAGE]>,
    cells: Offset<[Cell; CELLS_PER_PAGE]>,
    styles: Offset<StyleTable>,
    graphemes: Offset<GraphemeStore>,
}
```

**Why offsets matter:**
- Pages can be `memcpy`'d to disk without pointer fixup
- Pages can be mmap'd back and used immediately
- Pages can be sent over network for remote sync
- Pages can be compressed as opaque blobs

### Pin System for Stable References

When pages are evicted (hot→warm→cold), references must survive:

```rust
/// Stable reference that survives page reorganization
pub struct Pin {
    page_id: PageId,        // Logical page identifier
    line_offset: u32,       // Line within page
    generation: u64,        // Detects stale pins
}

/// What the viewport is anchored to
pub enum Viewport {
    /// Follow new output (default - bottom of terminal)
    Active,

    /// Pinned to top of scrollback
    Top,

    /// Pinned to specific location (user scrolled)
    Pinned(Pin),
}

impl TieredScrollback {
    /// Resolve pin to actual line, loading from cold storage if needed
    pub fn resolve(&mut self, pin: &Pin) -> Result<&Line, PinError> {
        // Check generation for staleness
        if pin.generation != self.generation(pin.page_id) {
            return Err(PinError::Stale);
        }

        // Load page if in cold storage
        self.ensure_loaded(pin.page_id)?;

        Ok(self.get_line(pin.page_id, pin.line_offset))
    }
}
```

### Memory Pooling with Preheating (from Ghostty)

Avoid allocation jitter during typing:

```rust
pub struct MemoryPool {
    /// Pre-allocated page pool
    pages: Vec<Box<Page>>,

    /// Free list for O(1) allocation
    free_pages: Vec<usize>,

    /// Pre-allocated node pool for page list
    nodes: Vec<PageNode>,
    free_nodes: Vec<usize>,
}

impl MemoryPool {
    pub fn new(preheat_pages: usize) -> Self {
        let mut pool = Self::default();

        // Pre-allocate pages at startup
        for _ in 0..preheat_pages {
            pool.pages.push(Page::zeroed());
            pool.free_pages.push(pool.pages.len() - 1);
        }

        pool
    }

    /// O(1) page allocation from pool
    pub fn alloc_page(&mut self) -> Option<&mut Page> {
        self.free_pages.pop().map(|idx| &mut self.pages[idx])
    }
}
```

---

## Cell Storage

### Style Deduplication (from Ghostty)

Most cells share styles. Store once, reference many:

```rust
/// Deduplicated style storage
pub struct StyleTable {
    styles: Vec<Style>,
    ref_counts: Vec<u32>,
    lookup: HashMap<Style, StyleId>,
}

impl StyleTable {
    pub fn intern(&mut self, style: Style) -> StyleId {
        if let Some(&id) = self.lookup.get(&style) {
            self.ref_counts[id.0 as usize] += 1;
            return id;
        }

        let id = StyleId(self.styles.len() as u16);
        self.styles.push(style);
        self.ref_counts.push(1);
        self.lookup.insert(style, id);
        id
    }
}

/// Cell with style reference (8 bytes vs 28+ bytes)
#[repr(C)]
pub struct Cell {
    codepoint: u32,    // 4 bytes - Unicode codepoint
    style_id: u16,     // 2 bytes - Index into StyleTable
    flags: u16,        // 2 bytes - Wide, grapheme cluster, etc.
}

/// Full style data (stored once per unique style)
pub struct Style {
    fg: Color,         // 4 bytes
    bg: Color,         // 4 bytes
    underline: Color,  // 4 bytes
    attrs: StyleAttrs, // 2 bytes
}
```

**Memory savings:**
- Without dedup: 28 bytes/cell for style
- With dedup: 2 bytes/cell + shared table
- Typical terminal: 5-20 unique styles
- Savings: 10-14x for style data

### RLE Line Storage (from Terminal.app + Windows Terminal)

```rust
/// Line with run-length encoded attributes
pub struct Line {
    /// UTF-8 text content
    text: CompactString,

    /// Attribute runs (typically 1-4 per line)
    runs: SmallVec<[AttrRun; 4]>,
}

pub struct AttrRun {
    start: u16,        // Byte offset in text
    len: u16,          // Byte length
    style_id: StyleId, // Reference to StyleTable
}

// Example: "$ ls -la" with default colors
// Terminal.app: 8 cells * 32 bytes = 256 bytes
// dterm: 8 bytes text + 1 run * 6 bytes = 14 bytes (18x smaller)
```

### RLE Compression Benchmarks (Measured)

80-column attribute-only results (text bytes excluded):

| Content Type | Runs | Uncompressed | Compressed | Ratio |
|--------------|------|--------------|------------|-------|
| Prompt (4 colors) | 4 | 800B | 80B | 10x |
| `ls -la` output | 5 | 800B | 94B | 8.5x |
| Code syntax | 27 | 800B | 402B | 2x |
| Rainbow (worst) | 80 | 800B | 1144B | 0.7x |

Width scaling for prompt-style lines:

| Columns | Runs | Ratio |
|---------|------|-------|
| 80 | 4 | 10x |
| 132 | 4 | 16.5x |
| 200 | 4 | 25x |
| 400 | 4 | 50x |

Mixed 10K-line workload (50% plain, 10% prompt, 20% ls, 10% code, 10% formatted):

| Metric | Value |
|--------|-------|
| Uncompressed attrs | 8.0 MB |
| Compressed attrs | 764 KB |
| Compression ratio | 10.5x |
| Memory saved | 7.2 MB |

### Grapheme Bitmap Allocator (from Ghostty)

Unicode grapheme clusters (emoji sequences, combining chars) need variable storage:

```rust
/// Bitmap allocator for grapheme data
pub struct GraphemeStore {
    /// Chunks of 4 codepoints (16 bytes each)
    chunks: Vec<[u32; 4]>,

    /// Bitmap: 1 = chunk in use, 0 = free
    bitmap: Vec<u64>,
}

impl GraphemeStore {
    /// Allocate space for multi-codepoint grapheme
    pub fn alloc(&mut self, codepoints: &[u32]) -> GraphemeRef {
        let chunks_needed = (codepoints.len() + 3) / 4;

        // Find contiguous free chunks via bit scan
        let start = self.find_free_run(chunks_needed);

        // Mark as used
        self.mark_used(start, chunks_needed);

        // Copy data
        for (i, chunk) in codepoints.chunks(4).enumerate() {
            self.chunks[start + i][..chunk.len()].copy_from_slice(chunk);
        }

        GraphemeRef { start: start as u32, len: codepoints.len() as u8 }
    }
}
```

---

## Rendering

### Dirty Bitmap Tracking (from Terminal.app + Ghostty)

Only render changed lines:

```rust
pub struct DirtyTracker {
    /// One bit per line, 64 lines per word
    bitmap: [u64; MAX_LINES / 64],

    /// Full redraw needed (resize, scroll, etc.)
    full_damage: bool,
}

impl DirtyTracker {
    #[inline]
    pub fn mark(&mut self, line: usize) {
        self.bitmap[line / 64] |= 1 << (line % 64);
    }

    #[inline]
    pub fn is_dirty(&self, line: usize) -> bool {
        self.bitmap[line / 64] & (1 << (line % 64)) != 0
    }

    /// Iterate dirty lines efficiently
    pub fn dirty_lines(&self) -> impl Iterator<Item = usize> + '_ {
        self.bitmap.iter().enumerate().flat_map(|(word_idx, &word)| {
            let base = word_idx * 64;
            std::iter::successors(
                (word != 0).then(|| word.trailing_zeros() as usize),
                move |&prev| {
                    let remaining = word >> (prev + 1);
                    (remaining != 0).then(|| prev + 1 + remaining.trailing_zeros() as usize)
                }
            ).map(move |bit| base + bit)
        })
    }

    pub fn clear(&mut self) {
        self.bitmap.fill(0);
        self.full_damage = false;
    }
}
```

### GPU Instanced Rendering

Single draw call for entire terminal:

```rust
#[repr(C)]
pub struct GlyphInstance {
    pos: [f32; 2],        // Screen position
    uv: [f32; 4],         // Texture atlas region
    fg: [f32; 4],         // Foreground color
    bg: [f32; 4],         // Background color
}

pub struct Renderer {
    glyph_atlas: GlyphAtlas,
    instance_buffer: wgpu::Buffer,
}

impl Renderer {
    pub fn render(&mut self, terminal: &Terminal, dirty: &DirtyTracker) {
        let mut instances = Vec::with_capacity(terminal.cols() * dirty.count());

        for line_idx in dirty.dirty_lines() {
            let line = terminal.line(line_idx);
            let y = line_idx as f32 * self.cell_height;

            for (col, cell) in line.cells().enumerate() {
                let x = col as f32 * self.cell_width;
                let uv = self.glyph_atlas.lookup(cell.codepoint);
                let style = terminal.style(cell.style_id);

                instances.push(GlyphInstance {
                    pos: [x, y],
                    uv,
                    fg: style.fg.to_array(),
                    bg: style.bg.to_array(),
                });
            }
        }

        // Single instanced draw call
        self.queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        render_pass.draw(0..4, 0..instances.len() as u32);
    }
}
```

---

## Parser

### SIMD Escape Scanning

Hot path: find escape sequences in ASCII stream:

```rust
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

        // Scalar fallback
        input[i..].iter().position(|&b| b == 0x1B).map(|p| i + p)
    }
}

/// Use memchr crate for production (battle-tested SIMD)
pub fn find_escape(input: &[u8]) -> Option<usize> {
    memchr::memchr(0x1B, input)
}
```

### Table-Driven State Machine (from Terminal.app + Ghostty)

Comptime-generated transition table:

```rust
/// Parser states (from vt100.net DEC ANSI parser)
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum State {
    Ground = 0,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    OscString,
    SosPmApcString,
}

/// Actions the parser can emit
pub enum Action {
    Print(char),
    Execute(u8),
    CsiDispatch(CsiParams),
    EscDispatch(u8, u8),
    OscDispatch(OscCommand),
    // ...
}

/// Transition table: [state][byte] -> (new_state, action)
/// Generated at compile time
const TRANSITION_TABLE: [[Transition; 256]; 14] = generate_table();

const fn generate_table() -> [[Transition; 256]; 14] {
    // ... 3000 lines of comptime logic
}

impl Parser {
    #[inline]
    pub fn advance(&mut self, byte: u8) -> Option<Action> {
        let transition = TRANSITION_TABLE[self.state as usize][byte as usize];
        self.state = transition.next_state;
        transition.action
    }
}
```

### Backpressure for High-Throughput

Stay responsive during `cat huge_file`:

```rust
pub struct StreamingParser {
    state: ParserState,
    pending_bytes: usize,
    last_render: Instant,

    coalesce_threshold: Duration,  // 8ms for 120Hz
    drop_threshold: usize,         // 1MB pending
}

impl StreamingParser {
    pub fn process(&mut self, input: &[u8]) -> ParseResult {
        // Always parse (never lose data)
        self.parse_into_buffer(input);
        self.pending_bytes += input.len();

        let elapsed = self.last_render.elapsed();

        if elapsed < self.coalesce_threshold && self.pending_bytes < self.drop_threshold {
            return ParseResult::Coalesced;
        }

        // Skip intermediate frames if overwhelmed
        if self.pending_bytes > self.drop_threshold {
            self.skip_to_latest();
        }

        self.last_render = Instant::now();
        self.pending_bytes = 0;
        ParseResult::Render
    }
}
```

---

## Search

### Indexed Search (O(1) vs O(n))

For 100K+ line sessions:

```rust
pub struct SearchIndex {
    /// Trigram index: "err" -> [line 42, line 100, ...]
    trigrams: HashMap<[u8; 3], RoaringBitmap>,

    /// Bloom filter for fast negative lookups
    bloom: BloomFilter,

    /// Word index for whole-word search
    words: HashMap<CompactString, RoaringBitmap>,
}

impl SearchIndex {
    /// Index a new line
    pub fn index_line(&mut self, line_num: usize, text: &str) {
        // Add to bloom filter
        self.bloom.insert(text.as_bytes());

        // Index trigrams
        for trigram in text.as_bytes().windows(3) {
            self.trigrams
                .entry(trigram.try_into().unwrap())
                .or_default()
                .insert(line_num as u32);
        }

        // Index words
        for word in text.split_whitespace() {
            self.words
                .entry(word.into())
                .or_default()
                .insert(line_num as u32);
        }
    }

    /// Search with trigram intersection
    pub fn search(&self, query: &str) -> impl Iterator<Item = u32> + '_ {
        // Fast negative check
        if !self.bloom.might_contain(query.as_bytes()) {
            return Box::new(std::iter::empty()) as Box<dyn Iterator<Item = u32>>;
        }

        // Intersect trigram posting lists
        let trigrams: Vec<_> = query.as_bytes().windows(3).collect();

        let candidates = trigrams.iter()
            .filter_map(|t| self.trigrams.get(*t))
            .fold(None, |acc: Option<RoaringBitmap>, bitmap| {
                Some(match acc {
                    None => bitmap.clone(),
                    Some(a) => a & bitmap,
                })
            })
            .unwrap_or_default();

        Box::new(candidates.iter())
    }
}
```

---

## Session Persistence

### Checkpoint/Restore

Never lose a multi-day session:

```rust
pub struct SessionCheckpoint {
    checkpoint_dir: PathBuf,
    last_full: u64,           // Sequence number
    delta_count: usize,
}

impl SessionCheckpoint {
    /// Incremental checkpoint (fast, frequent)
    pub fn checkpoint_delta(&mut self, terminal: &Terminal) -> io::Result<()> {
        let delta = terminal.changes_since(self.last_sequence);

        let path = self.checkpoint_dir.join(format!("delta_{}.bin", self.delta_count));
        let file = File::create(path)?;

        // Write compressed delta
        let mut encoder = zstd::Encoder::new(file, 3)?;
        bincode::serialize_into(&mut encoder, &delta)?;
        encoder.finish()?;

        self.delta_count += 1;

        // Full checkpoint every 100 deltas
        if self.delta_count >= 100 {
            self.checkpoint_full(terminal)?;
        }

        Ok(())
    }

    /// Full checkpoint (slower, less frequent)
    pub fn checkpoint_full(&mut self, terminal: &Terminal) -> io::Result<()> {
        let path = self.checkpoint_dir.join(format!("full_{}.bin", self.last_full + 1));

        // Write all pages to disk
        for (page_id, page) in terminal.pages() {
            let page_path = path.join(format!("page_{}.bin", page_id));
            // Pages with offsets can be written directly!
            std::fs::write(page_path, &page.data[..])?;
        }

        // Write metadata
        let meta = CheckpointMeta {
            cursor: terminal.cursor(),
            modes: terminal.modes(),
            styles: terminal.style_table().clone(),
            timestamp: SystemTime::now(),
        };
        bincode::serialize_into(File::create(path.join("meta.bin"))?, &meta)?;

        // Clean old deltas
        self.clean_old_deltas()?;
        self.last_full += 1;
        self.delta_count = 0;

        Ok(())
    }

    /// Restore from crash
    pub fn restore(&self) -> io::Result<Terminal> {
        // Find latest full checkpoint
        let full_path = self.find_latest_full()?;

        // Load pages (can mmap directly due to offset-based design)
        let pages = self.load_pages(&full_path)?;

        // Load metadata
        let meta: CheckpointMeta = bincode::deserialize_from(
            File::open(full_path.join("meta.bin"))?
        )?;

        // Apply deltas since full checkpoint
        let mut terminal = Terminal::from_checkpoint(pages, meta);
        for delta in self.load_deltas_since(meta.sequence)? {
            terminal.apply_delta(delta)?;
        }

        Ok(terminal)
    }
}
```

---

## Security Features (from Terminal.app)

### Secure Keyboard Entry

Protect against keyloggers:

```rust
#[cfg(target_os = "macos")]
pub fn enable_secure_input(enable: bool) {
    use core_foundation::base::Boolean;

    extern "C" {
        fn EnableSecureEventInput() -> Boolean;
        fn DisableSecureEventInput() -> Boolean;
        fn IsSecureEventInputEnabled() -> Boolean;
    }

    unsafe {
        if enable {
            EnableSecureEventInput();
        } else {
            DisableSecureEventInput();
        }
    }
}
```

### Bracketed Paste Validation

Detect escape-in-paste attacks:

```rust
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

pub fn validate_paste(content: &[u8]) -> PasteValidation {
    // Check if paste contains the end sequence (attack vector)
    if memchr::memmem::find(content, PASTE_END).is_some() {
        return PasteValidation::Dangerous {
            reason: "Paste contains escape sequence that could execute commands",
        };
    }

    // Check for other suspicious sequences
    if memchr::memmem::find(content, b"\x1b[").is_some() {
        return PasteValidation::Warning {
            reason: "Paste contains escape sequences",
        };
    }

    PasteValidation::Safe
}
```

---

## Shell Integration

### Marks and Bookmarks (from Terminal.app)

Navigate command history:

```rust
pub struct Mark {
    pin: Pin,              // Location in scrollback
    mark_type: MarkType,
    timestamp: SystemTime,
}

pub enum MarkType {
    /// Automatic mark at command prompt (OSC 133)
    CommandPrompt,

    /// Automatic mark at command output start
    CommandOutput { exit_code: Option<i32> },

    /// User-inserted bookmark
    Bookmark { name: Option<String> },
}

pub struct MarkIndex {
    marks: Vec<Mark>,
    by_type: HashMap<MarkType, Vec<usize>>,
}

impl MarkIndex {
    /// Jump to previous command prompt
    pub fn prev_prompt(&self, from: &Pin) -> Option<&Mark> {
        self.marks.iter()
            .rev()
            .find(|m| matches!(m.mark_type, MarkType::CommandPrompt) && m.pin < *from)
    }

    /// Jump to next command prompt
    pub fn next_prompt(&self, from: &Pin) -> Option<&Mark> {
        self.marks.iter()
            .find(|m| matches!(m.mark_type, MarkType::CommandPrompt) && m.pin > *from)
    }

    /// Select output of last command
    pub fn select_last_output(&self) -> Option<(Pin, Pin)> {
        let prompts: Vec<_> = self.marks.iter()
            .filter(|m| matches!(m.mark_type, MarkType::CommandPrompt))
            .collect();

        if prompts.len() >= 2 {
            let start = prompts[prompts.len() - 2].pin;
            let end = prompts[prompts.len() - 1].pin;
            Some((start, end))
        } else {
            None
        }
    }
}
```

### OSC 133 Integration

```rust
impl Terminal {
    pub fn handle_osc_133(&mut self, params: &[&[u8]]) {
        match params.first() {
            Some(b"A") => {
                // Prompt start
                self.marks.add(Mark {
                    pin: self.current_pin(),
                    mark_type: MarkType::CommandPrompt,
                    timestamp: SystemTime::now(),
                });
            }
            Some(b"C") => {
                // Command output start
                self.marks.add(Mark {
                    pin: self.current_pin(),
                    mark_type: MarkType::CommandOutput { exit_code: None },
                    timestamp: SystemTime::now(),
                });
            }
            Some(b"D") => {
                // Command finished
                let exit_code = params.get(1)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .and_then(|s| s.parse().ok());

                if let Some(mark) = self.marks.last_mut() {
                    if let MarkType::CommandOutput { exit_code: ref mut ec } = mark.mark_type {
                        *ec = exit_code;
                    }
                }
            }
            _ => {}
        }
    }
}
```

---

## Process Tracking (from Terminal.app)

For tab titles and close warnings:

```rust
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
}

impl ProcessInfo {
    #[cfg(unix)]
    pub fn from_pid(pid: u32) -> Option<Self> {
        use std::fs;

        // Read from /proc on Linux, sysctl on macOS
        #[cfg(target_os = "linux")]
        {
            let cmdline = fs::read_to_string(format!("/proc/{}/cmdline", pid)).ok()?;
            let argv: Vec<_> = cmdline.split('\0').map(String::from).collect();
            let cwd = fs::read_link(format!("/proc/{}/cwd", pid)).ok()?;
            let name = argv.first()?.rsplit('/').next()?.to_string();

            Some(Self { pid, name, argv, cwd })
        }

        #[cfg(target_os = "macos")]
        {
            // Use sysctl or libproc
            // ...
        }
    }
}

pub struct ProcessTracker {
    shell_pid: u32,
    foreground: Option<ProcessInfo>,
}

impl ProcessTracker {
    /// Update foreground process (called periodically)
    pub fn update(&mut self) {
        let fg_pid = self.get_foreground_pid();

        if Some(fg_pid) != self.foreground.as_ref().map(|p| p.pid) {
            self.foreground = ProcessInfo::from_pid(fg_pid);
        }
    }

    /// Get tab title
    pub fn title(&self) -> &str {
        self.foreground.as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("shell")
    }

    /// Should warn on close?
    pub fn has_running_process(&self) -> bool {
        self.foreground.as_ref()
            .map(|p| p.pid != self.shell_pid)
            .unwrap_or(false)
    }
}
```

---

## Performance Targets

### IMPORTANT: Scope Clarification

**dterm-core** is a library providing parser, grid, and state machine. It does NOT include
rendering. It integrates into **DashTerm2** (an iTerm2 fork) which provides the rendering layer.

Comparisons marked with `*` require DashTerm2 integration and are targets, not measurements.

### Measured (dterm-core only)

| Metric | vte (Alacritty parser) | dterm-core | Notes |
|--------|------------------------|------------|-------|
| Parse throughput (ASCII) | ~376 MiB/s | ~3.5 GiB/s | **9.4x faster** (fair: parser vs parser) |
| Parse throughput (mixed) | ~385 MiB/s | ~2.2 GiB/s | **5.9x faster** (fair: parser vs parser) |
| Parse throughput (escapes) | ~385 MiB/s | ~931 MiB/s | **2.4x faster** (fair: parser vs parser) |

### Targets (require DashTerm2 integration)

| Metric | Alacritty | dterm Target | Method |
|--------|-----------|--------------|--------|
| Memory (100K lines) | ~20 MB | ~2 MB | Tiered storage |
| Memory (1M lines) | ~200 MB | ~20 MB | Compression + disk |
| Memory (10M lines) | OOM | ~200 MB | Cold tier on disk |
| Search 1M lines | ~500ms | ~5ms | Trigram index |
| Render latency* | ~2ms | ~1ms | Dirty tracking + GPU |
| Crash recovery | Lost | Restored | Checkpoints |
| 24hr session | Memory grows | Memory capped | Budget enforcement |

### What We Cannot Fairly Compare

We cannot compare dterm-core to full terminals (Ghostty ~600 MB/s, Kitty ~500 MB/s, Alacritty ~400 MB/s)
because those numbers include rendering overhead that dterm-core doesn't have. Such comparisons would
be misleading. Full terminal benchmarks require DashTerm2 integration to be complete.

---

## Implementation Priority

### Phase 1: Foundation
1. Offset-based page structure
2. Basic tiered storage (hot only initially)
3. Style deduplication
4. Dirty bitmap tracking
5. Table-driven parser

### Phase 2: Persistence
6. Warm tier (LZ4 compression)
7. Cold tier (disk-backed)
8. Pin system
9. Session checkpoints

### Phase 3: Performance
10. SIMD parser
11. Indexed search
12. GPU instanced rendering
13. Memory pooling

### Phase 4: Features
14. Shell integration (OSC 133, marks)
15. Process tracking
16. Security features
17. Agent approval workflows

---

## Research Sources

| Source | Key Contribution |
|--------|------------------|
| Ghostty | Offset-based pages, pin system, style dedup, memory pools |
| Terminal.app | Dirty bitmap, marks/bookmarks, RLE attributes, security |
| Alacritty | Ring buffer, damage tracking, vte parser reference |
| Windows Terminal | RLE attributes, ConPTY reference |
| Kitty | SIMD parsing, graphics protocol |
| Contour | VT compliance, bulk text optimization |

---

## Summary

dterm's architecture combines:
- **Ghostty's** memory model (offset-based pages, pins, style dedup)
- **Terminal.app's** features (marks, security, process tracking)
- **Alacritty's** rendering (damage tracking, GPU)
- **Novel innovations** (tiered storage, indexed search, checkpoints)

The result: unlimited session duration, instant search, crash recovery, and agent-native workflows - none of which exist in current terminals.
