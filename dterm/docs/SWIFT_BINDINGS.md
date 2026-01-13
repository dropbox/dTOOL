# Swift UIBridge Bindings

**Created**: 2025-12-30 (Iteration 294)
**Status**: Complete

---

## Overview

Swift wrapper for the UI Bridge FFI layer, providing safe access to the dterm-core UI state machine from Swift/iOS/macOS applications.

---

## Files

| File | Purpose |
|------|---------|
| `crates/dterm-core/cbindgen.toml` | C header generation config |
| `crates/dterm-core/include/dterm.h` | Generated C header |
| `packages/dterm-swift/Sources/CDTermCore/include/dterm.h` | Copied header for Swift |
| `packages/dterm-swift/Sources/DTermCore/DTermUIBridge.swift` | Swift wrapper class |

---

## Swift API

### Creating a Bridge

```swift
import DTermCore

let bridge = DTermUIBridge()
```

### Terminal Lifecycle

```swift
// Create a terminal
try bridge.handleCreateTerminal(terminalId: 0)
assert(bridge.terminalState(terminalId: 0) == .active)

// Send input
let data = "hello".data(using: .utf8)!
try bridge.handleInput(terminalId: 0, data: data)

// Resize terminal
try bridge.handleResize(terminalId: 0, rows: 24, cols: 80)

// Request render
try bridge.handleRender(terminalId: 0)

// Complete render (after rendering finishes)
try bridge.completeRender(terminalId: 0)

// Destroy terminal
try bridge.handleDestroyTerminal(terminalId: 0)
assert(bridge.terminalState(terminalId: 0) == .disposed)

// Shutdown
try bridge.handleShutdown()
```

### State Queries

```swift
let state = bridge.state           // UIBridgeState
let count = bridge.pendingCount    // Int
let consistent = bridge.isConsistent // Bool
let termState = bridge.terminalState(terminalId: 0) // UITerminalState
```

---

## Types

### DTermUIBridge

Main class wrapping the FFI. Manages lifecycle of underlying Rust UIBridge.

### DTermUIError

Error enum with all error cases:
- `.invalidStateTransition`
- `.queueFull`
- `.invalidTerminalId`
- `.duplicateCallback`
- `.unknownError`

### UIBridgeState

State machine states:
- `.idle`
- `.processing`
- `.rendering`
- `.waitingForCallback`
- `.shuttingDown`

### UITerminalState

Terminal lifecycle states:
- `.inactive` - Not yet created
- `.active` - Created and usable
- `.disposed` - Destroyed (permanent)

### UIEventKind

Event types:
- `.input`
- `.resize`
- `.render`
- `.createTerminal`
- `.destroyTerminal`
- `.requestCallback`
- `.shutdown`

### UIEventInfo

Event metadata returned from `startProcessing()`:
- `id: UInt64`
- `kind: UIEventKind`
- `terminal: UInt32?`
- `callback: UInt32?`

---

## C Header Generation

The C header is generated using cbindgen with `prefix_with_name = true` to avoid global namespace collisions. Enum values are prefixed with their type name:

```c
// Before (collision risk)
IDLE, PROCESSING, RENDERING...

// After (safe)
DTERM_UI_STATE_IDLE, DTERM_UI_STATE_PROCESSING, DTERM_UI_STATE_RENDERING...
DTERM_UI_ERROR_OK, DTERM_UI_ERROR_INVALID_STATE_TRANSITION...
```

---

## Building

```bash
# Regenerate C header
cd crates/dterm-core
cbindgen --config cbindgen.toml --output include/dterm.h

# Copy to Swift package
cp include/dterm.h ../../packages/dterm-swift/Sources/CDTermCore/include/

# Build Swift package
cd ../../packages/dterm-swift
swift build
```

---

## Testing

```bash
# Rust tests
cargo test -p dterm-core ui::

# Swift build test
cd packages/dterm-swift
swift build
```
