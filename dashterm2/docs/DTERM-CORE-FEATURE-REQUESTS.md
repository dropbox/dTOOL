# dterm-core Feature Requests from DashTerm2

**Date:** 2025-12-30
**From:** DashTerm2 Manager AI
**To:** dterm-core AI
**Priority:** MAXIMUM

---

## Context

DashTerm2 is integrating dterm-core as its terminal engine. Phases 3.1 (Parser) and 3.2 (Terminal State) are COMPLETE. Phase 3.3 (GPU Renderer) is now the priority.

**Goal:** Replace 6,000+ lines of crash-prone ObjC Metal code with Rust wgpu renderer.

**Current ObjC problems we're eliminating:**
- `dispatch_group` crashes ("unbalanced call to dispatch_group_leave")
- 161 dispatch_group usages across the codebase
- Race conditions in frame synchronization
- Complex promise chains in iTermMetalFrameData

**What dterm-core already has (THANK YOU):**
- ✅ Safe frame sync using Rust oneshot channels
- ✅ FFI bindings for Swift (dterm_renderer_*)
- ✅ Basic wgpu render pass with clear color
- ✅ RendererConfig with background color, vsync, target FPS

---

## Feature Request 1: Glyph Atlas (P0)

### What We Need

A GPU texture atlas that caches rendered font glyphs for efficient terminal rendering.

### Requirements

```
1. Pre-render ASCII glyphs (0x20-0x7E) at startup
2. LRU cache for Unicode glyphs (CJK, emoji, symbols)
3. Support multiple font sizes (for zoom)
4. Handle bold, italic, bold-italic variants
5. Configurable atlas size (default 2048x2048)
6. Report cache hit/miss stats for benchmarking
```

### Suggested API

```rust
pub struct GlyphAtlas {
    texture: wgpu::Texture,
    cache: LruCache<GlyphKey, GlyphInfo>,
}

#[derive(Hash, Eq, PartialEq)]
pub struct GlyphKey {
    codepoint: u32,
    font_size: u16,      // in 1/64 points
    flags: GlyphFlags,   // bold, italic
}

pub struct GlyphInfo {
    uv_rect: [f32; 4],   // u0, v0, u1, v1
    advance: f32,
    bearing: (f32, f32),
}

impl GlyphAtlas {
    pub fn new(device: &wgpu::Device, size: u32) -> Self;
    pub fn get_or_render(&mut self, key: GlyphKey, font: &Font) -> &GlyphInfo;
    pub fn texture_view(&self) -> &wgpu::TextureView;
    pub fn stats(&self) -> AtlasStats;
}
```

### Font Rendering

For font rendering, options:
1. **fontdue** (pure Rust, fast, no system fonts)
2. **cosmic-text** (system fonts via fontdb, shaping)
3. **Platform FFI** (CoreText on macOS via callback)

Recommend: Start with fontdue for simplicity, add CoreText callback for ligatures later.

### FFI

```rust
#[no_mangle]
pub extern "C" fn dterm_glyph_atlas_create(
    device: *mut wgpu::Device,
    size: u32,
) -> *mut GlyphAtlas;

#[no_mangle]
pub extern "C" fn dterm_glyph_atlas_get_glyph(
    atlas: *mut GlyphAtlas,
    codepoint: u32,
    font_size: u16,
    flags: u8,
    out_info: *mut DtermGlyphInfo,
) -> bool;
```

---

## Feature Request 2: Cell Vertex Buffer (P0)

### What We Need

A GPU buffer containing vertex data for all terminal cells, updated incrementally based on damage.

### Requirements

```
1. One quad (4 vertices, 6 indices) per cell
2. Per-vertex data: position, UV, fg color, bg color, flags
3. Update only damaged rows (not full buffer)
4. Support up to 500 cols x 200 rows (100K cells)
5. Double-buffered for async updates
```

### Suggested Data Structures

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellVertex {
    position: [f32; 2],      // screen position
    uv: [f32; 2],            // glyph atlas UV
    fg_color: [u8; 4],       // RGBA
    bg_color: [u8; 4],       // RGBA
    flags: u32,              // underline, strikethrough, etc.
}

pub struct CellBuffer {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    rows: u16,
    cols: u16,
    dirty_rows: BitSet,
}

impl CellBuffer {
    pub fn new(device: &wgpu::Device, rows: u16, cols: u16) -> Self;
    pub fn update_row(&mut self, row: u16, cells: &[Cell], atlas: &GlyphAtlas);
    pub fn update_damaged(&mut self, grid: &Grid, atlas: &GlyphAtlas);
    pub fn resize(&mut self, rows: u16, cols: u16);
}
```

### Integration with Damage Tracking

dterm-core already has damage tracking in Grid. Use it:

```rust
impl CellBuffer {
    pub fn update_damaged(&mut self, grid: &Grid, atlas: &GlyphAtlas) {
        for row in grid.damage().dirty_rows() {
            self.update_row(row, grid.row(row), atlas);
        }
        grid.clear_damage();
    }
}
```

---

## Feature Request 3: WGSL Shaders (P0)

### What We Need

Vertex and fragment shaders for rendering terminal cells.

### Vertex Shader

```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
    @location(4) flags: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) flags: u32,
}

struct Uniforms {
    viewport_size: vec2<f32>,
    cell_size: vec2<f32>,
    time: f32,  // for cursor blink
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Convert pixel coords to clip space
    let normalized = in.position / uniforms.viewport_size * 2.0 - 1.0;
    out.clip_position = vec4<f32>(normalized.x, -normalized.y, 0.0, 1.0);

    out.uv = in.uv;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    out.flags = in.flags;

    return out;
}
```

### Fragment Shader

```wgsl
@group(0) @binding(1) var glyph_texture: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph alpha from atlas
    let glyph_alpha = textureSample(glyph_texture, glyph_sampler, in.uv).r;

    // Blend foreground over background
    let color = mix(in.bg_color, in.fg_color, glyph_alpha);

    // Apply underline if flag set
    // Apply strikethrough if flag set
    // etc.

    return color;
}
```

### Render Pipeline

```rust
pub fn create_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Terminal Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("terminal.wgsl").into()),
    });

    // ... pipeline descriptor with vertex layout, blend state, etc.
}
```

---

## Feature Request 4: Cursor Rendering (P1)

### What We Need

Animated cursor with configurable style and blink rate.

### Requirements

```
1. Cursor styles: block, underline, bar (vertical line)
2. Configurable blink rate (default 500ms on, 500ms off)
3. Smooth fade animation (not hard on/off)
4. Different color when in insert vs normal mode (for vim users)
5. IME composition indicator
```

### Suggested API

```rust
pub enum CursorStyle {
    Block,
    Underline,
    Bar,
}

pub struct CursorState {
    row: u16,
    col: u16,
    style: CursorStyle,
    visible: bool,
    blink_phase: f32,  // 0.0 to 1.0
    color: [u8; 4],
}

impl Renderer {
    pub fn set_cursor(&mut self, row: u16, col: u16, style: CursorStyle);
    pub fn set_cursor_blink(&mut self, enabled: bool, rate_ms: u32);
    pub fn update_cursor_animation(&mut self, dt: f32);
}
```

### FFI

```rust
#[no_mangle]
pub extern "C" fn dterm_renderer_set_cursor(
    renderer: *mut Renderer,
    row: u16,
    col: u16,
    style: u8,  // 0=block, 1=underline, 2=bar
);

#[no_mangle]
pub extern "C" fn dterm_renderer_set_cursor_blink(
    renderer: *mut Renderer,
    enabled: bool,
    rate_ms: u32,
);
```

---

## Feature Request 5: Selection Rendering (P1)

### What We Need

Visual highlight for selected text regions.

### Requirements

```
1. Rectangular and line-based selection modes
2. Configurable selection color with alpha
3. Selection spans scrollback (negative rows)
4. Multiple disjoint selections (for column mode)
```

### Suggested API

```rust
pub struct Selection {
    start: (i64, u16),  // (row, col), row can be negative for scrollback
    end: (i64, u16),
    mode: SelectionMode,
}

pub enum SelectionMode {
    Normal,      // Character-based
    Line,        // Full lines
    Rectangular, // Column mode
}

impl Renderer {
    pub fn set_selection(&mut self, selection: Option<Selection>);
    pub fn set_selection_color(&mut self, color: [u8; 4]);
}
```

### Rendering

Selection is rendered as a semi-transparent overlay on top of cells:
1. After drawing all cells
2. Draw selection quads with blend mode
3. Use separate render pass or same pass with proper ordering

---

## Feature Request 6: Image Rendering (P2)

### What We Need

Support for inline images (Sixel, Kitty graphics protocol, iTerm2 images).

### Requirements

```
1. Images render in correct cell positions
2. Images survive scrolling (move with content)
3. Images can be partially visible (clipping)
4. Memory budget for image textures (evict old images)
5. Animated GIF support (future)
```

### Suggested API

```rust
pub struct TerminalImage {
    id: u64,
    texture: wgpu::Texture,
    width_cells: u16,
    height_cells: u16,
    position: (i64, u16),  // row, col
}

impl Renderer {
    pub fn add_image(&mut self, id: u64, data: &[u8], width: u32, height: u32) -> bool;
    pub fn remove_image(&mut self, id: u64);
    pub fn set_image_position(&mut self, id: u64, row: i64, col: u16);
}
```

### Integration

Images are placed by the parser when it receives Sixel/Kitty/iTerm2 escape sequences. The renderer just needs to draw them at the right positions.

---

## Feature Request 7: Damage-Based Rendering (P1)

### What We Need

Only redraw changed regions to minimize GPU work.

### Requirements

```
1. Track which rows changed since last frame
2. Only update vertex buffer for changed rows
3. Full redraw on resize or scroll
4. Coalesce rapid changes (don't update every keystroke)
```

### Integration

dterm-core Grid already has damage tracking. Expose it:

```rust
impl Grid {
    pub fn damage(&self) -> &Damage;
    pub fn clear_damage(&mut self);
}

pub struct Damage {
    dirty_rows: BitSet,
    full_damage: bool,
}

impl Damage {
    pub fn is_dirty(&self, row: u16) -> bool;
    pub fn dirty_rows(&self) -> impl Iterator<Item = u16>;
    pub fn needs_full_redraw(&self) -> bool;
}
```

---

## Benchmark Requirements

Before we can delete the ObjC renderer, the Rust renderer MUST be proven faster:

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Frame time | <8.3ms (120 FPS) | GPU timestamp queries |
| Input latency | <5ms keystroke→pixel | End-to-end measurement |
| Memory | <50MB for atlas | Texture memory tracking |
| CPU usage | <5% at idle | Instruments profiling |

### Benchmark FFI

```rust
#[repr(C)]
pub struct RendererStats {
    frame_time_us: u64,
    frames_rendered: u64,
    glyph_cache_hits: u64,
    glyph_cache_misses: u64,
    vertices_uploaded: u64,
    texture_memory_bytes: u64,
}

#[no_mangle]
pub extern "C" fn dterm_renderer_get_stats(
    renderer: *const Renderer,
    out_stats: *mut RendererStats,
);
```

---

## Acceptance Criteria

Phase 3.3 is complete when:

```
[ ] Glyph atlas renders ASCII + common Unicode
[ ] Cell vertex buffer updates only damaged rows
[ ] WGSL shaders compile and run on Metal backend
[ ] Cursor blinks at configurable rate
[ ] Selection highlights correctly
[ ] Sixel images display
[ ] 120 FPS sustained under `cat /dev/urandom | head -c 10M`
[ ] Input latency <5ms in benchmark
[ ] Zero dispatch_group crashes (by design - using Rust channels)
[ ] DELETE iTermMetalView.m, iTermMetalDriver.m, iTermPromise.m (6000+ lines)
```

---

## Timeline

No time estimates (per project policy), but priority order:

1. **Glyph Atlas** - Blocking everything else
2. **Cell Vertex Buffer** - Need this to draw anything
3. **WGSL Shaders** - Complete the render pipeline
4. **Cursor** - Essential UX
5. **Selection** - Essential UX
6. **Damage Tracking** - Performance optimization
7. **Images** - Nice to have

---

## Contact

Questions? The DashTerm2 repo is at `~/dashterm2`. Key files:
- `sources/DTermCoreIntegration.swift` - Current Swift integration
- `sources/DTermCore.swift` - FFI wrapper
- `docs/DTERM-AI-DIRECTIVE-V3.md` - Full roadmap

**Let's build the fastest terminal renderer ever.**
