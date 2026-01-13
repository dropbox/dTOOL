//
//  Shaders.metal
//  DashTerm
//
//  Metal shaders for graph visualization rendering
//

#include <metal_stdlib>
using namespace metal;

// =============================================================================
// Common Types
// =============================================================================

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 texCoord [[attribute(1)]];
    float4 color [[attribute(2)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 texCoord;
    float4 color;
};

struct Uniforms {
    float4x4 projectionMatrix;
    float4x4 viewMatrix;
    float2 viewportSize;
    float time;
    float padding;
};

// =============================================================================
// Node Rendering
// =============================================================================

// Vertex shader for nodes
vertex VertexOut nodeVertexShader(
    VertexIn in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    VertexOut out;
    float4 worldPos = float4(in.position, 0.0, 1.0);
    out.position = uniforms.projectionMatrix * uniforms.viewMatrix * worldPos;
    out.texCoord = in.texCoord;
    out.color = in.color;
    return out;
}

// Fragment shader for nodes with rounded corners
fragment float4 nodeFragmentShader(
    VertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    // Calculate distance from center for rounded corners
    float2 center = float2(0.5, 0.5);
    float2 p = in.texCoord - center;

    // Rounded rectangle SDF
    float2 size = float2(0.4, 0.4);
    float radius = 0.1;
    float2 d = abs(p) - size + radius;
    float dist = length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;

    // Smooth edge
    float alpha = 1.0 - smoothstep(-0.02, 0.02, dist);

    // Border
    float border = smoothstep(-0.02, 0.0, dist) * (1.0 - smoothstep(0.0, 0.02, dist));
    float4 borderColor = float4(1.0, 1.0, 1.0, 0.5);

    float4 color = mix(in.color, borderColor, border);
    color.a *= alpha;

    return color;
}

// =============================================================================
// Edge Rendering
// =============================================================================

struct EdgeVertexIn {
    float2 position [[attribute(0)]];
    float2 direction [[attribute(1)]];
    float progress [[attribute(2)]];
    float4 color [[attribute(3)]];
};

struct EdgeVertexOut {
    float4 position [[position]];
    float2 texCoord;
    float progress;
    float4 color;
};

// Vertex shader for edges (line with thickness)
vertex EdgeVertexOut edgeVertexShader(
    EdgeVertexIn in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]],
    uint vertexID [[vertex_id]]
) {
    EdgeVertexOut out;

    // Calculate perpendicular for line thickness
    float2 perp = normalize(float2(-in.direction.y, in.direction.x));
    float thickness = 2.0;  // pixels

    // Offset based on vertex ID (0,1 = start, 2,3 = end)
    float side = (vertexID % 2 == 0) ? -1.0 : 1.0;
    float2 offset = perp * thickness * side / uniforms.viewportSize;

    float4 worldPos = float4(in.position + offset, 0.0, 1.0);
    out.position = uniforms.projectionMatrix * uniforms.viewMatrix * worldPos;
    out.texCoord = float2((vertexID < 2) ? 0.0 : 1.0, side * 0.5 + 0.5);
    out.progress = in.progress;
    out.color = in.color;

    return out;
}

// Fragment shader for edges with animation
fragment float4 edgeFragmentShader(
    EdgeVertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    // Dashed line pattern
    float dashLength = 0.1;
    float gapLength = 0.05;
    float pattern = fmod(in.texCoord.x + uniforms.time * 0.5, dashLength + gapLength);
    float dash = step(pattern, dashLength);

    // Flow animation for active edges
    float flowPos = fmod(uniforms.time * 2.0, 1.0);
    float flowGlow = exp(-pow((in.texCoord.x - flowPos) * 5.0, 2.0));

    float4 color = in.color;
    color.rgb += flowGlow * 0.3;
    color.a *= dash;

    // Anti-aliased edge
    float edgeDist = abs(in.texCoord.y - 0.5) * 2.0;
    color.a *= 1.0 - smoothstep(0.8, 1.0, edgeDist);

    return color;
}

// =============================================================================
// Arrow Head Rendering
// =============================================================================

// Fragment shader for arrow heads
fragment float4 arrowFragmentShader(
    VertexOut in [[stage_in]]
) {
    // Triangle SDF
    float2 p = in.texCoord - float2(0.5, 0.5);
    float2 n = normalize(float2(0.866, 0.5)); // 60 degree angle
    float d = max(dot(p, n), dot(p, float2(-n.x, n.y)));
    d = max(d, -p.y - 0.3);

    float alpha = 1.0 - smoothstep(-0.02, 0.02, d);

    return float4(in.color.rgb, in.color.a * alpha);
}

// =============================================================================
// Grid Background
// =============================================================================

fragment float4 gridFragmentShader(
    VertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    float2 uv = in.texCoord * uniforms.viewportSize;

    // Grid lines
    float gridSize = 50.0;
    float2 grid = abs(fract(uv / gridSize - 0.5) - 0.5) * gridSize;
    float minGrid = min(grid.x, grid.y);

    float gridLine = 1.0 - smoothstep(0.0, 1.5, minGrid);

    // Major grid lines every 5 cells
    float majorGridSize = gridSize * 5.0;
    float2 majorGrid = abs(fract(uv / majorGridSize - 0.5) - 0.5) * majorGridSize;
    float minMajorGrid = min(majorGrid.x, majorGrid.y);
    float majorGridLine = 1.0 - smoothstep(0.0, 2.0, minMajorGrid);

    float4 bgColor = float4(0.05, 0.05, 0.08, 1.0);
    float4 gridColor = float4(0.15, 0.15, 0.2, 1.0);
    float4 majorGridColor = float4(0.2, 0.2, 0.25, 1.0);

    float4 color = mix(bgColor, gridColor, gridLine * 0.5);
    color = mix(color, majorGridColor, majorGridLine * 0.7);

    return color;
}

// =============================================================================
// Glow Effect (for selected/active nodes)
// =============================================================================

fragment float4 glowFragmentShader(
    VertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    float2 center = float2(0.5, 0.5);
    float dist = distance(in.texCoord, center);

    // Pulsing glow
    float pulse = sin(uniforms.time * 3.0) * 0.1 + 0.9;
    float glow = exp(-dist * 3.0 * pulse);

    float4 color = in.color;
    color.a *= glow;

    return color;
}

// =============================================================================
// Running Node Animation
// =============================================================================

fragment float4 runningNodeFragmentShader(
    VertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    // Calculate distance from center for rounded corners
    float2 center = float2(0.5, 0.5);
    float2 p = in.texCoord - center;

    // Rounded rectangle SDF
    float2 size = float2(0.4, 0.4);
    float radius = 0.1;
    float2 d = abs(p) - size + radius;
    float dist = length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;

    // Smooth edge
    float alpha = 1.0 - smoothstep(-0.02, 0.02, dist);

    // Border
    float border = smoothstep(-0.02, 0.0, dist) * (1.0 - smoothstep(0.0, 0.02, dist));
    float4 borderColor = float4(1.0, 1.0, 1.0, 0.5);

    float4 color = mix(in.color, borderColor, border);
    color.a *= alpha;

    // Pulsing brightness for running state
    float pulse = sin(uniforms.time * 4.0) * 0.15 + 1.0;
    color.rgb *= pulse;

    // Animated spinning ring around the node
    float ringDist = abs(dist + 0.06) - 0.015;
    float ring = 1.0 - smoothstep(0.0, 0.02, ringDist);

    // Rotate the ring arc
    float angle = atan2(p.y, p.x);
    float spinAngle = uniforms.time * 5.0;
    float arcLength = 2.5;  // radians
    float arc = smoothstep(0.0, 0.3, fmod(angle - spinAngle + 3.14159, 6.28318) / arcLength);
    arc *= smoothstep(arcLength, arcLength - 0.3, fmod(angle - spinAngle + 3.14159, 6.28318));

    // Combine ring with arc mask
    float4 ringColor = float4(0.4, 0.8, 1.0, 1.0);  // Bright blue spinner
    color = mix(color, ringColor, ring * arc);

    return color;
}

// =============================================================================
// Active Edge Rendering (animated flow for edges connected to running nodes)
// =============================================================================

fragment float4 activeEdgeFragmentShader(
    EdgeVertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    // Animated flow particles
    float flowSpeed = 3.0;
    float flowPos = fmod(uniforms.time * flowSpeed, 1.0);

    // Multiple flow particles for continuous effect
    float particle1 = exp(-pow((in.texCoord.x - flowPos) * 8.0, 2.0));
    float particle2 = exp(-pow((in.texCoord.x - fmod(flowPos + 0.33, 1.0)) * 8.0, 2.0));
    float particle3 = exp(-pow((in.texCoord.x - fmod(flowPos + 0.66, 1.0)) * 8.0, 2.0));
    float particles = max(max(particle1, particle2), particle3);

    // Base edge with glow
    float baseAlpha = 0.6;

    // Pulsing brightness
    float pulse = sin(uniforms.time * 4.0) * 0.2 + 1.0;

    // Enhanced glow color for active edges (bright cyan/blue)
    float4 glowColor = float4(0.3, 0.9, 1.0, 1.0);
    float4 particleColor = float4(1.0, 1.0, 1.0, 1.0);

    float4 color = in.color;
    color.rgb = mix(color.rgb, glowColor.rgb, 0.5) * pulse;
    color.rgb += particles * particleColor.rgb * 0.8;
    color.a = baseAlpha + particles * 0.4;

    // Anti-aliased edge
    float edgeDist = abs(in.texCoord.y - 0.5) * 2.0;
    color.a *= 1.0 - smoothstep(0.7, 1.0, edgeDist);

    return color;
}

// =============================================================================
// Text Rendering (using texture atlas)
// =============================================================================

fragment float4 textFragmentShader(
    VertexOut in [[stage_in]],
    texture2d<float> fontAtlas [[texture(0)]],
    sampler fontSampler [[sampler(0)]]
) {
    // Sample grayscale glyph from R channel
    float glyphAlpha = fontAtlas.sample(fontSampler, in.texCoord).r;

    // Apply text color with glyph alpha
    return float4(in.color.rgb, in.color.a * glyphAlpha);
}

// =============================================================================
// Group Rendering (collapsible node groups)
// =============================================================================

fragment float4 groupFragmentShader(
    VertexOut in [[stage_in]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    // Calculate distance from center for rounded corners
    float2 center = float2(0.5, 0.5);
    float2 p = in.texCoord - center;

    // Rounded rectangle SDF (larger and more rounded than nodes)
    float2 size = float2(0.42, 0.42);
    float radius = 0.08;
    float2 d = abs(p) - size + radius;
    float dist = length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;

    // Smooth edge for fill
    float alpha = 1.0 - smoothstep(-0.02, 0.02, dist);

    // Dashed border effect
    float borderWidth = 0.025;
    float borderDist = abs(dist + borderWidth) - borderWidth * 0.5;
    float border = 1.0 - smoothstep(0.0, 0.015, borderDist);

    // Create dash pattern along the border
    float angle = atan2(p.y, p.x);
    float dashFreq = 20.0;
    float dashPattern = step(0.5, fract(angle * dashFreq / 6.28318 + uniforms.time * 0.5));

    // Border color (white with dash pattern)
    float4 borderColor = float4(1.0, 1.0, 1.0, 0.8 * dashPattern);

    // Fill with semi-transparent background
    float4 fillColor = in.color;
    fillColor.a *= 0.6;

    // Combine fill and border
    float4 color = mix(fillColor, borderColor, border);
    color.a *= alpha;

    // Add subtle folder icon effect in center
    float2 iconP = (in.texCoord - float2(0.5, 0.4)) * 8.0;
    float folder = step(abs(iconP.x), 1.5) * step(abs(iconP.y), 1.0);
    float tab = step(iconP.x, -0.5) * step(iconP.y, -0.8) * step(-1.2, iconP.y);
    folder = max(folder, tab);

    // Add icon to center
    float iconAlpha = folder * 0.2;
    color.rgb = mix(color.rgb, float3(1.0, 1.0, 1.0), iconAlpha * alpha);

    return color;
}
