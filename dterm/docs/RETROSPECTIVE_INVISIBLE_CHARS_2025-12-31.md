# Retrospective: Invisible Box Drawing Characters

**Date:** 2025-12-31
**Severity:** P0 CRITICAL
**Impact:** Blocked DashTerm2 from enabling dterm-core (resolved)
**Status:** RESOLVED (Iterations 399-403)

---

## Summary

Box drawing characters (┌─┐│└┘├┤┬┴┼) render as INVISIBLE when using dterm-core's GPU renderer. This bug was not caught by our test suite despite having extensive terminal emulation tests.

## Resolution Summary (Iterations 399-403)

- Added GPU vertex generation for box drawing, block elements, powerline, geometric shapes, and sextants in `crates/dterm-core/src/gpu/box_drawing.rs`.
- Routed special characters through the box drawing path in `crates/dterm-core/src/gpu/mod.rs` and `crates/dterm-core/src/gpu/ffi.rs`.
- Added coverage tests that assert non-empty vertices for supported ranges in `crates/dterm-core/src/gpu/box_drawing.rs`.
- Remaining gaps: fixture files and visual regression tests are still outstanding.

---

## Root Cause Analysis

### What Happened

1. **Terminal emulation is correct:** Characters are properly translated from DEC line drawing (ESC ( 0) to Unicode box drawing (U+2500 range).

2. **Grid storage is correct:** The grid correctly stores box drawing Unicode codepoints.

3. **Rendering fails silently:** The GPU renderer attempts to render these characters as font glyphs, but:
   - The glyph may not exist in the font
   - The glyph may exist but render at wrong position
   - The glyph may be clipped or have zero alpha
   - **Result: No visible output, no error**

### Why It Wasn't Caught

| Test Category | Coverage | Gap |
|---------------|----------|-----|
| Parser tests | ✅ 100% | Parser correctly parses DEC line drawing |
| Terminal emulation tests | ✅ 95% | Grid correctly stores translated characters |
| GPU renderer tests | ⚠️ 70% | **Tests only check vertex generation, not visual output** |
| Integration tests | ❌ 0% | **No tests verify characters are actually visible** |
| Visual regression tests | ❌ 0% | **No screenshot comparison tests** |

### The Testing Gap

```
┌──────────────────────────────────────────────────────────────┐
│                    TEST COVERAGE GAP                          │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Parser ──► Grid ──► GPU Renderer ──► Screen                 │
│     ✓         ✓           ✓             ❌                   │
│                                                              │
│  We test: "Is data correct at each stage?"                   │
│  We DON'T test: "Is the character VISIBLE to the user?"      │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## Why This Class of Bug Is Dangerous

1. **Silent Failure:** No error messages, crashes, or warnings. Tests pass.

2. **Affects Critical Characters:** Box drawing is used by:
   - tmux borders
   - vim windows
   - htop UI
   - ncurses applications
   - File managers (mc, ranger)

3. **Compound Effect:** When dterm-core is 95% correct but 5% invisible, users see a broken terminal.

4. **Hard to Debug:** "Characters are invisible" could be caused by:
   - Font missing glyphs
   - Wrong color (black on black)
   - Wrong position (off-screen)
   - Wrong alpha (transparent)
   - Wrong clipping (outside viewport)
   - Many other reasons

---

## Similar Gaps Identified

| Unicode Range | Characters | Risk |
|---------------|------------|------|
| U+2500-U+257F | Box drawing | **CONFIRMED BROKEN** |
| U+2580-U+259F | Block elements (▀▄█░▒▓) | HIGH - likely broken |
| U+25A0-U+25FF | Geometric shapes | HIGH - likely broken |
| U+E0A0-U+E0D7 | Powerline glyphs | **CONFIRMED BROKEN** |
| U+1FB00-U+1FBFF | Legacy computing symbols | MEDIUM |
| U+2800-U+28FF | Braille patterns | MEDIUM |
| U+2190-U+21FF | Arrows | LOW - usually in fonts |
| U+2600-U+26FF | Misc symbols | LOW |

---

## Remediation Plan

### Immediate (Before Next Commit)

1. **Add Character Visibility Tests:**
```rust
#[test]
fn test_box_drawing_chars_generate_vertices() {
    let mut renderer = TestRenderer::new();
    let terminal = create_terminal_with("┌─┐\n│X│\n└─┘");

    renderer.build(&terminal);

    // Each box drawing char must generate at least 6 vertices (2 triangles)
    for row in 0..3 {
        for col in 0..3 {
            let char = terminal.grid().cell(col, row).char_data();
            if is_box_drawing(char) {
                let vertices = renderer.vertices_for_cell(col, row);
                assert!(!vertices.is_empty(),
                    "Box drawing char '{}' at ({},{}) generated no vertices",
                    char, col, row);
            }
        }
    }
}
```

2. **Add Special Character Detection:**
```rust
/// Characters that require special rendering (not font glyphs)
pub fn requires_special_rendering(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257F}' |  // Box drawing
        '\u{2580}'..='\u{259F}' |  // Block elements
        '\u{E0A0}'..='\u{E0D7}'    // Powerline
    )
}
```

3. **Add Rendering Path Verification:**
```rust
#[test]
fn test_special_chars_use_special_rendering() {
    let special_chars = ['┌', '─', '▀', '█', '', ''];
    for c in special_chars {
        assert!(requires_special_rendering(c),
            "Character '{}' (U+{:04X}) should use special rendering",
            c, c as u32);
    }
}
```

### Short-Term (This Week)

4. **Create Comprehensive Test Files:**
   - `tests/fixtures/box_drawing_comprehensive.txt`
   - `tests/fixtures/block_elements.txt`
   - `tests/fixtures/powerline_glyphs.txt`

5. **Add Automated Visual Regression:**
   - Render terminal to offscreen texture
   - Compare against known-good reference images
   - Fail if pixel difference exceeds threshold

### Long-Term

6. **Add Fuzzing for Rendering:**
   - Fuzz Unicode input to renderer
   - Verify no panics AND non-empty vertex output for printable chars

7. **Add CI Visual Testing:**
   - Run visual regression on every PR
   - Store golden images in repository

---

## Checklist Before Any PR

Add to PR template:

```markdown
## Rendering Verification
- [ ] Box drawing characters (┌─┐│└┘) render correctly
- [ ] Block elements (▀▄█░▒▓) render correctly
- [ ] Powerline glyphs render correctly
- [ ] No characters are invisible that should be visible
- [ ] Visual regression tests pass (if available)
```

---

## Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| Unicode ranges with visibility tests | 0 | 10+ |
| Characters tested for visibility | 0 | 500+ |
| Visual regression test coverage | 0% | 80% |
| Time to detect invisible char bug | ∞ (never caught) | < 1 hour |

---

## Lessons Learned

1. **"Tests pass" ≠ "Feature works"**
   - Tests verified data correctness, not user experience
   - Need end-to-end visibility tests

2. **Silent failures are the worst failures**
   - Add assertions that fail loudly when rendering produces nothing
   - Log warnings for characters with no vertex output

3. **Special characters need special tests**
   - Don't assume font rendering handles everything
   - Test each Unicode block that requires special handling

4. **Integration testing is critical**
   - Unit tests for each component passed
   - Integration between components failed
   - Need tests that exercise the full pipeline

---

## Action Items

| Action | Owner | Status | Due |
|--------|-------|--------|-----|
| Add `requires_special_rendering()` function | WORKER | DONE (via `box_drawing::is_box_drawing`) | Jan 2 |
| Add box drawing vertex test | WORKER | DONE (tests in `box_drawing.rs`) | Jan 2 |
| Add powerline vertex test | WORKER | DONE (tests in `box_drawing.rs`) | Jan 2 |
| Create test fixture files | WORKER | DONE (iter 426) | Jan 3 |
| Implement box drawing rendering | WORKER | DONE (in `box_drawing.rs`) | Jan 3-5 |
| Add visual regression framework | WORKER | **DONE** (iter 427) | Jan 6-7 |

### Fixture Files (Iteration 426)

Created comprehensive test fixtures in `crates/dterm-core/tests/fixtures/`:
- `box_drawing_comprehensive.txt` - All U+2500-U+257F characters (128)
- `block_elements.txt` - All U+2580-U+259F characters (32)
- `powerline_glyphs.txt` - All U+E0A0-U+E0D7 characters (56)

Added fixture-based tests in `src/gpu/box_drawing.rs`:
- `test_fixture_box_drawing_generates_vertices`
- `test_fixture_block_elements_generates_vertices`
- `test_fixture_powerline_generates_vertices`
- `test_fixture_parsing`

### Visual Regression Framework (Iteration 427)

Implemented full visual regression testing infrastructure:

**New Files:**
- `src/gpu/visual_testing.rs` - Visual test harness with:
  - `VisualTestHarness` - Headless GPU context for offscreen rendering
  - `CompareConfig` - Configurable comparison settings
  - `CompareResult` - Detailed diff information
  - Golden image comparison with diff image generation
- `src/tests/visual_regression.rs` - 8 GPU visual tests

**Feature:**
- `visual-testing` feature in Cargo.toml enables visual tests
- Depends on `image` crate for PNG comparison

**Usage:**
```bash
# Run visual tests
cargo test --package dterm-core --features visual-testing -- visual_regression

# Generate golden images
UPDATE_GOLDEN=1 cargo test --package dterm-core --features visual-testing -- visual_regression
```

**Tests Added:**
- `test_box_drawing_3x3_produces_output` - Verify box drawing renders
- `test_box_drawing_comprehensive_produces_output` - Test sample of all box chars
- `test_block_elements_produce_output` - Verify block elements render
- `test_powerline_glyphs_produce_output` - Verify powerline renders
- `test_mixed_content_produces_output` - Combined content test
- `test_box_drawing_golden` - Golden image comparison
- `test_block_elements_golden` - Golden image comparison
- `test_powerline_golden` - Golden image comparison

---

*This bug was preventable. We must not ship invisible characters again.*
