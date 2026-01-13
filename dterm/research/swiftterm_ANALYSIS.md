# SwiftTerm Analysis

**Repository:** https://github.com/migueldeicaza/SwiftTerm
**License:** MIT
**Language:** Swift
**LOC:** ~11,360 (core engine), ~5,270 (UI layers)
**Author:** Miguel de Icaza
**Platforms:** iOS, macOS, tvOS, watchOS, visionOS, Linux, WebAssembly

---

## Executive Summary

SwiftTerm is the **best candidate for dterm-core integration on iOS/iPadOS**. It is:
- MIT licensed (Apache 2.0 compatible)
- Explicitly designed as a "reusable and pluggable engine"
- Native Swift with clean separation between engine and UI
- Actively maintained by a credible author (Miguel de Icaza)
- Used in production apps (Secure Shellfish, La Terminal, CodeEdit)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    SwiftTerm Package                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              CORE ENGINE (Platform-agnostic)            │    │
│  │                                                         │    │
│  │  Terminal.swift (5,477 lines)                          │    │
│  │    └─> State machine, escape sequences, modes          │    │
│  │                                                         │    │
│  │  Buffer.swift (1,160 lines)                            │    │
│  │    └─> Grid storage, scrollback, cursor                │    │
│  │                                                         │    │
│  │  EscapeSequenceParser.swift (657 lines)                │    │
│  │    └─> VT500 table-driven parser                       │    │
│  │                                                         │    │
│  │  SelectionService.swift (542 lines)                    │    │
│  │    └─> Selection handling                              │    │
│  │                                                         │    │
│  │  + CharData, Colors, CharSets, Sixel, etc.             │    │
│  └─────────────────────────────────────────────────────────┘    │
│                           │                                      │
│                           │ TerminalDelegate protocol            │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              UI LAYER (Platform-specific)               │    │
│  │                                                         │    │
│  │  Apple/AppleTerminalView.swift (1,425 lines)           │    │
│  │    └─> Shared macOS/iOS rendering with CoreText        │    │
│  │                                                         │    │
│  │  iOS/iOSTerminalView.swift (1,503 lines)               │    │
│  │    └─> UIKit UIView implementation                     │    │
│  │                                                         │    │
│  │  iOS/SwiftUITerminalView.swift (112 lines)             │    │
│  │    └─> SwiftUI wrapper                                 │    │
│  │                                                         │    │
│  │  Mac/ (conditionally compiled, similar structure)       │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Key Components

### 1. Terminal Engine (`Terminal.swift`)

The core state machine handling:
- Escape sequence interpretation
- Terminal modes (cursor, mouse, bracketed paste, etc.)
- Character set handling (G0-G3)
- Color management (ANSI, 256, TrueColor)
- Window manipulation commands
- OSC handling (titles, hyperlinks, clipboard, etc.)

**Key Integration Point:** `TerminalDelegate` protocol

```swift
public protocol TerminalDelegate: AnyObject {
    func showCursor(source: Terminal)
    func hideCursor(source: Terminal)
    func setTerminalTitle(source: Terminal, title: String)
    func send(source: Terminal, data: ArraySlice<UInt8>)
    func scrolled(source: Terminal, yDisp: Int)
    func bell(source: Terminal)
    func selectionChanged(source: Terminal)
    func mouseModeChanged(source: Terminal)
    func cursorStyleChanged(source: Terminal, newStyle: CursorStyle)
    func colorChanged(source: Terminal, idx: Int?)
    func clipboardCopy(source: Terminal, content: Data)
    // ... 25+ delegate methods total
}
```

### 2. Buffer Management (`Buffer.swift`)

- `CircularBufferLineList` for scrollback (ring buffer pattern)
- Cursor position tracking (x, y)
- Scroll region management (scrollTop, scrollBottom)
- Saved cursor state
- Tab stops

### 3. Parser (`EscapeSequenceParser.swift`)

- **Table-driven VT500 parser** (similar to DEC ANSI parser reference)
- States: Ground, Escape, CSI, OSC, DCS, etc.
- Actions: Print, Execute, CSI Dispatch, etc.
- `TransitionTable` for state machine

```swift
enum ParserState: UInt8 {
    case ground = 0
    case escape
    case escapeIntermediate
    case csiEntry
    case csiParam
    case csiIntermediate
    case csiIgnore
    case sosPmApcString
    case oscString
    case dcsEntry
    case dcsParam
    case dcsIgnore
    case dcsIntermediate
    case dcsPassthrough
}
```

### 4. UI Layer Separation

The UI is cleanly separated via two protocols:

**TerminalDelegate** (Engine → UI):
- Notifications from terminal engine to UI
- Title changes, bell, selection, cursor style, etc.

**TerminalViewDelegate** (UI → Application):
- Notifications from UI view to application
- Size changes, send data, scroll, open links, etc.

---

## Features

| Feature | Status | Notes |
|---------|--------|-------|
| VT100/VT220/xterm | ✅ | Comprehensive |
| Unicode/Emoji | ✅ | Combining characters, grapheme clusters |
| 256 Color | ✅ | Full palette |
| TrueColor | ✅ | 24-bit RGB |
| Sixel Graphics | ✅ | Via `SixelDcsHandler` |
| iTerm2 Images | ✅ | OSC 1337 |
| Hyperlinks | ✅ | OSC 8 |
| Mouse Tracking | ✅ | X10, Normal, Button, Any |
| Selection | ✅ | Via `SelectionService` |
| Search | ✅ | Via `SearchService` |
| Local Process | ✅ | macOS via `LocalProcess.swift` |
| Session Recording | ✅ | Termcast (asciinema format) |

---

## dterm-core Integration Strategy

### Option A: Replace Terminal Engine (Recommended)

Replace `Terminal.swift` and related core files with dterm-core via C FFI:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Modified SwiftTerm                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              dterm-core (Rust via C FFI)                │    │
│  │                                                         │    │
│  │  - Parser (SIMD-accelerated)                           │    │
│  │  - Grid (offset-based pages, memory pooling)           │    │
│  │  - Terminal state machine                              │    │
│  │  - Sixel, Kitty graphics                               │    │
│  │  - OSC 133 shell integration                           │    │
│  │  - All formal verification benefits                    │    │
│  └─────────────────────────────────────────────────────────┘    │
│                           │                                      │
│                           │ Swift wrapper (DTermBridge.swift)    │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              UI LAYER (Keep existing)                   │    │
│  │                                                         │    │
│  │  Apple/AppleTerminalView.swift                         │    │
│  │  iOS/iOSTerminalView.swift                             │    │
│  │  iOS/SwiftUITerminalView.swift                         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Swift Wrapper Example

```swift
// DTermBridge.swift - Swift wrapper for dterm-core FFI

import Foundation

class DTermTerminal {
    private var handle: OpaquePointer

    init(cols: Int, rows: Int) {
        handle = dterm_terminal_new(UInt16(cols), UInt16(rows))
    }

    deinit {
        dterm_terminal_free(handle)
    }

    func process(data: Data) {
        data.withUnsafeBytes { ptr in
            dterm_terminal_process_input(
                handle,
                ptr.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(data.count)
            )
        }
    }

    func getCell(row: Int, col: Int) -> DTermCell {
        var cell = DtermCell()
        dterm_grid_get_cell(gridHandle, Int32(row), Int32(col), &cell)
        return cell
    }

    var cursorX: Int { Int(dterm_terminal_cursor_x(handle)) }
    var cursorY: Int { Int(dterm_terminal_cursor_y(handle)) }
    var cols: Int { Int(dterm_terminal_cols(handle)) }
    var rows: Int { Int(dterm_terminal_rows(handle)) }
}
```

### API Mapping

| SwiftTerm API | dterm-core FFI |
|---------------|----------------|
| `Terminal(delegate:options:)` | `dterm_terminal_new(cols, rows)` |
| `terminal.feed(data:)` | `dterm_terminal_process_input()` |
| `terminal.resize(cols:rows:)` | `dterm_terminal_resize()` |
| `buffer.x`, `buffer.y` | `dterm_terminal_cursor_x/y()` |
| `buffer.getLine(row)` | `dterm_grid_get_cell()` loop |
| `terminal.title` | `dterm_terminal_title()` |
| `terminal.mouseMode` | `dterm_terminal_mouse_mode()` |

---

## Comparison: SwiftTerm vs dterm-core

| Aspect | SwiftTerm | dterm-core |
|--------|-----------|------------|
| **Language** | Swift | Rust |
| **Parser** | Table-driven VT500 | SIMD-accelerated + table |
| **Grid** | CircularBufferLineList | Offset-based pages |
| **Memory** | Standard Swift allocation | Memory pooling, preheating |
| **Verification** | Unit tests, fuzzing | TLA+, Kani proofs, MIRI, fuzzing |
| **Graphics** | Sixel, iTerm2 | Sixel, Kitty, iTerm2 |
| **Shell Integration** | Basic OSC 7 | Full OSC 133 blocks |
| **License** | MIT | Apache 2.0 |

### dterm-core Advantages

1. **Formal Verification**: TLA+ specs, 55+ Kani proofs, MIRI, AddressSanitizer
2. **Performance**: SIMD parser (10x faster on ASCII), memory pooling
3. **Architecture**: Offset-based pages enable serialization, memory mapping
4. **Shell Integration**: Full OSC 133 with command blocks
5. **Cross-Platform**: Same core for macOS, iOS, Windows, Linux

### SwiftTerm Advantages

1. **Native Swift**: No FFI overhead for Swift apps
2. **Mature UI**: Polished iOS/macOS views
3. **Production Proven**: Used in commercial apps
4. **Simpler**: Fewer moving parts

---

## Integration Effort Estimate

| Task | Complexity | Notes |
|------|------------|-------|
| Create Swift FFI wrapper | Medium | ~500 lines |
| Adapt delegate callbacks | Medium | Need callback mechanism from Rust |
| Replace Terminal class | High | Central integration point |
| Keep existing UI layer | Low | Minimal changes |
| Testing | Medium | Verify all escape sequences |

**Total:** 2-3 weeks for experienced developer

---

## Files to Study

### Core Engine (Replace with dterm-core)
```
Sources/SwiftTerm/
├── Terminal.swift          # 5,477 lines - Main state machine
├── Buffer.swift            # 1,160 lines - Grid/scrollback
├── BufferLine.swift        #   299 lines - Row storage
├── EscapeSequenceParser.swift # 657 lines - VT parser
├── SelectionService.swift  #   542 lines - Selection
├── CharData.swift          #   388 lines - Cell data
├── Colors.swift            #   292 lines - Color handling
├── CharSets.swift          #   262 lines - Character sets
├── CircularList.swift      #   434 lines - Ring buffer
└── SixelDcsHandler.swift   #   403 lines - Sixel graphics
```

### UI Layer (Keep)
```
Sources/SwiftTerm/
├── Apple/
│   ├── AppleTerminalView.swift    # 1,425 lines - Shared rendering
│   ├── TerminalViewDelegate.swift #    89 lines - UI delegate
│   ├── CaretView.swift            #    68 lines - Cursor view
│   └── Wcwidth.swift              #   589 lines - Width tables
└── iOS/
    ├── iOSTerminalView.swift      # 1,503 lines - UIKit view
    ├── SwiftUITerminalView.swift  #   112 lines - SwiftUI wrapper
    ├── iOSTextInput.swift         #   392 lines - Keyboard input
    ├── iOSAccessoryView.swift     #   397 lines - Accessory bar
    └── iOSCaretView.swift         #   134 lines - iOS cursor
```

---

## Comparison with Alacritty

For context, `alacritty_terminal` (the Rust crate) has similar structure:

| alacritty_terminal | SwiftTerm | dterm-core |
|-------------------|-----------|------------|
| `Term` struct | `Terminal` class | `Terminal` struct |
| `Grid<Cell>` | `Buffer` + `CircularList` | `Grid` + `PageStore` |
| `vte` crate | `EscapeSequenceParser` | `Parser` |
| `Selection` | `SelectionService` | `Selection` |
| `ViModeCursor` | N/A | `ViModeCursor` |

**Key difference:** Alacritty uses the separate `vte` crate for parsing, while SwiftTerm has integrated parser. dterm-core has integrated parser with SIMD acceleration.

---

## Recommendations

1. **Primary iOS/iPadOS Strategy**: Use SwiftTerm's UI layer with dterm-core engine
2. **Create `dterm-swift` package**: Swift wrapper for dterm-core FFI
3. **Preserve SwiftTerm API**: Make wrapper implement similar interface
4. **Test with SwiftTermApp**: Use the sample app for integration testing

---

## Resources

- **Repository:** https://github.com/migueldeicaza/SwiftTerm
- **API Docs:** https://migueldeicaza.github.io/SwiftTermDocs/documentation/swiftterm/
- **Sample App:** https://github.com/migueldeicaza/SwiftTermApp
- **Commercial Usage:** Secure Shellfish, La Terminal, CodeEdit
