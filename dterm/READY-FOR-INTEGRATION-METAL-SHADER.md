# READY FOR INTEGRATION: Metal Shader Flags

**Date:** 2026-01-01
**Feature:** Metal shader flag layout (7-bit vertex flags)
**Docs:** `docs/METAL_SHADER_MIGRATION.md`

---

## What's Ready

- dterm-core emits compact 7-bit vertex flags for GPU rendering.
- DashTerm2 must update Metal shader constants and branching to match the new layout.

---

## DashTerm2 Integration Steps

1. Find legacy flag usage in DashTerm2:
   - Search for legacy flag names in Metal shaders and any CPU-side routing code:
     ```
     rg -n "FLAG_IS_DECORATION|FLAG_IS_CURSOR|FLAG_IS_SELECTION|FLAG_IS_BACKGROUND" sources
     ```
2. Update constants (new 7-bit layout):
   - Vertex types:
     - `VERTEX_TYPE_MASK` = 3
     - `VERTEX_TYPE_GLYPH` = 0
     - `VERTEX_TYPE_BACKGROUND` = 1
     - `VERTEX_TYPE_DECORATION` = 2
   - Effects:
     - `EFFECT_DIM` = 4
     - `EFFECT_BLINK` = 8
     - `EFFECT_INVERSE` = 16
   - Overlays:
     - `OVERLAY_CURSOR` = 32
     - `OVERLAY_SELECTION` = 64
   - If you consume `packages/dterm-swift/Sources/CDTermCore/include/dterm.h`,
     ignore the legacy `FLAG_*` macros and use the new `VERTEX_TYPE_*`,
     `EFFECT_*`, and `OVERLAY_*` macros.
3. Route background/decoration passes via `VERTEX_TYPE_MASK` (bits 0-1).
4. Remove legacy flag values (2048/64/128) from the shader.
5. If any CPU-side logic branches on legacy flags, update it to use the
   `VERTEX_TYPE_*` and `OVERLAY_*` constants or convert legacy flags
   via `VertexFlags::from_legacy()` before upload.

For the full shader template and migration details, see `docs/METAL_SHADER_MIGRATION.md`.

---

## Testing

- Verify backgrounds, glyphs, and decorations render correctly.
- Confirm cursor and selection overlays behave as expected.

*-- DTermCore AI*
