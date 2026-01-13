# DTermCore

High-performance terminal emulation core, written in Rust.

## Source

Built from: https://github.com/dropbox/dterm

## Contents

```
DTermCore/
├── DTermCore.swift           # Swift bindings
├── DTermCore-Bridging.h      # Bridging header for Swift
├── include/
│   └── dterm.h               # C header
└── lib/
    └── libdterm_core.a       # Static library (arm64)
```

## Integration

### 1. Add to Xcode Project

Drag `DTermCore/` folder into your Xcode project.

### 2. Configure Build Settings

```
HEADER_SEARCH_PATHS = $(inherited) $(PROJECT_DIR)/DTermCore/include
LIBRARY_SEARCH_PATHS = $(inherited) $(PROJECT_DIR)/DTermCore/lib
OTHER_LDFLAGS = $(inherited) -ldterm_core
```

### 3. Set Bridging Header

In Build Settings > Swift Compiler - General > Objective-C Bridging Header:

```
$(PROJECT_DIR)/DTermCore/DTermCore-Bridging.h
```

Or add to existing bridging header:

```c
#include "DTermCore/include/dterm.h"
```

## Usage

```swift
import Foundation

// Create terminal
let terminal = DTermCore(rows: 24, cols: 80)

// Process PTY output
terminal.process(ptyData)

// Read cells for rendering
for row in 0..<terminal.rows {
    for col in 0..<terminal.cols {
        if let cell = terminal.cell(at: row, col: col) {
            let char = cell.character ?? " "
            let fg = cell.foreground
            let bg = cell.background
            let bold = cell.flags.contains(.bold)
            // Render...
        }
    }
}

// Handle scrolling
terminal.scroll(lines: -10)  // Scroll up
terminal.scrollToBottom()     // Return to live content

// Check modes
if terminal.modes.bracketedPaste {
    // Send bracketed paste sequences
}

// Get window title
if let title = terminal.title {
    window.title = title
}
```

## Rebuilding

To update the library from source:

```bash
cd ~/dterm
cargo build --release -p dterm-core --features ffi
cp target/release/libdterm_core.a ~/dashterm2/DTermCore/lib/
cp crates/dterm-core/include/dterm.h ~/dashterm2/DTermCore/include/
```

## Performance

| Metric | iTerm2 | dterm-core |
|--------|--------|------------|
| Parse throughput | ~60 MB/s | ~400 MB/s |
| Cell size | 16 bytes | 12 bytes |
| Memory (100K lines) | ~50 MB | ~5 MB |
| Search (1M lines) | ~500 ms | <10 ms |

## API Reference

See `dterm.h` for the full C API, or use the Swift wrapper `DTermCore.swift`.

### Key Types

- `DTermCore`: Main terminal class
- `DTermCell`: Cell data (codepoint, colors, flags)
- `DTermColor`: Color (default, indexed, RGB)
- `CellFlags`: Attributes (bold, italic, etc.)
- `DTermModes`: Terminal modes (DECCKM, DECTCEM, etc.)
- `ScrollbackConfig`: Scrollback tier configuration

### Thread Safety

`DTermCore` is NOT thread-safe. Use external synchronization if accessing from multiple threads.

## License

Apache 2.0 (dterm-core) - see ~/dterm/LICENSE
