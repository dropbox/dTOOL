# Rust Core Prototype Summary

**Created:** December 27, 2025
**Status:** PAUSED - Moved to branch `rust-core-prototype`
**Author:** AI Worker (iterations #1369-#1373)

## Overview

A first-draft Rust implementation of a terminal emulator core, intended to eventually replace the ObjC VT100Screen/VT100Terminal implementation for better performance and memory safety.

## What Was Built

### Code Structure (5,581 lines of Rust)

```
dashterm2-core/
├── src/
│   ├── lib.rs          (43 lines)   - Module exports
│   ├── cursor/mod.rs   (~300 lines) - Cursor position management
│   ├── buffer/mod.rs   (~800 lines) - Screen + scrollback buffers
│   ├── parser/mod.rs   (1,495 lines)- VT100/ANSI state machine
│   ├── terminal/mod.rs (1,395 lines)- Integrated terminal
│   └── ffi/mod.rs      (984 lines)  - C FFI for Swift/ObjC
├── tla/
│   └── VT100Parser.tla (509 lines)  - TLA+ specification
├── include/
│   └── dashterm2.h                  - C header for FFI
└── Cargo.toml
```

### Swift Integration

- `sources/DashTermCore.swift` (400+ lines) - Full Swift wrapper
- Linked as `libdashterm2_core.dylib` in Xcode project
- NOT actually used anywhere in the app

### Features Implemented

- Basic VT100 parser state machine (per vt100.net spec)
- CSI sequences (cursor movement, SGR attributes, erase)
- OSC sequences (window title, icon name)
- ESC sequences (basic)
- Screen buffer with configurable scrollback
- Cursor with bounds checking
- Cell attributes (colors, bold, italic, etc.)
- C FFI with opaque handles

### Tests

- 99 unit tests
- 6 doc tests
- Property-based tests (proptest)
- All reported passing

## Critical Gaps

### 1. No Performance Validation
- **No benchmarks** comparing to ObjC implementation
- Claims "high-performance" but no data to support it
- Unknown memory usage characteristics

### 2. Incomplete VT100/VT520 Support
Missing sequences that iTerm2 supports:
- DCS sequences (DECRQSS, DECUDK, Sixel, etc.)
- Many CSI sequences (DECSTBM, DECSC, DECRC, etc.)
- Mouse reporting
- Bracketed paste mode
- Focus reporting
- Many SGR attributes

### 3. No Threading Design
- Code comments say "NOT thread-safe"
- No design for concurrent access
- No lock-free data structures
- No actor model consideration

### 4. Formal Verification Not Executed
- TLA+ spec written but **not run through TLC model checker**
- Kani harnesses exist but **not verified**
- PropTest exists but coverage unknown

### 5. No Integration Testing
- No tests with real shell output
- No vim, htop, tmux test cases
- No comparison with ObjC implementation output

### 6. Missing iTerm2 Features
- No Sixel graphics
- No inline images
- No tmux integration
- No shell integration hooks
- No triggers
- No semantic history
- No marks

### 7. No Memory Stability Testing
- No stress tests
- No leak detection
- No fuzzing with AFL/libFuzzer
- No long-running stability tests

## Recommendations for Future Work

### Phase 1: Validation
1. **Benchmark** - Create perf comparison vs ObjC (throughput, latency, memory)
2. **Run TLA+** - Actually execute TLC model checker on the spec
3. **Run Kani** - Verify the proof harnesses
4. **Fuzz test** - Run AFL/libFuzzer on parser

### Phase 2: Design
1. **Threading model** - Design for concurrent access (probably actor model)
2. **Memory model** - Design for FFI boundary safety
3. **Feature scope** - Define exactly which VT sequences are needed

### Phase 3: Implementation
1. **Complete parser** - All VT sequences iTerm2 uses
2. **Shadow mode** - Run both implementations, compare output
3. **Gradual rollout** - Feature flag to switch implementations

### Phase 4: Integration
1. **Replace VT100Screen** - Swap in Rust implementation
2. **Performance tuning** - Profile and optimize
3. **Stability testing** - Long-running tests

## Files to Preserve

When removing from main branch, these should be moved to `rust-core-prototype` branch:
- `dashterm2-core/` (entire directory)
- `sources/DashTermCore.swift`
- Xcode project references to `libdashterm2_core.dylib`

## Conclusion

This is a reasonable **first draft skeleton** but is NOT production-ready. It needs:
- Actual performance validation
- Complete feature implementation
- Real formal verification (not just written specs)
- Threading design
- Integration testing

The right approach is to pause, design properly, then rebuild with rigor.
