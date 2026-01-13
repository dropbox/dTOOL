# WORKER DIRECTIVE: Fix Rendering Gaps Blocking DashTerm2

**Date:** 2025-12-31 (Final Update)
**From:** MANAGER
**To:** dterm-core WORKER
**Priority:** COMPLETE
**Status:** ALL RENDERING GAPS FIXED

---

## Executive Summary

DashTerm2 reports dterm-core rendering gaps are **100% complete**:

1. ~~**Box drawing characters are INVISIBLE**~~ **FIXED in commit #399**
2. ~~**Powerline glyphs don't render**~~ **FIXED in commit #401**
3. ~~**Dotted/dashed underlines missing**~~ **FIXED in commit #402**
4. ~~**iTerm2 inline images missing**~~ **FIXED in commit #403**

**All rendering gaps have been addressed.**

---

## COMPLETED: Box Drawing Fix (Commit #399)

The root cause was identified and fixed:
- **Problem:** Hybrid renderer (`gpu/ffi.rs`) was NOT calling `box_drawing::is_box_drawing()`
- **Fix:** Added box drawing detection BEFORE platform/atlas glyph lookup
- **Tests added:** Comprehensive visibility tests in `box_drawing.rs`
- **See:** `docs/RETROSPECTIVE_INVISIBLE_CHARS_2025-12-31.md`

---

## COMPLETED: Powerline Glyphs (Commit #401)

Implemented ~30 Powerline glyphs in `box_drawing.rs`:

### Unicode Ranges Implemented

```
U+E0A0-U+E0A3  Version control symbols (branch, line, lock, column)
U+E0B0-U+E0BF  Arrow and triangle separators (16 chars)
U+E0C0-U+E0C7  Flame and pixelated separators (8 chars)
U+E0C8, E0CA   Ice/waveform separators
U+E0CC-U+E0CD  Honeycomb separators
U+E0D0, E0D2   Trapezoid separators
```

---

## COMPLETED: Dotted/Dashed Underlines (Commit #402)

Implemented SGR 4:x subparameter parsing and rendering:

### Key Changes
- Parser: Added colon handling and `csi_dispatch_with_subparams()` for SGR sequences
- Grid: Added `DOTTED_UNDERLINE` (4:4) and `DASHED_UNDERLINE` (4:5) flags
- GPU: Added `UnderlineStyle::Dotted` and `UnderlineStyle::Dashed` variants
- Terminal: Added `handle_sgr_with_subparams()` for subparameter handling

---

## COMPLETED: iTerm2 Inline Images (Commit #403)

Implemented OSC 1337 File protocol support:

### Implementation Details

1. **OSC 1337 File Parsing** - Already implemented in `iterm_image/mod.rs`
   - Parameters: name, size, width, height, preserveAspectRatio, inline
   - Base64 decoding of image data
   - Image format detection (PNG, JPEG, GIF, BMP, WebP, TIFF, AVIF, JXL)

2. **Image Storage** - `InlineImageStorage` with LRU eviction

3. **Cursor Advancement** - Added in `terminal/mod.rs:handle_osc_1337_file()`
   - Cursor advances past image area based on height spec
   - Returns to column 0 after image

4. **FFI Functions** - Added in `gpu/ffi.rs`:
   - `dterm_terminal_inline_image_count()` - Get count of stored images
   - `dterm_terminal_inline_image_info()` - Get metadata (row, col, dimensions)
   - `dterm_terminal_inline_image_data()` - Get raw image data pointer
   - `dterm_terminal_inline_image_clear()` - Clear all images
   - `DtermInlineImageInfo` struct for FFI metadata transfer

### Platform Integration (for dashterm2)

The platform should:
1. Call `dterm_terminal_inline_image_count()` to check for new images
2. Use `dterm_terminal_inline_image_info()` to get metadata
3. Use `dterm_terminal_inline_image_data()` to get raw image bytes
4. Decode using platform APIs (CGImage, UIImage)
5. Render at grid position using calculated cell dimensions

---

## Verification Checklist

All items verified:

- [x] `cat tests/box_drawing_test.txt` renders correctly (FIXED)
- [x] Powerline prompts display arrows and separators (FIXED)
- [x] `echo -e "\e[4:4mDotted\e[0m \e[4:5mDashed\e[0m"` shows correct underlines (FIXED)
- [x] iTerm2 OSC 1337 File parsed and stored (FIXED)
- [x] All existing tests pass (1998 tests)
- [x] No performance regression

---

## Summary by Commit

| Commit | Feature | Files Changed |
|--------|---------|---------------|
| #399 | Box drawing visibility | `gpu/ffi.rs`, `gpu/box_drawing.rs` |
| #401 | Powerline glyphs | `gpu/box_drawing.rs`, `ffi/mod.rs` |
| #402 | Dotted/dashed underlines | `parser/`, `grid/`, `gpu/`, `terminal/` |
| #403 | iTerm2 inline images | `iterm_image/`, `terminal/`, `gpu/ffi.rs` |

---

*Directive complete - All rendering gaps fixed*
