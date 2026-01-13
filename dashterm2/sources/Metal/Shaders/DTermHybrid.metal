//
//  DTermHybrid.metal
//  DashTerm2
//
//  Metal shaders for dterm-core hybrid renderer.
//
//  These shaders consume DtermCellVertex data from Rust and render
//  terminal cells using a glyph atlas texture.
//
//  Created by DashTerm2 AI Worker on 2024-12-30.
//

#include <metal_stdlib>
using namespace metal;

// =============================================================================
// Vertex Flag Constants (New 7-bit Layout) - per METAL_SHADER_MIGRATION.md
// =============================================================================
//
// Bit:  6  5  4  3  2  1  0
//      [OV][  EFF  ][ TYPE ]

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

// Extract vertex type from flags
uint get_vertex_type(uint flags) {
    return flags & VERTEX_TYPE_MASK;
}

// Vertex input from dterm-core (64 bytes, matches DtermCellVertex)
struct DTermCellVertex {
    float2 position;     // Cell grid coordinates
    float2 uv;           // Atlas UV (normalized 0-1)
    float4 fg_color;     // Foreground RGBA
    float4 bg_color;     // Background RGBA
    uint flags;          // Style flags
    uint _padding[3];    // Alignment padding
};

// Uniforms from dterm-core (64 bytes, matches DtermUniforms)
struct DTermUniforms {
    float viewport_width;   // Viewport width in pixels
    float viewport_height;  // Viewport height in pixels
    float cell_width;       // Cell width in pixels
    float cell_height;      // Cell height in pixels
    float atlas_size;       // Atlas texture size in pixels
    float time;             // Animation time in seconds
    int cursor_x;           // Cursor X (-1 if hidden)
    int cursor_y;           // Cursor Y (-1 if hidden)
    float4 cursor_color;    // Cursor RGBA
    float4 _padding;        // Alignment padding
};

// Vertex output to fragment shader
struct VertexOut {
    float4 position [[position]];   // Clip-space position
    float2 texCoord;                // Atlas texture coordinates
    float4 fgColor;                 // Foreground color
    float4 bgColor;                 // Background color
    uint flags;                     // Style flags
};

// Background vertex shader
// Renders solid color quads for cell backgrounds
vertex VertexOut dtermBackgroundVertex(
    uint vertexID [[vertex_id]],
    uint instanceID [[instance_id]],
    constant DTermCellVertex *vertices [[buffer(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    DTermCellVertex v = vertices[instanceID * 6 + vertexID];
    VertexOut out;

    // Convert cell grid position to clip space (-1 to 1)
    float2 pixelPos = v.position * float2(uniforms.cell_width, uniforms.cell_height);
    float2 clipPos;
    clipPos.x = (pixelPos.x / uniforms.viewport_width) * 2.0 - 1.0;
    clipPos.y = 1.0 - (pixelPos.y / uniforms.viewport_height) * 2.0;  // Flip Y

    out.position = float4(clipPos, 0.0, 1.0);
    out.texCoord = v.uv;
    out.fgColor = v.fg_color;
    out.bgColor = v.bg_color;
    out.flags = v.flags;

    return out;
}

// Background fragment shader
fragment float4 dtermBackgroundFragment(
    VertexOut in [[stage_in]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    float4 color = in.bgColor;

    // Handle inverse video
    if ((in.flags & EFFECT_INVERSE) != 0u) {
        color = in.fgColor;
    }

    // Selection highlight
    if ((in.flags & OVERLAY_SELECTION) != 0u) {
        color = mix(color, float4(0.3, 0.5, 0.8, 1.0), 0.5);
    }

    // Cursor highlight
    if ((in.flags & OVERLAY_CURSOR) != 0u) {
        color = mix(color, uniforms.cursor_color, 0.7);
    }

    return color;
}

// Glyph vertex shader
// Renders textured quads from the glyph atlas
vertex VertexOut dtermGlyphVertex(
    uint vertexID [[vertex_id]],
    uint instanceID [[instance_id]],
    constant DTermCellVertex *vertices [[buffer(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    DTermCellVertex v = vertices[instanceID * 6 + vertexID];
    VertexOut out;

    // Convert cell grid position to clip space (-1 to 1)
    float2 pixelPos = v.position * float2(uniforms.cell_width, uniforms.cell_height);
    float2 clipPos;
    clipPos.x = (pixelPos.x / uniforms.viewport_width) * 2.0 - 1.0;
    clipPos.y = 1.0 - (pixelPos.y / uniforms.viewport_height) * 2.0;  // Flip Y

    out.position = float4(clipPos, 0.0, 1.0);
    out.texCoord = v.uv;
    out.fgColor = v.fg_color;
    out.bgColor = v.bg_color;
    out.flags = v.flags;

    return out;
}

// Glyph fragment shader
fragment float4 dtermGlyphFragment(
    VertexOut in [[stage_in]],
    texture2d<float> atlas [[texture(0)]],
    sampler atlasSampler [[sampler(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    float alpha = atlas.sample(atlasSampler, in.texCoord).r;
    float4 color = in.fgColor;

    // Handle inverse video
    if ((in.flags & EFFECT_INVERSE) != 0u) {
        color = in.bgColor;
    }

    // Apply dim effect
    if ((in.flags & EFFECT_DIM) != 0u) {
        color.rgb *= 0.5;
    }

    // Apply blink effect
    if ((in.flags & EFFECT_BLINK) != 0u) {
        float blink = sin(uniforms.time * 3.14159) * 0.5 + 0.5;
        alpha *= blink;
    }

    if (alpha < 0.01) {
        discard_fragment();
    }

    return float4(color.rgb, alpha);
}

// Combined vertex shader for instanced cell rendering
// Each cell is 6 vertices (2 triangles forming a quad)
vertex VertexOut dtermCellVertex(
    uint vertexID [[vertex_id]],
    constant DTermCellVertex *vertices [[buffer(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    DTermCellVertex v = vertices[vertexID];
    VertexOut out;

    // Convert cell grid position to clip space
    float2 pixelPos = v.position * float2(uniforms.cell_width, uniforms.cell_height);
    float2 clipPos;
    clipPos.x = (pixelPos.x / uniforms.viewport_width) * 2.0 - 1.0;
    clipPos.y = 1.0 - (pixelPos.y / uniforms.viewport_height) * 2.0;

    out.position = float4(clipPos, 0.0, 1.0);
    out.texCoord = v.uv;
    out.fgColor = v.fg_color;
    out.bgColor = v.bg_color;
    out.flags = v.flags;

    return out;
}

// Combined fragment shader - routes by vertex type (bits 0-1)
fragment float4 dtermCellFragment(
    VertexOut in [[stage_in]],
    texture2d<float> atlas [[texture(0)]],
    sampler atlasSampler [[sampler(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    uint vertex_type = get_vertex_type(in.flags);

    // ==== BACKGROUND (type = 1) ====
    if (vertex_type == VERTEX_TYPE_BACKGROUND) {
        float4 color = in.bgColor;

        if ((in.flags & EFFECT_INVERSE) != 0u) {
            color = in.fgColor;
        }
        if ((in.flags & OVERLAY_CURSOR) != 0u) {
            color = mix(color, uniforms.cursor_color, 0.7);
        }
        if ((in.flags & OVERLAY_SELECTION) != 0u) {
            color = mix(color, float4(0.3, 0.5, 0.8, 1.0), 0.4);
        }
        return color;
    }

    // ==== DECORATION (type = 2) ====
    if (vertex_type == VERTEX_TYPE_DECORATION) {
        float4 color = in.fgColor;

        if ((in.flags & EFFECT_INVERSE) != 0u) {
            color = in.bgColor;
        }
        if ((in.flags & EFFECT_DIM) != 0u) {
            color.rgb *= 0.5;
        }
        return color;
    }

    // ==== GLYPH (type = 0, default) ====
    float alpha = atlas.sample(atlasSampler, in.texCoord).r;
    float4 color = in.fgColor;

    if ((in.flags & EFFECT_INVERSE) != 0u) {
        color = in.bgColor;
    }
    if ((in.flags & EFFECT_DIM) != 0u) {
        color.rgb *= 0.5;
    }
    if ((in.flags & EFFECT_BLINK) != 0u) {
        float blink = sin(uniforms.time * 3.14159) * 0.5 + 0.5;
        alpha *= blink;
    }
    if (alpha < 0.01) {
        discard_fragment();
    }
    return float4(color.rgb, alpha);
}

// Underline/strikethrough vertex shader
// Renders line decorations separately for clean antialiasing
vertex VertexOut dtermDecorationVertex(
    uint vertexID [[vertex_id]],
    constant float2 *positions [[buffer(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]],
    constant float4 *colors [[buffer(2)]])
{
    VertexOut out;

    float2 pos = positions[vertexID];
    float2 pixelPos = pos * float2(uniforms.cell_width, uniforms.cell_height);
    float2 clipPos;
    clipPos.x = (pixelPos.x / uniforms.viewport_width) * 2.0 - 1.0;
    clipPos.y = 1.0 - (pixelPos.y / uniforms.viewport_height) * 2.0;

    out.position = float4(clipPos, 0.0, 1.0);
    out.texCoord = float2(0.0);
    out.fgColor = colors[vertexID / 6];  // One color per quad (6 vertices)
    out.bgColor = float4(0.0);
    out.flags = 0;

    return out;
}

// Underline/strikethrough fragment shader
fragment float4 dtermDecorationFragment(VertexOut in [[stage_in]])
{
    return in.fgColor;
}

// Cursor vertex shader (for block/bar/underline cursor styles)
vertex VertexOut dtermCursorVertex(
    uint vertexID [[vertex_id]],
    constant float2 *quad [[buffer(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    VertexOut out;

    // Cursor position in cell coordinates
    float2 cursorBase = float2(uniforms.cursor_x, uniforms.cursor_y);
    float2 pos = cursorBase + quad[vertexID];

    float2 pixelPos = pos * float2(uniforms.cell_width, uniforms.cell_height);
    float2 clipPos;
    clipPos.x = (pixelPos.x / uniforms.viewport_width) * 2.0 - 1.0;
    clipPos.y = 1.0 - (pixelPos.y / uniforms.viewport_height) * 2.0;

    out.position = float4(clipPos, 0.0, 1.0);
    out.texCoord = float2(0.0);
    out.fgColor = uniforms.cursor_color;
    out.bgColor = float4(0.0);
    out.flags = OVERLAY_CURSOR;

    return out;
}

// Cursor fragment shader with optional animation
fragment float4 dtermCursorFragment(
    VertexOut in [[stage_in]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    // Cursor blink animation
    float blink = sin(uniforms.time * 2.0 * 3.14159) * 0.5 + 0.5;
    float alpha = in.fgColor.a * (0.5 + blink * 0.5);

    return float4(in.fgColor.rgb, alpha);
}

// MARK: - Image Rendering (Sixel, Kitty Graphics)

// Image vertex input (32 bytes)
struct DTermImageVertex {
    float2 position;     // Pixel position (not cell coordinates)
    float2 uv;           // Texture UV (0-1)
    float4 tint;         // Tint/alpha multiplier
};

// Image uniforms (matches DTermUniforms but may add image-specific fields)
struct DTermImageUniforms {
    float viewport_width;
    float viewport_height;
    float cell_width;
    float cell_height;
    float atlas_size;       // Not used for images
    float time;
    int cursor_x;           // Not used for images
    int cursor_y;           // Not used for images
    float4 cursor_color;    // Not used for images
    float4 _padding;
};

// Image vertex output
struct ImageVertexOut {
    float4 position [[position]];
    float2 texCoord;
    float4 tint;
};

// Image vertex shader
// Renders images at pixel-perfect positions
vertex ImageVertexOut dtermImageVertex(
    uint vertexID [[vertex_id]],
    constant DTermImageVertex *vertices [[buffer(0)]],
    constant DTermUniforms &uniforms [[buffer(1)]])
{
    DTermImageVertex v = vertices[vertexID];
    ImageVertexOut out;

    // Position is already in pixel coordinates
    float2 clipPos;
    clipPos.x = (v.position.x / uniforms.viewport_width) * 2.0 - 1.0;
    clipPos.y = 1.0 - (v.position.y / uniforms.viewport_height) * 2.0;

    out.position = float4(clipPos, 0.0, 1.0);
    out.texCoord = v.uv;
    out.tint = v.tint;

    return out;
}

// Image fragment shader
// Samples RGBA texture with alpha blending
fragment float4 dtermImageFragment(
    ImageVertexOut in [[stage_in]],
    texture2d<float> imageTexture [[texture(0)]],
    sampler imageSampler [[sampler(0)]])
{
    // Bounds check for texture coordinates
    if (in.texCoord.x < 0.0 || in.texCoord.x > 1.0 ||
        in.texCoord.y < 0.0 || in.texCoord.y > 1.0) {
        discard_fragment();
    }

    float4 color = imageTexture.sample(imageSampler, in.texCoord);

    // Apply tint (for alpha fade effects, colorization, etc.)
    color *= in.tint;

    // Discard fully transparent pixels
    if (color.a < 0.01) {
        discard_fragment();
    }

    return color;
}
