# Metal Shader Migration Guide for DashTerm2

**Date:** 2025-12-31
**Iteration:** 459
**Status:** Ready for integration

---

## Overview

dterm-core has migrated to a new 7-bit vertex flag layout for GPU rendering. This document provides the Metal shader code needed for DashTerm2 integration.

## DashTerm2 Flag Quick Reference (Required)

DashTerm2 must use the new 7-bit layout values in its Metal shader:

- `VERTEX_TYPE_DECORATION` = 2
- `OVERLAY_CURSOR` = 32
- `OVERLAY_SELECTION` = 64

Do not use the legacy values (2048/64/128); those were tied to the old scattered-bit layout.
If you are consuming the generated C header (`packages/dterm-swift/Sources/CDTermCore/include/dterm.h`),
note that it still defines legacy `FLAG_*` macros for compatibility. Use the `VERTEX_TYPE_*`,
`EFFECT_*`, and `OVERLAY_*` macros for the 7-bit layout.

---

## Flag Layout Comparison

### Old Layout (Scattered Bits)

```c
// Old flags - scattered across many bits
#define FLAG_BOLD           1      // bit 0  - not used in shader
#define FLAG_DIM            2      // bit 1
#define FLAG_UNDERLINE      4      // bit 2  - not used in shader
#define FLAG_BLINK          8      // bit 3
#define FLAG_INVERSE        16     // bit 4
#define FLAG_STRIKETHROUGH  32     // bit 5  - not used in shader
#define FLAG_IS_CURSOR      64     // bit 6  - legacy shader
#define FLAG_IS_SELECTION   128    // bit 7  - legacy shader
#define FLAG_IS_BACKGROUND  256    // bit 8
#define FLAG_IS_DECORATION  2048   // bit 11
```

### New Layout (Compact 7 Bits)

```metal
// New flags - compact 7-bit layout
// Bit:  6  5  4  3  2  1  0
//      [OV][  EFF  ][ TYPE ]

// Vertex type (bits 0-1)
constant uint VERTEX_TYPE_MASK = 3;
constant uint VERTEX_TYPE_GLYPH = 0;       // Sample from atlas
constant uint VERTEX_TYPE_BACKGROUND = 1;  // Solid bg color
constant uint VERTEX_TYPE_DECORATION = 2;  // Solid fg color (underlines, box drawing)

// Effects (bits 2-4)
constant uint EFFECT_DIM = 4;      // 1 << 2
constant uint EFFECT_BLINK = 8;    // 1 << 3
constant uint EFFECT_INVERSE = 16; // 1 << 4

// Overlays (bits 5-6)
constant uint OVERLAY_CURSOR = 32;    // 1 << 5
constant uint OVERLAY_SELECTION = 64; // 1 << 6
```

---

## Metal Shader Template

The following Metal shader code implements the new flag layout:

```metal
#include <metal_stdlib>
using namespace metal;

// =============================================================================
// Vertex Flag Constants (New 7-bit Layout)
// =============================================================================

// Vertex type (bits 0-1) - determines shader path
constant uint VERTEX_TYPE_MASK = 3u;
constant uint VERTEX_TYPE_GLYPH = 0u;       // Sample atlas texture
constant uint VERTEX_TYPE_BACKGROUND = 1u;  // Solid background color
constant uint VERTEX_TYPE_DECORATION = 2u;  // Solid foreground (box drawing, underlines)

// Effect flags (bits 2-4) - modify appearance
constant uint EFFECT_DIM = 4u;      // 1 << 2 - reduce brightness 50%
constant uint EFFECT_BLINK = 8u;    // 1 << 3 - animate alpha
constant uint EFFECT_INVERSE = 16u; // 1 << 4 - swap fg/bg colors

// Overlay flags (bits 5-6) - cursor and selection
constant uint OVERLAY_CURSOR = 32u;    // 1 << 5 - cursor highlight
constant uint OVERLAY_SELECTION = 64u; // 1 << 6 - selection highlight

// =============================================================================
// Helper Functions
// =============================================================================

// Extract vertex type from flags
uint get_vertex_type(uint flags) {
    return flags & VERTEX_TYPE_MASK;
}

// =============================================================================
// Uniforms
// =============================================================================

struct Uniforms {
    float viewport_width;
    float viewport_height;
    float cell_width;
    float cell_height;
    float atlas_size;
    float time;
    int cursor_x;
    int cursor_y;
    float4 cursor_color;
    float4 selection_color;
    uint cursor_style;     // 0=Block, 1=Underline, 2=Bar
    uint cursor_blink_ms;  // 0 = no blink
};

// =============================================================================
// Vertex Types
// =============================================================================

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 uv       [[attribute(1)]];
    float4 fg_color [[attribute(2)]];
    float4 bg_color [[attribute(3)]];
    uint   flags    [[attribute(4)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float4 fg_color;
    float4 bg_color;
    uint   flags;
    float2 cell_position;
};

// =============================================================================
// Vertex Shader
// =============================================================================

vertex VertexOut vertex_main(
    VertexIn in [[stage_in]],
    constant Uniforms &uniforms [[buffer(1)]]
) {
    VertexOut out;

    // Convert cell position to NDC (-1 to 1)
    float x = (in.position.x * uniforms.cell_width / uniforms.viewport_width) * 2.0 - 1.0;
    float y = 1.0 - (in.position.y * uniforms.cell_height / uniforms.viewport_height) * 2.0;

    out.position = float4(x, y, 0.0, 1.0);
    out.uv = in.uv;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    out.flags = in.flags;
    out.cell_position = in.position;

    return out;
}

// =============================================================================
// Fragment Shader
// =============================================================================

// Cursor blink factor (0.0 or 1.0)
float cursor_blink_factor(constant Uniforms &uniforms) {
    if (uniforms.cursor_blink_ms == 0u) {
        return 1.0;
    }
    float blink_period = float(uniforms.cursor_blink_ms) / 1000.0;
    float cycle = fract(uniforms.time / (blink_period * 2.0));
    return step(cycle, 0.5);
}

// Check if fragment is in cursor region based on cursor style
float is_in_cursor_region(
    float local_x,
    float local_y,
    constant Uniforms &uniforms
) {
    if (uniforms.cursor_style == 0u) {
        // Block cursor: entire cell
        return 1.0;
    } else if (uniforms.cursor_style == 1u) {
        // Underline cursor: bottom 10%
        float underline_height = max(0.1, 2.0 / uniforms.cell_height);
        return (local_y >= (1.0 - underline_height)) ? 1.0 : 0.0;
    } else if (uniforms.cursor_style == 2u) {
        // Bar cursor: left 10%
        float bar_width = max(0.1, 2.0 / uniforms.cell_width);
        return (local_x <= bar_width) ? 1.0 : 0.0;
    }
    return 0.0;
}

fragment float4 fragment_main(
    VertexOut in [[stage_in]],
    texture2d<float> atlas [[texture(0)]],
    sampler atlas_sampler [[sampler(0)]],
    constant Uniforms &uniforms [[buffer(1)]]
) {
    // Route based on vertex type (bits 0-1)
    uint vertex_type = get_vertex_type(in.flags);

    // ==== DECORATION PASS ====
    // Decorations (underlines, strikethrough, box drawing) are solid color quads
    if (vertex_type == VERTEX_TYPE_DECORATION) {
        float4 color = in.fg_color;

        // Handle inverse video - swap to bg_color
        if ((in.flags & EFFECT_INVERSE) != 0u) {
            color = in.bg_color;
        }

        // Handle dim - reduce brightness
        if ((in.flags & EFFECT_DIM) != 0u) {
            color = float4(color.rgb * 0.5, color.a);
        }

        return color;
    }

    // ==== BACKGROUND PASS ====
    if (vertex_type == VERTEX_TYPE_BACKGROUND) {
        float4 color = in.bg_color;

        // Handle inverse video
        if ((in.flags & EFFECT_INVERSE) != 0u) {
            color = in.fg_color;
        }

        // Handle selection highlight
        if ((in.flags & OVERLAY_SELECTION) != 0u) {
            color = mix(color, uniforms.selection_color, uniforms.selection_color.a);
        }

        // Handle cursor
        if ((in.flags & OVERLAY_CURSOR) != 0u) {
            float local_x = fract(in.cell_position.x);
            float local_y = fract(in.cell_position.y);
            float in_cursor = is_in_cursor_region(local_x, local_y, uniforms);

            if (in_cursor > 0.5) {
                float blink = cursor_blink_factor(uniforms);
                color = mix(color, uniforms.cursor_color, blink);
            }
        }

        return color;
    }

    // ==== GLYPH PASS ====
    // Default: vertex_type == VERTEX_TYPE_GLYPH
    float alpha = atlas.sample(atlas_sampler, in.uv).r;

    // Discard transparent pixels
    if (alpha < 0.01) {
        discard_fragment();
    }

    float4 color = in.fg_color;

    // Handle inverse video
    if ((in.flags & EFFECT_INVERSE) != 0u) {
        color = in.bg_color;
    }

    // Handle dim
    if ((in.flags & EFFECT_DIM) != 0u) {
        color = color * 0.5;
    }

    // Handle blink
    if ((in.flags & EFFECT_BLINK) != 0u) {
        float blink = step(0.5, fract(uniforms.time * 2.0));
        color.a = color.a * blink;
    }

    // Apply atlas alpha
    color.a = color.a * alpha;

    return color;
}
```

---

## Migration Checklist

### Step 1: Update Flag Constants

Replace old flag constants with new ones:

| Old Constant | New Constant | Change |
|--------------|--------------|--------|
| `FLAG_IS_BACKGROUND` (256) | `VERTEX_TYPE_BACKGROUND` (1) | Check bits 0-1 |
| `FLAG_IS_DECORATION` (2048) | `VERTEX_TYPE_DECORATION` (2) | Check bits 0-1 |
| `FLAG_DIM` (2) | `EFFECT_DIM` (4) | Bit 2 instead of bit 1 |
| `FLAG_BLINK` (8) | `EFFECT_BLINK` (8) | Same bit (unchanged) |
| `FLAG_INVERSE` (16) | `EFFECT_INVERSE` (16) | Same bit (unchanged) |
| `FLAG_IS_CURSOR` (64) | `OVERLAY_CURSOR` (32) | Bit 5 instead of bit 6 |
| `FLAG_IS_SELECTION` (128) | `OVERLAY_SELECTION` (64) | Bit 6 instead of bit 7 |

### Step 2: Update Fragment Shader Logic

Change from:
```metal
if ((flags & FLAG_IS_BACKGROUND) != 0) { ... }
```

To:
```metal
if (get_vertex_type(flags) == VERTEX_TYPE_BACKGROUND) { ... }
```

### Step 3: Remove Dead Code

Delete these constants (not used in shader):
- `FLAG_BOLD`
- `FLAG_UNDERLINE`
- `FLAG_STRIKETHROUGH`
- `FLAG_DOUBLE_UNDERLINE`
- `FLAG_CURLY_UNDERLINE`
- `FLAG_DOTTED_UNDERLINE`
- `FLAG_DASHED_UNDERLINE`
- `FLAG_DEFAULT_BG`

---

## Testing

After migration:

1. **Background colors** - Should render correctly (vertex type 1)
2. **Glyphs** - Should sample atlas correctly (vertex type 0)
3. **Box drawing** - Should render as solid color (vertex type 2)
4. **Underlines** - Should render as solid color (vertex type 2)
5. **Cursor** - Should highlight with correct style (OVERLAY_CURSOR)
6. **Selection** - Should highlight correctly (OVERLAY_SELECTION)
7. **Dim text** - Should render at 50% brightness (EFFECT_DIM)
8. **Inverse video** - Should swap fg/bg (EFFECT_INVERSE)
9. **Blink** - Should animate alpha (EFFECT_BLINK)

---

## Compatibility Notes

### Gradual Migration

If you need to support both old and new flag formats during migration,
dterm-core provides `VertexFlags::from_legacy()` in Rust which converts
old flags to the new format. The FFI layer can use this for compatibility.

### Performance

The new layout is more efficient:
- Only 7 bits used (vs 15 scattered bits before)
- Simple mask operation for vertex type extraction
- Cleaner shader branching

---

## Reference

- WGSL shader: `crates/dterm-core/src/gpu/shader.wgsl`
- Rust API: `crates/dterm-core/src/gpu/vertex_flags.rs`
- Audit doc: `docs/AUDIT_GPU_FLAGS.md`
