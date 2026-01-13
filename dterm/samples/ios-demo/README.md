# DTermDemo - iOS Sample App

A sample iOS/iPadOS/macOS app demonstrating dterm-core integration via the dterm-swift package.

## Features

This demo app showcases:

- **Terminal rendering**: Cell-by-cell display with full color support
- **Text attributes**: Bold, italic, underline, strikethrough
- **256-color palette**: Standard 16 colors + 6x6x6 cube + grayscale
- **True color (24-bit RGB)**: Full RGB color support
- **Unicode & emoji**: Box drawing, emoji rendering
- **Cursor display**: Block cursor with position tracking
- **Scrollback**: Line history with scroll support
- **Terminal state**: Title, modes, shell integration state

## Building

### Prerequisites

- Xcode 15+ with Swift 5.9+
- macOS 13+ or iOS 16+ deployment target
- dterm-core Rust library built and linked (required for running)

### Current Status

The Swift source files compile successfully, but linking requires the dterm-core Rust
library to be built and properly linked. The dterm-swift package provides:
- Swift type definitions and API wrappers
- C header file with FFI declarations
- Placeholder C source (actual implementation in Rust)

### Compilation Check

To verify Swift source compilation (without linking):

```bash
cd samples/ios-demo
swift build 2>&1 | grep -E "(Compiling|error:.*\.swift)"
```

Expected output: `Compiling DTermDemo` with no Swift errors.
Link errors for `_dterm_*` symbols are expected until the Rust library is linked.

### Full Build (Requires Rust Library)

For a complete build that can run:

1. Build the dterm-core Rust library with FFI and GPU support:
   ```bash
   cd /path/to/dterm
   cargo build --release -p dterm-core --features ffi,gpu
   ```

2. Build and run the demo:
   ```bash
   cd samples/ios-demo
   swift build
   ./.build/debug/DTermDemo
   ```

The `Package.swift` already includes linker settings pointing to `target/release`.

3. For iOS, cross-compile for arm64:
   ```bash
   rustup target add aarch64-apple-ios
   cargo build --target aarch64-apple-ios --release -p dterm-core --features ffi
   ```

### macOS vttest Replay Mode

If you have a recorded vttest command log, the macOS demo can replay it
automatically and exit when done:

```bash
DTERM_VTTEST_COMMAND_LOG=/tmp/vttest.log \
DTERM_VTTEST_LOG=/tmp/vttest_run.log \
DTERM_VTTEST_EXIT_ON_COMPLETE=1 \
samples/ios-demo/.build/debug/DTermDemo
```

### Xcode Integration

For full testing with Xcode:

1. Open `samples/ios-demo/Package.swift` in Xcode
2. Add the compiled Rust static library to the project
3. Update build settings to link the library
4. Select the DTermDemo scheme
5. Choose your target device (My Mac, iPhone Simulator, etc.)
6. Build and run (Cmd+R)

### For iOS Device Deployment

To deploy to a real iOS device:

1. Build the dterm-core library for iOS (arm64):
   ```bash
   rustup target add aarch64-apple-ios
   cargo build --target aarch64-apple-ios --release -p dterm-core
   ```

2. Create an Xcode project that links the static library

3. Sign with your Apple Developer account

## Architecture

```
DTermDemo/
├── DTermDemoApp.swift     # App entry point
├── ContentView.swift      # Main view + TerminalState
├── TerminalView.swift     # Terminal rendering (Canvas-based)
└── ControlsView.swift     # Demo controls panel
```

### Key Components

#### TerminalState

An `@ObservableObject` that manages:
- The `DTermTerminal` instance
- Demo data generation
- Terminal delegate callbacks

#### TerminalView

SwiftUI view that renders terminal content:
- Uses `Canvas` for efficient cell-by-cell rendering
- Handles all 256 colors + true color
- Displays cursor position
- Renders text attributes

#### ControlsView

Demo control panel providing:
- Text input field for manual escape sequences
- Demo, Clear, Box, and +Line buttons
- Status display (size, cursor, scrollback)

## Demo Functions

| Button | Action |
|--------|--------|
| Demo | Run full demo sequence with colors, attributes, unicode |
| Clear | Clear screen (ED 2 + cursor home) |
| Box | Draw a yellow box using cursor movement |
| +Line | Add a line to scrollback |

## Escape Sequences Demonstrated

- **RIS** (`\e c`): Reset to initial state
- **OSC 0** (`\e]0;title\a`): Set window title
- **SGR**: All color and attribute codes
- **CUP** (`\e[row;colH`): Cursor position
- **ED** (`\e[2J`): Erase display
- **DECSC/DECRC** (`\e7`, `\e8`): Save/restore cursor

## Memory Model

The demo uses the default scrollback settings:
- Ring buffer: 1000 lines
- Hot tier: 10,000 lines (uncompressed)
- Warm tier: 100,000 lines (LZ4 compressed)
- Memory budget: 100 MB

## License

Apache 2.0 - Same as dterm-core
