//! GPU rendering module for dterm-core.
//!
//! This module provides a cross-platform GPU renderer using wgpu (Metal on macOS,
//! Vulkan on Linux/Windows, WebGPU in browser).
//!
//! ## Design Goals
//!
//! 1. **Safe frame synchronization** - Uses Rust channels instead of platform
//!    primitives like `dispatch_group`. Cannot crash with "unbalanced" errors.
//!
//! 2. **Damage-based rendering** - Only redraws changed regions, integrated
//!    with dterm-core's damage tracking.
//!
//! 3. **Cross-platform** - Same code runs on Metal, Vulkan, and WebGPU.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────┐
//! │  Platform (Swift/ObjC via FFI)                 │
//! │  - Owns CAMetalLayer (macOS) or similar       │
//! │  - Provides drawables to renderer             │
//! └───────────────┬────────────────────────────────┘
//!                 │ FFI
//!                 ▼
//! ┌────────────────────────────────────────────────┐
//! │  Renderer                                       │
//! │  - wgpu device/queue                           │
//! │  - Glyph atlas                                 │
//! │  - Frame sync (oneshot channels)              │
//! └───────────────┬────────────────────────────────┘
//!                 │
//!                 ▼
//! ┌────────────────────────────────────────────────┐
//! │  Terminal (dterm-core)                         │
//! │  - Screen buffer                               │
//! │  - Damage tracker                              │
//! │  - Scrollback                                  │
//! └────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! // Create renderer
//! let renderer = Renderer::new(&device, &queue);
//!
//! // Request a frame
//! let frame_request = renderer.request_frame();
//!
//! // Platform provides drawable
//! frame_request.complete(surface_texture);
//!
//! // Wait for drawable (safe timeout)
//! if let Some(texture) = renderer.wait_for_frame(Duration::from_millis(16)) {
//!     renderer.render(&terminal, &texture);
//!     texture.present();
//! }
//! ```

// GPU module uses intentional casts that are safe for terminal/GPU dimensions:
// - u32 -> f32: Atlas sizes and grid coordinates are bounded
// - u8 -> f64: Color components are always 0-255
// - Too many arguments: GPU vertex data requires multiple parameters
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::too_many_arguments)]

mod atlas;
mod box_drawing;
mod frame_sync;
mod images;
mod pipeline;
mod types;
pub mod vertex_flags;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "visual-testing")]
pub mod visual_testing;

pub use atlas::{AtlasConfig, GlyphAtlas, GlyphEntry, GlyphKey};
pub use box_drawing::{generate_box_drawing_vertices, is_box_drawing};
pub use frame_sync::{FrameRequest, FrameStatus, FrameSync};
pub use images::{
    ImageEntry, ImageFormat, ImageHandle, ImagePlacement, ImageTextureCache, DEFAULT_IMAGE_BUDGET,
    MAX_IMAGE_DIMENSION,
};
pub use pipeline::{
    CellPipeline, CellVertex, CellVertexBuilder, CursorStyle, Uniforms, FLAG_BLINK, FLAG_BOLD,
    FLAG_CURLY_UNDERLINE, FLAG_DEFAULT_BG, FLAG_DIM, FLAG_DOUBLE_UNDERLINE, FLAG_INVERSE,
    FLAG_IS_BACKGROUND, FLAG_IS_CURSOR, FLAG_IS_DECORATION, FLAG_IS_SELECTION, FLAG_STRIKETHROUGH,
    FLAG_UNDERLINE,
};
pub use types::{DtermBlendMode, RenderError, RenderResult, RendererConfig};
pub use vertex_flags::{
    EffectFlags, OverlayFlags, VertexFlags, VertexType, EFFECT_BLINK, EFFECT_DIM, EFFECT_INVERSE,
    OVERLAY_CURSOR, OVERLAY_SELECTION, VERTEX_TYPE_BACKGROUND, VERTEX_TYPE_DECORATION,
    VERTEX_TYPE_GLYPH, VERTEX_TYPE_MASK,
};

use crate::grid::{CellFlags, ColorType, Damage, ExtendedStyle, StyleAttrs};
use crate::selection::TextSelection;
use crate::terminal::{CursorStyle as TerminalCursorStyle, Rgb, Terminal};
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};
use std::time::Instant;
use wgpu;

const DEFAULT_ATLAS_SIZE: u32 = 512;
const DEFAULT_CELL_WIDTH: f32 = 8.0;
const DEFAULT_CELL_HEIGHT: f32 = 16.0;
const DEFAULT_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// GPU renderer for terminal content.
///
/// This struct manages:
/// - wgpu device and queue
/// - Render pipeline for terminal cells
/// - Glyph atlas (future)
/// - Frame synchronization
pub struct Renderer {
    /// wgpu device handle
    device: Arc<wgpu::Device>,
    /// wgpu command queue
    queue: Arc<wgpu::Queue>,
    /// Frame synchronization state
    frame_sync: Mutex<FrameSync>,
    /// Cell render pipeline
    pipeline: Mutex<CellPipeline>,
    /// Glyph atlas (CPU-side cache)
    glyph_atlas: Mutex<Option<GlyphAtlas>>,
    /// Current atlas texture size (pixels)
    atlas_size: AtomicU32,
    /// Renderer start time for animations
    start_time: Instant,
    /// Configuration
    config: RendererConfig,
    /// Next frame ID
    next_frame_id: AtomicU64,
    /// Whether the background image state changed since last render
    background_dirty: AtomicBool,
}

impl Renderer {
    /// Create a new GPU renderer.
    ///
    /// # Arguments
    /// * `device` - wgpu device handle
    /// * `queue` - wgpu command queue
    /// * `config` - Renderer configuration
    ///
    /// # Returns
    /// A new `Renderer` instance.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, config: RendererConfig) -> Self {
        Self::new_with_format(device, queue, config, DEFAULT_SURFACE_FORMAT)
    }

    /// Create a new GPU renderer with an explicit surface format.
    ///
    /// This lets platform code pass the swapchain format selected by wgpu.
    pub fn new_with_format(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: RendererConfig,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let mut pipeline = CellPipeline::new(&device, surface_format);
        pipeline.init_atlas(&device, DEFAULT_ATLAS_SIZE);
        Self {
            device,
            queue,
            frame_sync: Mutex::new(FrameSync::new()),
            pipeline: Mutex::new(pipeline),
            glyph_atlas: Mutex::new(None),
            atlas_size: AtomicU32::new(DEFAULT_ATLAS_SIZE),
            start_time: Instant::now(),
            config,
            next_frame_id: AtomicU64::new(0),
            background_dirty: AtomicBool::new(false),
        }
    }

    /// Provide a glyph atlas for text rendering.
    ///
    /// The atlas can be constructed with platform font data and will be used
    /// on subsequent renders.
    pub fn set_glyph_atlas(&self, atlas: GlyphAtlas) {
        let size = atlas.size();
        {
            let mut pipeline = self.pipeline.lock();
            if self.atlas_size.load(Ordering::Relaxed) != size {
                pipeline.init_atlas(&self.device, size);
                self.atlas_size.store(size, Ordering::Relaxed);
            }
        }
        let mut slot = self.glyph_atlas.lock();
        *slot = Some(atlas);
    }

    /// Set font variants for bold, italic, and bold-italic text.
    ///
    /// Returns true if a glyph atlas exists and variants were set, false otherwise.
    pub fn set_font_variants(
        &self,
        bold: Option<fontdue::Font>,
        italic: Option<fontdue::Font>,
        bold_italic: Option<fontdue::Font>,
    ) -> bool {
        let mut slot = self.glyph_atlas.lock();
        if let Some(atlas) = slot.as_mut() {
            atlas.set_font_variants(bold, italic, bold_italic);
            true
        } else {
            false
        }
    }

    /// Set the background image and blend mode for rendering.
    ///
    /// Takes ownership of the `TextureView`. The caller must not use the view
    /// after calling this function.
    pub fn set_background_image(
        &self,
        view: wgpu::TextureView,
        blend_mode: DtermBlendMode,
        opacity: f32,
    ) {
        let mut pipeline = self.pipeline.lock();
        pipeline.set_background_image(&self.device, &self.queue, view, blend_mode, opacity);
        self.background_dirty.store(true, Ordering::Relaxed);
    }

    /// Clear the background image.
    pub fn clear_background_image(&self) {
        let mut pipeline = self.pipeline.lock();
        pipeline.clear_background_image(&self.device, &self.queue);
        self.background_dirty.store(true, Ordering::Relaxed);
    }

    /// Get cell dimensions based on current font atlas.
    ///
    /// Returns (cell_width, cell_height) in pixels.
    /// If no font is set, returns default values (8.0, 16.0).
    pub fn cell_dimensions(&self) -> (f32, f32) {
        let atlas_slot = self.glyph_atlas.lock();
        if let Some(atlas) = atlas_slot.as_ref() {
            (atlas.cell_width(), atlas.line_height())
        } else {
            (DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT)
        }
    }

    /// Request a new frame to be rendered.
    ///
    /// Returns a `FrameRequest` that the platform code uses to provide the
    /// drawable/surface texture. The request can be completed or dropped
    /// without causing crashes.
    ///
    /// # Example
    /// ```ignore
    /// let request = renderer.request_frame();
    /// // Platform gets the drawable...
    /// request.complete(surface_texture);
    /// ```
    pub fn request_frame(&self) -> FrameRequest {
        let frame_id = self
            .next_frame_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut sync = self.frame_sync.lock();
        sync.request_frame(frame_id)
    }

    /// Wait for a frame to be ready, with timeout.
    ///
    /// This method blocks until either:
    /// - The frame is ready (returns `FrameStatus::Ready`)
    /// - The timeout expires (returns `FrameStatus::Timeout`)
    /// - The request was cancelled (returns `FrameStatus::Cancelled`)
    ///
    /// **Safe**: This cannot crash with "unbalanced" errors like `dispatch_group`.
    /// If the timeout expires or the request is cancelled, the internal state
    /// is cleaned up automatically.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    /// The frame status indicating readiness.
    pub fn wait_for_frame(&self, timeout: std::time::Duration) -> FrameStatus {
        let sync = self.frame_sync.lock();
        sync.wait_for_frame(timeout)
    }

    /// Render the terminal to the provided surface texture.
    ///
    /// # Arguments
    /// * `terminal` - The terminal state to render
    /// * `surface_view` - The texture view to render to
    ///
    /// # Errors
    /// Returns `RenderError` if rendering fails.
    pub fn render(
        &self,
        terminal: &Terminal,
        surface_view: &wgpu::TextureView,
    ) -> RenderResult<()> {
        let grid = terminal.grid();
        let rows = grid.rows();
        let cols = grid.cols();
        let cursor = grid.cursor();
        let cursor_visible = terminal.cursor_visible();
        let selection = terminal.text_selection();

        let mut pipeline = self.pipeline.lock();
        let mut atlas_slot = self.glyph_atlas.lock();

        let mut cell_width = DEFAULT_CELL_WIDTH;
        let mut cell_height = DEFAULT_CELL_HEIGHT;
        let mut atlas_size = self.atlas_size.load(Ordering::Relaxed);

        if let Some(atlas) = atlas_slot.as_ref() {
            cell_width = atlas.cell_width();
            cell_height = atlas.line_height();
            let size = atlas.size();
            if size != atlas_size {
                pipeline.init_atlas(&self.device, size);
                self.atlas_size.store(size, Ordering::Relaxed);
            }
            atlas_size = size;
        }

        let cursor_color = terminal
            .cursor_color()
            .unwrap_or_else(|| terminal.default_foreground());

        // Convert terminal cursor style to GPU cursor style
        let term_cursor_style = terminal.cursor_style();
        let cursor_style = terminal_cursor_style_to_gpu(term_cursor_style);
        let blink_ms = if cursor_should_blink(term_cursor_style) {
            530
        } else {
            0
        };

        let mut uniforms = Uniforms {
            viewport_width: cell_width * f32::from(cols),
            viewport_height: cell_height * f32::from(rows),
            cell_width,
            cell_height,
            atlas_size: atlas_size as f32,
            time: self.start_time.elapsed().as_secs_f32(),
            cursor_x: if cursor_visible {
                i32::from(cursor.col)
            } else {
                -1
            },
            cursor_y: if cursor_visible {
                i32::from(cursor.row)
            } else {
                -1
            },
            cursor_color: rgb_to_f32(cursor_color),
            selection_color: [0.2, 0.4, 0.8, 0.5], // Default selection color
            cursor_style: cursor_style as u32,
            cursor_blink_ms: blink_ms,
            _padding: [0; 2],
        };

        pipeline.update_uniforms(&self.queue, &uniforms);

        let mut builder = CellVertexBuilder::new(cell_width, cell_height);

        for row in 0..rows {
            let Some(row_data) = grid.row(row) else {
                continue;
            };
            for col in 0..cols {
                let Some(&cell) = row_data.get(col) else {
                    continue;
                };

                let mut resolved = resolve_cell_style(terminal, grid, cell, row, col);

                // Check if cursor is at this position
                if cursor_visible && cursor.row == row && cursor.col == col {
                    resolved.flags |= FLAG_IS_CURSOR;
                }

                // Check if this cell is selected
                if selection.contains(i32::from(row), col) {
                    resolved.flags |= FLAG_IS_SELECTION;
                }

                builder.add_background(u32::from(col), u32::from(row), resolved.bg, resolved.flags);

                if resolved.draw_glyph && !cell.is_wide_continuation() {
                    // Check if this is a box drawing character - render with geometric primitives
                    if box_drawing::is_box_drawing(resolved.glyph) {
                        let box_verts = box_drawing::generate_box_drawing_vertices(
                            resolved.glyph,
                            u32::from(col),
                            u32::from(row),
                            resolved.fg,
                        );
                        for v in box_verts {
                            builder.add_raw_vertex(v);
                        }
                    } else if let Some(atlas) = atlas_slot.as_mut() {
                        // Normal font glyph rendering
                        let key = GlyphKey::new(
                            resolved.glyph,
                            atlas.default_font_size(),
                            resolved.is_bold,
                            resolved.is_italic,
                        );
                        if let Some(entry) = atlas.ensure(key).copied() {
                            if entry.width > 0 && entry.height > 0 {
                                let (u_min, v_min, u_max, v_max) = entry.uv_coords(atlas.size());
                                builder.add_glyph(
                                    u32::from(col),
                                    u32::from(row),
                                    [u_min, v_min],
                                    [u_max, v_max],
                                    resolved.fg,
                                    resolved.bg,
                                    resolved.flags,
                                );
                            }
                        }
                    }
                }

                // Add decorations (underline/strikethrough)
                // Decoration color defaults to fg, but can be overridden by underline_color
                let decoration_color = resolved.underline_color.unwrap_or(resolved.fg);

                match resolved.underline_style {
                    UnderlineStyle::Single => {
                        builder.add_single_underline(
                            u32::from(col),
                            u32::from(row),
                            decoration_color,
                            resolved.flags,
                        );
                    }
                    UnderlineStyle::Double => {
                        builder.add_double_underline(
                            u32::from(col),
                            u32::from(row),
                            decoration_color,
                            resolved.flags,
                        );
                    }
                    UnderlineStyle::Curly => {
                        builder.add_curly_underline(
                            u32::from(col),
                            u32::from(row),
                            decoration_color,
                            resolved.flags,
                        );
                    }
                    UnderlineStyle::Dotted => {
                        builder.add_dotted_underline(
                            u32::from(col),
                            u32::from(row),
                            decoration_color,
                            resolved.flags,
                        );
                    }
                    UnderlineStyle::Dashed => {
                        builder.add_dashed_underline(
                            u32::from(col),
                            u32::from(row),
                            decoration_color,
                            resolved.flags,
                        );
                    }
                    UnderlineStyle::None => {}
                }

                if resolved.has_strikethrough {
                    builder.add_strikethrough(
                        u32::from(col),
                        u32::from(row),
                        resolved.fg, // Strikethrough uses fg color
                        resolved.flags,
                    );
                }
            }
        }

        if let Some(atlas) = atlas_slot.as_ref() {
            let size = atlas.size();
            if size != atlas_size {
                pipeline.init_atlas(&self.device, size);
                self.atlas_size.store(size, Ordering::Relaxed);
                atlas_size = size;
                uniforms.atlas_size = atlas_size as f32;
                pipeline.update_uniforms(&self.queue, &uniforms);
            }
        }

        if let Some(atlas) = atlas_slot.as_mut() {
            if atlas.has_pending() {
                for (_, entry, bitmap) in atlas.take_pending() {
                    pipeline.upload_glyph(
                        &self.queue,
                        u32::from(entry.x),
                        u32::from(entry.y),
                        u32::from(entry.width),
                        u32::from(entry.height),
                        &bitmap,
                    );
                }
            }
        }

        pipeline.update_vertices(&self.device, &self.queue, &builder.vertices());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("DTermCore Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("DTermCore Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.background_color.0 as f64 / 255.0,
                            g: self.config.background_color.1 as f64 / 255.0,
                            b: self.config.background_color.2 as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        pipeline.render(&mut encoder, surface_view);

        self.queue.submit(std::iter::once(encoder.finish()));
        self.background_dirty.store(false, Ordering::Relaxed);

        Ok(())
    }

    /// Render the terminal with damage-based optimization.
    ///
    /// This method only renders cells that have changed since the last frame,
    /// significantly reducing GPU work when only small portions of the screen
    /// have changed.
    ///
    /// # Arguments
    /// * `terminal` - The terminal state to render
    /// * `surface_view` - The texture view to render to
    /// * `damage` - The damage region to render (None = full render)
    ///
    /// # Errors
    /// Returns `RenderError` if rendering fails.
    pub fn render_with_damage(
        &self,
        terminal: &Terminal,
        surface_view: &wgpu::TextureView,
        damage: Option<&Damage>,
    ) -> RenderResult<()> {
        let grid = terminal.grid();
        let rows = grid.rows();
        let cols = grid.cols();
        let cursor = grid.cursor();
        let cursor_visible = terminal.cursor_visible();
        let selection = terminal.text_selection();

        let background_dirty = self.background_dirty.load(Ordering::Relaxed);
        // Determine if we have damage to process
        let has_damage = background_dirty || damage.is_none_or(|d| d.has_damage());
        if !has_damage {
            // No damage - skip rendering entirely
            return Ok(());
        }

        let is_full_damage = background_dirty || damage.is_none_or(|d| d.is_full());

        let mut pipeline = self.pipeline.lock();
        let mut atlas_slot = self.glyph_atlas.lock();

        let mut cell_width = DEFAULT_CELL_WIDTH;
        let mut cell_height = DEFAULT_CELL_HEIGHT;
        let mut atlas_size = self.atlas_size.load(Ordering::Relaxed);

        if let Some(atlas) = atlas_slot.as_ref() {
            cell_width = atlas.cell_width();
            cell_height = atlas.line_height();
            let size = atlas.size();
            if size != atlas_size {
                pipeline.init_atlas(&self.device, size);
                self.atlas_size.store(size, Ordering::Relaxed);
            }
            atlas_size = size;
        }

        let cursor_color = terminal
            .cursor_color()
            .unwrap_or_else(|| terminal.default_foreground());

        // Convert terminal cursor style to GPU cursor style
        let term_cursor_style = terminal.cursor_style();
        let cursor_style = terminal_cursor_style_to_gpu(term_cursor_style);
        let blink_ms = if cursor_should_blink(term_cursor_style) {
            530
        } else {
            0
        };

        let mut uniforms = Uniforms {
            viewport_width: cell_width * f32::from(cols),
            viewport_height: cell_height * f32::from(rows),
            cell_width,
            cell_height,
            atlas_size: atlas_size as f32,
            time: self.start_time.elapsed().as_secs_f32(),
            cursor_x: if cursor_visible {
                i32::from(cursor.col)
            } else {
                -1
            },
            cursor_y: if cursor_visible {
                i32::from(cursor.row)
            } else {
                -1
            },
            cursor_color: rgb_to_f32(cursor_color),
            selection_color: [0.2, 0.4, 0.8, 0.5], // Default selection color
            cursor_style: cursor_style as u32,
            cursor_blink_ms: blink_ms,
            _padding: [0; 2],
        };

        pipeline.update_uniforms(&self.queue, &uniforms);

        let mut builder = CellVertexBuilder::new(cell_width, cell_height);

        // Use damage-based iteration when available
        if is_full_damage {
            // Full damage - render all cells
            for row in 0..rows {
                self.render_row(
                    terminal,
                    grid,
                    row,
                    0,
                    cols,
                    cursor,
                    cursor_visible,
                    selection,
                    &mut builder,
                    &mut atlas_slot,
                );
            }
        } else if let Some(damage) = damage {
            // Partial damage - use iter_bounds for efficient iteration
            for bounds in damage.iter_bounds(rows, cols) {
                let row = bounds.line;
                let left = bounds.left;
                let right = bounds.right;

                self.render_row(
                    terminal,
                    grid,
                    row,
                    left,
                    right,
                    cursor,
                    cursor_visible,
                    selection,
                    &mut builder,
                    &mut atlas_slot,
                );
            }
        }

        // Update atlas size if changed during glyph lookups
        if let Some(atlas) = atlas_slot.as_ref() {
            let size = atlas.size();
            if size != atlas_size {
                pipeline.init_atlas(&self.device, size);
                self.atlas_size.store(size, Ordering::Relaxed);
                atlas_size = size;
                uniforms.atlas_size = atlas_size as f32;
                pipeline.update_uniforms(&self.queue, &uniforms);
            }
        }

        // Upload any pending glyphs
        if let Some(atlas) = atlas_slot.as_mut() {
            if atlas.has_pending() {
                for (_, entry, bitmap) in atlas.take_pending() {
                    pipeline.upload_glyph(
                        &self.queue,
                        u32::from(entry.x),
                        u32::from(entry.y),
                        u32::from(entry.width),
                        u32::from(entry.height),
                        &bitmap,
                    );
                }
            }
        }

        pipeline.update_vertices(&self.device, &self.queue, &builder.vertices());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("DTermCore Damage Render Encoder"),
            });

        // For damage-based rendering, we still need to clear on full damage
        // For partial damage, we rely on the previous frame's content
        if is_full_damage {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("DTermCore Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.background_color.0 as f64 / 255.0,
                            g: self.config.background_color.1 as f64 / 255.0,
                            b: self.config.background_color.2 as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        pipeline.render(&mut encoder, surface_view);

        self.queue.submit(std::iter::once(encoder.finish()));
        self.background_dirty.store(false, Ordering::Relaxed);

        Ok(())
    }

    /// Render a single row (or portion of a row) to the vertex builder.
    ///
    /// This is a helper method used by both full and damage-based rendering.
    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &self,
        terminal: &Terminal,
        grid: &crate::grid::Grid,
        row: u16,
        col_start: u16,
        col_end: u16,
        cursor: crate::grid::Cursor,
        cursor_visible: bool,
        selection: &TextSelection,
        builder: &mut CellVertexBuilder,
        atlas_slot: &mut parking_lot::MutexGuard<'_, Option<GlyphAtlas>>,
    ) {
        let Some(row_data) = grid.row(row) else {
            return;
        };

        for col in col_start..col_end {
            let Some(&cell) = row_data.get(col) else {
                continue;
            };

            let mut resolved = resolve_cell_style(terminal, grid, cell, row, col);

            // Check if cursor is at this position
            if cursor_visible && cursor.row == row && cursor.col == col {
                resolved.flags |= FLAG_IS_CURSOR;
            }

            // Check if this cell is selected
            if selection.contains(i32::from(row), col) {
                resolved.flags |= FLAG_IS_SELECTION;
            }

            builder.add_background(u32::from(col), u32::from(row), resolved.bg, resolved.flags);

            if resolved.draw_glyph && !cell.is_wide_continuation() {
                // Check if this is a box drawing character - render with geometric primitives
                if box_drawing::is_box_drawing(resolved.glyph) {
                    let box_verts = box_drawing::generate_box_drawing_vertices(
                        resolved.glyph,
                        u32::from(col),
                        u32::from(row),
                        resolved.fg,
                    );
                    for v in box_verts {
                        builder.add_raw_vertex(v);
                    }
                } else if let Some(atlas) = atlas_slot.as_mut() {
                    // Normal font glyph rendering
                    let key = GlyphKey::new(
                        resolved.glyph,
                        atlas.default_font_size(),
                        resolved.is_bold,
                        resolved.is_italic,
                    );
                    if let Some(entry) = atlas.ensure(key).copied() {
                        if entry.width > 0 && entry.height > 0 {
                            let (u_min, v_min, u_max, v_max) = entry.uv_coords(atlas.size());
                            builder.add_glyph(
                                u32::from(col),
                                u32::from(row),
                                [u_min, v_min],
                                [u_max, v_max],
                                resolved.fg,
                                resolved.bg,
                                resolved.flags,
                            );
                        }
                    }
                }
            }

            // Add decorations (underline/strikethrough)
            // Decoration color defaults to fg, but can be overridden by underline_color
            let decoration_color = resolved.underline_color.unwrap_or(resolved.fg);

            match resolved.underline_style {
                UnderlineStyle::Single => {
                    builder.add_single_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                UnderlineStyle::Double => {
                    builder.add_double_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                UnderlineStyle::Curly => {
                    builder.add_curly_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                UnderlineStyle::Dotted => {
                    builder.add_dotted_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                UnderlineStyle::Dashed => {
                    builder.add_dashed_underline(
                        u32::from(col),
                        u32::from(row),
                        decoration_color,
                        resolved.flags,
                    );
                }
                UnderlineStyle::None => {}
            }

            if resolved.has_strikethrough {
                builder.add_strikethrough(
                    u32::from(col),
                    u32::from(row),
                    resolved.fg, // Strikethrough uses fg color
                    resolved.flags,
                );
            }
        }
    }

    /// Get the wgpu device handle.
    pub fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    /// Get the wgpu queue handle.
    pub fn queue(&self) -> &Arc<wgpu::Queue> {
        &self.queue
    }
}

/// Underline style for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
    /// Dotted underline (SGR 4:4) - series of small dots.
    Dotted,
    /// Dashed underline (SGR 4:5) - series of short dashes.
    Dashed,
}

pub(crate) struct ResolvedCellStyle {
    pub(crate) fg: [f32; 4],
    pub(crate) bg: [f32; 4],
    pub(crate) flags: u32,
    pub(crate) is_bold: bool,
    pub(crate) is_italic: bool,
    pub(crate) draw_glyph: bool,
    pub(crate) glyph: char,
    /// What type of underline to draw (if any)
    pub(crate) underline_style: UnderlineStyle,
    /// Whether to draw strikethrough
    pub(crate) has_strikethrough: bool,
    /// Underline color (may differ from fg color via SGR 58/59)
    pub(crate) underline_color: Option<[f32; 4]>,
}

pub(crate) fn resolve_cell_style(
    terminal: &Terminal,
    grid: &crate::grid::Grid,
    cell: crate::grid::Cell,
    row: u16,
    col: u16,
) -> ResolvedCellStyle {
    let mut attrs = StyleAttrs::empty();
    let (fg, bg, bg_is_default) = if cell.uses_style_id() {
        if let Some(ext_style) = grid.styles().get_extended(cell.style_id()) {
            attrs = ext_style.style.attrs;
            (
                resolve_style_color(terminal, &ext_style, true),
                resolve_style_color(terminal, &ext_style, false),
                ext_style.bg_type == ColorType::Default,
            )
        } else {
            (
                terminal.default_foreground(),
                terminal.default_background(),
                true,
            )
        }
    } else {
        let colors = cell.colors();
        (
            resolve_inline_color(terminal, grid, cell, row, col, true),
            resolve_inline_color(terminal, grid, cell, row, col, false),
            colors.bg_is_default(),
        )
    };

    let cell_flags = cell.flags();
    let is_bold = attrs.contains(StyleAttrs::BOLD) || cell_flags.contains(CellFlags::BOLD);
    let is_italic = attrs.contains(StyleAttrs::ITALIC) || cell_flags.contains(CellFlags::ITALIC);
    let is_dim = attrs.contains(StyleAttrs::DIM) || cell_flags.contains(CellFlags::DIM);
    let is_blink = attrs.contains(StyleAttrs::BLINK) || cell_flags.contains(CellFlags::BLINK);
    let is_inverse = attrs.contains(StyleAttrs::INVERSE) || cell_flags.contains(CellFlags::INVERSE);
    let is_hidden = attrs.contains(StyleAttrs::HIDDEN) || cell_flags.contains(CellFlags::HIDDEN);
    let has_strikethrough =
        attrs.contains(StyleAttrs::STRIKETHROUGH) || cell_flags.contains(CellFlags::STRIKETHROUGH);

    // Determine underline style.
    // For CellFlags, dotted/dashed use bit combinations, so check those first:
    // - Dotted = UNDERLINE | CURLY_UNDERLINE
    // - Dashed = DOUBLE_UNDERLINE | CURLY_UNDERLINE
    // For StyleAttrs, dotted/dashed have their own bits.
    let underline_style = if attrs.contains(StyleAttrs::DOTTED_UNDERLINE)
        || cell_flags.contains(CellFlags::DOTTED_UNDERLINE)
    {
        UnderlineStyle::Dotted
    } else if attrs.contains(StyleAttrs::DASHED_UNDERLINE)
        || cell_flags.contains(CellFlags::DASHED_UNDERLINE)
    {
        UnderlineStyle::Dashed
    } else if attrs.contains(StyleAttrs::CURLY_UNDERLINE)
        || cell_flags.contains(CellFlags::CURLY_UNDERLINE)
    {
        UnderlineStyle::Curly
    } else if attrs.contains(StyleAttrs::DOUBLE_UNDERLINE)
        || cell_flags.contains(CellFlags::DOUBLE_UNDERLINE)
    {
        UnderlineStyle::Double
    } else if attrs.contains(StyleAttrs::UNDERLINE) || cell_flags.contains(CellFlags::UNDERLINE) {
        UnderlineStyle::Single
    } else {
        UnderlineStyle::None
    };

    let is_underline = underline_style != UnderlineStyle::None;

    let mut flags = 0;
    if is_bold {
        flags |= FLAG_BOLD;
    }
    if is_dim {
        flags |= FLAG_DIM;
    }
    if is_underline {
        flags |= FLAG_UNDERLINE;
    }
    if is_blink {
        flags |= FLAG_BLINK;
    }
    if is_inverse {
        flags |= FLAG_INVERSE;
    }
    if has_strikethrough {
        flags |= FLAG_STRIKETHROUGH;
    }
    if bg_is_default {
        flags |= FLAG_DEFAULT_BG;
    }

    let glyph = cell.char();
    let draw_glyph = !is_hidden && glyph != ' ';

    let fg_color = if is_hidden { bg } else { fg };

    // Check for custom underline color (SGR 58/59 via CellExtra)
    let underline_color = if is_underline {
        grid.cell_extra(row, col)
            .and_then(|extra| extra.underline_color())
            .map(|[r, g, b]| {
                [
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    1.0,
                ]
            })
    } else {
        None
    };

    ResolvedCellStyle {
        fg: rgb_to_f32(fg_color),
        bg: rgb_to_f32(bg),
        flags,
        is_bold,
        is_italic,
        draw_glyph,
        glyph,
        underline_style,
        has_strikethrough,
        underline_color,
    }
}

fn resolve_style_color(terminal: &Terminal, style: &ExtendedStyle, fg: bool) -> Rgb {
    let (color_type, index, rgb) = if fg {
        (style.fg_type, style.fg_index, style.style.fg.to_rgb())
    } else {
        (style.bg_type, style.bg_index, style.style.bg.to_rgb())
    };

    match color_type {
        ColorType::Default => {
            if fg {
                terminal.default_foreground()
            } else {
                terminal.default_background()
            }
        }
        ColorType::Indexed => terminal.get_palette_color(index),
        ColorType::Rgb => Rgb::new(rgb.0, rgb.1, rgb.2),
    }
}

fn resolve_inline_color(
    terminal: &Terminal,
    grid: &crate::grid::Grid,
    cell: crate::grid::Cell,
    row: u16,
    col: u16,
    fg: bool,
) -> Rgb {
    let default = if fg {
        terminal.default_foreground()
    } else {
        terminal.default_background()
    };

    let colors = cell.colors();
    if fg {
        if colors.fg_is_default() {
            return default;
        }
        if colors.fg_is_indexed() {
            return terminal.get_palette_color(colors.fg_index());
        }
        if colors.fg_is_rgb() {
            if let Some(extra) = grid.cell_extra(row, col) {
                if let Some(rgb) = extra.fg_rgb() {
                    return Rgb::new(rgb[0], rgb[1], rgb[2]);
                }
            }
        }
    } else {
        if colors.bg_is_default() {
            return default;
        }
        if colors.bg_is_indexed() {
            return terminal.get_palette_color(colors.bg_index());
        }
        if colors.bg_is_rgb() {
            if let Some(extra) = grid.cell_extra(row, col) {
                if let Some(rgb) = extra.bg_rgb() {
                    return Rgb::new(rgb[0], rgb[1], rgb[2]);
                }
            }
        }
    }

    default
}

pub(crate) fn rgb_to_f32(rgb: Rgb) -> [f32; 4] {
    [
        f32::from(rgb.r) / 255.0,
        f32::from(rgb.g) / 255.0,
        f32::from(rgb.b) / 255.0,
        1.0,
    ]
}

/// Convert terminal cursor style to GPU cursor style.
pub(crate) fn terminal_cursor_style_to_gpu(style: TerminalCursorStyle) -> CursorStyle {
    match style {
        TerminalCursorStyle::BlinkingBlock | TerminalCursorStyle::SteadyBlock => CursorStyle::Block,
        TerminalCursorStyle::BlinkingUnderline | TerminalCursorStyle::SteadyUnderline => {
            CursorStyle::Underline
        }
        TerminalCursorStyle::BlinkingBar | TerminalCursorStyle::SteadyBar => CursorStyle::Bar,
    }
}

/// Check if terminal cursor style should blink.
pub(crate) fn cursor_should_blink(style: TerminalCursorStyle) -> bool {
    matches!(
        style,
        TerminalCursorStyle::BlinkingBlock
            | TerminalCursorStyle::BlinkingUnderline
            | TerminalCursorStyle::BlinkingBar
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_config_default() {
        let config = RendererConfig::default();
        assert_eq!(config.background_color, (0, 0, 0));
    }

    #[test]
    fn test_animation_time_progresses() {
        // Verify that animation time is computed correctly (not stuck at 0.0).
        // This is critical for cursor blink and other time-based animations.
        use std::time::Instant;

        let start_time = Instant::now();

        // Sleep briefly to ensure time elapses
        std::thread::sleep(std::time::Duration::from_millis(10));

        let elapsed = start_time.elapsed().as_secs_f32();

        // Time should have progressed (at least 10ms = 0.01s)
        assert!(
            elapsed >= 0.005, // Allow some margin for timing variations
            "Animation time should progress: got {} seconds",
            elapsed
        );

        // Time should be reasonable (not negative, not astronomically large)
        assert!(
            elapsed < 1.0,
            "Animation time should be reasonable: got {} seconds",
            elapsed
        );
    }

    #[test]
    fn test_cursor_style_conversion() {
        // Test block styles
        assert_eq!(
            terminal_cursor_style_to_gpu(TerminalCursorStyle::BlinkingBlock),
            CursorStyle::Block
        );
        assert_eq!(
            terminal_cursor_style_to_gpu(TerminalCursorStyle::SteadyBlock),
            CursorStyle::Block
        );

        // Test underline styles
        assert_eq!(
            terminal_cursor_style_to_gpu(TerminalCursorStyle::BlinkingUnderline),
            CursorStyle::Underline
        );
        assert_eq!(
            terminal_cursor_style_to_gpu(TerminalCursorStyle::SteadyUnderline),
            CursorStyle::Underline
        );

        // Test bar styles
        assert_eq!(
            terminal_cursor_style_to_gpu(TerminalCursorStyle::BlinkingBar),
            CursorStyle::Bar
        );
        assert_eq!(
            terminal_cursor_style_to_gpu(TerminalCursorStyle::SteadyBar),
            CursorStyle::Bar
        );
    }

    #[test]
    fn test_cursor_blink_detection() {
        // Blinking styles should return true
        assert!(cursor_should_blink(TerminalCursorStyle::BlinkingBlock));
        assert!(cursor_should_blink(TerminalCursorStyle::BlinkingUnderline));
        assert!(cursor_should_blink(TerminalCursorStyle::BlinkingBar));

        // Steady styles should return false
        assert!(!cursor_should_blink(TerminalCursorStyle::SteadyBlock));
        assert!(!cursor_should_blink(TerminalCursorStyle::SteadyUnderline));
        assert!(!cursor_should_blink(TerminalCursorStyle::SteadyBar));
    }

    #[test]
    fn test_cursor_style_gpu_values() {
        // Verify GPU cursor style values match shader expectations
        assert_eq!(CursorStyle::Block as u32, 0);
        assert_eq!(CursorStyle::Underline as u32, 1);
        assert_eq!(CursorStyle::Bar as u32, 2);
    }
}
