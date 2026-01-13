# Worker Report 1523: Scrollback Cell FFI Analysis

## Summary

This iteration investigated adding scrollback cell access FFI to dterm-core. The analysis revealed a significant library/codebase mismatch that prevents simply adding new FFI functions.

## Discovery

### The Problem

The pre-built `libdterm_core.a` was built from a **different Rust codebase** than `dashterm-core/` in the repo:

| Feature | Repo's dashterm-core | Pre-built library |
|---------|---------------------|-------------------|
| Terminal FFI | Yes | Yes |
| Parser FFI (`dterm_parser_*`) | **No** | Yes |
| Search FFI | Yes | Yes |
| Scrollback cell FFI | **Yes** (in code) | No |

The pre-built library includes parser FFI functions that are NOT present in the repo's Rust code:
- `dterm_parser_new`
- `dterm_parser_free`
- `dterm_parser_feed`
- `dterm_parser_reset`

These are actively used by `DTermCoreParserAdapter.swift`.

### Why This Matters

If we rebuild `libdterm_core.a` from the repo's `dashterm-core/`:
- We GET the new scrollback cell functions
- We LOSE the parser FFI functions
- Build FAILS with undefined symbols for parser functions

### The Library Origins

The pre-built library appears to have been built from a private/extended version of the Rust code that includes both:
1. All terminal/search FFI from the repo
2. Parser FFI not present in the repo

## Changes Made

### Added to Header (dterm.h)
Future-ready declarations for scrollback cell access:
- `dterm_terminal_get_scrollback_cell()`
- `dterm_terminal_scrollback_cell_hyperlink()`

These declarations are documented as NOT YET IMPLEMENTED in the library.

### NOT Added to Swift
The Swift methods were written but then removed because the library doesn't have the symbols:
- `DTermCore.scrollbackCell(at:col:)`
- `DTermCore.scrollbackHyperlinkAt(scrollbackRow:col:)`
- `DTermCoreIntegration.scrollbackCell(at:col:)`
- `DTermCoreIntegration.scrollbackHyperlinkAt(scrollbackRow:col:)`

## Resolution Path

To enable scrollback cell access in the future:

### Option 1: Add Parser FFI to Repo (Recommended)
1. Add parser FFI functions to `dashterm-core/src/ffi/mod.rs`
2. This requires implementing:
   - `dterm_parser_t` struct (opaque parser handle)
   - `dterm_parser_new()` - create parser
   - `dterm_parser_free()` - destroy parser
   - `dterm_parser_feed()` - feed bytes, emit callbacks
   - `dterm_parser_reset()` - reset parser state
3. Rebuild library with `./scripts/build-dashterm-core.sh`
4. Copy to `DTermCore/lib/libdterm_core.a`
5. Uncomment Swift methods in `DTermCore.swift`

### Option 2: Find Original Codebase
If the original extended Rust codebase exists:
1. Locate it (possibly private repo or local build environment)
2. Add scrollback cell FFI there
3. Rebuild and update the library

### Option 3: Remove Parser Dependency
1. Remove `DTermCoreParserAdapter.swift`
2. Update any code that depends on it
3. Rebuild library from repo's dashterm-core
4. Add Swift scrollback methods

## Files Modified

- `DTermCore/include/dterm.h` - Added future FFI declarations with documentation
- `DTermCore/lib/libdterm_core.a` - Restored from commit 444ffeba5 (has parser FFI)

## Verification

- Build: SUCCESS
- Tests: 4754 passed, 0 failures

## Next AI Instructions

The scrollback cell FFI infrastructure is prepared:
1. Rust FFI functions exist in `dashterm-core/src/ffi/mod.rs` (lines 784-873)
2. Header declarations exist in `dterm.h` (lines 1769-1831)
3. Swift method signatures are documented in the header comments

To complete scrollback cell access:
1. Follow "Option 1: Add Parser FFI to Repo" above
2. The parser likely needs to wrap the existing `Parser` struct in `dashterm-core/src/parser/mod.rs`
3. Study commit 622ddff46 for how the library was structured when parser FFI worked

