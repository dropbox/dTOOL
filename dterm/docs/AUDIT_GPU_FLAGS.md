# Audit: GPU Vertex Flags

**Date:** 2025-12-31
**Updated:** Iteration 492
**Status:** OPTION B IMPLEMENTED - New `VertexFlags` API available

---

## Current State (New Layout in Use)

- The renderer now uses the compact 7-bit layout (`VertexFlags`, `shader.wgsl`).
- Legacy `FLAG_*` values remain only for migration/FFI compatibility.
- DashTerm2 must update its Metal shader to match the new layout.
- Legacy `FLAG_*` references below are retained for the pre-migration audit.

## Legacy Flag Inventory (Deprecated Layout)

| Flag | Value | Rust | Metal Shader | Actually Used? |
|------|-------|------|--------------|----------------|
| FLAG_BOLD | 1 | Yes | Defined | **NO** - dead code |
| FLAG_DIM | 2 | Yes | Used | Yes - dims color |
| FLAG_UNDERLINE | 4 | Yes | Defined | **NO** - dead code |
| FLAG_BLINK | 8 | Yes | Used | Yes - animates alpha |
| FLAG_INVERSE | 16 | Yes | Used | Yes - swaps fg/bg |
| FLAG_STRIKETHROUGH | 32 | Yes | Defined | **NO** - dead code |
| FLAG_IS_CURSOR | 64 | Yes | Used | Yes - cursor highlight |
| FLAG_IS_SELECTION | 128 | Yes | Used | Yes - selection highlight |
| FLAG_IS_BACKGROUND | 256 | Yes | Used | Yes - solid bg quad |
| FLAG_DOUBLE_UNDERLINE | 512 | Yes | Defined | **NO** - dead code |
| FLAG_CURLY_UNDERLINE | 1024 | Yes | Defined | **NO** - dead code |
| FLAG_IS_DECORATION | 2048 | Yes | Used | Yes - solid fg quad |
| FLAG_DEFAULT_BG | 4096 | Yes | Defined | **NO** - dead code |
| FLAG_DOTTED_UNDERLINE | 8192 | Yes | Defined | **NO** - dead code |
| FLAG_DASHED_UNDERLINE | 16384 | Yes | Defined | **NO** - dead code |

**Result (legacy shader): 8 flags were dead code in the shader.**

---

## Analysis

### Legacy Flags Actually Used by the Old Shader (7)

These flags affected rendering behavior in the old layout:

```metal
// Vertex type routing
FLAG_IS_BACKGROUND  → return bgColor (solid background)
FLAG_IS_DECORATION  → return fgColor (solid foreground, box drawing/underlines)

// Modifiers
FLAG_IS_CURSOR      → blend with cursor color
FLAG_IS_SELECTION   → blend with selection color
FLAG_INVERSE        → swap fg/bg colors
FLAG_DIM            → multiply color by 0.5
FLAG_BLINK          → animate alpha with sin()
```

### Legacy Flags Defined But Never Used (8)

These are **dead code** - defined in shader but never checked:

```
FLAG_BOLD              - Not used (bold is font variant, not shader effect)
FLAG_UNDERLINE         - Not used (underlines are separate vertices)
FLAG_STRIKETHROUGH     - Not used (strikethrough is separate vertices)
FLAG_DOUBLE_UNDERLINE  - Not used (just vertex generation hint)
FLAG_CURLY_UNDERLINE   - Not used (just vertex generation hint)
FLAG_DOTTED_UNDERLINE  - Not used (just vertex generation hint)
FLAG_DASHED_UNDERLINE  - Not used (just vertex generation hint)
FLAG_DEFAULT_BG        - Not used (purpose unclear)
```

---

## Why Refactor Happened (Legacy Layout)

### Conflating Two Concerns

The flags mix two different concepts:

1. **Vertex Type** - What kind of geometry is this?
   - Background quad
   - Glyph quad (needs atlas lookup)
   - Decoration quad (solid color)

2. **Style Attributes** - How to modify appearance?
   - Bold, italic (font selection - not GPU)
   - Dim, blink, inverse (GPU effects)
   - Cursor, selection (overlay effects)

### Clean Architecture Would Be:

```rust
// Vertex type - determines shader path
enum VertexType {
    Background,    // Solid bgColor
    Glyph,         // Sample atlas, apply fgColor
    Decoration,    // Solid fgColor (underlines, box drawing)
}

// Style modifiers - bit flags for effects
struct StyleFlags {
    dim: bool,
    blink: bool,
    inverse: bool,
    cursor: bool,
    selection: bool,
}
```

But instead we have 15 flags all mixed together.

---

## Specific Issues (Legacy Layout)

### Issue 1: FLAG_IS_DECORATION is Overloaded

`FLAG_IS_DECORATION` is used for:
- Underlines (single, double, curly, dotted, dashed)
- Strikethrough
- Box drawing characters
- Block elements
- Powerline glyphs

All these different things use the same flag. The shader just returns `fgColor` for all of them.

### Issue 2: Underline Type Flags Are Useless

These flags exist but the shader doesn't use them:
- FLAG_DOUBLE_UNDERLINE = 512
- FLAG_CURLY_UNDERLINE = 1024
- FLAG_DOTTED_UNDERLINE = 8192
- FLAG_DASHED_UNDERLINE = 16384

They're set during vertex generation but the shader treats all decorations the same.

### Issue 3: Collision Risk

The other AI proposed `FLAG_IS_GEOMETRIC = 512`, but that value is **already used** by `FLAG_DOUBLE_UNDERLINE`! This would cause a collision.

### Issue 4: FLAG_DEFAULT_BG Purpose Unclear

`FLAG_DEFAULT_BG = 4096` is defined but never used in the shader. What is it for?

---

## Recommendations

### Option A: Minimal Cleanup (Low Risk)

1. Remove unused flag definitions from Metal shader
2. Keep Rust flags (they're used for vertex generation)
3. Add comments explaining which flags affect shader vs generation

### Option B: Proper Refactor (Medium Risk)

1. Replace single u32 flags with structured data:
   ```rust
   struct VertexData {
       vertex_type: VertexType,  // enum: Background, Glyph, Decoration
       effects: EffectFlags,     // bit flags: dim, blink, inverse
       overlays: OverlayFlags,   // bit flags: cursor, selection
   }
   ```

2. Update shader to use cleaner logic:
   ```metal
   switch (in.vertex_type) {
       case Background: return bgColor;
       case Decoration: return fgColor;
       case Glyph: return sample_atlas();
   }
   ```

### Option C: Just Fix the Bug (Minimal)

If box drawing doesn't render, check:
1. Is `FLAG_IS_DECORATION` being set on box drawing vertices?
2. Is the shader receiving the flag correctly?

The flags ARE ugly but they should work if used correctly.

---

## What dterm-core Should Do

### Immediate (No Code Changes Needed)

The box drawing implementation looks correct:
- `box_drawing.rs` sets `VERTEX_TYPE_DECORATION` on all vertices
- Shader routes decoration quads via `VERTEX_TYPE_DECORATION`
- This should work

### If Box Drawing Still Broken

Add debug logging to verify:
```rust
// In ffi.rs around line 2620
if super::box_drawing::is_box_drawing(resolved.glyph) {
    let verts = generate_box_drawing_vertices(...);
    // Verify vertices are decoration type (bits 0-1 == 2)
    assert!(verts
        .iter()
        .all(|v| (v.flags & VERTEX_TYPE_MASK) == VERTEX_TYPE_DECORATION));
}
```

### Future Cleanup

1. Remove dead flags from shader (FLAG_BOLD, FLAG_UNDERLINE, etc.)
2. Document which flags are "vertex type" vs "style modifier"
3. Consider enum-based vertex type instead of bit flags

---

## Implementation Status (Iteration 455-458)

### Option B: Fully Implemented ✅

**Iteration 455:** Created `VertexFlags` module with type-safe API
**Iteration 458:** Completed full migration including shader

**Module:** `gpu/vertex_flags.rs`

```rust
// Type-safe vertex type enum (bits 0-1)
pub enum VertexType {
    Glyph = 0,       // Sample atlas texture
    Background = 1,  // Solid background
    Decoration = 2,  // Solid foreground (box drawing, underlines)
}

// Effect flags (bits 2-4)
pub struct EffectFlags {
    DIM, BLINK, INVERSE
}

// Overlay flags (bits 5-6)
pub struct OverlayFlags {
    CURSOR, SELECTION
}

// Combined struct with builder pattern
let flags = VertexFlags::decoration().with_dim().with_cursor();
let packed: u32 = flags.pack();
```

**Bit Layout (New):**
```
Bit:  31                    7  6  5  4  3  2  1  0
      [      Reserved      ][OV][  EFF  ][ TYPE ]
```

### Migration Complete ✅

**Phase 1: Rust API Migration** ✅
- `VertexFlags` module provides type-safe construction
- `VertexFlags::from_legacy()` converts old `FLAG_*` constants

**Phase 2: Shader Migration** ✅ (Iteration 458)
- `shader.wgsl` updated to use new 7-bit layout
- Uses `get_vertex_type()` helper for clean routing
- Old FLAG_* constants marked as legacy with deprecation docs

**Files Modified in Iteration 458:**
- `gpu/shader.wgsl` - New flag constants and fragment shader logic
- `gpu/pipeline.rs` - Uses `VertexFlags` for vertex generation
- `gpu/box_drawing.rs` - Uses `VERTEX_TYPE_DECORATION`

**Key Insight:** The new API uses different bit positions than the old flags:

| Concept | Old Bits | New Bits |
|---------|----------|----------|
| Background | 256 (bit 8) | 1 (bits 0-1) |
| Decoration | 2048 (bit 11) | 2 (bits 0-1) |
| Dim | 2 (bit 1) | 4 (bit 2) |
| Cursor | 64 (bit 6) | 32 (bit 5) |
| Selection | 128 (bit 7) | 64 (bit 6) |

**DashTerm2 Impact:** The Metal shader in DashTerm2 needs to be updated
to use the new bit layout. Until then, FFI code should continue using
`VertexFlags::from_legacy()` for conversion.

---

## Conclusion

**Migration complete.** The new 7-bit flag layout is now used throughout:

1. **shader.wgsl** - Uses new constants (`VERTEX_TYPE_*`, `EFFECT_*`, `OVERLAY_*`)
2. **pipeline.rs** - Generates vertices with new flag format via `VertexFlags`
3. **box_drawing.rs** - Uses `VERTEX_TYPE_DECORATION` for all box drawing

**Next steps:**
1. Update DashTerm2 Metal shader to use new bit layout
   - **Template available:** `docs/METAL_SHADER_MIGRATION.md`
2. Optionally deprecate/remove old `FLAG_*` constants after DashTerm2 migration

---

## DashTerm2 Integration

**Iteration 459:** Metal shader migration guide created.

See `docs/METAL_SHADER_MIGRATION.md` for:
- Complete Metal shader template with new flag constants
- Step-by-step migration checklist
- Testing guide
- Compatibility notes for gradual migration
