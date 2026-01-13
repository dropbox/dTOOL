# DTermCore Swift Package

Swift bindings for dterm-core, a high-performance terminal emulation library.

## Overview

DTermCore provides Swift bindings for the dterm-core Rust library, enabling high-performance terminal emulation on Apple platforms (iOS, macOS, tvOS, watchOS, visionOS).

## Features

- VT100/VT220/xterm terminal emulation
- Unicode and emoji support
- 256 colors and 24-bit true color
- Mouse tracking (X10, SGR encoding)
- Bracketed paste mode
- OSC 7 (current working directory)
- OSC 8 (hyperlinks)
- OSC 52 (clipboard)
- OSC 133 (shell integration)
- Sixel and Kitty graphics
- Damage tracking for efficient rendering

## Requirements

- Swift 5.9+
- iOS 15+, macOS 12+, tvOS 15+, watchOS 8+, visionOS 1+
- dterm-core static library (see Building section)

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(path: "../dterm/packages/dterm-swift")
]
```

### Building dterm-core

You must build the dterm-core Rust library and link it to your project:

```bash
# Build for macOS
cargo build --release -p dterm-core

# Build for iOS (requires cross-compilation toolchain)
cargo build --release -p dterm-core --target aarch64-apple-ios
```

The static library will be at:
- macOS: `target/release/libdterm_core.a`
- iOS: `target/aarch64-apple-ios/release/libdterm_core.a`

## Usage

```swift
import DTermCore

// Create a terminal
let terminal = DTermTerminal(rows: 24, cols: 80)
terminal.delegate = self

// Process input from PTY
terminal.process(data: inputData)

// Check for response data (DSR, DA, etc.)
// Delegate will be notified via terminalHasResponse(_:data:)

// Render cells
for row in 0..<terminal.rows {
    for col in 0..<terminal.cols {
        if let cell = terminal.getCell(row: row, col: col) {
            // cell.character - the character to render
            // cell.foreground - packed foreground color
            // cell.background - packed background color
            // cell.flags - bold, italic, underline, etc.
        }
    }
}

// Handle mouse events
if terminal.mouseTrackingEnabled {
    if let data = terminal.encodeMousePress(button: 0, col: x, row: y) {
        // Send data to PTY
    }
}
```

## API Reference

### DTermTerminal

Main terminal class.

**Properties:**
- `rows`, `cols` - Terminal dimensions
- `cursorRow`, `cursorCol` - Cursor position
- `cursorVisible` - Whether cursor is visible
- `title` - Window title (OSC 0/2)
- `isAlternateScreen` - Whether alternate screen is active
- `modes` - Current terminal modes
- `shellState` - Shell integration state (OSC 133)
- `currentWorkingDirectory` - From OSC 7
- `scrollbackLines` - Lines in scrollback
- `displayOffset` - Current scroll position
- `needsRedraw` - Whether full redraw needed

**Methods:**
- `process(data:)` - Process input bytes
- `resize(rows:cols:)` - Resize terminal
- `reset()` - Reset to initial state
- `getCell(row:col:)` - Get cell at position
- `getLineText(row:)` - Get text content of row
- `scroll(delta:)` - Scroll display
- `scrollToTop()`, `scrollToBottom()` - Jump to scroll position
- `clearDamage()` - Clear damage after rendering
- `encodeMousePress/Release/Motion/Wheel(...)` - Encode mouse events
- `encodeFocusEvent(focused:)` - Encode focus events

### DTermCell

Cell data for rendering.

**Properties:**
- `codepoint` - Unicode codepoint (0 for empty)
- `character` - Character (optional)
- `foreground`, `background` - Packed colors
- `underlineColor` - Underline color
- `flags` - Cell attributes (bold, italic, etc.)
- `isEmpty` - Whether cell is empty

### CellFlags

Cell attribute flags (OptionSet).

- `.bold`, `.italic`, `.underline`, `.blink`
- `.inverse`, `.invisible`, `.strikethrough`, `.faint`
- `.doubleUnderline`, `.curlyUnderline`, `.dottedUnderline`, `.dashedUnderline`
- `.superscript`, `.subscript`, `.wide`

### DTermModes

Terminal mode state.

**Properties:**
- `cursorVisible`, `cursorStyle`, `cursorBlink`
- `applicationCursorKeys`, `applicationKeypad`
- `alternateScreen`, `autoWrap`, `originMode`, `insertMode`
- `bracketedPaste`
- `mouseMode`, `mouseEncoding`
- `focusReporting`, `synchronizedOutput`
- `reverseVideo`, `columnMode132`, `reverseWraparound`

## License

Apache 2.0
