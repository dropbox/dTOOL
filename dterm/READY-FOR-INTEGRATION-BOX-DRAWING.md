# READY FOR INTEGRATION: Box Drawing

**Date:** 2025-12-31
**Feature:** Box Drawing Character Rendering
**Commits:** #398, #399, #400, #401 (this commit)

---

## What's Ready

### 1. GPU Renderer Fix (Commit #399)

The hybrid renderer now properly renders box drawing characters:

```rust
// In dterm_hybrid_renderer_build():
if super::box_drawing::is_box_drawing(resolved.glyph) {
    let box_verts = super::box_drawing::generate_box_drawing_vertices(
        resolved.glyph,
        u32::from(col),
        u32::from(row),
        resolved.fg,
    );
    for v in box_verts {
        builder.vertices.push(v);
    }
}
```

### 2. Grid Adapter FFI (This Commit)

New FFI function for setting `isBoxDrawingCharacter` flag:

```c
// dterm.h
bool dterm_is_box_drawing_character(uint32_t codepoint);
```

**Usage in DashTerm2:**
```objc
// In dtermGridAdapter or iTermMetalPerFrameState
uint32_t codepoint = dtermCell.codepoint;
cell.isBoxDrawingCharacter = dterm_is_box_drawing_character(codepoint);
```

---

## Unicode Ranges Supported

| Range | Name | Count |
|-------|------|-------|
| U+2500-U+257F | Box Drawing | 128 chars |
| U+2580-U+259F | Block Elements | 32 chars |
| U+25E2-U+25FF | Geometric Shapes | 30 chars |
| U+1FB00-U+1FB3C | Legacy Terminal | 61 chars |

---

## Testing

### In DTermCore

```bash
cargo test --package dterm-core --features ffi
# All tests pass including visibility tests
```

### In DashTerm2

After integration:
```bash
# 1. Enable settings
defaults write com.dashterm2 dtermCoreRendererEnabled -bool YES
defaults write com.dashterm2 dtermCoreGridEnabled -bool YES

# 2. Test box drawing
cat ~/dashterm2/tests/box_drawing.txt

# Expected: Visible box characters, not blank spaces
# ╔══════════════════════════════════════╗
# ║  Box drawing test - all chars visible ║
# ╚══════════════════════════════════════╝
```

---

## DashTerm2 Integration Steps

1. **Pull latest dterm-core:**
   ```bash
   cd ~/dterm && git pull
   ```

2. **Update CDTermCore package:**
   - Regenerate headers: `./scripts/generate-headers.sh`
   - Update Swift package

3. **Use new FFI function in grid adapter:**
   ```objc
   - (void)updateCellFromDterm:(DtermCell)dtermCell atRow:(int)row col:(int)col {
       // ... existing code ...
       cell.isBoxDrawingCharacter = dterm_is_box_drawing_character(dtermCell.codepoint);
   }
   ```

4. **Test with box_drawing.txt**

5. **Re-enable dterm-core settings if tests pass**

---

## Known Limitations

- **Powerline glyphs (U+E0A0-U+E0D7):** Not yet supported
- **Some geometric shapes:** May fall back to font rendering

---

*-- DTermCore AI*
