//! Render pipeline for terminal cells.
//!
//! This module manages:
//! - Shader compilation and pipeline creation
//! - Vertex buffer for cell quads
//! - Uniform buffer for viewport/cell dimensions
//! - Bind groups for texture and uniforms
//!
//! # Render Loop Integration
//!
//! ```ignore
//! // Create pipeline once at initialization
//! let pipeline = RenderPipeline::new(&device, surface_format)?;
//!
//! // In render loop:
//! let mut builder = CellVertexBuilder::with_capacity(cols, rows, cell_width, cell_height);
//! for row in 0..rows {
//!     for col in 0..cols {
//!         let cell = grid.get_cell(row, col);
//!         builder.add_background(col, row, bg_color, cell.flags().bits());
//!         builder.add_glyph(col, row, glyph_entry, fg_color, cell.flags().bits());
//!     }
//! }
//!
//! // Update GPU buffers
//! pipeline.update_uniforms(&queue, uniforms);
//! pipeline.update_vertices(&device, &queue, builder.vertices());
//!
//! // Draw
//! let mut render_pass = encoder.begin_render_pass(&descriptor);
//! pipeline.draw(&mut render_pass);
//! ```

// GPU pipeline code uses intentional casts that are safe for terminal dimensions:
// - u32 -> f32: Terminal grid coordinates are bounded (max ~4096x4096)
// - usize -> u32: Vertex counts are bounded by grid size
// - Padding fields are required by GPU uniform alignment
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::pub_underscore_fields)]

use super::types::DtermBlendMode;
use super::vertex_flags::{VertexFlags, VERTEX_TYPE_BACKGROUND};
use bytemuck::{Pod, Zeroable};
use wgpu::{self, util::DeviceExt};

/// Vertex data for a cell quad.
///
/// Each cell is rendered as a quad (2 triangles, 6 vertices).
/// This struct contains all per-vertex data.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CellVertex {
    /// Position in cell grid coordinates (fractional for sub-cell positioning)
    pub position: [f32; 2],
    /// UV coordinates in atlas texture (normalized 0-1)
    pub uv: [f32; 2],
    /// Foreground color (RGBA, 0-1)
    pub fg_color: [f32; 4],
    /// Background color (RGBA, 0-1)
    pub bg_color: [f32; 4],
    /// Flags (bold, dim, underline, etc.)
    pub flags: u32,
    /// Padding for alignment (pub(crate) for box_drawing module)
    pub(crate) _padding: [u32; 3],
}

impl CellVertex {
    /// Create a new vertex with default values.
    pub fn new(position: [f32; 2], uv: [f32; 2]) -> Self {
        Self {
            position,
            uv,
            fg_color: [1.0, 1.0, 1.0, 1.0],
            bg_color: [0.0, 0.0, 0.0, 1.0],
            flags: 0,
            _padding: [0; 3],
        }
    }

    /// Create a background vertex (no UV needed).
    pub fn background(position: [f32; 2], bg_color: [f32; 4]) -> Self {
        Self {
            position,
            uv: [0.0, 0.0],
            fg_color: [1.0, 1.0, 1.0, 1.0],
            bg_color,
            flags: VERTEX_TYPE_BACKGROUND,
            _padding: [0; 3],
        }
    }

    /// Set colors.
    pub fn with_colors(mut self, fg: [f32; 4], bg: [f32; 4]) -> Self {
        self.fg_color = fg;
        self.bg_color = bg;
        self
    }

    /// Set flags.
    pub fn with_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }
}

// =============================================================================
// Legacy GPU Shader Flag Constants
// =============================================================================
//
// DEPRECATED: These constants use the old bit layout. New code should use
// the `vertex_flags` module with its type-safe VertexFlags API.
//
// Old bit layout (scattered):
//   FLAG_BOLD = 1, FLAG_DIM = 2, FLAG_IS_BACKGROUND = 256, FLAG_IS_DECORATION = 2048
//
// New bit layout (compact 7 bits):
//   VERTEX_TYPE (bits 0-1), EFFECTS (bits 2-4), OVERLAYS (bits 5-6)
//
// These are kept for backward compatibility with external code that may
// use them (FFI consumers, DashTerm2 Metal shader). Use `VertexFlags::from_legacy()`
// to convert old flags to the new format.
// =============================================================================

/// Bold text style flag (legacy - not used in new shader).
#[allow(dead_code)]
pub const FLAG_BOLD: u32 = 1;
/// Dim text style flag (legacy - use EFFECT_DIM instead).
pub const FLAG_DIM: u32 = 2;
/// Underlined text style flag (legacy - not used in shader).
#[allow(dead_code)]
pub const FLAG_UNDERLINE: u32 = 4;
/// Blinking text style flag (legacy - use EFFECT_BLINK instead).
pub const FLAG_BLINK: u32 = 8;
/// Inverse video (legacy - use EFFECT_INVERSE instead).
pub const FLAG_INVERSE: u32 = 16;
/// Strikethrough text style flag (legacy - not used in shader).
#[allow(dead_code)]
pub const FLAG_STRIKETHROUGH: u32 = 32;
/// Cell is under the cursor flag (legacy - use OVERLAY_CURSOR instead).
pub const FLAG_IS_CURSOR: u32 = 64;
/// Cell is selected flag (legacy - use OVERLAY_SELECTION instead).
pub const FLAG_IS_SELECTION: u32 = 128;
/// Vertex is for background quad (legacy - use VERTEX_TYPE_BACKGROUND instead).
pub const FLAG_IS_BACKGROUND: u32 = 256;
/// Cell uses the default background color (legacy - not used in new shader).
#[allow(dead_code)]
pub const FLAG_DEFAULT_BG: u32 = 4096;
/// Double underline text style flag (legacy - decoration vertex type handles all underlines).
#[allow(dead_code)]
pub const FLAG_DOUBLE_UNDERLINE: u32 = 512;
/// Curly underline text style flag (legacy - decoration vertex type handles all underlines).
#[allow(dead_code)]
pub const FLAG_CURLY_UNDERLINE: u32 = 1024;
/// Vertex is for decoration quad (legacy - use VERTEX_TYPE_DECORATION instead).
pub const FLAG_IS_DECORATION: u32 = 2048;
/// Dotted underline text style flag (legacy - decoration vertex type handles all underlines).
#[allow(dead_code)]
pub const FLAG_DOTTED_UNDERLINE: u32 = 8192;
/// Dashed underline text style flag (legacy - decoration vertex type handles all underlines).
#[allow(dead_code)]
pub const FLAG_DASHED_UNDERLINE: u32 = 16384;

/// Cursor style for rendering.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum CursorStyle {
    /// Solid block cursor (fills the entire cell)
    #[default]
    Block = 0,
    /// Underline cursor (horizontal line at bottom of cell)
    Underline = 1,
    /// Bar cursor (vertical line at left edge of cell)
    Bar = 2,
}

// Implement Pod and Zeroable for CursorStyle
// SAFETY: CursorStyle is repr(u32) with valid bit patterns for all values
unsafe impl Pod for CursorStyle {}
unsafe impl Zeroable for CursorStyle {}

/// Uniform data passed to shaders.
///
/// This struct is laid out for 16-byte alignment as required by GPU uniform buffers.
/// Total size: 80 bytes (5 x 16-byte aligned blocks)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Uniforms {
    /// Viewport width in pixels
    pub viewport_width: f32,
    /// Viewport height in pixels
    pub viewport_height: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
    // -- 16 bytes --
    /// Atlas texture size in pixels
    pub atlas_size: f32,
    /// Time for animations (seconds)
    pub time: f32,
    /// Cursor X position (cell coordinates, -1 if hidden)
    pub cursor_x: i32,
    /// Cursor Y position (cell coordinates, -1 if hidden)
    pub cursor_y: i32,
    // -- 16 bytes --
    /// Cursor color (RGBA)
    pub cursor_color: [f32; 4],
    // -- 16 bytes --
    /// Selection color (RGBA) - used for highlighting selected text
    pub selection_color: [f32; 4],
    // -- 16 bytes --
    /// Cursor style (0=Block, 1=Underline, 2=Bar)
    pub cursor_style: u32,
    /// Cursor blink rate in milliseconds (0 = no blink)
    pub cursor_blink_ms: u32,
    /// Padding for alignment
    pub _padding: [u32; 2],
    // -- 16 bytes --
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            viewport_width: 800.0,
            viewport_height: 600.0,
            cell_width: 8.0,
            cell_height: 16.0,
            atlas_size: 512.0,
            time: 0.0,
            cursor_x: 0,
            cursor_y: 0,
            cursor_color: [1.0, 1.0, 1.0, 1.0],
            selection_color: [0.2, 0.4, 0.8, 0.5], // Semi-transparent blue
            cursor_style: CursorStyle::Block as u32,
            cursor_blink_ms: 530, // Standard cursor blink rate
            _padding: [0; 2],
        }
    }
}

/// Uniforms for background image blending.
///
/// This struct is 16 bytes (aligned to 16-byte boundary).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BackgroundUniforms {
    /// Whether background image sampling is enabled (0/1).
    pub enabled: u32,
    /// Blend mode (see DtermBlendMode).
    pub blend_mode: u32,
    /// Background image opacity (0.0-1.0).
    pub opacity: f32,
    /// Padding for 16-byte alignment.
    pub _padding: u32,
}

impl Default for BackgroundUniforms {
    fn default() -> Self {
        Self {
            enabled: 0,
            blend_mode: DtermBlendMode::Normal as u32,
            opacity: 1.0,
            _padding: 0,
        }
    }
}

/// Render pipeline for terminal cells.
pub struct CellPipeline {
    /// wgpu render pipeline
    pipeline: wgpu::RenderPipeline,
    /// Bind group layout
    bind_group_layout: wgpu::BindGroupLayout,
    /// Current bind group (updated when atlas changes)
    bind_group: Option<wgpu::BindGroup>,
    /// Uniform buffer
    uniform_buffer: wgpu::Buffer,
    /// Vertex buffer
    vertex_buffer: wgpu::Buffer,
    /// Vertex buffer capacity (number of vertices)
    vertex_capacity: usize,
    /// Current number of vertices to draw
    vertex_count: u32,
    /// Atlas texture
    atlas_texture: Option<wgpu::Texture>,
    /// Atlas texture view
    atlas_texture_view: Option<wgpu::TextureView>,
    /// Sampler for atlas
    sampler: wgpu::Sampler,
    /// Background image uniforms
    background_uniform_buffer: wgpu::Buffer,
    /// Default background texture (1x1)
    background_texture: wgpu::Texture,
    /// Current background texture view
    background_texture_view: wgpu::TextureView,
}

impl CellPipeline {
    /// Create a new cell rendering pipeline.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // Load shader
        let shader_source = include_str!("shader.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DTermCore Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DTermCore Bind Group Layout"),
            entries: &[
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Background texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Background uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DTermCore Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // fg_color
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // bg_color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // flags
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        };

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("DTermCore Cell Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Don't cull - we might flip for some effects
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DTermCore Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create background uniform buffer
        let background_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("DTermCore Background Uniform Buffer"),
                contents: bytemuck::cast_slice(&[BackgroundUniforms::default()]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create default background texture (1x1)
        let background_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DTermCore Background Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let background_texture_view =
            background_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create initial vertex buffer (will grow as needed)
        // Start with capacity for 80x24 terminal (1920 cells * 12 vertices each)
        let initial_capacity = 80 * 24 * 12; // 12 vertices per cell (2 quads)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DTermCore Vertex Buffer"),
            size: (initial_capacity * std::mem::size_of::<CellVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DTermCore Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            bind_group: None,
            uniform_buffer,
            vertex_buffer,
            vertex_capacity: initial_capacity,
            vertex_count: 0,
            atlas_texture: None,
            atlas_texture_view: None,
            sampler,
            background_uniform_buffer,
            background_texture,
            background_texture_view,
        }
    }

    /// Initialize the atlas texture.
    pub fn init_atlas(&mut self, device: &wgpu::Device, size: u32) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DTermCore Atlas Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // Single-channel for glyph alpha
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.atlas_texture = Some(texture);
        self.atlas_texture_view = Some(view);

        // Recreate bind group with new texture
        self.recreate_bind_group(device);
    }

    /// Upload glyph data to the atlas texture.
    pub fn upload_glyph(
        &mut self,
        queue: &wgpu::Queue,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        data: &[u8],
    ) {
        if let Some(texture) = &self.atlas_texture {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Recreate the bind group (needed when atlas texture changes).
    fn recreate_bind_group(&mut self, device: &wgpu::Device) {
        if let Some(view) = &self.atlas_texture_view {
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DTermCore Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.background_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.background_uniform_buffer.as_entire_binding(),
                    },
                ],
            }));
        }
    }

    /// Update uniforms.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &Uniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
    }

    /// Update background image uniforms.
    pub fn update_background_uniforms(&self, queue: &wgpu::Queue, uniforms: &BackgroundUniforms) {
        queue.write_buffer(
            &self.background_uniform_buffer,
            0,
            bytemuck::cast_slice(&[*uniforms]),
        );
    }

    /// Set a background image texture and blend settings.
    ///
    /// Takes ownership of the `TextureView`. The caller must not use the view
    /// after calling this function.
    pub fn set_background_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: wgpu::TextureView,
        blend_mode: DtermBlendMode,
        opacity: f32,
    ) {
        self.background_texture_view = view;
        let uniforms = BackgroundUniforms {
            enabled: 1,
            blend_mode: blend_mode as u32,
            opacity: opacity.clamp(0.0, 1.0),
            _padding: 0,
        };
        self.update_background_uniforms(queue, &uniforms);
        self.recreate_bind_group(device);
    }

    /// Clear the background image and revert to default state.
    pub fn clear_background_image(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.background_texture_view = self
            .background_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let uniforms = BackgroundUniforms {
            enabled: 0,
            blend_mode: DtermBlendMode::Normal as u32,
            opacity: 1.0,
            _padding: 0,
        };
        self.update_background_uniforms(queue, &uniforms);
        self.recreate_bind_group(device);
    }

    /// Update vertex buffer with cell data.
    ///
    /// Returns the number of vertices to draw.
    pub fn update_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[CellVertex],
    ) {
        // Grow buffer if needed
        if vertices.len() > self.vertex_capacity {
            let new_capacity = vertices.len().next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DTermCore Vertex Buffer"),
                size: (new_capacity * std::mem::size_of::<CellVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = new_capacity;
        }

        // Upload vertices
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        self.vertex_count = vertices.len() as u32;
    }

    /// Render cells to the given texture view.
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let Some(bind_group) = &self.bind_group else {
            return; // No atlas initialized yet
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DTermCore Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Don't clear - assume background was rendered
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }

    /// Get the number of vertices to draw.
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }
}

/// Helper to build cell vertices from terminal state.
///
/// Vertices use cell-grid coordinates (col, row) with 1.0 unit per cell.
/// The shader transforms these to pixel coordinates using uniform cell dimensions.
pub struct CellVertexBuilder {
    /// Background vertices (solid color quads)
    backgrounds: Vec<CellVertex>,
    /// Glyph vertices (textured quads from atlas)
    glyphs: Vec<CellVertex>,
    /// Decoration vertices (underlines, strikethrough, box drawing)
    decorations: Vec<CellVertex>,
}

impl CellVertexBuilder {
    /// Create a new builder.
    ///
    /// Pre-allocates capacity for a typical 80x24 terminal (6 vertices per cell).
    ///
    /// Note: `cell_width` and `cell_height` parameters are accepted for API
    /// compatibility but not stored - the shader handles cell sizing via uniforms.
    pub fn new(_cell_width: f32, _cell_height: f32) -> Self {
        let cell_count = 80 * 24;
        Self {
            // 6 vertices per cell for backgrounds
            backgrounds: Vec::with_capacity(cell_count * 6),
            // ~50% of cells have glyphs typically
            glyphs: Vec::with_capacity(cell_count * 3),
            // ~5% of cells have decorations typically
            decorations: Vec::with_capacity(cell_count / 2),
        }
    }

    /// Create a new builder with capacity for the given terminal dimensions.
    ///
    /// Note: `cell_width` and `cell_height` parameters are accepted for API
    /// compatibility but not stored - the shader handles cell sizing via uniforms.
    pub fn with_capacity(cols: u32, rows: u32, _cell_width: f32, _cell_height: f32) -> Self {
        let cell_count = (cols * rows) as usize;
        Self {
            backgrounds: Vec::with_capacity(cell_count * 6),
            glyphs: Vec::with_capacity(cell_count * 3),
            decorations: Vec::with_capacity(cell_count / 2),
        }
    }

    /// Clear all vertices.
    pub fn clear(&mut self) {
        self.backgrounds.clear();
        self.glyphs.clear();
        self.decorations.clear();
    }

    /// Add a background quad for a cell.
    pub fn add_background(&mut self, col: u32, row: u32, bg_color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // Two triangles for the quad
        // Convert legacy flags to new format and set vertex type to Background
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_background()
            .pack();

        // Triangle 1: top-left, bottom-left, top-right
        self.backgrounds
            .push(CellVertex::background([x, y], bg_color).with_flags(base_flags));
        self.backgrounds
            .push(CellVertex::background([x, y + 1.0], bg_color).with_flags(base_flags));
        self.backgrounds
            .push(CellVertex::background([x + 1.0, y], bg_color).with_flags(base_flags));

        // Triangle 2: top-right, bottom-left, bottom-right
        self.backgrounds
            .push(CellVertex::background([x + 1.0, y], bg_color).with_flags(base_flags));
        self.backgrounds
            .push(CellVertex::background([x, y + 1.0], bg_color).with_flags(base_flags));
        self.backgrounds
            .push(CellVertex::background([x + 1.0, y + 1.0], bg_color).with_flags(base_flags));
    }

    /// Add a glyph quad for a cell.
    pub fn add_glyph(
        &mut self,
        col: u32,
        row: u32,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        fg_color: [f32; 4],
        bg_color: [f32; 4],
        flags: u32,
    ) {
        let x = col as f32;
        let y = row as f32;

        // Convert legacy flags to new format and set vertex type to Glyph.
        let base_flags = VertexFlags::from_legacy(flags).vertex_type_glyph().pack();

        // Two triangles for the quad
        // Triangle 1: top-left, bottom-left, top-right
        self.glyphs.push(CellVertex {
            position: [x, y],
            uv: [uv_min[0], uv_min[1]],
            fg_color,
            bg_color,
            flags: base_flags,
            _padding: [0; 3],
        });
        self.glyphs.push(CellVertex {
            position: [x, y + 1.0],
            uv: [uv_min[0], uv_max[1]],
            fg_color,
            bg_color,
            flags: base_flags,
            _padding: [0; 3],
        });
        self.glyphs.push(CellVertex {
            position: [x + 1.0, y],
            uv: [uv_max[0], uv_min[1]],
            fg_color,
            bg_color,
            flags: base_flags,
            _padding: [0; 3],
        });

        // Triangle 2: top-right, bottom-left, bottom-right
        self.glyphs.push(CellVertex {
            position: [x + 1.0, y],
            uv: [uv_max[0], uv_min[1]],
            fg_color,
            bg_color,
            flags: base_flags,
            _padding: [0; 3],
        });
        self.glyphs.push(CellVertex {
            position: [x, y + 1.0],
            uv: [uv_min[0], uv_max[1]],
            fg_color,
            bg_color,
            flags: base_flags,
            _padding: [0; 3],
        });
        self.glyphs.push(CellVertex {
            position: [x + 1.0, y + 1.0],
            uv: [uv_max[0], uv_max[1]],
            fg_color,
            bg_color,
            flags: base_flags,
            _padding: [0; 3],
        });
    }

    /// Add a single underline decoration for a cell.
    ///
    /// Renders a 1px line at the bottom of the cell (approximately at baseline).
    /// The underline height is 1/16th of the cell height (minimum 1 pixel visually).
    pub fn add_single_underline(&mut self, col: u32, row: u32, color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // Underline at ~85% of cell height (near baseline)
        // Height is 1/16th of cell (approximately 1 pixel at standard sizes)
        let underline_top = 0.85;
        let underline_height = 1.0 / 16.0;
        let underline_bottom = underline_top + underline_height;

        // Convert legacy flags to new format and set vertex type to Decoration
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_decoration()
            .pack();

        // Two triangles for the underline quad
        self.decorations.push(CellVertex {
            position: [x, y + underline_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + underline_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + underline_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });

        self.decorations.push(CellVertex {
            position: [x + 1.0, y + underline_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + underline_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + underline_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
    }

    /// Add a double underline decoration for a cell.
    ///
    /// Renders two 1px lines with a small gap between them.
    pub fn add_double_underline(&mut self, col: u32, row: u32, color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // First underline at ~80% of cell height
        // Second underline at ~90% of cell height
        let line_height = 1.0 / 16.0;
        let line1_top = 0.80;
        let line1_bottom = line1_top + line_height;
        let line2_top = 0.90;
        let line2_bottom = line2_top + line_height;

        // Convert legacy flags to new format and set vertex type to Decoration
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_decoration()
            .pack();

        // First underline (2 triangles)
        self.decorations.push(CellVertex {
            position: [x, y + line1_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + line1_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + line1_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });

        self.decorations.push(CellVertex {
            position: [x + 1.0, y + line1_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + line1_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + line1_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });

        // Second underline (2 triangles)
        self.decorations.push(CellVertex {
            position: [x, y + line2_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + line2_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + line2_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });

        self.decorations.push(CellVertex {
            position: [x + 1.0, y + line2_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + line2_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + line2_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
    }

    /// Add a curly/wavy underline decoration for a cell.
    ///
    /// Renders a wavy line commonly used for spell-check highlighting.
    /// The wave is approximated using 4 segments per cell.
    pub fn add_curly_underline(&mut self, col: u32, row: u32, color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // Curly underline centered at ~87% of cell height
        // Wave amplitude is ~3% of cell height
        let center_y = 0.87;
        let amplitude = 0.03;
        let line_thickness = 1.0 / 16.0;

        // Convert legacy flags to new format and set vertex type to Decoration
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_decoration()
            .pack();

        // Create 4 segments for the wave (approximates sine wave)
        // Each segment is 0.25 cell width
        let segments = 4;
        let segment_width = 1.0 / segments as f32;

        for i in 0..segments {
            let seg_x = x + i as f32 * segment_width;
            let next_x = seg_x + segment_width;

            // Calculate wave offsets (alternating up/down)
            let offset1 = if i % 2 == 0 { -amplitude } else { amplitude };
            let offset2 = if i % 2 == 0 { amplitude } else { -amplitude };

            let y1_top = y + center_y + offset1 - line_thickness / 2.0;
            let y1_bottom = y + center_y + offset1 + line_thickness / 2.0;
            let y2_top = y + center_y + offset2 - line_thickness / 2.0;
            let y2_bottom = y + center_y + offset2 + line_thickness / 2.0;

            // Trapezoid approximation (2 triangles per segment)
            self.decorations.push(CellVertex {
                position: [seg_x, y1_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [seg_x, y1_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [next_x, y2_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });

            self.decorations.push(CellVertex {
                position: [next_x, y2_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [seg_x, y1_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [next_x, y2_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
        }
    }

    /// Add a dotted underline decoration for a cell (SGR 4:4).
    ///
    /// Renders a series of small dots beneath the text.
    pub fn add_dotted_underline(&mut self, col: u32, row: u32, color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // Dotted underline at ~85% of cell height (same as single underline)
        let underline_y = 0.85;
        let dot_size = 0.08; // Small square dots
        // Convert legacy flags to new format and set vertex type to Decoration
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_decoration()
            .pack();

        // Create 4 dots across the cell width
        for i in 0..4 {
            let dot_x = x + 0.08 + (i as f32 * 0.24);
            let dot_top = y + underline_y - dot_size / 2.0;
            let dot_bottom = y + underline_y + dot_size / 2.0;
            let dot_left = dot_x;
            let dot_right = dot_x + dot_size;

            // Two triangles for the dot quad
            self.decorations.push(CellVertex {
                position: [dot_left, dot_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dot_left, dot_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dot_right, dot_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });

            self.decorations.push(CellVertex {
                position: [dot_right, dot_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dot_left, dot_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dot_right, dot_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
        }
    }

    /// Add a dashed underline decoration for a cell (SGR 4:5).
    ///
    /// Renders two short horizontal dashes beneath the text.
    pub fn add_dashed_underline(&mut self, col: u32, row: u32, color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // Dashed underline at ~85% of cell height (same as single underline)
        let line_top = y + 0.85 - 1.0 / 32.0;
        let line_bottom = y + 0.85 + 1.0 / 32.0;
        // Convert legacy flags to new format and set vertex type to Decoration
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_decoration()
            .pack();

        // Two dashes with a gap in between
        let dash_configs = [
            (x + 0.05, x + 0.40),  // First dash: 35% of cell width
            (x + 0.60, x + 0.95),  // Second dash: 35% of cell width
        ];

        for (dash_left, dash_right) in dash_configs {
            // Two triangles for the dash quad
            self.decorations.push(CellVertex {
                position: [dash_left, line_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dash_left, line_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dash_right, line_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });

            self.decorations.push(CellVertex {
                position: [dash_right, line_top],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dash_left, line_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
            self.decorations.push(CellVertex {
                position: [dash_right, line_bottom],
                uv: [0.0, 0.0],
                fg_color: color,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                flags: base_flags,
                _padding: [0; 3],
            });
        }
    }

    /// Add a strikethrough decoration for a cell.
    ///
    /// Renders a horizontal line through the middle of the cell.
    pub fn add_strikethrough(&mut self, col: u32, row: u32, color: [f32; 4], flags: u32) {
        let x = col as f32;
        let y = row as f32;

        // Strikethrough at ~45% of cell height (middle of x-height area)
        let strike_top = 0.45;
        let strike_height = 1.0 / 16.0;
        let strike_bottom = strike_top + strike_height;

        // Convert legacy flags to new format and set vertex type to Decoration
        let base_flags = VertexFlags::from_legacy(flags)
            .vertex_type_decoration()
            .pack();

        // Two triangles for the strikethrough quad
        self.decorations.push(CellVertex {
            position: [x, y + strike_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + strike_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + strike_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });

        self.decorations.push(CellVertex {
            position: [x + 1.0, y + strike_top],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x, y + strike_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
        self.decorations.push(CellVertex {
            position: [x + 1.0, y + strike_bottom],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags: base_flags,
            _padding: [0; 3],
        });
    }

    /// Add a raw vertex directly (for box drawing, etc).
    ///
    /// This is used by box drawing character rendering which generates
    /// its own vertices with correct positions and flags.
    /// Raw vertices go to decorations since they're typically box drawing.
    pub fn add_raw_vertex(&mut self, vertex: CellVertex) {
        self.decorations.push(vertex);
    }

    /// Take ownership of the separated vertex buffers.
    ///
    /// Returns (backgrounds, glyphs, decorations).
    pub fn into_separated(self) -> (Vec<CellVertex>, Vec<CellVertex>, Vec<CellVertex>) {
        (self.backgrounds, self.glyphs, self.decorations)
    }

    /// Take ownership of all vertices combined (for backward compatibility).
    ///
    /// Returns backgrounds + glyphs + decorations concatenated.
    /// Prefer `into_separated()` for multi-pass rendering.
    #[deprecated(note = "Use into_separated() for multi-pass rendering")]
    pub fn into_vertices(self) -> Vec<CellVertex> {
        let mut combined = self.backgrounds;
        combined.extend(self.glyphs);
        combined.extend(self.decorations);
        combined
    }

    /// Get counts of each vertex type.
    pub fn vertex_counts(&self) -> (usize, usize, usize) {
        (self.backgrounds.len(), self.glyphs.len(), self.decorations.len())
    }

    /// Get total vertex count across all buffers.
    pub fn total_vertex_count(&self) -> usize {
        self.backgrounds.len() + self.glyphs.len() + self.decorations.len()
    }

    /// Get all vertices combined (backgrounds + glyphs + decorations).
    ///
    /// Returns a new Vec containing all vertices. For multi-pass rendering,
    /// prefer accessing `backgrounds()`, `glyphs()`, and `decorations()` separately.
    pub fn vertices(&self) -> Vec<CellVertex> {
        let mut combined = Vec::with_capacity(self.total_vertex_count());
        combined.extend_from_slice(&self.backgrounds);
        combined.extend_from_slice(&self.glyphs);
        combined.extend_from_slice(&self.decorations);
        combined
    }
}

#[cfg(test)]
#[allow(clippy::borrow_as_ptr, clippy::float_cmp)]
mod tests {
    use super::*;
    // Re-import new vertex flag constants for tests
    use super::super::vertex_flags::{
        EFFECT_BLINK, EFFECT_DIM, EFFECT_INVERSE, OVERLAY_CURSOR, OVERLAY_SELECTION,
        VERTEX_TYPE_BACKGROUND, VERTEX_TYPE_DECORATION, VERTEX_TYPE_GLYPH,
    };

    #[test]
    fn test_uniforms_size() {
        // Ensure uniforms struct is properly aligned for GPU
        assert_eq!(std::mem::size_of::<Uniforms>() % 16, 0);
        // Verify size is 80 bytes (5 x 16-byte blocks)
        assert_eq!(std::mem::size_of::<Uniforms>(), 80);
    }

    #[test]
    fn test_background_uniforms_size() {
        assert_eq!(std::mem::size_of::<BackgroundUniforms>() % 16, 0);
        assert_eq!(std::mem::size_of::<BackgroundUniforms>(), 16);
    }

    #[test]
    fn test_cell_vertex_size() {
        // Verify vertex size matches expected layout
        assert_eq!(std::mem::size_of::<CellVertex>(), 64);
    }

    #[test]
    fn test_vertex_builder_background() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        builder.add_background(0, 0, [0.0, 0.0, 0.0, 1.0], 0);

        // Should create 6 vertices (2 triangles)
        assert_eq!(builder.vertices().len(), 6);

        // All should have vertex type = Background (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Background,
                "Should have vertex type Background"
            );
        }
    }

    #[test]
    fn test_vertex_builder_glyph() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        builder.add_glyph(
            0,
            0,
            [0.0, 0.0],
            [0.1, 0.1],
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            0,
        );

        // Should create 6 vertices (2 triangles)
        assert_eq!(builder.vertices().len(), 6);

        // Glyph vertex type is 0, so vertex_type should be Glyph (not Background)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Glyph,
                "Should have vertex type Glyph"
            );
        }
    }

    #[test]
    fn test_vertex_builder_glyph_converts_legacy_flags() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let legacy_flags =
            super::FLAG_DIM | super::FLAG_INVERSE | super::FLAG_IS_CURSOR | super::FLAG_IS_SELECTION;
        builder.add_glyph(
            0,
            0,
            [0.0, 0.0],
            [0.1, 0.1],
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            legacy_flags,
        );

        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Glyph,
                "Should keep glyph vertex type"
            );
            assert!(v.flags & EFFECT_DIM != 0, "Should map DIM effect");
            assert!(v.flags & EFFECT_INVERSE != 0, "Should map INVERSE effect");
            assert!(v.flags & OVERLAY_CURSOR != 0, "Should map CURSOR overlay");
            assert!(
                v.flags & OVERLAY_SELECTION != 0,
                "Should map SELECTION overlay"
            );
        }
    }

    #[test]
    fn test_vertex_builder_single_underline() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let color = [1.0, 0.0, 0.0, 1.0]; // Red
        builder.add_single_underline(0, 0, color, 0);

        // Should create 6 vertices (2 triangles for the underline quad)
        assert_eq!(builder.vertices().len(), 6);

        // All should have vertex type = Decoration (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
            // Decoration color should be in fg_color
            assert_eq!(v.fg_color, color, "Decoration color should be in fg_color");
        }
    }

    #[test]
    fn test_vertex_builder_double_underline() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let color = [0.0, 1.0, 0.0, 1.0]; // Green
        builder.add_double_underline(0, 0, color, 0);

        // Should create 12 vertices (2 lines x 6 vertices each)
        assert_eq!(builder.vertices().len(), 12);

        // All should have vertex type = Decoration (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
        }
    }

    #[test]
    fn test_vertex_builder_curly_underline() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let color = [0.0, 0.0, 1.0, 1.0]; // Blue
        builder.add_curly_underline(0, 0, color, 0);

        // Should create 24 vertices (4 segments x 6 vertices each)
        assert_eq!(builder.vertices().len(), 24);

        // All should have vertex type = Decoration (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
        }
    }

    #[test]
    fn test_vertex_builder_dotted_underline() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let color = [1.0, 0.5, 0.0, 1.0]; // Orange
        builder.add_dotted_underline(0, 0, color, 0);

        // Should create 24 vertices (4 dots x 6 vertices each)
        assert_eq!(builder.vertices().len(), 24);

        // All should have vertex type = Decoration (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
        }
    }

    #[test]
    fn test_vertex_builder_dashed_underline() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let color = [0.5, 0.0, 1.0, 1.0]; // Purple
        builder.add_dashed_underline(0, 0, color, 0);

        // Should create 12 vertices (2 dashes x 6 vertices each)
        assert_eq!(builder.vertices().len(), 12);

        // All should have vertex type = Decoration (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
        }
    }

    #[test]
    fn test_vertex_builder_strikethrough() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let color = [1.0, 1.0, 0.0, 1.0]; // Yellow
        builder.add_strikethrough(0, 0, color, 0);

        // Should create 6 vertices (2 triangles for the strikethrough quad)
        assert_eq!(builder.vertices().len(), 6);

        // All should have vertex type = Decoration (new format)
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
        }
    }

    #[test]
    fn test_vertex_type_values_are_distinct() {
        // Verify vertex type values are distinct (new format)
        assert_ne!(VERTEX_TYPE_GLYPH, VERTEX_TYPE_BACKGROUND);
        assert_ne!(VERTEX_TYPE_GLYPH, VERTEX_TYPE_DECORATION);
        assert_ne!(VERTEX_TYPE_BACKGROUND, VERTEX_TYPE_DECORATION);

        // Verify effect flags are distinct
        assert_ne!(EFFECT_DIM, EFFECT_BLINK);
        assert_ne!(EFFECT_DIM, EFFECT_INVERSE);
        assert_ne!(EFFECT_BLINK, EFFECT_INVERSE);

        // Verify overlay flags are distinct
        assert_ne!(OVERLAY_CURSOR, OVERLAY_SELECTION);
    }

    #[test]
    fn test_decoration_position_bounds() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);

        // Add decorations at various positions
        builder.add_single_underline(5, 10, [1.0; 4], 0);

        // Check that vertices are within cell bounds (in cell coordinates)
        for v in builder.vertices() {
            assert!(
                v.position[0] >= 5.0 && v.position[0] <= 6.0,
                "X should be in cell 5"
            );
            assert!(
                v.position[1] >= 10.0 && v.position[1] <= 11.0,
                "Y should be in cell 10"
            );
        }
    }

    #[test]
    fn test_underline_preserves_input_flags() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        // Use new effect flags - DIM should be preserved
        let input_flags = FLAG_DIM; // Legacy FLAG_DIM = 2 maps to EFFECT_DIM = 4
        builder.add_single_underline(0, 0, [1.0; 4], input_flags);

        // Should preserve effect flags AND set vertex type to Decoration
        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            assert!(
                vf.effects.contains(super::super::vertex_flags::EffectFlags::DIM),
                "Should preserve DIM effect"
            );
            assert_eq!(
                vf.vertex_type,
                super::super::vertex_flags::VertexType::Decoration,
                "Should have vertex type Decoration"
            );
        }
    }
}
