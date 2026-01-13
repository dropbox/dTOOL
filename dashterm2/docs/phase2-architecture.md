# Phase 2: Core Engine Architecture Proposal

**Status:** DRAFT
**Date:** 2025-12-27
**Author:** Research synthesis from Alacritty, WezTerm, and AI Agent API analysis

---

## Executive Summary

Phase 2 rebuilds DashTerm2's terminal core in Rust. The goal is NOT to build an IDE, NOT to create new APIs, NOT to replace text/CLI.

**The goal is to make text/CLI work so well that AI agents automatically get better results.**

**Design principle:** Claude Code in Terminal.app today should work BETTER in DashTerm2 tomorrow - with zero changes to Claude Code.

### What "Make It Better" Means

| Improvement | How It Helps AI Agents |
|-------------|------------------------|
| Faster parsing | Commands complete sooner |
| No crashes | No lost output, no restarts |
| Reliable exit codes | Shell integration that always works |
| Lower latency | Streaming output arrives faster |
| Memory safety | No corruption, no hangs |

We're not replacing text with JSON. We're not building MCP servers. We're making the terminal itself better at being a terminal.

---

## Research Findings

### Alacritty Architecture

| Component | Approach | Reusability |
|-----------|----------|-------------|
| `vte` crate | Table-driven parser, Paul Williams' state machine | **Excellent** - standalone, MIT licensed |
| `alacritty_terminal` | Complete terminal core, separate from GUI | **Good** - could embed directly |
| Grid storage | Ring buffer with zero-offset rotation | **Good** - efficient scrolling |
| Renderer | OpenGL ES 2.0+ with glyph cache | Low - we use Metal |
| IPC | JSON over Unix socket (config only) | Low - not for terminal state |

**Key insight:** Alacritty's `vte` crate is battle-tested and used by many terminals. Their ring buffer grid is clever - O(1) scrolling without memory copies.

### WezTerm Architecture

| Component | Approach | Reusability |
|-----------|----------|-------------|
| `termwiz` | Standalone terminal library | **Excellent** - embeddable |
| `vtparse` | Custom DEC ANSI parser | Good alternative to vte |
| Mux | Domain/Window/Tab/Pane hierarchy | **Good** - clean design |
| Lua scripting | Full terminal state access | Reference for our API |
| CLI API | `wezterm cli list`, `send-text` | **Excellent** - proves external access works |
| Renderer | wgpu with WGSL shaders | Portable - could use |

**Key insight:** WezTerm's Lua API proves you CAN expose terminal state safely. Their `pane:get_lines_as_text()` is exactly what AI agents need.

### AI Agent Needs

| Agent | Current Approach | Pain Points |
|-------|------------------|-------------|
| Claude Code | subprocess, parse stdout | No command boundaries, exit code hacks |
| Aider | pexpect for TTY, fallback subprocess | Unicode crashes, encoding issues |
| Cursor | Sandboxed execution | Wants semantic output parsing |
| Warp | "Blocks" (command+output pairs) | Closest to ideal, but proprietary |

**Key insight:** DashTerm2 already has most building blocks via shell integration (FinalTerm protocol). The gap is exposing them externally.

---

## Architecture Decision: Build vs Reuse

### Option A: Use alacritty_terminal directly

```
┌─────────────────────────────────────────┐
│ DashTerm2 macOS App (Swift/ObjC)        │
│ ├── UI layer (existing)                 │
│ └── Terminal backend ──┐                │
└────────────────────────┼────────────────┘
                         │ FFI
                         ▼
┌─────────────────────────────────────────┐
│ alacritty_terminal (Rust, unmodified)   │
│ ├── vte parser                          │
│ ├── Grid storage                        │
│ └── PTY handling                        │
└─────────────────────────────────────────┘
```

**Pros:** Fast to start, battle-tested code
**Cons:** No AI API hooks, limited customization, dependency on external project

### Option B: Fork alacritty_terminal

Same as A, but fork and add:
- AI Agent API hooks
- MCP server integration
- Semantic output annotations

**Pros:** Start with working code, add what we need
**Cons:** Merge conflicts with upstream, technical debt

### Option C: Build new core using vte crate

```
┌─────────────────────────────────────────┐
│ DashTerm2 macOS App (Swift/ObjC)        │
│ ├── UI layer (existing)                 │
│ ├── Metal renderer (existing)           │
│ └── Terminal backend ──┐                │
└────────────────────────┼────────────────┘
                         │ FFI (C ABI)
                         ▼
┌─────────────────────────────────────────┐
│ dashterm-core (Rust, new)               │
│ ├── vte parser (crate)                  │
│ ├── Grid (custom, ring buffer)          │
│ ├── PTY (custom)                        │
│ ├── AI Agent API ◄── NEW                │
│ │   ├── Command boundaries              │
│ │   ├── Output streaming                │
│ │   └── State subscriptions             │
│ └── MCP Server ◄── NEW                  │
└─────────────────────────────────────────┘
```

**Pros:** Full control, AI-native from start, clean API surface
**Cons:** More work upfront, need to implement grid/PTY ourselves

### Recommendation: Option C

Build a new `dashterm-core` crate using `vte` for parsing but custom everything else. Reasons:

1. **AI API is core, not bolted on** - Design for it from day one
2. **Clean FFI surface** - One C ABI boundary, no Alacritty internal details leaking
3. **No upstream dependency** - We control our destiny
4. **Learn from both** - Take best ideas from Alacritty (ring buffer) and WezTerm (Lua-style API)

---

## Proposed Architecture

### Crate Structure

```
dashterm-core/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── parser/             # VT100/xterm parsing (uses vte crate)
│   ├── grid/               # Screen buffer (ring buffer design)
│   ├── terminal/           # Terminal state machine
│   ├── pty/                # PTY handling
│   ├── api/                # AI Agent API
│   │   ├── commands.rs     # Command tracking
│   │   ├── events.rs       # Event subscriptions
│   │   └── mcp.rs          # MCP server
│   └── ffi/                # C ABI exports
└── tests/
```

### Core Types

```rust
/// Terminal session state
pub struct Terminal {
    grid: Grid<Cell>,
    parser: vte::Parser,
    cursor: Cursor,
    // Command tracking (AI-native)
    current_command: Option<Command>,
    command_history: VecDeque<Command>,
}

/// Command with full metadata (for AI agents)
pub struct Command {
    pub id: Uuid,
    pub text: String,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub exit_code: Option<i32>,
    pub output_range: Range<usize>,  // Rows in grid
    pub cwd: PathBuf,
}

/// Events for AI agent subscriptions
pub enum TerminalEvent {
    CommandStarted { id: Uuid, text: String },
    CommandFinished { id: Uuid, exit_code: i32, duration: Duration },
    OutputChunk { command_id: Uuid, text: String, stream: Stream },
    DirectoryChanged { old: PathBuf, new: PathBuf },
    PromptShown { prompt: String },
}
```

### FFI Surface (C ABI)

```c
// Create/destroy
dashterm_terminal_t* dashterm_terminal_new(void);
void dashterm_terminal_free(dashterm_terminal_t*);

// Input
void dashterm_terminal_write(dashterm_terminal_t*, const uint8_t* data, size_t len);

// Read state
size_t dashterm_terminal_get_rows(dashterm_terminal_t*);
size_t dashterm_terminal_get_cols(dashterm_terminal_t*);
void dashterm_terminal_get_cursor(dashterm_terminal_t*, size_t* row, size_t* col);
char* dashterm_terminal_get_line(dashterm_terminal_t*, size_t row);  // Caller frees

// AI Agent API
char* dashterm_terminal_get_current_command(dashterm_terminal_t*);  // JSON
char* dashterm_terminal_get_command_history(dashterm_terminal_t*, size_t limit);  // JSON array
int32_t dashterm_terminal_get_last_exit_code(dashterm_terminal_t*);
char* dashterm_terminal_get_cwd(dashterm_terminal_t*);

// Event subscription
typedef void (*dashterm_event_callback)(const char* event_json, void* userdata);
void dashterm_terminal_subscribe(dashterm_terminal_t*, dashterm_event_callback, void* userdata);
```

### Swift Integration

```swift
class DashTermCore {
    private var terminal: OpaquePointer

    init() {
        terminal = dashterm_terminal_new()
    }

    deinit {
        dashterm_terminal_free(terminal)
    }

    func write(_ data: Data) {
        data.withUnsafeBytes { ptr in
            dashterm_terminal_write(terminal, ptr.baseAddress, ptr.count)
        }
    }

    var currentCommand: Command? {
        guard let json = dashterm_terminal_get_current_command(terminal) else { return nil }
        defer { free(json) }
        return try? JSONDecoder().decode(Command.self, from: Data(cString: json))
    }

    func subscribe(_ callback: @escaping (TerminalEvent) -> Void) {
        // Bridge callback through FFI
    }
}
```

---

## Implementation Phases

### Phase 2.1: Minimal Viable Core (4-6 weeks)

1. Set up Rust workspace with `vte` dependency
2. Implement ring buffer Grid
3. Implement basic Terminal state machine
4. FFI bridge to Swift
5. Replace ONE component in DashTerm2 (e.g., escape sequence parsing)
6. Benchmark: must be faster than current ObjC

**Exit criteria:** DashTerm2 runs with Rust parser, no regressions

### Phase 2.2: Full Terminal Core (6-8 weeks)

1. Complete VT100/xterm implementation
2. PTY handling in Rust
3. Full grid with scrollback
4. All FFI functions working
5. Remove old ObjC terminal code

**Exit criteria:** DashTerm2 runs entirely on dashterm-core

### Phase 2.3: AI Agent API (4-6 weeks)

1. Command tracking (start/end/exit code)
2. Event subscription system
3. MCP server implementation
4. Documentation for agent developers

**Exit criteria:** Claude Code can query terminal state via MCP

### Phase 2.4: Performance & Polish (2-4 weeks)

1. GPU-accelerated text rendering (if not already)
2. Benchmark against Alacritty/WezTerm
3. Memory profiling
4. Edge case hardening

**Exit criteria:** Faster than iTerm2, no crashes in fuzzing

---

## Dependencies

### Rust Crates

| Crate | Purpose | Version |
|-------|---------|---------|
| `vte` | Escape sequence parsing | 0.15+ |
| `parking_lot` | Fast mutexes | 0.12+ |
| `crossbeam` | Channels for events | 0.8+ |
| `serde` | JSON serialization | 1.0+ |
| `uuid` | Command IDs | 1.0+ |
| `tokio` | Async for MCP server | 1.0+ |

### Build Requirements

- Rust 1.75+ (for stable async traits)
- Xcode 15+ (for Swift interop)
- `cbindgen` for generating C headers

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| FFI complexity | Start simple, expand gradually. Use cbindgen. |
| Performance regression | Benchmark continuously. Don't ship if slower. |
| Feature parity | Keep old code path until new is complete. |
| VT100 edge cases | Use vttest suite. Port existing DashTerm2 tests. |
| MCP protocol changes | Abstract behind our API. |

---

## Success Metrics

1. **Performance:** Terminal output rendering ≤1ms latency
2. **Stability:** Zero crashes in 1M random escape sequences (fuzzing)
3. **Memory:** ≤50% memory usage of current implementation
4. **Compatibility:** Pass vttest suite
5. **AI API:** Claude Code can query command history and exit codes

---

## Advanced: Semantic Compression for LLMs

**Research reference:** DeepSeek-OCR (https://arxiv.org/abs/2510.18234)

DeepSeek demonstrated that visual text can be compressed 10-20x while maintaining high accuracy for LLM processing. The same principle applies to terminal output.

### The Problem

AI agents receive raw terminal output:
```
Compiling foo v0.1.0
   Compiling bar v0.2.0
   Compiling baz v0.3.0
   ... (500 more lines)
   Compiling qux v0.99.0
    Finished dev [unoptimized + debuginfo] target(s) in 42.17s
```

This is 500+ lines / ~2000 tokens. The semantic content is: "Build succeeded in 42s".

### Terminal-Native vs PDF-Native

DeepSeek-OCR compresses PDF pages. Terminal output is fundamentally different:

| PDF/Document | Terminal |
|--------------|----------|
| Variable page size | Fixed width (80, 120, $COLUMNS) |
| Visual layout | Line-oriented, sequential |
| Fonts, images | ANSI escapes (colors, cursor) |
| Static | Streaming, real-time |
| Paragraphs | Command + output "blocks" |

**The natural compression unit is a command block, not a page.**

```
┌─────────────────────────────────────────────────────────────┐
│ $ npm install                                    [PROMPT]   │
├─────────────────────────────────────────────────────────────┤
│ added 847 packages in 32s                                   │
│ 3 moderate severity vulnerabilities              [OUTPUT]   │
├─────────────────────────────────────────────────────────────┤
│ exit: 0 | 32.4s | 9 lines                       [METADATA]  │
└─────────────────────────────────────────────────────────────┘
```

### Terminal-Aware Tokenization

```rust
/// Terminal-specific tokens (not just raw text)
pub enum TerminalToken {
    // Structural
    Prompt { user: String, host: String, cwd: PathBuf },
    Command { text: String },
    OutputLine { text: String, stream: Stream },

    // Semantic (pattern-detected)
    Error { message: String, file: Option<PathBuf>, line: Option<u32> },
    Warning { message: String },
    Progress { pct: u8, label: String },
    TableRow { columns: Vec<String> },

    // Compression markers
    Repetition { pattern: String, count: u32 },
    Elided { summary: String, original_lines: u32 },
}
```

### Width-Aware Processing

Terminal width (usually 80 or 120) affects patterns:

| Pattern | Detection | Compression |
|---------|-----------|-------------|
| Wrapped lines | Line ends at exactly $COLUMNS | Join into single logical line |
| Column alignment | Consistent spacing (ls -l, ps) | Parse as table |
| Carriage return | `\r` without `\n` | Keep only final state |
| ANSI escapes | `\e[...m` sequences | Strip for analysis, preserve for display |
| Progress bars | Repeated `\r` updates | `[PROGRESS: 0% → 100%]` |

### The Opportunity

DashTerm2 could include a **semantic compressor** that reduces output to its essential meaning:

| Raw Output | Compressed | Token Savings |
|------------|------------|---------------|
| 500 lines of "Compiling X" | `[BUILD: 502 crates compiled, 42.17s]` | ~98% |
| Progress bar updates | `[PROGRESS: 47% → 100%]` | ~99% |
| Repeating log lines | `[LOG: "Connection retry" ×47]` | ~95% |
| Stack trace | `[ERROR: NullPointerException at Foo.java:42, 12 frames]` | ~80% |

### Implementation Approach

```rust
/// Semantic output compressor
pub struct OutputCompressor {
    patterns: Vec<CompressionPattern>,
    model: Option<CompactModel>,  // Optional ML model for complex cases
}

pub enum CompressionPattern {
    /// Repeated lines with minor variations
    RepetitionCollapse { threshold: usize },
    /// Progress indicators (%, bars, spinners)
    ProgressSummary,
    /// Build output (Compiling, Linking, etc.)
    BuildSummary,
    /// Stack traces
    StackTraceSummary { max_frames: usize },
    /// Log lines with timestamps
    LogDeduplication { window: Duration },
}

impl OutputCompressor {
    /// Compress output for LLM consumption
    pub fn compress(&self, raw: &str) -> CompressedOutput {
        // Rule-based compression first
        // ML model for ambiguous cases
    }
}
```

### Training Custom Models

For complex cases (code output, mixed formats), we could train small models:

1. **Dataset:** Pairs of (raw terminal output, semantic summary)
2. **Architecture:** Small transformer (50M-500M params) or distilled from larger model
3. **Deployment:** Run locally in DashTerm2, or offer as opt-in cloud service

### Integration with AI Agent API

```rust
pub enum OutputFormat {
    Raw,                    // Full output
    Compressed,             // Semantic compression
    Hybrid {                // Compressed + raw for key sections
        compress_threshold: usize,
    },
}

// AI agent requests compressed output
let output = terminal.get_command_output(cmd_id, OutputFormat::Compressed);
```

### Benefits

1. **Reduced token usage** - 10-100x fewer tokens for verbose output
2. **Faster responses** - Less data to process
3. **Lower cost** - Fewer API tokens = lower bills
4. **Better context** - More room for actual code/conversation
5. **Automatic** - Terminal does it, agents don't need to change

### Phase 5 (Future)

This is advanced work for after the core is solid. Order:
1. Rule-based compression (heuristics)
2. Pattern library (build tools, test runners, etc.)
3. ML model for edge cases
4. Training pipeline for custom models

---

## Open Questions

1. **Should we support alternate screen buffer from day 1?** (vim, less, etc.)
2. **How much of WezTerm's multiplexing do we need?** (or leave to tmux)
3. **MCP vs custom protocol for AI API?** (MCP is standard but limited)
4. **iOS first or macOS first for Phase 3?**
5. **Semantic compression: rule-based first or ML from start?**

---

## Next Steps

1. [x] Create `dashterm-core` Rust crate (Worker #1421 - 2025-12-27)
2. [x] Implement minimal vte wrapper (Worker #1421 - 2025-12-27)
3. [x] Implement ring buffer Grid (Worker #1421 - 2025-12-27)
4. [x] FFI C ABI exports for Swift interop (Worker #1421 - 2025-12-27)
5. [x] FFI proof-of-concept with Swift - integrate into DashTerm2 app (Worker #1505-1507)
   - DTermCore.swift wrapper provides full Swift API
   - DTermCoreIntegration.swift integrates with PTYSession
   - PTYSession.m feeds PTY data to dterm-core in parallel with iTerm2 parser
   - Advanced setting `dtermCoreEnabled` controls activation (default: NO)
   - All 4662 tests pass including DTermCoreComparisonTests
6. [x] Benchmark against current implementation (Worker #1508 - 2025-12-29)
   - dterm-core performance: 140-166 MB/s for realistic workloads
   - ASCII: 156 MB/s, Escape sequences: 166 MB/s, Wide chars: 151 MB/s
   - Single line latency: <100µs
7. [x] Enable side-by-side comparison mode for validation (Worker #1509 - 2025-12-29)
   - New advanced setting `dtermCoreValidationEnabled` enables real-time validation
   - `DTermCoreIntegration.swift` has `ValidationResult`, `validateCursor()`, `validateCell()`, `quickValidate()` methods
   - `PTYSession.m` performs periodic validation every 100 display updates
   - Samples cursor position and grid of cells (every 4th row, every 8th column)
   - Logs discrepancies to console with cell-by-cell comparison details
   - All 4662 tests pass
8. [x] Identify first component to replace (parser vs grid vs rendering) (Worker #1596 - 2025-12-29)
   - Decision: **Parser** is first component to replace
   - DTermCoreParserAdapter already converts dterm-core events → VT100Token
   - 4754 tests pass, including parser comparison tests
   - Next: Wire adapter into VT100Terminal to use dterm-core tokens in production

### Component Replacement Analysis

| Component | Complexity | Risk | Dependencies | Recommendation |
|-----------|------------|------|--------------|----------------|
| **Parser** | Low | Low | VT100Parser → VT100Token | **Start here** |
| **Grid** | Medium | Medium | VT100Grid ↔ Screen, LineBuffer | Second |
| **Rendering** | High | High | Metal shaders, TextDrawing | Last |

**Rationale for starting with Parser:**
- dterm-core already parses escape sequences correctly (validated by tests)
- VT100Parser has clean interface: `putStreamData:` → `addParsedTokensToVector:`
- Parser is isolated - produces tokens, doesn't modify screen directly
- Easy to compare: dterm-core output vs VT100Token output
- Low risk: can run both parsers, compare results, fall back to iTerm2 if mismatch

**Parser Replacement Strategy:**
1. [x] Create adapter: dterm-core events → VT100Token objects (DTermCoreParserAdapter.swift)
2. [x] Run both parsers on same input (DTermCoreComparisonTests.swift)
3. [x] Compare token streams (DTermCoreParserAdapter.compareTokens())
4. [x] When confident, switch to dterm-core parser only - **DONE** (Worker #1597)
5. [ ] Remove VT100Parser.m (after extended validation)

### Step 9: Wire DTermCoreParserAdapter into VT100Terminal - COMPLETE

**Goal:** Make dterm-core parser the default parser with fallback.

**Status:** COMPLETE (Worker #1597 - 2025-12-29)

**Implementation:**
- `dtermCoreParserOutputEnabled` advanced setting controls parser switch
- VT100ScreenMutableState.m modified to use dterm-core tokens when enabled
- Both parsers still run for comparison/fallback
- Enhanced token comparison logging (type, CSI params, ASCII content)
- 4754 tests pass

**Tasks:**
1. [x] Advanced setting exists: `dtermCoreParserOutputEnabled` (default: NO)
2. [x] Modified VT100ScreenMutableState to use DTermCoreParserAdapter tokens
3. [x] Enhanced token comparison logging with CSI param and ASCII content checks
4. [ ] Measure performance delta (dterm-core should be faster)
5. [ ] Run vttest suite to verify compatibility
6. [ ] Enable by default after sufficient validation

### Step 10: Performance Validation and vttest

**Goal:** Validate dterm-core parser is faster and compatible.

**Status:** IN PROGRESS (Worker #1598 - 2025-12-29)

**Tasks:**
1. [x] Benchmark iTerm2 parser vs dterm-core parser on realistic workloads
2. [x] Run vttest-style validation tests (DTermCoreComparisonTests, DTermCoreParserAdapterTests)
3. [ ] Run interactive vttest suite for full compatibility verification
4. [ ] Document any incompatibilities found
5. [ ] Fix incompatibilities in dterm-core or adapter
6. [ ] Consider enabling by default when validation passes

#### Benchmark Results (2025-12-29)

**Parser Throughput Comparison:**

| Workload | dterm-core | VT100Parser | Speedup |
|----------|------------|-------------|---------|
| ASCII | 138-147 MB/s | 304-326 MB/s | **0.45x** |
| SGR Escapes | 157-162 MB/s | 18-20 MB/s | **7.94-8.74x** |
| Cursor Movement | 69-74 MB/s | 14-17 MB/s | **4.34-4.76x** |
| Erase/Edit | 34-46 MB/s | 17-18 MB/s | **1.91-2.71x** |
| OSC Sequences | 67-74 MB/s | 35-37 MB/s | **1.85-2.02x** |
| Hyperlinks | 108-117 MB/s | 32-33 MB/s | **3.34-3.53x** |
| **Realistic** | **108-135 MB/s** | **47-50 MB/s** | **2.19-2.89x** |

**Single Line Latency (Interactive Feel):**
- dterm-core: 0.22 µs
- VT100Parser: 0.34 µs
- Speedup: **1.57x faster**

**Key Findings:**
1. dterm-core is **2-3x faster** for realistic terminal workloads (mixed text + escapes)
2. dterm-core is **4-9x faster** for escape-heavy sequences (SGR, cursor movement)
3. VT100Parser has superior ASCII-only throughput (SIMD-optimized fast path)
4. Both parsers have excellent single-line latency (<1µs)

**ASCII Performance Note:**
VT100Parser's ASCII fast path uses NEON/SSE2 SIMD instructions for bulk scanning.
dterm-core would benefit from similar SIMD optimization for ASCII runs. This is a
potential improvement for Phase 2 optimization work.

#### Test Coverage

**DTermCoreComparisonTests:** 190 tests pass (33 skipped under ASAN)
- Basic character output, cursor positioning
- VT100/xterm escape sequences (CSI, ESC, OSC)
- SGR attributes (bold, underline, colors, true color)
- Scrolling and scrollback buffer
- Alternate screen buffer
- Window title (OSC sequences)
- Wide characters (CJK)
- Tab stops

**DTermCoreParserAdapterTests:** 40+ tests pass
- Control characters (CR, LF, BS, TAB, BEL)
- CSI sequences (cursor movement, erase, SGR)
- ESC sequences (save/restore cursor, index)
- Parser comparison (dterm-core vs VT100Parser)
- Unicode and multi-byte UTF-8

**DTermCoreVsITerm2BenchmarkTests:** 8 benchmark tests pass
- ASCII, SGR, Cursor, Erase, OSC, Hyperlink, Realistic workloads
- Single line latency measurements
