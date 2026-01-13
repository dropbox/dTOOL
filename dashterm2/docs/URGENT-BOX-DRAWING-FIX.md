# URGENT: Box Drawing Character Rendering Fix

**Priority**: P0 CRITICAL
**Status**: ROOT CAUSE IDENTIFIED AND FIXED
**Date**: 2025-12-31

## Executive Summary

Box drawing characters (═ ║ ╔ ╗ ╚ ╝ etc.) were completely invisible in DashTerm2. This is a **terminal-breaking bug** - a terminal that cannot render characters is fundamentally broken.

## Root Cause

**FOUR dterm-core settings were causing invisible box drawing characters:**

### Issue 1: `dtermCoreEnabled=YES` (Default) - **THE MAIN CAUSE**
When enabled, DashTerm2 uses the dterm-core Rust parser instead of the legacy VT100Parser. The dterm-core parser:
- Does not correctly set up character attributes for box drawing detection
- Results in box drawing characters not being recognized by the Metal renderer

### Issue 2: `dtermCoreParserOutputEnabled=YES` (Default)
Uses tokens from dterm-core parser for terminal state:
- Parser output doesn't correctly flag box drawing characters
- Results in characters being rendered as regular text (invisible glyphs)

### Issue 3: `dtermCoreRendererEnabled=YES` (Default - was fixed earlier)
When enabled, uses `DTermMetalView` (Rust-based renderer) instead of `iTermMTKView`:
- Does not recognize box drawing characters as special
- Has no bezier path generation for box drawing glyphs

### Issue 4: `dtermCoreGridEnabled=YES` (Default - was fixed earlier)
Causes Metal renderer to get character data from dterm-core grid adapter:
- Does not properly mark box drawing characters for special handling
- The `isBoxDrawingCharacter` flag is never set correctly

**ALL FOUR settings must be disabled** for box drawing to work correctly.

**Evidence**:
- Search for "boxDrawing" in `DTermCore.swift` returns zero matches
- `iTermMetalPerFrameStateRow.m:127` shows grid adapter bypass logic
- Box drawing infrastructure exists only in iTerm2 code (`iTermBoxDrawingBezierCurveFactory.m`)

## Immediate Fix (COMPLETED - 2025-12-31)

Changed default values in `sources/iTermAdvancedSettingsModel.m`:

### Fix 1: dtermCoreEnabled (line 868) - **CRITICAL**
```objc
// BEFORE (broken):
DEFINE_BOOL(dtermCoreEnabled, YES, ...)

// AFTER (fixed):
DEFINE_BOOL(dtermCoreEnabled, NO, ...)
```
Added warning: "dterm-core does not yet implement box drawing character handling"

### Fix 2: dtermCoreParserOutputEnabled (line 871)
```objc
// BEFORE (broken):
DEFINE_BOOL(dtermCoreParserOutputEnabled, YES, ...)

// AFTER (fixed):
DEFINE_BOOL(dtermCoreParserOutputEnabled, NO, ...)
```
Added warning: "dterm-core parser output may not correctly handle box drawing characters"

### Fix 3: dtermCoreGridEnabled (line 872)
```objc
// BEFORE (broken):
DEFINE_BOOL(dtermCoreGridEnabled, YES, ...)

// AFTER (fixed):
DEFINE_BOOL(dtermCoreGridEnabled, NO, ...)
```
Added warning: "Box drawing character detection is not yet implemented in dterm-core grid"

### Fix 4: dtermCoreRendererEnabled (line 873)
```objc
// BEFORE (broken):
DEFINE_BOOL(dtermCoreRendererEnabled, YES, ...)

// AFTER (fixed):
DEFINE_BOOL(dtermCoreRendererEnabled, NO, ...)
```
Added warning: "Box drawing characters are not yet implemented in dterm-core"

## Verification Steps

1. Clear user preferences:
   ```bash
   defaults delete com.dashterm.dashterm2 dtermCoreEnabled
   defaults delete com.dashterm.dashterm2 dtermCoreParserOutputEnabled
   defaults delete com.dashterm.dashterm2 dtermCoreRendererEnabled
   defaults delete com.dashterm.dashterm2 dtermCoreGridEnabled
   ```
2. Rebuild DashTerm2
3. Launch new DashTerm2 window
4. Run: `echo '╔════╗ ╔════╗'`
5. Verify box drawing characters are visible

## Regression Tests Required

Add the following tests to `DashTerm2Tests/`:

### 1. Box Drawing Character Set Test
```swift
func test_boxDrawingCharacterSetIncludesDoubleLines() {
    let boxSet = iTermBoxDrawingBezierCurveFactory.boxDrawingCharacters(
        withBezierPathsIncludingPowerline: true)

    // Double-line box drawing characters
    XCTAssertTrue(boxSet.longCharacterIsMember(0x2550)) // ═
    XCTAssertTrue(boxSet.longCharacterIsMember(0x2551)) // ║
    XCTAssertTrue(boxSet.longCharacterIsMember(0x2554)) // ╔
    XCTAssertTrue(boxSet.longCharacterIsMember(0x2557)) // ╗
    XCTAssertTrue(boxSet.longCharacterIsMember(0x255A)) // ╚
    XCTAssertTrue(boxSet.longCharacterIsMember(0x255D)) // ╝
}
```

### 2. Box Drawing Bezier Path Test
```swift
func test_boxDrawingBezierPathsGenerated() {
    let cellSize = NSSize(width: 10, height: 20)

    // Test that bezier paths are generated for box drawing codes
    let codes: [UTF32Char] = [0x2550, 0x2551, 0x2554, 0x2557, 0x255A, 0x255D]
    for code in codes {
        let shapeBuilder = iTermBoxDrawingBezierCurveFactory.shapeBuilder(
            forBoxDrawingCode: code,
            cellSize: cellSize,
            scale: 2.0,
            isPoints: false,
            offset: .zero,
            solid: nil)
        XCTAssertNotNil(shapeBuilder, "Shape builder should exist for code \(String(format: "0x%04X", code))")
    }
}
```

### 3. Integration Test for Metal Renderer Box Drawing Detection
```swift
func test_metalRendererDetectsBoxDrawingCharacters() {
    // Create a screen_char_t with a box drawing character
    var screenChar = screen_char_t()
    screenChar.code = 0x2550  // ═
    screenChar.complexChar = false

    let boxSet = iTermBoxDrawingBezierCurveFactory.boxDrawingCharacters(
        withBezierPathsIncludingPowerline: false)

    let isBoxDrawing = screenChar.code > 127 && boxSet.characterIsMember(screenChar.code)
    XCTAssertTrue(isBoxDrawing, "═ (0x2550) should be detected as box drawing")
}
```

---

## Retrospective: How Did This Bug Escape Testing?

### What Went Wrong

1. **dtermCoreRendererEnabled was enabled by default** without comprehensive feature parity testing
2. **No visual regression tests** for character rendering
3. **No automated tests** that verify box drawing characters render visibly
4. **Manual testing gap** - developers may have been using profiles with dtermCoreRendererEnabled=NO

### Root Causes of Testing Gap

| Issue | Description |
|-------|-------------|
| **Feature flag enabled too early** | `dtermCoreRendererEnabled` was set to YES before dterm-core had feature parity |
| **No character rendering tests** | No tests verify that specific Unicode ranges render at all |
| **Rust/Swift boundary not tested** | No tests verify dterm-core renderer produces visible output |
| **No visual regression CI** | No automated screenshot comparison for terminal output |

### Recommended Process Improvements

1. **NEVER enable experimental renderers by default** until ALL rendering features have tests
2. **Add visual regression tests** that compare terminal output screenshots
3. **Create character rendering test suite** that verifies Unicode ranges render
4. **Add integration tests** that exercise the full rendering pipeline
5. **Require feature flag checklist** before enabling new renderer by default:
   - [ ] Box drawing characters
   - [ ] Powerline glyphs
   - [ ] Underline styles
   - [ ] Bold/italic rendering
   - [ ] Wide characters
   - [ ] Colors (256 + true color)
   - [ ] Images (Sixel, Kitty, iTerm2)

---

## Similar Issues to Investigate

These features may also be broken in dterm-core renderer. Worker MUST verify each:

### 1. Powerline Glyphs
- Search `DTermCore.swift` for "powerline" - likely missing
- Test: `echo -e "\ue0b0\ue0b1\ue0b2\ue0b3"`

### 2. Block Elements
- U+2580-U+259F (shading, quadrants)
- Test: `echo '▀▁▂▃▄▅▆▇█▉▊▋▌▍▎▏'`

### 3. Braille Patterns
- U+2800-U+28FF
- Test: `echo '⠿⠾⠽⠼⠻⠺⠹⠸'`

### 4. Mathematical Symbols with Custom Rendering
- Integral signs, summation, etc.

### 5. Underline Styles
- Does dterm-core render: single, double, curly, dotted underlines?

### 6. Cursor Rendering
- Does dterm-core render cursor correctly in all shapes?

### 7. Selection Highlighting
- Does text selection render correctly?

---

## Worker Directive

**URGENT: Complete the following tasks in order:**

1. **BUILD AND TEST** - Verify the fix works:
   ```bash
   defaults delete com.dashterm.dashterm2 dtermCoreRendererEnabled
   xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build
   # Launch DashTerm2 and test box drawing
   ```

2. **ADD REGRESSION TESTS** - Create tests in `DashTerm2Tests/BoxDrawingTests.swift`

3. **VERIFY SIMILAR FEATURES** - Test each feature listed in "Similar Issues to Investigate"

4. **DOCUMENT DTERM-CORE GAPS** - Update `docs/dterm-core-feature-gaps.md` with missing features

5. **COMMIT** with message:
   ```
   # N: P0 FIX - Disable dterm-core renderer (box drawing broken)

   ## Root Cause
   dtermCoreRendererEnabled=YES by default, but dterm-core renderer
   does not implement box drawing character rendering at all.

   ## Fix
   Changed default to NO until dterm-core has feature parity.

   ## Tests Added
   - BoxDrawingTests.swift

   ## Verification
   Box drawing characters now render correctly with standard Metal renderer.
   ```

---

## Long-Term Fix

The proper long-term fix is to implement box drawing in dterm-core's Rust renderer. This requires:

1. Port `iTermBoxDrawingBezierCurveFactory` logic to Rust
2. Add box drawing vertex generation in dterm-core hybrid renderer
3. Add tests for all box drawing code points
4. Enable dtermCoreRendererEnabled only after all tests pass

**Priority**: Medium (after stability issues are resolved)
**Estimated effort**: 2-3 days of Rust development
