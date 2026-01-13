# DashTerm2 Development Strategy

## Overview

DashTerm2 serves as the **test harness** for developing the new dTerm core.

| Repo | Purpose |
|------|---------|
| **dterm** (`~/dterm`) | Rust core + future clean UIs |
| **dashterm2** (`~/dashterm2`) | iTerm2 fork as test harness |

---

## Role of DashTerm2

DashTerm2 is an iTerm2 fork with 367 bug fixes and 3,578 regression tests. It provides:

1. **Working Terminal UI** - Metal renderer, window management, all features
2. **Test Environment** - Real terminal usage to validate the new core
3. **Reference Oracle** - VT100 behavior to compare against
4. **Regression Tests** - Existing test suite to verify correctness

---

## Phase 1: Core Development

The new Rust core is built in `dterm` and tested via FFI integration here.

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
     BUILD THERE                   TEST HERE
```

**Integration approach:**
1. Build `dterm-core` as a dynamic library
2. Create Swift bridge in dashterm2 (`sources/DTermBridge.swift`)
3. Optionally route terminal emulation through new core
4. Compare output with existing VT100Screen
5. Run regression tests against new core

---

## Swift Bridge

```swift
// sources/DTermBridge.swift
import Foundation

/// Bridge to dterm-core Rust library
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

    func resize(cols: UInt32, rows: UInt32) {
        dterm_resize(handle, cols, rows)
    }

    func getCell(x: UInt32, y: UInt32) -> DTermCell {
        return dterm_get_cell(handle, x, y)
    }

    deinit {
        if let h = handle {
            dterm_destroy(h)
        }
    }
}
```

---

## Comparison Testing

To validate the new core produces correct output:

```swift
// Compare old and new implementations
func validateOutput(input: Data) -> Bool {
    // Old implementation
    let oldScreen = VT100Screen()
    oldScreen.process(input)

    // New implementation
    let newCore = DTermCore(cols: 80, rows: 24)
    newCore.process(input)

    // Compare cell-by-cell
    for y in 0..<24 {
        for x in 0..<80 {
            let oldCell = oldScreen.getCell(x: x, y: y)
            let newCell = newCore.getCell(x: UInt32(x), y: UInt32(y))
            if oldCell != newCell {
                print("Mismatch at (\(x), \(y))")
                return false
            }
        }
    }
    return true
}
```

---

## Phase 2: Archive

Once the dterm core is proven and clean UIs are built:

1. DashTerm2 becomes **archived reference only**
2. All active development moves to `dterm`
3. This repo remains for:
   - Historical reference
   - Test case source
   - VT100 behavior oracle

---

## What NOT to Do

- **Don't** add major new features to dashterm2
- **Don't** refactor the ObjC/Swift codebase
- **Don't** ship dashterm2 as a product
- **Do** use it as a test harness
- **Do** fix bugs that block testing
- **Do** add tests that help validate the new core

---

## Repository Links

- **dterm:** https://github.com/dropbox/dterm
- **dashterm2:** https://github.com/dropbox/dTOOL/dashterm2

See also: `dterm/docs/DEVELOPMENT-STRATEGY.md`
