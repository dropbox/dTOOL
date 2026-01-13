# DTermCore Full Parity Roadmap

**Date:** 2025-12-31
**Goal:** Achieve 100% feature parity so we can DELETE all legacy iTerm2 parsing/rendering code
**Status:** ~75% complete

---

## Executive Summary

DTermCore (Rust) is our future. Once we achieve full parity, we will:
1. **DELETE** VT100Parser.m and all legacy parsing code
2. **DELETE** iTermTextDrawingHelper legacy rendering paths
3. **DELETE** duplicate state management in VT100Screen
4. Ship a faster, safer, more maintainable terminal

**Current blocker:** Box drawing characters are invisible because dterm-core grid/renderer don't implement special character rendering. This is why all dterm-core settings are currently disabled.

---

## Current Feature Coverage

| Category | Coverage | Status |
|----------|----------|--------|
| Basic Terminal Emulation | 95% | Nearly complete |
| SGR Attributes | 95% | Missing dotted/dashed underline |
| Mouse Support | 100% | **COMPLETE** |
| OSC Sequences | 80% | Missing some iTerm2-specific |
| DCS Sequences | 40% | Major gaps |
| Private Modes | 95% | Nearly complete |
| Image Protocols | 70% | Missing iTerm2 inline images |
| Special Character Rendering | 0% | **CRITICAL GAP** |

---

## Phase 1: CRITICAL - Enable dterm-core by Default (P0)

**Goal:** Fix blocking issues so dterm-core can be re-enabled

### 1.1 Box Drawing Character Rendering
**Files:** `DTermCore/src/renderer/`, `DTermMetalView.swift`
**Effort:** 3-5 days

The dterm-core renderer must detect and render box drawing characters using bezier paths, matching `iTermBoxDrawingBezierCurveFactory.m`.

**Characters to support:**
```
U+2500-U+257F  Box Drawing (light, heavy, double lines, corners, tees)
U+2580-U+259F  Block Elements (shades, quadrants)
U+25E2-U+25FF  Geometric shapes (triangles)
U+1FB00-U+1FB3C Sextant characters
```

**Implementation:**
1. Port `iTermBoxDrawingBezierCurveFactory` bezier path generation to Rust
2. Add box drawing detection in `DTermMetalView` vertex generation
3. Generate Metal vertex data for bezier paths
4. Test with: `cat /Users/ayates/dashterm2/tests/box_drawing.txt`

### 1.2 Powerline Glyph Rendering
**Files:** `DTermCore/src/renderer/`
**Effort:** 2-3 days

```
U+E0A0-U+E0A3  Version control symbols
U+E0B0-U+E0D7  Arrow and separator glyphs
```

**Implementation:**
1. Detect Powerline code points in renderer
2. Use PDF assets or generate bezier paths
3. Handle wide vs narrow rendering based on `makeSomePowerlineSymbolsWide`

### 1.3 Underline Style Completion
**Files:** `DTermCore/src/cell.rs`, `DTermCore/src/parser/sgr.rs`
**Effort:** 1 day

Add missing underline styles:
- `SGR 4:4` - Dotted underline
- `SGR 4:5` - Dashed underline

Update `CellFlags` to include bits for these styles.

---

## Phase 2: Image Protocol Completion (P1)

### 2.1 iTerm2 Inline Image Protocol
**Files:** `DTermCore/src/osc/`, new Rust module
**Effort:** 3-5 days

Currently missing entirely. Need to implement:
- OSC 1337 parsing (File=name=...:inline=1:...)
- Base64 image decoding
- Image storage and retrieval via FFI
- Placeholder cell generation

**Reference:** `VT100InlineImageHelper.m`

### 2.2 Sixel Rendering Integration
**Status:** Parsing complete, rendering integration needed
**Effort:** 1-2 days

Sixel images are parsed but may not render correctly through dterm-core renderer.

### 2.3 Kitty Graphics Polish
**Status:** Protocol complete, verify all features
**Effort:** 1 day

Verify: animations, compositing, virtual placements

---

## Phase 3: Advanced Terminal Features (P2)

### 3.1 DCS Sequence Completion
**Effort:** 5-7 days

| Sequence | Description | Priority |
|----------|-------------|----------|
| DECRQPSR | Presentation State Report | Medium |
| DECRQCRA | Checksum Rectangular Area | Low |
| DRCS | Downloadable Character Sets | Low |
| DA3 | Tertiary Device Attributes | Low |

### 3.2 Rectangular Area Operations
**Effort:** 3-4 days

| Sequence | Description |
|----------|-------------|
| DECCARA | Change Attributes in Rectangular Area |
| DECRARA | Reverse Attributes in Rectangular Area |
| DECFRA | Fill Rectangular Area |
| DECERA | Erase Rectangular Area |
| DECIC | Insert Column |
| DECDC | Delete Column |

### 3.3 Double Height/Width Lines
**Effort:** 2-3 days

- DECDHL (Double Height Line) - top/bottom halves
- DECDWL (Double Width Line)

---

## Phase 4: iTerm2-Specific Features (P3)

### 4.1 tmux Integration Mode
**Effort:** 5-7 days

DCS hooks for tmux control mode. This is complex because it involves bidirectional communication.

### 4.2 SSH Conductor Mode
**Effort:** 3-5 days

iTerm2's SSH integration for file transfer, remote commands.

### 4.3 Shell Integration Enhancements
**Effort:** 2-3 days

Extended OSC 133 features specific to iTerm2.

---

## Phase 5: DELETE Legacy Code (Final)

Once all phases complete, systematically remove:

### Parsing Layer Deletion
```
DELETE: sources/VT100Parser.m
DELETE: sources/VT100Token.m
DELETE: sources/VT100StateMachine.m
DELETE: sources/VT100CSIParser.m
DELETE: sources/VT100OscParser.m
DELETE: sources/VT100DcsParser.m
```

### Rendering Layer Deletion
```
DELETE: sources/iTermTextDrawingHelper.m (legacy paths)
DELETE: sources/iTermAttributedStringBuilder.m (legacy paths)
SIMPLIFY: sources/iTermMetalPerFrameState.m (remove dual-path logic)
```

### State Management Consolidation
```
SIMPLIFY: sources/VT100Screen.m (use dterm-core as source of truth)
SIMPLIFY: sources/VT100ScreenState.m
DELETE: Dual-buffer synchronization code
```

### Settings Cleanup
```
DELETE: dtermCoreEnabled (always YES)
DELETE: dtermCoreParserOutputEnabled (always YES)
DELETE: dtermCoreGridEnabled (always YES)
DELETE: dtermCoreRendererEnabled (always YES)
DELETE: dtermCoreValidationEnabled (no longer needed)
DELETE: dtermCoreParserComparisonEnabled (no longer needed)
```

---

## Timeline Estimate

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| Phase 1 (Critical) | 2 weeks | None |
| Phase 2 (Images) | 1.5 weeks | Phase 1 |
| Phase 3 (Advanced) | 2 weeks | Phase 1 |
| Phase 4 (iTerm2) | 2 weeks | Phase 1-3 |
| Phase 5 (Deletion) | 1 week | Phase 1-4 |

**Total:** ~8-9 weeks to full parity and legacy deletion

---

## Testing Requirements

Each phase must include:

1. **Unit tests** in Rust for parser correctness
2. **Integration tests** comparing dterm-core output to reference
3. **Visual regression tests** for rendering
4. **vttest compatibility** - must pass all vttest suites
5. **Real-world testing** with: vim, tmux, htop, mc, ncurses apps

### Test Files to Create
```
tests/box_drawing_comprehensive.txt
tests/powerline_glyphs.txt
tests/block_elements.txt
tests/underline_styles.txt
tests/image_protocols/
tests/dcs_sequences/
```

---

## Success Criteria

dterm-core is at full parity when:

1. [ ] All 4 dterm-core settings can be enabled by default
2. [ ] Box drawing characters render identically to legacy
3. [ ] Powerline prompts render correctly
4. [ ] All image protocols work (Sixel, Kitty, iTerm2)
5. [ ] vttest passes 100%
6. [ ] No visual differences in: vim, tmux, htop, mc
7. [ ] Performance is equal or better than legacy
8. [ ] All legacy parsing code can be deleted without regression

---

## Message to DTermCore Team

**PRIORITY DIRECTIVE:**

We are committing to dterm-core as our ONLY terminal engine. The goal is to DELETE all legacy iTerm2 parsing and rendering code once parity is achieved.

**Immediate focus (this week):**
1. Box drawing character rendering in DTermMetalView
2. Powerline glyph rendering
3. Dotted/dashed underline support

**Why this matters:**
- Single codebase = fewer bugs
- Rust = memory safety, no more crashes from buffer overflows
- Performance = Rust parser is already faster
- Maintainability = one system to understand, not two

**Current blocker:**
Box drawing is invisible, forcing us to disable dterm-core entirely. This is unacceptable for a shipping terminal.

**Action required:**
Start with Phase 1.1 (box drawing) immediately. Once box drawing works, we can re-enable dterm-core and iterate on the remaining features.

---

## References

- `sources/iTermBoxDrawingBezierCurveFactory.m` - Box drawing bezier path generation
- `sources/iTermCharacterSource.m` - Character rendering dispatch
- `sources/VT100Parser.m` - Legacy parser (to be deleted)
- `DTermCore/src/` - Rust terminal engine
- `tests/box_drawing.txt` - Test file for box drawing
- `tests/osc8.txt` - Hyperlink test file
