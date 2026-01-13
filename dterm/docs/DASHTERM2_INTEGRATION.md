# dterm-core Integration into dashterm2

**Phase 7: Integration Guide**

This document describes how to integrate `dterm-core` (formally verified Rust terminal core) into `dashterm2` (iTerm2 fork).

---

## Overview

dterm-core provides:
- **Parser**: VT100/xterm escape sequence parsing (~400 MB/s vs iTerm2's ~60 MB/s)
- **Grid**: Terminal grid with 12-byte cells (vs iTerm2's 16 bytes)
- **Scrollback**: Tiered storage (hot/warm/cold) with memory budget
- **Search**: O(1) trigram index (vs iTerm2's O(n) linear scan)
- **Checkpoints**: Crash recovery via periodic snapshots

---

## Daemon/Library Mode (Instant Spawn)

dterm-core is a library. Instant spawn is a UI-layer integration pattern that
keeps shared caches warm and avoids per-window cold starts.

**Recommended pattern:**
- Start a background "core service" at app launch or first window open.
- Load fonts and initialize shared caches (glyph atlas, search index, config).
- New windows connect over IPC (XPC on macOS, named pipes on Windows, Unix
  sockets on Linux).
- Each window gets its own terminal session; shared caches live in the daemon.

**When to use:**
- Large font sets or heavy glyph caching.
- Many windows/tabs opened frequently.
- You want "instant open" behavior similar to footserver.

If you do not need instant spawn, keep dterm-core in-process and skip this.

---

## Quick Start

### 1. Build the Library

```bash
# In ~/dterm/
cargo build --release -p dterm-core --features ffi

# Output files:
# - target/release/libdterm_core.a (static library, ~19 MB)
# - target/release/libdterm_core.dylib (dynamic library, ~750 KB)
# - crates/dterm-core/include/dterm.h (C header)
```

### 2. Copy to dashterm2

```bash
# Copy library and header
cp ~/dterm/target/release/libdterm_core.a ~/dashterm2/lib/
cp ~/dterm/crates/dterm-core/include/dterm.h ~/dashterm2/include/

# Or use symlinks for development
ln -sf ~/dterm/target/release/libdterm_core.a ~/dashterm2/lib/
ln -sf ~/dterm/crates/dterm-core/include/dterm.h ~/dashterm2/include/
```

### 3. Update Xcode Project

Add to build settings:
```
HEADER_SEARCH_PATHS = $(inherited) $(PROJECT_DIR)/include
LIBRARY_SEARCH_PATHS = $(inherited) $(PROJECT_DIR)/lib
OTHER_LDFLAGS = $(inherited) -ldterm_core
```

---

## C API Reference

### Terminal (High-Level)

The recommended API for integration. Combines parser, grid, and scrollback.

```c
#include "dterm.h"

// Create terminal (80 cols, 24 rows)
dterm_terminal_t* term = dterm_terminal_new(24, 80);

// Create with custom scrollback
dterm_terminal_t* term = dterm_terminal_new_with_scrollback(
    24,              // rows
    80,              // cols
    10000,           // ring_buffer_size
    1000,            // hot_limit
    10000,           // warm_limit
    100 * 1024 * 1024 // memory_budget (100 MB)
);

// Process PTY output
dterm_terminal_process(term, data, len);

// Get cursor position
uint16_t row = dterm_terminal_cursor_row(term);
uint16_t col = dterm_terminal_cursor_col(term);

// Get cell for rendering
dterm_cell_t cell;
if (dterm_terminal_get_cell(term, row, col, &cell)) {
    uint32_t codepoint = cell.codepoint;
    uint32_t fg = cell.fg;
    uint32_t bg = cell.bg;
    uint16_t flags = cell.flags;
}

// Check modes
dterm_modes_t modes;
dterm_terminal_get_modes(term, &modes);
bool cursor_visible = modes.cursor_visible;
bool alternate_screen = modes.alternate_screen;

// Scrolling
dterm_terminal_scroll_display(term, -10); // Scroll up 10 lines
dterm_terminal_scroll_to_top(term);
dterm_terminal_scroll_to_bottom(term);

// Resize
dterm_terminal_resize(term, new_rows, new_cols);

// Cleanup
dterm_terminal_free(term);
```

### Cell Flags

```c
// Cell flag bits (from dterm.h)
#define CELL_FLAG_BOLD        (1 << 0)
#define CELL_FLAG_DIM         (1 << 1)
#define CELL_FLAG_ITALIC      (1 << 2)
#define CELL_FLAG_UNDERLINE   (1 << 3)
#define CELL_FLAG_BLINK       (1 << 4)
#define CELL_FLAG_INVERSE     (1 << 5)
#define CELL_FLAG_INVISIBLE   (1 << 6)
#define CELL_FLAG_STRIKETHROUGH (1 << 7)
#define CELL_FLAG_WIDE        (1 << 8)
#define CELL_FLAG_WIDE_SPACER (1 << 9)
```

### Color Encoding

```c
// Colors are packed as 32-bit values:
// - Default: 0x00000000
// - Indexed (0-255): 0x01RRGGBB where palette[index] = RGB
// - RGB: 0x02RRGGBB (true color)

uint32_t fg = cell.fg;
if (fg == 0) {
    // Use default foreground
} else if ((fg >> 24) == 0x01) {
    // Indexed color (use palette)
    uint8_t index = fg & 0xFF;
} else if ((fg >> 24) == 0x02) {
    // True color
    uint8_t r = (fg >> 16) & 0xFF;
    uint8_t g = (fg >> 8) & 0xFF;
    uint8_t b = fg & 0xFF;
}
```

---

## Swift Bridge

Create `DTermBridge.swift` in dashterm2:

```swift
import Foundation

/// Swift wrapper for dterm-core terminal emulation
final class DTermCore {
    private var terminal: OpaquePointer?

    /// Create a new terminal
    init(rows: UInt16, cols: UInt16) {
        terminal = dterm_terminal_new(rows, cols)
    }

    /// Create with custom scrollback settings
    init(rows: UInt16, cols: UInt16, scrollback: ScrollbackConfig) {
        terminal = dterm_terminal_new_with_scrollback(
            rows,
            cols,
            scrollback.ringBufferSize,
            scrollback.hotLimit,
            scrollback.warmLimit,
            scrollback.memoryBudget
        )
    }

    deinit {
        if let terminal = terminal {
            dterm_terminal_free(terminal)
        }
    }

    /// Process PTY output data
    func process(_ data: Data) {
        guard let terminal = terminal else { return }
        data.withUnsafeBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return }
            dterm_terminal_process(
                terminal,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                ptr.count
            )
        }
    }

    /// Get terminal dimensions
    var rows: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_rows(terminal)
    }

    var cols: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cols(terminal)
    }

    /// Get cursor position
    var cursorRow: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cursor_row(terminal)
    }

    var cursorCol: UInt16 {
        guard let terminal = terminal else { return 0 }
        return dterm_terminal_cursor_col(terminal)
    }

    var cursorVisible: Bool {
        guard let terminal = terminal else { return true }
        return dterm_terminal_cursor_visible(terminal)
    }

    /// Get cell at position
    func cell(at row: UInt16, col: UInt16) -> DTermCell? {
        guard let terminal = terminal else { return nil }
        var cell = dterm_cell_t()
        if dterm_terminal_get_cell(terminal, row, col, &cell) {
            return DTermCell(cell)
        }
        return nil
    }

    /// Get terminal modes
    var modes: DTermModes {
        guard let terminal = terminal else {
            return DTermModes()
        }
        var modes = dterm_modes_t()
        dterm_terminal_get_modes(terminal, &modes)
        return DTermModes(modes)
    }

    /// Get window title
    var title: String? {
        guard let terminal = terminal,
              let cStr = dterm_terminal_title(terminal) else {
            return nil
        }
        return String(cString: cStr)
    }

    /// Check if alternate screen is active
    var isAlternateScreen: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_is_alternate_screen(terminal)
    }

    /// Resize terminal
    func resize(rows: UInt16, cols: UInt16) {
        guard let terminal = terminal else { return }
        dterm_terminal_resize(terminal, rows, cols)
    }

    /// Reset terminal
    func reset() {
        guard let terminal = terminal else { return }
        dterm_terminal_reset(terminal)
    }

    /// Scrolling
    func scroll(lines: Int32) {
        guard let terminal = terminal else { return }
        dterm_terminal_scroll_display(terminal, lines)
    }

    func scrollToTop() {
        guard let terminal = terminal else { return }
        dterm_terminal_scroll_to_top(terminal)
    }

    func scrollToBottom() {
        guard let terminal = terminal else { return }
        dterm_terminal_scroll_to_bottom(terminal)
    }

    /// Scrollback info
    var scrollbackLines: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_scrollback_lines(terminal))
    }

    var displayOffset: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_display_offset(terminal))
    }

    /// Check if redraw needed
    var needsRedraw: Bool {
        guard let terminal = terminal else { return false }
        return dterm_terminal_needs_redraw(terminal)
    }

    /// Clear damage tracking after render
    func clearDamage() {
        guard let terminal = terminal else { return }
        dterm_terminal_clear_damage(terminal)
    }

    // MARK: - Shell Integration (OSC 133)

    /// Get the current shell integration state
    var shellState: DTermShellState {
        guard let terminal = terminal else { return .ground }
        return DTermShellState(dterm_terminal_shell_state(terminal))
    }

    /// Get the number of completed output blocks
    var blockCount: Int {
        guard let terminal = terminal else { return 0 }
        return Int(dterm_terminal_block_count(terminal))
    }

    /// Get an output block by index
    func block(at index: Int) -> DTermOutputBlock? {
        guard let terminal = terminal else { return nil }
        var block = DtermOutputBlock()
        if dterm_terminal_get_block(terminal, index, &block) {
            return DTermOutputBlock(block)
        }
        return nil
    }

    /// Get the current (in-progress) output block
    var currentBlock: DTermOutputBlock? {
        guard let terminal = terminal else { return nil }
        var block = DtermOutputBlock()
        if dterm_terminal_get_current_block(terminal, &block) {
            return DTermOutputBlock(block)
        }
        return nil
    }

    /// Find the output block containing a given row
    func blockIndex(atRow row: Int) -> Int? {
        guard let terminal = terminal else { return nil }
        let index = dterm_terminal_block_at_row(terminal, row)
        return index == UInt.max ? nil : Int(index)
    }

    /// Get the exit code of the last completed block
    var lastExitCode: Int32? {
        guard let terminal = terminal else { return nil }
        var exitCode: Int32 = 0
        if dterm_terminal_last_exit_code(terminal, &exitCode) {
            return exitCode
        }
        return nil
    }
}

// MARK: - Shell Integration Types

/// Shell integration state (OSC 133)
enum DTermShellState {
    /// Ground state - waiting for prompt
    case ground
    /// Receiving prompt text (after OSC 133 ; A)
    case receivingPrompt
    /// User is entering command (after OSC 133 ; B)
    case enteringCommand
    /// Command is executing (after OSC 133 ; C)
    case executing

    init(_ state: DtermShellState) {
        switch state {
        case GROUND:
            self = .ground
        case RECEIVING_PROMPT:
            self = .receivingPrompt
        case ENTERING_COMMAND:
            self = .enteringCommand
        case EXECUTING:
            self = .executing
        default:
            self = .ground
        }
    }
}

/// Output block state
enum DTermBlockState {
    /// Only prompt has been received
    case promptOnly
    /// User is entering a command
    case enteringCommand
    /// Command is executing
    case executing
    /// Command has completed with exit code
    case complete

    init(_ state: DtermBlockState) {
        switch state {
        case PROMPT_ONLY:
            self = .promptOnly
        case ENTERING_COMMAND:
            self = .enteringCommand
        case EXECUTING:
            self = .executing
        case COMPLETE:
            self = .complete
        default:
            self = .promptOnly
        }
    }
}

/// An output block representing a command and its output
struct DTermOutputBlock {
    /// Unique identifier for this block
    let id: UInt64
    /// Current state of this block
    let state: DTermBlockState
    /// Row where the prompt started (absolute line number)
    let promptStartRow: Int
    /// Column where the prompt started
    let promptStartCol: UInt16
    /// Row where the command text started (nil if not set)
    let commandStartRow: Int?
    /// Column where the command text started (nil if not set)
    let commandStartCol: UInt16?
    /// Row where command output started (nil if not set)
    let outputStartRow: Int?
    /// Row where this block ends (exclusive, nil if not set)
    let endRow: Int?
    /// Command exit code (nil if not complete or unknown)
    let exitCode: Int32?

    init(_ block: DtermOutputBlock) {
        self.id = block.id
        self.state = DTermBlockState(block.state)
        self.promptStartRow = Int(block.prompt_start_row)
        self.promptStartCol = block.prompt_start_col
        self.commandStartRow = block.has_command_start ? Int(block.command_start_row) : nil
        self.commandStartCol = block.has_command_start ? block.command_start_col : nil
        self.outputStartRow = block.has_output_start ? Int(block.output_start_row) : nil
        self.endRow = block.has_end_row ? Int(block.end_row) : nil
        self.exitCode = block.has_exit_code ? block.exit_code : nil
    }
}

/// Scrollback configuration
struct ScrollbackConfig {
    var ringBufferSize: Int
    var hotLimit: Int
    var warmLimit: Int
    var memoryBudget: Int

    static var `default`: ScrollbackConfig {
        ScrollbackConfig(
            ringBufferSize: 10000,
            hotLimit: 1000,
            warmLimit: 10000,
            memoryBudget: 100 * 1024 * 1024 // 100 MB
        )
    }
}

/// Cell data
struct DTermCell {
    let codepoint: UnicodeScalar?
    let foreground: DTermColor
    let background: DTermColor
    let flags: CellFlags

    init(_ cell: dterm_cell_t) {
        if cell.codepoint > 0 {
            self.codepoint = UnicodeScalar(cell.codepoint)
        } else {
            self.codepoint = nil
        }
        self.foreground = DTermColor(packed: cell.fg)
        self.background = DTermColor(packed: cell.bg)
        self.flags = CellFlags(rawValue: cell.flags)
    }

    var character: Character? {
        guard let scalar = codepoint else { return nil }
        return Character(scalar)
    }
}

/// Cell flags
struct CellFlags: OptionSet {
    let rawValue: UInt16

    static let bold = CellFlags(rawValue: 1 << 0)
    static let dim = CellFlags(rawValue: 1 << 1)
    static let italic = CellFlags(rawValue: 1 << 2)
    static let underline = CellFlags(rawValue: 1 << 3)
    static let blink = CellFlags(rawValue: 1 << 4)
    static let inverse = CellFlags(rawValue: 1 << 5)
    static let invisible = CellFlags(rawValue: 1 << 6)
    static let strikethrough = CellFlags(rawValue: 1 << 7)
    static let wide = CellFlags(rawValue: 1 << 8)
    static let wideSpacer = CellFlags(rawValue: 1 << 9)
}

/// Color representation
enum DTermColor {
    case `default`
    case indexed(UInt8)
    case rgb(r: UInt8, g: UInt8, b: UInt8)

    init(packed: UInt32) {
        if packed == 0 {
            self = .default
        } else {
            let type = packed >> 24
            if type == 0x01 {
                self = .indexed(UInt8(packed & 0xFF))
            } else {
                self = .rgb(
                    r: UInt8((packed >> 16) & 0xFF),
                    g: UInt8((packed >> 8) & 0xFF),
                    b: UInt8(packed & 0xFF)
                )
            }
        }
    }
}

/// Terminal modes
struct DTermModes {
    var cursorVisible: Bool = true
    var applicationCursorKeys: Bool = false
    var alternateScreen: Bool = false
    var autoWrap: Bool = true
    var originMode: Bool = false
    var insertMode: Bool = false
    var bracketedPaste: Bool = false

    init() {}

    init(_ modes: dterm_modes_t) {
        self.cursorVisible = modes.cursor_visible
        self.applicationCursorKeys = modes.application_cursor_keys
        self.alternateScreen = modes.alternate_screen
        self.autoWrap = modes.auto_wrap
        self.originMode = modes.origin_mode
        self.insertMode = modes.insert_mode
        self.bracketedPaste = modes.bracketed_paste
    }
}
```

---

## Integration Strategy

### Phase 7.1: Replace Parser

1. Create `DTermBridge.swift` (above)
2. In `VT100Terminal.m`, add option to use dterm-core parser
3. Run comparison tests:
   - Feed same input to both parsers
   - Verify actions match exactly

```objc
// VT100Terminal.m (hybrid mode for testing)
@interface VT100Terminal ()
@property (nonatomic, strong) DTermCore *dtermCore;
@property (nonatomic) BOOL useDTermParser;
@end

- (void)processData:(NSData *)data {
    if (self.useDTermParser && self.dtermCore) {
        [self.dtermCore process:data];
        // Sync state from dterm-core to iTerm2 structures
    } else {
        // Original iTerm2 parser
    }
}
```

### Phase 7.2: Replace Grid

**Goal:** Replace dashterm2's `VT100Grid` with dterm-core grid for rendering while maintaining compatibility.

#### Step 1: Create DTermGridAdapter

Create an adapter that provides iTerm2-compatible cell access using dterm-core:

```swift
// DTermCore/DTermGridAdapter.swift
import Foundation

/// Adapter that provides VT100Grid-compatible interface using dterm-core
class DTermGridAdapter {
    private var bridge: DTermCoreBridge

    init(bridge: DTermCoreBridge) {
        self.bridge = bridge
    }

    /// Get cell at display coordinates (row relative to scroll position)
    func cell(atRow row: Int32, col: Int32) -> DTermCell? {
        var cell = dterm_cell_t()
        guard dterm_terminal_get_cell(bridge.terminal, UInt16(row), UInt16(col), &cell) else {
            return nil
        }
        return DTermCell(cell)
    }

    /// Get cursor position
    var cursorRow: Int32 {
        Int32(dterm_terminal_cursor_row(bridge.terminal))
    }

    var cursorCol: Int32 {
        Int32(dterm_terminal_cursor_col(bridge.terminal))
    }

    /// Grid dimensions
    var rows: Int32 { Int32(dterm_terminal_rows(bridge.terminal)) }
    var cols: Int32 { Int32(dterm_terminal_cols(bridge.terminal)) }

    /// Scrollback info
    var scrollbackLines: Int { dterm_terminal_scrollback_lines(bridge.terminal) }
    var displayOffset: Int { dterm_terminal_display_offset(bridge.terminal) }

    /// Damage tracking
    var needsRedraw: Bool { dterm_terminal_needs_redraw(bridge.terminal) }
    func clearDamage() { dterm_terminal_clear_damage(bridge.terminal) }
}
```

#### Step 2: Add Grid Toggle to Advanced Settings

```swift
// iTermAdvancedSettingsModel.m
+ (BOOL)dtermCoreGridEnabled {
    return [[NSUserDefaults standardUserDefaults] boolForKey:@"DTermCoreGridEnabled"];
}
```

#### Step 3: Wire into PTYTextView Rendering

Modify `PTYTextView`'s Metal renderer to optionally read from dterm-core:

```objc
// PTYTextView.m
- (void)drawRect:(NSRect)dirtyRect {
    if ([iTermAdvancedSettingsModel dtermCoreGridEnabled] && self.dtermGridAdapter) {
        [self drawWithDTermGrid:dirtyRect];
    } else {
        [self drawWithVT100Grid:dirtyRect];
    }
}

- (void)drawWithDTermGrid:(NSRect)dirtyRect {
    // Read cells from dterm-core grid
    for (int row = 0; row < self.rows; row++) {
        for (int col = 0; col < self.cols; col++) {
            DTermCell *cell = [self.dtermGridAdapter cellAtRow:row col:col];
            if (cell) {
                [self renderCell:cell atRow:row col:col];
            }
        }
    }
}
```

#### Step 4: Synchronize Scroll State

```swift
// DTermCore/DTermScrollSync.swift
class DTermScrollSync {
    func scrollDisplay(delta: Int32) {
        dterm_terminal_scroll_display(bridge.terminal, delta)
    }

    func scrollToTop() {
        dterm_terminal_scroll_to_top(bridge.terminal)
    }

    func scrollToBottom() {
        dterm_terminal_scroll_to_bottom(bridge.terminal)
    }

    func syncScrollPosition(from scrollView: NSScrollView) {
        // Calculate delta from scroll view position
        let delta = calculateDelta(scrollView.documentVisibleRect)
        scrollDisplay(delta: delta)
    }
}
```

#### Step 5: Add Comparison Testing

```swift
// DTermCoreGridComparisonTests.swift
func testGridEquivalence() {
    let input = loadVTTestOutput()

    // Process through both
    iterm2Screen.execute(input)
    dtermBridge.process(input)

    // Compare every cell
    for row in 0..<rows {
        for col in 0..<cols {
            let iterm2Cell = iterm2Screen.cell(at: row, col: col)
            let dtermCell = dtermGridAdapter.cell(atRow: Int32(row), col: Int32(col))

            XCTAssertEqual(iterm2Cell.code, dtermCell?.codepoint)
            XCTAssertEqual(iterm2Cell.foregroundColor, dtermCell?.fg)
            XCTAssertEqual(iterm2Cell.backgroundColor, dtermCell?.bg)
        }
    }
}
```

#### Step 6: Memory Benchmarks

```swift
// DTermCoreGridBenchmarks.swift
func benchmarkGridMemory() {
    measure {
        // Fill 10K scrollback with random content
        for _ in 0..<10000 {
            dtermBridge.process(randomLine())
        }

        let dtermMemory = Process.processInfo.physicalFootprint
        print("dterm-core memory: \(dtermMemory / 1024 / 1024) MB")
    }
}
```

#### Phase 7.2 Completion Criteria

- [ ] DTermGridAdapter provides all cell data needed for rendering
- [ ] Metal renderer can read from dterm-core grid when enabled
- [ ] Grid comparison tests pass (100% cell equivalence)
- [ ] Scroll position synchronized between views
- [ ] Memory usage reduced (target: 12 bytes/cell vs 16 bytes/cell)
- [ ] No visual regression in terminal rendering

### Phase 7.3: Replace Scrollback

1. Replace `LineBuffer` with dterm-core scrollback
2. Update scroll handling to use `dterm_terminal_scroll_display()`
3. Verify 1M+ lines works without OOM

---

## Testing

### Comparison Testing

```swift
// ComparisonTest.swift
func testParserEquivalence() {
    let input = loadTestInput("vttest_output.bin")

    let iterm2 = VT100Terminal()
    let dterm = DTermCore(rows: 24, cols: 80)

    // Feed same input
    iterm2.process(input)
    dterm.process(input)

    // Compare every cell
    for row in 0..<24 {
        for col in 0..<80 {
            let expected = iterm2.cell(at: row, col: col)
            let actual = dterm.cell(at: UInt16(row), col: UInt16(col))
            XCTAssertEqual(expected.character, actual?.character)
        }
    }
}
```

### Performance Testing

```bash
# Run parser benchmark
~/dterm/target/release/parser_bench

# Expected output:
# parse_vttest: 450 MB/s
# parse_random: 380 MB/s
```

---

## Files to Create in dashterm2

```
dashterm2/
├── DTermCore/
│   ├── DTermBridge.swift       # Swift wrapper (above)
│   └── DTermBridge-Bridging.h  # Bridging header
├── include/
│   └── dterm.h                 # C header (from dterm-core)
├── lib/
│   └── libdterm_core.a         # Static library (from dterm-core)
└── DashTerm2.xcodeproj/        # Update build settings
```

### Bridging Header

```c
// DTermBridge-Bridging.h
#ifndef DTermBridge_Bridging_h
#define DTermBridge_Bridging_h

#include "dterm.h"

#endif
```

---

## Performance Targets

| Metric | iTerm2 Baseline | dterm-core Target | Status |
|--------|-----------------|-------------------|--------|
| Parse throughput | ~60 MB/s | 400+ MB/s | Ready |
| Cell size | 16 bytes | 12 bytes | Ready |
| Memory (100K lines) | ~50 MB | ~5 MB | Ready |
| Memory (1M lines) | ~500 MB | ~50 MB | Ready |
| Search (1M lines) | ~500 ms | <10 ms | Ready |

---

## Troubleshooting

### Library not found

```
ld: library not found for -ldterm_core
```

Check:
1. `lib/libdterm_core.a` exists
2. `LIBRARY_SEARCH_PATHS` includes `$(PROJECT_DIR)/lib`

### Symbol not found

```
Undefined symbols for architecture arm64: "_dterm_terminal_new"
```

Check:
1. Library built with `--features ffi`
2. Header matches library version

### Architecture mismatch

```
ld: building for macOS-arm64 but attempting to link with file built for macOS-x86_64
```

Rebuild with:
```bash
cargo build --release -p dterm-core --features ffi --target aarch64-apple-darwin
```

---

## Next Steps

1. Create `DTermCore/` directory in dashterm2
2. Copy library and header
3. Add `DTermBridge.swift`
4. Update Xcode project settings
5. Start with parser integration (Phase 7.1)
6. Run comparison tests
7. Benchmark performance
8. Proceed to grid/scrollback (Phase 7.2, 7.3)

---

## References

- dterm FFI Header: `~/dterm/crates/dterm-core/include/dterm.h`
- dterm Strategy: `~/dterm/docs/STRATEGY.md`
- dterm Roadmap: `~/dterm/docs/ROADMAP.md`
- dashterm2 Roadmap: `~/dashterm2/ROADMAP.md`
