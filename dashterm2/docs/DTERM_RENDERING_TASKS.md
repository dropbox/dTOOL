# dterm-core Rendering Architecture: Box Drawing Support

**Date:** 2025-12-31
**Status:** Design Specification
**Affects:** dterm-core hybrid renderer (`~/dterm`), DashTerm2 Metal shaders (`~/dashterm2`)

---

## Problem Statement

DashTerm2's GPU renderer (`DTermMetalView`) has two rendering issues:

1. **Blurry text** - Minor issue, related to texture filtering
2. **Missing box borders** - Critical issue, box drawing characters don't render

**Root cause:** The dterm-core hybrid renderer treats box drawing characters (U+2500-U+257F) as font glyphs, but:
- fontdue may not have these glyphs
- Even if present, font-rendered box drawing looks poor (gaps, inconsistent weights)

**Solution:** Render box drawing characters as **geometric line segments**, not font glyphs. This is the standard approach used by all professional terminals.

---

## Reference Implementation: iTerm2

**File:** `~/dashterm2/sources/iTermBoxDrawingBezierCurveFactory.m`

iTerm2 uses a coordinate-based system to define each box drawing character as line segments.

### Coordinate System (from iTerm2 source, line 1107-1117)

```
         X-axis (columns):
         a         b       c         d     e           f                g
         |         |       |         |     |           |                |
         left    hc-1   hc-½     center  hc+½        hc+1            right
         0.0                       0.5                                 1.0

Y-axis (rows):
1  top (0.0)
2  vc-1
3  vc-½
4  center (0.5)
5  vc+½
6  vc+1
7  bottom (1.0)
```

Where:
- `hc` = horizontal center (0.5)
- `vc` = vertical center (0.5)
- `l` = left edge (0.0)
- `r` = right edge (1.0)
- `t` = top edge (0.0)
- `b` = bottom edge (1.0)

### Line Segment Encoding (from iTerm2 source)

Each box character is encoded as space-separated line segments. Each segment is 4 characters: `X1Y1X2Y2`

| Character | Unicode | iTerm2 Encoding | Meaning |
|-----------|---------|-----------------|---------|
| ─ | U+2500 | `a4g4` | Horizontal line from left to right at center |
| │ | U+2502 | `d1d7` | Vertical line from top to bottom at center |
| ┌ | U+250C | `g4d4 d4d7` | Right-half horizontal + bottom-half vertical |
| ┐ | U+2510 | `a4d4 d4d7` | Left-half horizontal + bottom-half vertical |
| └ | U+2514 | `d1d4 d4g4` | Top-half vertical + right-half horizontal |
| ┘ | U+2518 | `a4d4 d4d1` | Left-half horizontal + top-half vertical |
| ├ | U+251C | `d1d7 d4g4` | Full vertical + right-half horizontal |
| ┤ | U+2524 | `d1d7 a4d4` | Full vertical + left-half horizontal |
| ┬ | U+252C | `a4g4 d4d7` | Full horizontal + bottom-half vertical |
| ┴ | U+2534 | `a4g4 d1d4` | Full horizontal + top-half vertical |
| ┼ | U+253C | `a4g4 d1d7` | Full horizontal + full vertical |

### Heavy Lines (2-pixel width)

Heavy lines use offset coordinates (3 and 5 instead of 4):

| Character | Unicode | iTerm2 Encoding | Meaning |
|-----------|---------|-----------------|---------|
| ━ | U+2501 | `a3g3 a5g5` | Two parallel horizontal lines |
| ┃ | U+2503 | `c1c7 e1e7` | Two parallel vertical lines |
| ┏ | U+250F | `g3c3 c3c7 g5e5 e5e7` | Heavy corner (two L-shapes) |

### Double Lines (two separate lines)

| Character | Unicode | Pattern |
|-----------|---------|---------|
| ═ | U+2550 | Two horizontal lines with gap |
| ║ | U+2551 | Two vertical lines with gap |
| ╔ | U+2554 | Double corner |

---

## Architectural Design for dterm-core

### Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Hybrid Renderer                           │
├─────────────────────────────────────────────────────────────────┤
│  build(terminal) {                                               │
│    for each cell:                                               │
│      emit_background()                                          │
│                                                                 │
│      if is_box_drawing(codepoint):                             │
│        emit_box_drawing()  ──► FLAG_IS_GEOMETRIC                │
│      else if is_block_element(codepoint):                      │
│        emit_block_element() ──► FLAG_IS_GEOMETRIC               │
│      else:                                                      │
│        emit_glyph()  ──► atlas UV lookup                        │
│  }                                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Metal Shader (DashTerm2)                     │
├─────────────────────────────────────────────────────────────────┤
│  if FLAG_IS_BACKGROUND:                                         │
│    return bgColor                                               │
│  else if FLAG_IS_GEOMETRIC:                                     │
│    return fgColor  // solid color, no texture                   │
│  else:                                                          │
│    alpha = atlas.sample(uv)                                     │
│    return fgColor * alpha                                       │
└─────────────────────────────────────────────────────────────────┘
```

### Component 1: Vertex Flags

**File:** `~/dterm/crates/dterm-core/src/gpu/vertex.rs` (or appropriate location)

These flags tell the GPU shader how to render each vertex. This is **not debugging** - it's core rendering logic.

```rust
/// Vertex flags for GPU rendering pipeline.
///
/// These flags determine shader behavior:
/// - Glyph: Sample atlas texture, apply alpha to foreground color
/// - Background: Render solid background color
/// - Geometric: Render solid foreground color (no texture lookup)
pub mod flags {
    /// Vertex represents a background quad.
    /// Shader returns: bgColor
    pub const FLAG_IS_BACKGROUND: u32 = 1 << 8;  // 256

    /// Vertex represents a geometric shape (box drawing, block element).
    /// Shader returns: fgColor (solid, no atlas lookup)
    pub const FLAG_IS_GEOMETRIC: u32 = 1 << 9;   // 512
}
```

**Why FLAG_IS_GEOMETRIC is required (not optional):**

Without this flag, the shader would:
1. Try to sample the atlas texture at UV coordinates
2. Get garbage or zero alpha (no glyph entry for box chars)
3. Discard the fragment → invisible

With this flag, the shader:
1. Skips atlas lookup entirely
2. Returns solid foreground color
3. Renders clean geometric line

### Component 2: Box Drawing Module

**File:** `~/dterm/crates/dterm-core/src/gpu/box_drawing.rs` (new file)

```rust
//! Box drawing character decomposition into geometric line segments.
//!
//! ## Why Geometric Rendering?
//!
//! Font-based rendering of box drawing produces:
//! - Gaps at corners where lines should meet
//! - Inconsistent line weights between characters
//! - Aliasing artifacts from font hinting
//!
//! Geometric rendering produces:
//! - Pixel-perfect lines
//! - Perfect alignment at intersections
//! - Consistent appearance regardless of font
//!
//! ## Reference
//!
//! This follows the approach used by:
//! - iTerm2: `iTermBoxDrawingBezierCurveFactory.m`
//! - Alacritty: `alacritty_terminal/src/term/cell.rs`
//! - kitty: `kitty/fonts/box_drawing.py`
//!
//! See: ~/dashterm2/docs/DTERM_RENDERING_TASKS.md for full specification.

/// Line segment within a cell, using normalized coordinates.
///
/// Coordinates are normalized to [0.0, 1.0]:
/// - (0.0, 0.0) = top-left corner
/// - (0.5, 0.5) = cell center
/// - (1.0, 1.0) = bottom-right corner
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Start X (0.0 = left, 0.5 = center, 1.0 = right)
    pub x0: f32,
    /// Start Y (0.0 = top, 0.5 = center, 1.0 = bottom)
    pub y0: f32,
    /// End X
    pub x1: f32,
    /// End Y
    pub y1: f32,
}

impl Segment {
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    // Horizontal segments
    pub const H_LEFT: Self = Self::new(0.0, 0.5, 0.5, 0.5);   // ─ left half
    pub const H_RIGHT: Self = Self::new(0.5, 0.5, 1.0, 0.5);  // ─ right half
    pub const H_FULL: Self = Self::new(0.0, 0.5, 1.0, 0.5);   // ─ full width

    // Vertical segments
    pub const V_TOP: Self = Self::new(0.5, 0.0, 0.5, 0.5);    // │ top half
    pub const V_BOTTOM: Self = Self::new(0.5, 0.5, 0.5, 1.0); // │ bottom half
    pub const V_FULL: Self = Self::new(0.5, 0.0, 0.5, 1.0);   // │ full height
}

/// Line weight for box drawing characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// Light: 1 pixel line
    Light,
    /// Heavy: 2 pixel line
    Heavy,
    /// Double: 2 parallel 1-pixel lines with gap
    Double,
}

/// Decoded box drawing character.
#[derive(Debug, Clone)]
pub struct BoxChar {
    /// Line segments that compose this character
    pub segments: &'static [Segment],
    /// Line weight
    pub weight: Weight,
}

/// Decode a Unicode codepoint into box drawing segments.
///
/// Returns `None` if the codepoint is not a box drawing character.
///
/// # Example
/// ```
/// let box_char = decode(0x2500); // ─
/// assert!(box_char.is_some());
/// assert_eq!(box_char.unwrap().segments.len(), 1);
/// ```
pub fn decode(codepoint: u32) -> Option<BoxChar> {
    use Segment::*;
    use Weight::*;

    // Static segment arrays (zero runtime allocation)
    static H: [Segment; 1] = [Segment::H_FULL];
    static V: [Segment; 1] = [Segment::V_FULL];
    static DR: [Segment; 2] = [Segment::H_RIGHT, Segment::V_BOTTOM]; // ┌
    static DL: [Segment; 2] = [Segment::H_LEFT, Segment::V_BOTTOM];  // ┐
    static UR: [Segment; 2] = [Segment::V_TOP, Segment::H_RIGHT];    // └
    static UL: [Segment; 2] = [Segment::V_TOP, Segment::H_LEFT];     // ┘
    static VR: [Segment; 2] = [Segment::V_FULL, Segment::H_RIGHT];   // ├
    static VL: [Segment; 2] = [Segment::V_FULL, Segment::H_LEFT];    // ┤
    static HD: [Segment; 2] = [Segment::H_FULL, Segment::V_BOTTOM];  // ┬
    static HU: [Segment; 2] = [Segment::H_FULL, Segment::V_TOP];     // ┴
    static CROSS: [Segment; 2] = [Segment::H_FULL, Segment::V_FULL]; // ┼

    let (segments, weight): (&[Segment], Weight) = match codepoint {
        // ═══════════════════════════════════════════════════════════════
        // LIGHT BOX DRAWING (U+2500-U+251F, U+2574-U+257F)
        // ═══════════════════════════════════════════════════════════════
        0x2500 => (&H, Light),      // ─ BOX DRAWINGS LIGHT HORIZONTAL
        0x2502 => (&V, Light),      // │ BOX DRAWINGS LIGHT VERTICAL
        0x250C => (&DR, Light),     // ┌ BOX DRAWINGS LIGHT DOWN AND RIGHT
        0x2510 => (&DL, Light),     // ┐ BOX DRAWINGS LIGHT DOWN AND LEFT
        0x2514 => (&UR, Light),     // └ BOX DRAWINGS LIGHT UP AND RIGHT
        0x2518 => (&UL, Light),     // ┘ BOX DRAWINGS LIGHT UP AND LEFT
        0x251C => (&VR, Light),     // ├ BOX DRAWINGS LIGHT VERTICAL AND RIGHT
        0x2524 => (&VL, Light),     // ┤ BOX DRAWINGS LIGHT VERTICAL AND LEFT
        0x252C => (&HD, Light),     // ┬ BOX DRAWINGS LIGHT DOWN AND HORIZONTAL
        0x2534 => (&HU, Light),     // ┴ BOX DRAWINGS LIGHT UP AND HORIZONTAL
        0x253C => (&CROSS, Light),  // ┼ BOX DRAWINGS LIGHT VERTICAL AND HORIZONTAL

        // Light line fragments
        0x2574 => (&[H_LEFT], Light),   // ╴ BOX DRAWINGS LIGHT LEFT
        0x2575 => (&[V_TOP], Light),    // ╵ BOX DRAWINGS LIGHT UP
        0x2576 => (&[H_RIGHT], Light),  // ╶ BOX DRAWINGS LIGHT RIGHT
        0x2577 => (&[V_BOTTOM], Light), // ╷ BOX DRAWINGS LIGHT DOWN

        // ═══════════════════════════════════════════════════════════════
        // HEAVY BOX DRAWING (U+2501, U+2503, U+250F-U+254B odd)
        // ═══════════════════════════════════════════════════════════════
        0x2501 => (&H, Heavy),      // ━ BOX DRAWINGS HEAVY HORIZONTAL
        0x2503 => (&V, Heavy),      // ┃ BOX DRAWINGS HEAVY VERTICAL
        0x250F => (&DR, Heavy),     // ┏ BOX DRAWINGS HEAVY DOWN AND RIGHT
        0x2513 => (&DL, Heavy),     // ┓ BOX DRAWINGS HEAVY DOWN AND LEFT
        0x2517 => (&UR, Heavy),     // ┗ BOX DRAWINGS HEAVY UP AND RIGHT
        0x251B => (&UL, Heavy),     // ┛ BOX DRAWINGS HEAVY UP AND LEFT
        0x2523 => (&VR, Heavy),     // ┣ BOX DRAWINGS HEAVY VERTICAL AND RIGHT
        0x252B => (&VL, Heavy),     // ┫ BOX DRAWINGS HEAVY VERTICAL AND LEFT
        0x2533 => (&HD, Heavy),     // ┳ BOX DRAWINGS HEAVY DOWN AND HORIZONTAL
        0x253B => (&HU, Heavy),     // ┻ BOX DRAWINGS HEAVY UP AND HORIZONTAL
        0x254B => (&CROSS, Heavy),  // ╋ BOX DRAWINGS HEAVY VERTICAL AND HORIZONTAL

        // Heavy line fragments
        0x2578 => (&[H_LEFT], Heavy),   // ╸ BOX DRAWINGS HEAVY LEFT
        0x2579 => (&[V_TOP], Heavy),    // ╹ BOX DRAWINGS HEAVY UP
        0x257A => (&[H_RIGHT], Heavy),  // ╺ BOX DRAWINGS HEAVY RIGHT
        0x257B => (&[V_BOTTOM], Heavy), // ╻ BOX DRAWINGS HEAVY DOWN

        // ═══════════════════════════════════════════════════════════════
        // DOUBLE BOX DRAWING (U+2550-U+256C)
        // ═══════════════════════════════════════════════════════════════
        0x2550 => (&H, Double),     // ═ BOX DRAWINGS DOUBLE HORIZONTAL
        0x2551 => (&V, Double),     // ║ BOX DRAWINGS DOUBLE VERTICAL
        0x2554 => (&DR, Double),    // ╔ BOX DRAWINGS DOUBLE DOWN AND RIGHT
        0x2557 => (&DL, Double),    // ╗ BOX DRAWINGS DOUBLE DOWN AND LEFT
        0x255A => (&UR, Double),    // ╚ BOX DRAWINGS DOUBLE UP AND RIGHT
        0x255D => (&UL, Double),    // ╝ BOX DRAWINGS DOUBLE UP AND LEFT
        0x2560 => (&VR, Double),    // ╠ BOX DRAWINGS DOUBLE VERTICAL AND RIGHT
        0x2563 => (&VL, Double),    // ╣ BOX DRAWINGS DOUBLE VERTICAL AND LEFT
        0x2566 => (&HD, Double),    // ╦ BOX DRAWINGS DOUBLE DOWN AND HORIZONTAL
        0x2569 => (&HU, Double),    // ╩ BOX DRAWINGS DOUBLE UP AND HORIZONTAL
        0x256C => (&CROSS, Double), // ╬ BOX DRAWINGS DOUBLE VERTICAL AND HORIZONTAL

        // Not a box drawing character
        _ => return None,
    };

    Some(BoxChar { segments, weight })
}

/// Check if a codepoint is any geometric character (box, block, powerline).
///
/// These characters should NOT use the glyph atlas - they are rendered
/// as solid-color geometry.
pub fn is_geometric(codepoint: u32) -> bool {
    matches!(codepoint,
        0x2500..=0x257F |  // Box Drawing
        0x2580..=0x259F |  // Block Elements
        0xE0A0..=0xE0D4    // Powerline Extra Symbols
    )
}

/// Check if a codepoint is specifically a box drawing character.
pub fn is_box_drawing(codepoint: u32) -> bool {
    matches!(codepoint, 0x2500..=0x257F)
}

/// Check if a codepoint is a block element character.
pub fn is_block_element(codepoint: u32) -> bool {
    matches!(codepoint, 0x2580..=0x259F)
}
```

### Component 3: Hybrid Renderer Integration

**File:** `~/dterm/crates/dterm-core/src/gpu/hybrid.rs`

Modify the `build()` function to detect geometric characters:

```rust
use super::box_drawing;
use super::flags::{FLAG_IS_BACKGROUND, FLAG_IS_GEOMETRIC};

impl HybridRenderer {
    /// Build vertex data for the terminal grid.
    ///
    /// Returns the number of vertices generated.
    pub fn build(&mut self, terminal: &Terminal) -> u32 {
        self.vertices.clear();

        for row in 0..terminal.rows() {
            for col in 0..terminal.cols() {
                let cell = terminal.cell(row, col);

                // 1. Always emit background vertex first
                self.emit_background(row, col, &cell);

                // 2. Check cell content
                let codepoint = cell.codepoint();
                if codepoint <= 0x20 {
                    // Space or control char - background only
                    continue;
                }

                // 3. Route to appropriate renderer
                if let Some(box_char) = box_drawing::decode(codepoint) {
                    // Box drawing: geometric line segments
                    self.emit_box_drawing(row, col, &cell, &box_char);
                } else if box_drawing::is_block_element(codepoint) {
                    // Block element: geometric filled rect
                    self.emit_block_element(row, col, &cell, codepoint);
                } else {
                    // Regular glyph: atlas texture lookup
                    self.emit_glyph(row, col, &cell, codepoint);
                }
            }
        }

        self.vertices.len() as u32
    }

    /// Emit vertices for a box drawing character.
    fn emit_box_drawing(
        &mut self,
        row: u16,
        col: u16,
        cell: &Cell,
        box_char: &box_drawing::BoxChar,
    ) {
        let fg = cell.foreground_rgba();
        let bg = cell.background_rgba();

        // Line thickness in pixels
        let thickness = match box_char.weight {
            box_drawing::Weight::Light => 1.0,
            box_drawing::Weight::Heavy => 2.0,
            box_drawing::Weight::Double => 1.0,
        };

        // Emit each segment as a quad
        for segment in box_char.segments {
            self.emit_line_quad(
                row as f32,
                col as f32,
                segment,
                thickness,
                fg,
                bg,
            );

            // For double lines, emit second parallel line
            if box_char.weight == box_drawing::Weight::Double {
                let offset = 3.0;  // 3 pixel gap
                self.emit_line_quad_offset(
                    row as f32,
                    col as f32,
                    segment,
                    thickness,
                    offset,
                    fg,
                    bg,
                );
            }
        }
    }

    /// Emit a line segment as a quad (6 vertices = 2 triangles).
    fn emit_line_quad(
        &mut self,
        row: f32,
        col: f32,
        segment: &box_drawing::Segment,
        thickness: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        // Convert normalized segment coords to cell-space
        let x0 = col + segment.x0;
        let y0 = row + segment.y0;
        let x1 = col + segment.x1;
        let y1 = row + segment.y1;

        // Expand line to quad based on orientation
        let half_thick_x = (thickness / 2.0) / self.cell_width;
        let half_thick_y = (thickness / 2.0) / self.cell_height;

        let (qx0, qy0, qx1, qy1) = if (y0 - y1).abs() < 0.001 {
            // Horizontal line: expand vertically
            (x0, y0 - half_thick_y, x1, y0 + half_thick_y)
        } else {
            // Vertical line: expand horizontally
            (x0 - half_thick_x, y0, x0 + half_thick_x, y1)
        };

        // Emit 6 vertices for quad (2 triangles)
        // UV = (0,0) since no atlas lookup
        // flags = FLAG_IS_GEOMETRIC
        self.emit_quad_vertices(
            qx0, qy0, qx1, qy1,
            0.0, 0.0, 0.0, 0.0,  // UV coords (unused)
            fg,
            bg,
            FLAG_IS_GEOMETRIC,
        );
    }
}
```

### Component 4: FFI Export

**File:** `~/dterm/crates/dterm-core/src/ffi/mod.rs`

Ensure cbindgen exports the flag constants:

```rust
/// Vertex flag: background quad (solid bgColor, no texture)
#[no_mangle]
pub static FLAG_IS_BACKGROUND: u32 = 256;

/// Vertex flag: geometric shape (solid fgColor, no texture)
/// Used for box drawing characters, block elements, powerline symbols
#[no_mangle]
pub static FLAG_IS_GEOMETRIC: u32 = 512;
```

After running cbindgen, verify `dterm.h` contains:

```c
extern const uint32_t FLAG_IS_BACKGROUND;
extern const uint32_t FLAG_IS_GEOMETRIC;
```

---

## DashTerm2 Changes

### Metal Shader Update

**File:** `~/dashterm2/sources/Metal/Shaders/DTermHybrid.metal`

```metal
// Vertex type flags - must match dterm-core
constant uint FLAG_IS_BACKGROUND = 256;
constant uint FLAG_IS_GEOMETRIC  = 512;

fragment float4 dtermCellFragment(
    VertexOut in [[stage_in]],
    texture2d<float> atlas [[texture(0)]],
    sampler atlasSampler [[sampler(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    // 1. Background pass: solid background color
    if ((in.flags & FLAG_IS_BACKGROUND) != 0) {
        float4 color = in.bgColor;
        if ((in.flags & FLAG_IS_CURSOR) != 0) {
            color = mix(color, uniforms.cursor_color, 0.7);
        }
        if ((in.flags & FLAG_IS_SELECTION) != 0) {
            color = mix(color, float4(0.3, 0.5, 0.8, 1.0), 0.4);
        }
        return color;
    }

    // 2. Geometric pass: solid foreground color (no texture)
    //    Used for: box drawing, block elements, powerline
    if ((in.flags & FLAG_IS_GEOMETRIC) != 0) {
        return in.fgColor;
    }

    // 3. Glyph pass: sample atlas texture for alpha
    float alpha = atlas.sample(atlasSampler, in.texCoord).r;

    float4 color = in.fgColor;
    if ((in.flags & FLAG_INVERSE) != 0) {
        color = in.bgColor;
    }
    if ((in.flags & FLAG_DIM) != 0) {
        color.rgb *= 0.5;
    }
    if ((in.flags & FLAG_BLINK) != 0) {
        float blink = sin(uniforms.time * 3.14159) * 0.5 + 0.5;
        alpha *= blink;
    }
    if (alpha < 0.01) {
        discard_fragment();
    }

    return float4(color.rgb, alpha);
}
```

---

## Testing

### Unit Tests (dterm-core)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_light_horizontal() {
        let bc = decode(0x2500).unwrap();
        assert_eq!(bc.segments.len(), 1);
        assert_eq!(bc.weight, Weight::Light);
    }

    #[test]
    fn decode_light_cross() {
        let bc = decode(0x253C).unwrap();
        assert_eq!(bc.segments.len(), 2);
    }

    #[test]
    fn decode_heavy_horizontal() {
        let bc = decode(0x2501).unwrap();
        assert_eq!(bc.weight, Weight::Heavy);
    }

    #[test]
    fn decode_non_box_char() {
        assert!(decode('A' as u32).is_none());
        assert!(decode(' ' as u32).is_none());
    }

    #[test]
    fn is_geometric_checks() {
        assert!(is_geometric(0x2500));  // ─
        assert!(is_geometric(0x2588));  // █ (block)
        assert!(is_geometric(0xE0B0));  // Powerline
        assert!(!is_geometric('A' as u32));
    }
}
```

### Visual Tests (DashTerm2)

```bash
# Run in DashTerm2 terminal to verify rendering

# Light box
printf '┌───────┐\n│ Light │\n└───────┘\n'

# Heavy box
printf '┏━━━━━━━┓\n┃ Heavy ┃\n┗━━━━━━━┛\n'

# Double box
printf '╔═══════╗\n║ Double║\n╚═══════╝\n'

# All corners and intersections
printf '┌─┬─┐\n├─┼─┤\n└─┴─┘\n'

# tmux-style split
printf '───────────────────\n│ left   │ right  │\n───────────────────\n'
```

---

## Implementation Checklist

| # | Task | File | Status |
|---|------|------|--------|
| 1 | Add FLAG_IS_GEOMETRIC constant | `src/gpu/vertex.rs` or `src/gpu/flags.rs` | TODO |
| 2 | Create `box_drawing.rs` module | `src/gpu/box_drawing.rs` | TODO |
| 3 | Implement `decode()` function | `src/gpu/box_drawing.rs` | TODO |
| 4 | Implement `is_geometric()` function | `src/gpu/box_drawing.rs` | TODO |
| 5 | Integrate into `build()` | `src/gpu/hybrid.rs` | TODO |
| 6 | Implement `emit_box_drawing()` | `src/gpu/hybrid.rs` | TODO |
| 7 | Implement `emit_line_quad()` | `src/gpu/hybrid.rs` | TODO |
| 8 | Export FLAG constants via FFI | `src/ffi/mod.rs` | TODO |
| 9 | Run cbindgen to update `dterm.h` | Build step | TODO |
| 10 | Update Metal shader | DashTerm2 repo | TODO |
| 11 | Add unit tests | `src/gpu/box_drawing.rs` | TODO |
| 12 | Visual verification | Manual | TODO |

---

## File Locations Summary

| Component | Repository | Path |
|-----------|------------|------|
| Box drawing module | dterm | `~/dterm/crates/dterm-core/src/gpu/box_drawing.rs` |
| Hybrid renderer | dterm | `~/dterm/crates/dterm-core/src/gpu/hybrid.rs` |
| Vertex flags | dterm | `~/dterm/crates/dterm-core/src/gpu/vertex.rs` |
| FFI exports | dterm | `~/dterm/crates/dterm-core/src/ffi/mod.rs` |
| C header | dterm | `~/dterm/crates/dterm-core/dterm.h` (generated) |
| Metal shader | dashterm2 | `~/dashterm2/sources/Metal/Shaders/DTermHybrid.metal` |
| Reference impl | dashterm2 | `~/dashterm2/sources/iTermBoxDrawingBezierCurveFactory.m` |

---

## References

- **iTerm2 source:** `~/dashterm2/sources/iTermBoxDrawingBezierCurveFactory.m` (lines 1107-1200)
- **Unicode Box Drawing:** https://unicode.org/charts/PDF/U2500.pdf
- **Unicode Block Elements:** https://unicode.org/charts/PDF/U2580.pdf
- **Alacritty rendering:** https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/term/cell.rs
