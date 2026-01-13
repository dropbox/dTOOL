# dTerm Development Strategy

## Overview

dTerm is developed in two phases, using two repositories:

| Repo | Purpose |
|------|---------|
| **dterm** (`~/dterm`) | Rust core + future clean UIs |
| **dashterm2** (`~/dashterm2`) | iTerm2 fork as test harness |

---

## Phase 1: Core Development

Build the Rust core in `dterm`, test it via FFI integration with `dashterm2`.

```
~/dterm/                       ~/dashterm2/
┌──────────────────┐           ┌──────────────────┐
│  dterm-core      │           │  iTerm2 UI       │
│  (Rust)          │───FFI───→ │  (ObjC/Swift)    │
│                  │           │                  │
│  • Parser        │           │  • Window mgmt   │
│  • Grid          │           │  • Metal render  │
│  • Scrollback    │           │  • Input         │
│  • State machine │           │  • All features  │
└──────────────────┘           └──────────────────┘
     BUILD HERE                    TEST HERE
```

**Why this approach:**
- Real terminal UI to test against immediately
- Don't block on building new UI from scratch
- Validate core correctness with actual terminal usage
- iTerm2's VT100 behavior as reference oracle
- 3,578 existing regression tests in dashterm2

**What we build in dterm:**
- `dterm-core` - Terminal state machine, parser, grid, scrollback
- TLA+ specifications
- Kani proofs
- Fuzz targets

**What dashterm2 provides:**
- Working macOS terminal UI
- Metal renderer
- Window/tab management
- All iTerm2 features (tmux, triggers, etc.)
- Test environment

---

## Phase 2: Clean UIs

Once the core is proven and verified, build clean native UIs in `dterm`.

```
~/dterm/
├── dterm-core/        # Proven Rust core
├── dterm-macos/       # New SwiftUI app
├── dterm-ios/         # Touch-native SwiftUI
├── dterm-windows/     # WinUI app
└── dterm-linux/       # GTK app
```

**At this point:**
- `dashterm2` becomes archived reference only
- All development moves to `dterm`
- Clean Apache 2.0 codebase ships

---

## Integration Points

### FFI Interface

The core exposes a C-compatible interface:

```rust
// dterm-core/src/ffi.rs
#[no_mangle]
pub extern "C" fn dterm_create(cols: u32, rows: u32) -> *mut Terminal;

#[no_mangle]
pub extern "C" fn dterm_process(term: *mut Terminal, data: *const u8, len: usize);

#[no_mangle]
pub extern "C" fn dterm_resize(term: *mut Terminal, cols: u32, rows: u32);

#[no_mangle]
pub extern "C" fn dterm_get_cell(term: *const Terminal, x: u32, y: u32) -> Cell;

#[no_mangle]
pub extern "C" fn dterm_destroy(term: *mut Terminal);
```

### Swift Integration (dashterm2)

```swift
// dashterm2/sources/DTermBridge.swift
import Foundation

class DTermCore {
    private var handle: OpaquePointer?

    init(cols: UInt32, rows: UInt32) {
        handle = dterm_create(cols, rows)
    }

    func process(data: Data) {
        data.withUnsafeBytes { ptr in
            dterm_process(handle, ptr.baseAddress, data.count)
        }
    }

    deinit {
        dterm_destroy(handle)
    }
}
```

---

## Testing Strategy

### Unit Tests (dterm)
- Parser handles all escape sequences
- Grid operations are correct
- State machine transitions are valid
- Kani proofs pass

### Integration Tests (dashterm2)
- Core produces correct output for real terminal usage
- Rendering matches expected behavior
- Performance meets targets
- No regressions from existing tests

### Comparison Testing
- Run same input through both old VT100Screen and new dterm-core
- Output must match exactly
- Any difference is a bug to investigate

---

## Timeline

1. **Now:** Set up FFI scaffold in dterm
2. **Next:** Implement parser with verification
3. **Then:** Implement grid and scrollback
4. **Then:** Integrate with dashterm2 for testing
5. **Then:** Iterate until core passes all tests
6. **Finally:** Build clean UIs in dterm

---

## Repository Links

- **dterm:** https://github.com/dropbox/dTOOL/dterm
- **dashterm2:** https://github.com/dropbox/dashterm2

See also: `dashterm2/docs/DEVELOPMENT-STRATEGY.md`
