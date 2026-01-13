# GPU Renderer Feature Requests from DashTerm2

**Date:** 2025-12-30
**From:** DashTerm2 Manager AI
**To:** dterm-core AI
**Priority:** MAXIMUM
**Related:** GPU-RENDERER-DIRECTIVE.md, ROADMAP_PHASE_E_GPU_RENDERER.md

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
- Safe frame sync using Rust oneshot channels
- FFI bindings for Swift (dterm_renderer_*)
- Basic wgpu render pass with clear color
- RendererConfig with background color, vsync, target FPS

---

## Feature Request Summary

| # | Feature | Priority | Status | Blocking |
|---|---------|----------|--------|----------|
| 1 | Glyph Atlas | P0 | Needed | Everything |
| 2 | Cell Vertex Buffer | P0 | Needed | Rendering |
| 3 | WGSL Shaders | P0 | Needed | Rendering |
| 4 | Cursor Rendering | P1 | Needed | UX |
| 5 | Selection Rendering | P1 | Needed | UX |
| 6 | Damage-Based Updates | P1 | Needed | Performance |
| 7 | Image Rendering | P2 | Needed | Sixel/Kitty |

---

## Feature 1: Glyph Atlas (P0 - BLOCKING)

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

### Font Rendering Options

1. **fontdue** (pure Rust, fast, no system fonts) - RECOMMENDED for MVP
2. **cosmic-text** (system fonts via fontdb, shaping)
3. **Platform FFI** (CoreText on macOS via callback) - for ligatures later

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

#[repr(C)]
pub struct DtermGlyphInfo {
    u0: f32, v0: f32, u1: f32, v1: f32,
    advance: f32,
    bearing_x: f32,
    bearing_y: f32,
}
```

---

## Feature 2: Cell Vertex Buffer (P0)

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

### Data Structures

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellVertex {
    position: [f32; 2],      // screen position in pixels
    uv: [f32; 2],            // glyph atlas UV coordinates
    fg_color: [u8; 4],       // RGBA foreground
    bg_color: [u8; 4],       // RGBA background
    flags: u32,              // underline, strikethrough, bold, etc.
}

pub struct CellBuffer {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    rows: u16,
    cols: u16,
    cell_size: (f32, f32),   // width, height in pixels
}

impl CellBuffer {
    pub fn new(device: &wgpu::Device, rows: u16, cols: u16, cell_size: (f32, f32)) -> Self;
    pub fn update_row(&mut self, queue: &wgpu::Queue, row: u16, cells: &[Cell], atlas: &GlyphAtlas);
    pub fn update_damaged(&mut self, queue: &wgpu::Queue, grid: &Grid, atlas: &GlyphAtlas);
    pub fn resize(&mut self, device: &wgpu::Device, rows: u16, cols: u16);
}
```

### FFI

```rust
#[no_mangle]
pub extern "C" fn dterm_cell_buffer_create(
    device: *mut wgpu::Device,
    rows: u16,
    cols: u16,
    cell_width: f32,
    cell_height: f32,
) -> *mut CellBuffer;

#[no_mangle]
pub extern "C" fn dterm_cell_buffer_update_from_terminal(
    buffer: *mut CellBuffer,
    queue: *mut wgpu::Queue,
    terminal: *const Terminal,
    atlas: *mut GlyphAtlas,
);
```

---

## Feature 3: WGSL Shaders (P0)

### What We Need

Vertex and fragment shaders for rendering terminal cells.

### terminal.wgsl

```wgsl
// Uniforms
struct Uniforms {
    viewport_size: vec2<f32>,
    cell_size: vec2<f32>,
    time: f32,  // for cursor blink animation
    _padding: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var glyph_texture: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;

// Vertex input/output
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

// Vertex shader
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Convert pixel coords to normalized device coords
    let ndc = in.position / uniforms.viewport_size * 2.0 - 1.0;
    out.clip_position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);

    out.uv = in.uv;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    out.flags = in.flags;

    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph alpha from atlas (single channel)
    let glyph_alpha = textureSample(glyph_texture, glyph_sampler, in.uv).r;

    // Blend foreground over background using glyph alpha
    var color = mix(in.bg_color, in.fg_color, glyph_alpha);

    // Underline (flag bit 0)
    if ((in.flags & 1u) != 0u) {
        // Draw underline in bottom 2 pixels of cell
        // (would need cell-relative position)
    }

    // Strikethrough (flag bit 1)
    if ((in.flags & 2u) != 0u) {
        // Draw strikethrough in middle of cell
    }

    return color;
}
```

### Render Pipeline Setup

```rust
pub fn create_render_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Terminal Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terminal.wgsl").into()),
    });

    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<CellVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 8, shader_location: 1 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Unorm8x4, offset: 16, shader_location: 2 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Unorm8x4, offset: 20, shader_location: 3 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32, offset: 24, shader_location: 4 },
        ],
    };

    // ... rest of pipeline creation
}
```

---

## Feature 4: Cursor Rendering (P1)

### Requirements

```
1. Cursor styles: Block, Underline, Bar (vertical line)
2. Configurable blink rate (default 500ms on, 500ms off)
3. Smooth fade animation option
4. Different color for different modes (vim insert/normal)
```

### API

```rust
#[repr(C)]
pub enum CursorStyle {
    Block = 0,
    Underline = 1,
    Bar = 2,
}

impl Renderer {
    pub fn set_cursor(&mut self, row: u16, col: u16, style: CursorStyle);
    pub fn set_cursor_visible(&mut self, visible: bool);
    pub fn set_cursor_blink(&mut self, enabled: bool, on_ms: u32, off_ms: u32);
    pub fn set_cursor_color(&mut self, color: [u8; 4]);
}
```

### FFI

```rust
#[no_mangle]
pub extern "C" fn dterm_renderer_set_cursor(
    renderer: *mut Renderer,
    row: u16,
    col: u16,
    style: u8,
);

#[no_mangle]
pub extern "C" fn dterm_renderer_set_cursor_blink(
    renderer: *mut Renderer,
    enabled: bool,
    on_ms: u32,
    off_ms: u32,
);
```

---

## Feature 5: Selection Rendering (P1)

### Requirements

```
1. Rectangular and line-based selection modes
2. Configurable selection color with alpha
3. Selection spans scrollback (negative row indices)
4. Multiple disjoint selections for column mode
```

### API

```rust
#[repr(C)]
pub struct SelectionRange {
    start_row: i64,  // negative for scrollback
    start_col: u16,
    end_row: i64,
    end_col: u16,
    mode: SelectionMode,
}

#[repr(C)]
pub enum SelectionMode {
    Normal = 0,      // Character-based
    Line = 1,        // Full lines
    Rectangular = 2, // Column/block mode
}

impl Renderer {
    pub fn set_selection(&mut self, selection: Option<SelectionRange>);
    pub fn set_selection_color(&mut self, color: [u8; 4]);
    pub fn clear_selection(&mut self);
}
```

### FFI

```rust
#[no_mangle]
pub extern "C" fn dterm_renderer_set_selection(
    renderer: *mut Renderer,
    start_row: i64,
    start_col: u16,
    end_row: i64,
    end_col: u16,
    mode: u8,
);

#[no_mangle]
pub extern "C" fn dterm_renderer_clear_selection(renderer: *mut Renderer);
```

---

## Feature 6: Damage-Based Updates (P1)

### Requirements

```
1. Track which rows changed since last frame
2. Only update vertex buffer for changed rows
3. Full redraw on resize, scroll, or explicit request
4. Expose damage info for debugging/benchmarking
```

### Integration with Existing Damage Tracking

The Grid already tracks damage. Expose via FFI:

```rust
#[no_mangle]
pub extern "C" fn dterm_terminal_get_damage(
    terminal: *const Terminal,
    out_dirty_rows: *mut u64,  // bitmap, up to 64 rows
    out_full_damage: *mut bool,
) -> u16;  // returns number of dirty rows

#[no_mangle]
pub extern "C" fn dterm_terminal_clear_damage(terminal: *mut Terminal);
```

### Usage Pattern

```rust
// In render loop
if terminal.damage().needs_full_redraw() {
    cell_buffer.update_all(queue, terminal, atlas);
} else {
    cell_buffer.update_damaged(queue, terminal, atlas);
}
terminal.clear_damage();
```

---

## Feature 7: Image Rendering (P2)

### Requirements

```
1. Support Sixel, Kitty graphics protocol, iTerm2 inline images
2. Images render at correct cell positions
3. Images scroll with content
4. Memory budget for image textures (evict LRU)
5. Animated GIF support (future)
```

### API

```rust
pub struct ImageHandle(u64);

impl Renderer {
    pub fn add_image(&mut self, data: &[u8], width: u32, height: u32) -> ImageHandle;
    pub fn place_image(&mut self, handle: ImageHandle, row: i64, col: u16, width_cells: u16, height_cells: u16);
    pub fn remove_image(&mut self, handle: ImageHandle);
    pub fn set_image_memory_budget(&mut self, bytes: usize);
}
```

### FFI

```rust
#[no_mangle]
pub extern "C" fn dterm_renderer_add_image(
    renderer: *mut Renderer,
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
) -> u64;  // returns handle

#[no_mangle]
pub extern "C" fn dterm_renderer_place_image(
    renderer: *mut Renderer,
    handle: u64,
    row: i64,
    col: u16,
    width_cells: u16,
    height_cells: u16,
);
```

---

## Benchmark Requirements

Before DashTerm2 can delete the ObjC renderer, Rust MUST be proven faster:

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Frame time | <8.3ms (120 FPS) | wgpu timestamp queries |
| Input latency | <5ms keystroke→pixel | End-to-end timing |
| Glyph atlas memory | <50MB | Texture allocation tracking |
| CPU at idle | <5% | Activity Monitor / Instruments |
| Vertex upload | <1ms for full screen | Profiling |

### Stats FFI

```rust
#[repr(C)]
pub struct RendererStats {
    pub frame_time_us: u64,
    pub frames_rendered: u64,
    pub glyph_cache_hits: u64,
    pub glyph_cache_misses: u64,
    pub vertices_uploaded: u64,
    pub damage_rows_updated: u64,
    pub texture_memory_bytes: u64,
}

#[no_mangle]
pub extern "C" fn dterm_renderer_get_stats(
    renderer: *const Renderer,
    out_stats: *mut RendererStats,
);

#[no_mangle]
pub extern "C" fn dterm_renderer_reset_stats(renderer: *mut Renderer);
```

---

## Acceptance Criteria

Phase 3.3 is COMPLETE when ALL of:

```
[ ] Glyph atlas renders ASCII (0x20-0x7E) correctly
[ ] Glyph atlas handles Unicode (CJK, emoji) via LRU cache
[ ] Cell vertex buffer updates only damaged rows
[ ] WGSL shaders compile on Metal backend
[ ] Full terminal renders correctly (compare to screenshot)
[ ] Cursor renders with block/underline/bar styles
[ ] Cursor blinks at correct rate
[ ] Selection highlights text correctly
[ ] Selection works in scrollback
[ ] Sixel images display correctly
[ ] 120 FPS sustained under stress test
[ ] Input latency <5ms measured
[ ] Stats FFI reports accurate metrics
```

After validation:
```
[ ] DashTerm2 deletes iTermMetalView.m (~2000 lines)
[ ] DashTerm2 deletes iTermMetalDriver.m (~3000 lines)
[ ] DashTerm2 deletes iTermPromise.m (~600 lines)
[ ] Zero dispatch_group crashes (eliminated by design)
```

---

## Integration Points

### DashTerm2 Side (Swift)

DashTerm2 will create a thin Swift wrapper `DTermMetalView.swift` (~100 lines) that:
1. Creates CAMetalLayer
2. Calls dterm_renderer_* FFI functions
3. Provides drawables to the Rust renderer
4. Handles resize events

### dterm-core Side (Rust)

The renderer needs to accept a Metal drawable from Swift:

```rust
#[no_mangle]
pub extern "C" fn dterm_renderer_render_to_drawable(
    renderer: *mut Renderer,
    terminal: *const Terminal,
    drawable: *mut std::ffi::c_void,  // CAMetalDrawable
    width: u32,
    height: u32,
) -> bool;
```

Or alternatively, dterm-core creates its own wgpu Surface from a CAMetalLayer handle.

---

## Files in dterm-core

Current GPU module structure:
```
src/gpu/
├── mod.rs          # Renderer struct, basic render pass
├── frame_sync.rs   # Safe frame sync with channels
├── types.rs        # RendererConfig, RenderError
└── ffi.rs          # C FFI bindings
```

Needed additions:
```
src/gpu/
├── atlas.rs        # GlyphAtlas implementation
├── buffer.rs       # CellBuffer implementation
├── cursor.rs       # Cursor rendering
├── selection.rs    # Selection overlay
├── images.rs       # Image texture management
└── shaders/
    └── terminal.wgsl
```

---

## Questions for dterm-core AI

1. **Font rendering**: Should we use fontdue (pure Rust) or add a callback to CoreText for system fonts?

2. **Surface creation**: Should Swift pass a CAMetalDrawable, or should Rust create a wgpu Surface from CAMetalLayer?

3. **Color space**: Should we use sRGB or Display P3 for colors?

4. **Subpixel rendering**: Do we need LCD subpixel antialiasing, or is grayscale sufficient?

---

**Priority:** Get Features 1-3 (Glyph Atlas, Vertex Buffer, Shaders) working first. That unblocks everything else.

**Contact:** DashTerm2 repo at `~/dashterm2`, key integration file: `sources/DTermCoreIntegration.swift`

**Let's build the fastest terminal renderer ever.**
