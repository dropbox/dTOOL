# Worker 1609 Investigation Report: DTermCore Library Issues

**Date:** 2025-12-29
**Previous Worker:** #1608
**Status:** Investigation only - no code changes

## Summary

Investigated pre-existing test failures in DTermCore integration tests. Found that:
1. Original library has 7 failures (3 real true-color tests + 4 expected failures)
2. Rebuilding the library causes 15 failures (8 additional regressions)

## Test Failures (Original Library)

The following tests fail with the original `libdterm_core.a` (20,505,016 bytes):

### Real Failures (3)
- `test_trueColor_background` - RGB values returned as 0,0,0 instead of expected values
- `test_trueColor_foreground` - RGB values returned as 0,0,0 instead of expected values
- `test_underlineColor_indexed` - Similar color parsing issue

### Expected Failures (4)
These use XCTExpectFailure - marked as known issues.

## Investigation Findings

### Rebuilding the Library

When the library is rebuilt with either Rust 1.84 or 1.90:
- Size changes from 20.5MB to 22.3MB
- 12 additional tests start failing:
  - Wide character tests (emoji, CJK)
  - Damage tracking tests
  - Scrollback tests
  - Default color tests
  - Double underline tests

### Root Cause Hypothesis

The original library was built with unknown cargo settings that produced a smaller, different binary. Possible causes:
1. Different cargo feature flags
2. Different optimization settings
3. Different dependency versions
4. Strip/debug settings

### Code Analysis

The FFI code in `dashterm-core/src/ffi/mod.rs` appears correct:
- `pack_color()` correctly packs RGB as `0x01_RRGGBB`
- `dterm_terminal_get_cell()` calls `pack_color()` for fg/bg
- Rust unit tests pass for these functions

The issue seems to be in the library binary, not the source code.

## Recommendations for Next Worker

### Option A: Fix Original Library Behavior
Investigate why the original library returns incorrect true-color values even though:
1. Rust tests pass
2. The `dterm_cell_fg_rgb` FFI function works correctly

### Option B: Fix Rebuild Regressions
1. Identify the exact cargo command used to build original library
2. Compare symbol tables between old and new libraries
3. Use `cargo build --release -vv` to see exact compiler flags

### Option C: Skip and Continue
Mark the 3 true-color tests as expected failures and continue with renderer integration work.

## Files Relevant to Investigation

- `/Users/ayates/dashterm2/DTermCore/lib/libdterm_core.a` - Original library
- `/Users/ayates/dashterm2/dashterm-core/src/ffi/mod.rs` - FFI implementation
- `/Users/ayates/dashterm2/sources/DTermCore.swift` - Swift wrapper
- `/Users/ayates/dashterm2/DashTerm2Tests/DTermCoreComparisonTests.swift` - Failing tests
