// Terminal cell rendering shader
//
// This shader renders terminal cells as textured quads. Each cell consists of:
// - A background quad (solid color)
// - A foreground glyph (textured quad from atlas)
//
// The shader supports:
// - Per-cell foreground/background colors
// - Glyph rendering from atlas texture
// - Bold/dim text rendering
// - Cursor rendering
// - Selection highlighting

// Uniforms (80 bytes, 16-byte aligned)
struct Uniforms {
    // Viewport dimensions (pixels)
    viewport_width: f32,
    viewport_height: f32,
    // Cell dimensions (pixels)
    cell_width: f32,
    cell_height: f32,
    // -- 16 bytes --
    // Atlas texture size (pixels)
    atlas_size: f32,
    // Time for cursor blink animation (seconds)
    time: f32,
    // Cursor position (cell coordinates, -1 if hidden)
    cursor_x: i32,
    cursor_y: i32,
    // -- 16 bytes --
    // Cursor color (RGBA)
    cursor_color: vec4<f32>,
    // -- 16 bytes --
    // Selection color (RGBA)
    selection_color: vec4<f32>,
    // -- 16 bytes --
    // Cursor style (0=Block, 1=Underline, 2=Bar)
    cursor_style: u32,
    // Cursor blink rate in milliseconds (0 = no blink)
    cursor_blink_ms: u32,
    // Padding
    _padding: vec2<u32>,
    // -- 16 bytes --
}

struct BackgroundUniforms {
    enabled: u32,
    blend_mode: u32,
    opacity: f32,
    _padding: u32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Glyph atlas texture
@group(0) @binding(1)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

// Background image texture
@group(0) @binding(3)
var background_texture: texture_2d<f32>;

@group(0) @binding(4)
var<uniform> background_uniforms: BackgroundUniforms;

// Vertex input (per-vertex data)
struct VertexInput {
    @location(0) position: vec2<f32>,      // Position in cell grid (0,0 to cols,rows)
    @location(1) uv: vec2<f32>,            // UV coordinates in atlas
    @location(2) fg_color: vec4<f32>,      // Foreground color (RGBA)
    @location(3) bg_color: vec4<f32>,      // Background color (RGBA)
    @location(4) flags: u32,               // Bit flags: bold, italic, underline, etc.
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) flags: u32,
    @location(4) cell_position: vec2<f32>, // Position in cell coordinates for cursor/selection
}

// =============================================================================
// Vertex Flag Bit Layout (7 bits used)
// =============================================================================
//
// Bit:  31                    7  6  5  4  3  2  1  0
//       [      Reserved      ][OV][  EFF  ][ TYPE ]
//
// TYPE (bits 0-1): VertexType
//   0 = Glyph       - Sample from atlas texture
//   1 = Background  - Solid background color
//   2 = Decoration  - Solid foreground color (underlines, box drawing)
//
// EFF (bits 2-4): EffectFlags
//   bit 2 = DIM     - Reduce brightness by 50%
//   bit 3 = BLINK   - Animate alpha
//   bit 4 = INVERSE - Swap fg/bg colors
//
// OV (bits 5-6): OverlayFlags
//   bit 5 = CURSOR    - Cursor highlight
//   bit 6 = SELECTION - Selection highlight
// =============================================================================

// Vertex type constants (bits 0-1)
const VERTEX_TYPE_MASK: u32 = 3u;
const VERTEX_TYPE_GLYPH: u32 = 0u;
const VERTEX_TYPE_BACKGROUND: u32 = 1u;
const VERTEX_TYPE_DECORATION: u32 = 2u;

// Effect flags (bits 2-4)
const EFFECT_DIM: u32 = 4u;      // 1 << 2
const EFFECT_BLINK: u32 = 8u;    // 1 << 3
const EFFECT_INVERSE: u32 = 16u; // 1 << 4

// Overlay flags (bits 5-6)
const OVERLAY_CURSOR: u32 = 32u;    // 1 << 5
const OVERLAY_SELECTION: u32 = 64u; // 1 << 6

// Helper function to extract vertex type
fn get_vertex_type(flags: u32) -> u32 {
    return flags & VERTEX_TYPE_MASK;
}

// Cursor styles
const CURSOR_STYLE_BLOCK: u32 = 0u;
const CURSOR_STYLE_UNDERLINE: u32 = 1u;
const CURSOR_STYLE_BAR: u32 = 2u;

// Background blend modes
const BLEND_MODE_NORMAL: u32 = 0u;
const BLEND_MODE_MULTIPLY: u32 = 1u;
const BLEND_MODE_SCREEN: u32 = 2u;
const BLEND_MODE_OVERLAY: u32 = 3u;

// Vertex shader for background quads
@vertex
fn vs_background(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert cell position to NDC (-1 to 1)
    let x = (input.position.x * uniforms.cell_width / uniforms.viewport_width) * 2.0 - 1.0;
    let y = 1.0 - (input.position.y * uniforms.cell_height / uniforms.viewport_height) * 2.0;

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.uv = input.uv;
    output.fg_color = input.fg_color;
    output.bg_color = input.bg_color;
    // Set vertex type to Background (clear type bits, then set to 1)
    output.flags = (input.flags & ~VERTEX_TYPE_MASK) | VERTEX_TYPE_BACKGROUND;
    output.cell_position = input.position; // Pass cell coordinates for cursor style calculation

    return output;
}

// Vertex shader for glyph quads
@vertex
fn vs_glyph(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert cell position to NDC (-1 to 1)
    let x = (input.position.x * uniforms.cell_width / uniforms.viewport_width) * 2.0 - 1.0;
    let y = 1.0 - (input.position.y * uniforms.cell_height / uniforms.viewport_height) * 2.0;

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.uv = input.uv;
    output.fg_color = input.fg_color;
    output.bg_color = input.bg_color;
    output.flags = input.flags;
    output.cell_position = input.position; // Pass cell coordinates for cursor style calculation

    return output;
}

// Fragment shader for background quads
@fragment
fn fs_background(input: VertexOutput) -> @location(0) vec4<f32> {
    var color = input.bg_color;

    // Handle inverse video
    if (input.flags & EFFECT_INVERSE) != 0u {
        color = input.fg_color;
    }

    // Handle selection highlighting (blend with selection color)
    if (input.flags & OVERLAY_SELECTION) != 0u {
        color = mix(color, vec4<f32>(0.3, 0.5, 0.8, 1.0), 0.5);
    }

    return color;
}

// Fragment shader for glyph quads
@fragment
fn fs_glyph(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the atlas texture
    let alpha = textureSample(atlas_texture, atlas_sampler, input.uv).r;

    // If no glyph (transparent), discard
    if alpha < 0.01 {
        discard;
    }

    var color = input.fg_color;

    // Handle inverse video
    if (input.flags & EFFECT_INVERSE) != 0u {
        color = input.bg_color;
    }

    // Handle dim text
    if (input.flags & EFFECT_DIM) != 0u {
        color = color * 0.5;
    }

    // Handle blink (optional - can be disabled for performance)
    if (input.flags & EFFECT_BLINK) != 0u {
        let blink = step(0.5, fract(uniforms.time * 2.0));
        color.a = color.a * blink;
    }

    // Apply atlas alpha
    color.a = color.a * alpha;

    return color;
}

// Fragment shader for cursor
@fragment
fn fs_cursor(input: VertexOutput) -> @location(0) vec4<f32> {
    var color = uniforms.cursor_color;

    // Cursor blink (1Hz)
    let blink = step(0.5, fract(uniforms.time));
    color.a = color.a * blink;

    return color;
}

// Combined vertex shader (used when drawing all quads in one pass)
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert cell position to NDC (-1 to 1)
    let x = (input.position.x * uniforms.cell_width / uniforms.viewport_width) * 2.0 - 1.0;
    let y = 1.0 - (input.position.y * uniforms.cell_height / uniforms.viewport_height) * 2.0;

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.uv = input.uv;
    output.fg_color = input.fg_color;
    output.bg_color = input.bg_color;
    output.flags = input.flags;
    output.cell_position = input.position; // Pass cell coordinates for cursor style calculation

    return output;
}

// Helper function to compute cursor blink factor
fn cursor_blink_factor() -> f32 {
    // If blink rate is 0, cursor is always visible
    if uniforms.cursor_blink_ms == 0u {
        return 1.0;
    }
    // Convert blink rate from ms to seconds for period calculation
    let blink_period_s = f32(uniforms.cursor_blink_ms) / 1000.0;
    // Full cycle is 2x blink period (on + off)
    let cycle_time = fract(uniforms.time / (blink_period_s * 2.0));
    // Return 1.0 for first half of cycle (cursor visible), 0.0 for second half
    return step(cycle_time, 0.5);
}

// Helper function to check if we're in the cursor region based on cursor style
// Returns 1.0 if this fragment should be part of the cursor, 0.0 otherwise
// local_x and local_y are normalized positions within the cell (0.0 to 1.0)
fn is_in_cursor_region(local_x: f32, local_y: f32) -> f32 {
    if uniforms.cursor_style == CURSOR_STYLE_BLOCK {
        // Block cursor: entire cell
        return 1.0;
    } else if uniforms.cursor_style == CURSOR_STYLE_UNDERLINE {
        // Underline cursor: bottom 10% of cell (or at least 2 pixels)
        let underline_height = max(0.1, 2.0 / uniforms.cell_height);
        if local_y >= (1.0 - underline_height) {
            return 1.0;
        }
    } else if uniforms.cursor_style == CURSOR_STYLE_BAR {
        // Bar cursor: left 10% of cell (or at least 2 pixels)
        let bar_width = max(0.1, 2.0 / uniforms.cell_width);
        if local_x <= bar_width {
            return 1.0;
        }
    }
    return 0.0;
}

fn apply_background_blend(base: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let src_alpha = clamp(background_uniforms.opacity * src.a, 0.0, 1.0);
    var blended_rgb = src.rgb;

    if background_uniforms.blend_mode == BLEND_MODE_MULTIPLY {
        blended_rgb = base.rgb * src.rgb;
    } else if background_uniforms.blend_mode == BLEND_MODE_SCREEN {
        blended_rgb = vec3<f32>(1.0) - (vec3<f32>(1.0) - base.rgb) * (vec3<f32>(1.0) - src.rgb);
    } else if background_uniforms.blend_mode == BLEND_MODE_OVERLAY {
        let low = 2.0 * base.rgb * src.rgb;
        let high = vec3<f32>(1.0) - 2.0 * (vec3<f32>(1.0) - base.rgb) * (vec3<f32>(1.0) - src.rgb);
        blended_rgb = select(high, low, base.rgb < vec3<f32>(0.5));
    }

    let out_rgb = mix(base.rgb, blended_rgb, src_alpha);
    return vec4<f32>(out_rgb, base.a);
}

// Combined fragment shader
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Route based on vertex type (bits 0-1)
    let vertex_type = get_vertex_type(input.flags);

    // Decoration pass (underline, strikethrough, box drawing)
    // Decorations are rendered as solid color quads with their color in fg_color
    if vertex_type == VERTEX_TYPE_DECORATION {
        // Decoration color is passed in fg_color
        var color = input.fg_color;

        // Handle inverse video - swap to bg_color
        if (input.flags & EFFECT_INVERSE) != 0u {
            color = input.bg_color;
        }

        // Handle dim - reduce brightness
        if (input.flags & EFFECT_DIM) != 0u {
            color = vec4<f32>(color.rgb * 0.5, color.a);
        }

        return color;
    }

    // Background pass
    if vertex_type == VERTEX_TYPE_BACKGROUND {
        var color = input.bg_color;

        if (input.flags & EFFECT_INVERSE) != 0u {
            color = input.fg_color;
        }

        // Background image blending (handled by separate uniform flag now)
        if background_uniforms.enabled != 0u {
            let uv = vec2<f32>(
                (input.cell_position.x * uniforms.cell_width) / uniforms.viewport_width,
                (input.cell_position.y * uniforms.cell_height) / uniforms.viewport_height
            );
            let image_color = textureSample(background_texture, atlas_sampler, uv);
            color = apply_background_blend(color, image_color);
        }

        if (input.flags & OVERLAY_SELECTION) != 0u {
            // Use selection color from uniforms
            color = mix(color, uniforms.selection_color, uniforms.selection_color.a);
        }

        if (input.flags & OVERLAY_CURSOR) != 0u {
            // Compute local position within the cell (0.0 to 1.0)
            // cell_position is in cell coordinates, fractional part gives position within cell
            let local_x = fract(input.cell_position.x);
            let local_y = fract(input.cell_position.y);

            // Check if this fragment should be part of the cursor based on style
            let in_cursor = is_in_cursor_region(local_x, local_y);

            if in_cursor > 0.5 {
                // Apply cursor color with blink animation
                let blink = cursor_blink_factor();
                color = mix(color, uniforms.cursor_color, blink);
            }
        }

        return color;
    }

    // Glyph pass (vertex_type == VERTEX_TYPE_GLYPH or fallback)
    let alpha = textureSample(atlas_texture, atlas_sampler, input.uv).r;

    if alpha < 0.01 {
        discard;
    }

    var color = input.fg_color;

    if (input.flags & EFFECT_INVERSE) != 0u {
        color = input.bg_color;
    }

    if (input.flags & EFFECT_DIM) != 0u {
        color = color * 0.5;
    }

    if (input.flags & EFFECT_BLINK) != 0u {
        let blink = step(0.5, fract(uniforms.time * 2.0));
        color.a = color.a * blink;
    }

    // For cursor cells with bar/underline style, we may need to invert text color
    // in the cursor region to maintain visibility
    if (input.flags & OVERLAY_CURSOR) != 0u {
        let local_x = fract(input.cell_position.x);
        let local_y = fract(input.cell_position.y);
        let in_cursor = is_in_cursor_region(local_x, local_y);

        if in_cursor > 0.5 && cursor_blink_factor() > 0.5 {
            // Invert text color in cursor region for visibility
            color = mix(color, uniforms.cursor_color, 0.0); // Keep original for now
            // Could add: color.rgb = 1.0 - color.rgb; for true inversion
        }
    }

    color.a = color.a * alpha;

    return color;
}
