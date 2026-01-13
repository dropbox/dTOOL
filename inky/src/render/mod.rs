//! Rendering pipeline: Node tree → Buffer → Diff → Terminal output.
//!
//! This module implements inky's high-performance rendering system. The pipeline
//! transforms a tree of UI nodes into terminal output with minimal allocations
//! and efficient dirty-cell tracking.
//!
//! # Pipeline Architecture
//!
//! ```text
//! ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │  Node    │ -> │  Layout  │ -> │  Buffer  │ -> │ Terminal │
//! │  Tree    │    │  (Taffy) │    │  (Cells) │    │  Output  │
//! └──────────┘    └──────────┘    └──────────┘    └──────────┘
//!                                       │
//!                                       v
//!                               ┌──────────────┐
//!                               │  GPU Buffer  │
//!                               │  (optional)  │
//!                               └──────────────┘
//! ```
//!
//! # Core Types
//!
//! ## Cell
//!
//! [`Cell`] represents a single terminal cell (one character position). Each cell
//! stores a grapheme, foreground/background colors, and style flags. Cells are
//! 10 bytes each for efficient memory layout.
//!
//! ```ignore
//! use inky::render::{Cell, CellFlags, PackedColor};
//!
//! let mut cell = Cell::default();
//! cell.set_char('█');
//! cell.fg = PackedColor::named(Color::Red);
//! cell.bg = PackedColor::named(Color::Black);
//! cell.flags = CellFlags::BOLD | CellFlags::UNDERLINE;
//! ```
//!
//! ## Buffer
//!
//! [`Buffer`] is a 2D grid of cells with dirty tracking. The buffer tracks which
//! cells have changed since the last render, enabling efficient incremental updates.
//!
//! ```ignore
//! use inky::render::Buffer;
//!
//! let mut buffer = Buffer::new(80, 24);
//!
//! // Write text at position
//! buffer.write_str(0, 0, "Hello");
//!
//! // Check and clear dirty state
//! let dirty_cells = buffer.dirty_cells();
//! buffer.clear_dirty();
//! ```
//!
//! ## Painter
//!
//! [`Painter`] provides higher-level drawing operations on a buffer, including
//! text rendering with styles, box drawing, and clipping.
//!
//! # GPU Integration
//!
//! The [`gpu`] submodule provides types for GPU-accelerated rendering:
//!
//! - [`gpu::GpuCell`] - 8-byte packed cell format for GPU buffers
//! - [`gpu::GpuBuffer`] trait - Interface for GPU buffer implementations
//! - [`buffer_to_gpu_cells`] - Convert CPU buffer to GPU format
//! - [`buffer_to_gpu_cells_dirty`] - Convert only dirty cells (incremental)
//!
//! ```ignore
//! use inky::render::{Buffer, buffer_to_gpu_cells, copy_buffer_to_gpu_dirty};
//! use inky::render::gpu::{GpuBuffer, CpuGpuBuffer};
//!
//! let buffer = Buffer::new(200, 50);
//! let mut gpu_buffer = CpuGpuBuffer::new(200, 50);
//!
//! // Full conversion (initial render)
//! let gpu_cells = buffer_to_gpu_cells(&buffer);
//!
//! // Incremental update (subsequent renders)
//! copy_buffer_to_gpu_dirty(&buffer, &mut gpu_buffer);
//! ```
//!
//! # Performance Characteristics
//!
//! | Operation | Time (10K cells) | Throughput |
//! |-----------|------------------|------------|
//! | Buffer creation | ~7µs | 142K/s |
//! | Single cell write | ~2ns | 414M/s |
//! | GPU conversion | ~24µs | 418M cells/s |
//! | Full GPU cycle | ~34µs | 30K frames/s |
//!
//! # Memory Layout
//!
//! | Buffer Type | 80×24 | 200×50 |
//! |-------------|-------|--------|
//! | CPU (10-byte cells) | 19.2 KB | 100 KB |
//! | GPU (8-byte cells) | 15.4 KB | 80 KB |
//!
//! # Thread Safety
//!
//! Buffers are `Send` but not `Sync`. For multi-threaded rendering, use separate
//! buffers per thread and merge results. GPU buffers implement appropriate safety
//! traits based on the underlying implementation.
//!
//! [`Cell`]: crate::render::Cell
//! [`Buffer`]: crate::render::Buffer
//! [`Painter`]: crate::render::Painter
//! [`gpu`]: crate::render::gpu
//! [`gpu::GpuCell`]: crate::render::gpu::GpuCell
//! [`gpu::GpuBuffer`]: crate::render::gpu::GpuBuffer
//! [`buffer_to_gpu_cells`]: crate::render::gpu::buffer_to_gpu_cells
//! [`buffer_to_gpu_cells_dirty`]: crate::render::gpu::buffer_to_gpu_cells_dirty

mod buffer;
mod cell;
pub mod gpu;
pub mod ipc;
mod painter;

pub use buffer::{cells_equal, rows_differ, Buffer};
pub use cell::{Cell, CellFlags, PackedColor};
pub use gpu::{
    buffer_to_gpu_cells, buffer_to_gpu_cells_dirty, copy_buffer_to_gpu, copy_buffer_to_gpu_dirty,
};
pub use ipc::{list_shared_buffers, shared_buffer_path, SharedMemoryBuffer};
pub use painter::Painter;

use crate::layout::LayoutEngine;
use crate::node::{Node, WidgetContext};

/// Render a node tree to a buffer.
///
/// Returns the cursor screen position if any TextNode had a cursor_position set.
pub fn render_to_buffer(
    node: &Node,
    engine: &LayoutEngine,
    buffer: &mut Buffer,
) -> Option<(u16, u16)> {
    let mut painter = Painter::new(buffer);
    render_node(&mut painter, node, engine, 0, 0);
    painter.cursor_screen_pos()
}

fn render_node(
    painter: &mut Painter,
    node: &Node,
    engine: &LayoutEngine,
    offset_x: u16,
    offset_y: u16,
) {
    let layout = match engine.get(node.id()) {
        Some(l) => l,
        None => return,
    };

    let abs_x = offset_x + layout.x;
    let abs_y = offset_y + layout.y;

    match node {
        Node::Root(_) | Node::Box(_) | Node::Static(_) => {
            // Paint background/border if style specifies
            painter.paint_box(node.style(), abs_x, abs_y, layout.width, layout.height);

            // Recursively paint children
            for child in node.children() {
                render_node(painter, child, engine, abs_x, abs_y);
            }
        }
        Node::Text(text_node) => {
            use crate::node::TextContent;
            let line_style = text_node.line_style.as_ref();
            let effective_style = line_style.map(|style| style.merge(&text_node.text_style));
            let text_style = effective_style.as_ref().unwrap_or(&text_node.text_style);
            match &text_node.content {
                TextContent::Plain(content) => {
                    // Use cursor-tracking paint if cursor position is set
                    if text_node.cursor_position.is_some() {
                        painter.paint_text_with_cursor(
                            content.as_str(),
                            text_style,
                            line_style,
                            abs_x,
                            abs_y,
                            layout.width,
                            layout.height,
                            text_node.cursor_position,
                        );
                    } else {
                        painter.paint_text(
                            content.as_str(),
                            text_style,
                            line_style,
                            abs_x,
                            abs_y,
                            layout.width,
                            layout.height,
                        );
                    }
                }
                TextContent::Spans(spans) => {
                    if text_node.cursor_position.is_some() {
                        painter.paint_spans_with_cursor(
                            spans,
                            text_style,
                            line_style,
                            abs_x,
                            abs_y,
                            layout.width,
                            layout.height,
                            text_node.cursor_position,
                        );
                    } else {
                        painter.paint_spans(
                            spans,
                            text_style,
                            line_style,
                            abs_x,
                            abs_y,
                            layout.width,
                            layout.height,
                        );
                    }
                }
            }
        }
        Node::Custom(custom_node) => {
            // Render the custom widget
            let ctx = WidgetContext {
                x: abs_x,
                y: abs_y,
                width: layout.width,
                height: layout.height,
            };
            custom_node.widget().render(&ctx, painter);

            // Recursively paint any children the widget may have
            for child in custom_node.widget().children() {
                render_node(painter, child, engine, abs_x, abs_y);
            }
        }
    }
}
