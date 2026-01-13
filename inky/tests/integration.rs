#![allow(clippy::unwrap_used)]
//! Integration tests for the inky terminal UI library.
//!
//! These tests verify the full pipeline from node creation through layout,
//! rendering, and GPU conversion.

use inky::layout::LayoutEngine;
use inky::node::{BoxNode, CustomNode, Node, TextNode, Widget, WidgetContext};
use inky::render::gpu::{
    buffer_to_gpu_cells, buffer_to_gpu_cells_dirty, copy_buffer_to_gpu, copy_buffer_to_gpu_dirty,
    CpuGpuBuffer, GpuBuffer,
};
use inky::render::{render_to_buffer, Buffer, Painter};
use inky::style::{AlignItems, BorderStyle, Color, FlexDirection};

/// Test full render pipeline: Node → Layout → Buffer → GPU
#[test]
fn test_full_render_pipeline() {
    // Create a node tree
    let node: Node = BoxNode::new()
        .width(20)
        .height(5)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Hello, World!"))
        .into();

    // Build layout
    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();

    // Render to buffer
    let mut buffer = Buffer::new(80, 24);
    render_to_buffer(&node, &engine, &mut buffer);

    // Verify border was rendered
    assert_eq!(buffer.get(0, 0).unwrap().char(), '┌');
    assert_eq!(buffer.get(19, 0).unwrap().char(), '┐');
    assert_eq!(buffer.get(0, 4).unwrap().char(), '└');
    assert_eq!(buffer.get(19, 4).unwrap().char(), '┘');

    // Verify text was rendered inside border
    assert_eq!(buffer.get(1, 1).unwrap().char(), 'H');
    assert_eq!(buffer.get(2, 1).unwrap().char(), 'e');
}

#[test]
fn test_custom_widget_measurement() {
    struct FixedWidget;

    impl Widget for FixedWidget {
        fn render(&self, _ctx: &WidgetContext, _painter: &mut Painter) {}

        fn measure(&self, _available_width: u16, _available_height: u16) -> (u16, u16) {
            (6, 2)
        }
    }

    let custom = CustomNode::new(FixedWidget);
    let custom_id = custom.id;

    let node: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Start)
        .child(custom)
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(20, 5).unwrap();

    let layout = engine.get(custom_id).unwrap();
    assert_eq!(layout.width, 6);
    assert_eq!(layout.height, 2);
}

/// Test GPU conversion maintains content integrity
#[test]
fn test_gpu_conversion_integrity() {
    let mut buffer = Buffer::new(40, 10);
    buffer.write_str(0, 0, "GPU Test String", Color::White, Color::Black);
    buffer.write_str(0, 1, "Second Line", Color::Red, Color::Blue);

    // Convert to GPU cells
    let gpu_cells = buffer_to_gpu_cells(&buffer);

    assert_eq!(gpu_cells.len(), 40 * 10);
    assert_eq!(gpu_cells[0].char(), 'G');
    assert_eq!(gpu_cells[1].char(), 'P');
    assert_eq!(gpu_cells[2].char(), 'U');
    assert_eq!(gpu_cells[40].char(), 'S'); // First char of second line
}

/// Test dirty cell tracking and incremental GPU updates
#[test]
fn test_incremental_gpu_updates() {
    let mut buffer = Buffer::new(20, 10);
    let mut gpu_buffer = CpuGpuBuffer::new(20, 10);

    // Initial write and full copy
    buffer.write_str(0, 0, "Initial", Color::White, Color::Black);
    copy_buffer_to_gpu(&buffer, &mut gpu_buffer);
    buffer.clear_dirty();

    // Modify one cell
    buffer.write_str(0, 0, "M", Color::Red, Color::Black);

    // Get dirty cells - should only be 1
    let dirty = buffer_to_gpu_cells_dirty(&buffer);
    assert_eq!(dirty.len(), 1);
    assert_eq!(dirty[0].1.char(), 'M');

    // Incremental update
    copy_buffer_to_gpu_dirty(&buffer, &mut gpu_buffer);

    // Verify update
    assert_eq!(gpu_buffer.get(0, 0).unwrap().char(), 'M');
    // Rest unchanged
    assert_eq!(gpu_buffer.get(1, 0).unwrap().char(), 'n');
}

/// Test nested box rendering
#[test]
fn test_nested_boxes() {
    let inner = BoxNode::new()
        .width(10)
        .height(3)
        .border(BorderStyle::Single);

    let outer: Node = BoxNode::new()
        .width(20)
        .height(7)
        .border(BorderStyle::Double)
        .flex_direction(FlexDirection::Column)
        .padding(1)
        .child(inner)
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&outer).unwrap();
    engine.compute(80, 24).unwrap();

    let mut buffer = Buffer::new(80, 24);
    render_to_buffer(&outer, &engine, &mut buffer);

    // Outer border (double)
    assert_eq!(buffer.get(0, 0).unwrap().char(), '╔');
    assert_eq!(buffer.get(19, 0).unwrap().char(), '╗');

    // Inner border (single) - offset by padding
    assert_eq!(buffer.get(2, 2).unwrap().char(), '┌');
    assert_eq!(buffer.get(11, 2).unwrap().char(), '┐');
}

/// Test buffer resize preserves content
#[test]
fn test_buffer_resize() {
    let mut buffer = Buffer::new(10, 10);
    buffer.write_str(0, 0, "Hello", Color::White, Color::Black);

    // Resize larger
    buffer.resize(20, 20);
    assert_eq!(buffer.width(), 20);
    assert_eq!(buffer.height(), 20);
    assert_eq!(buffer.get(0, 0).unwrap().char(), 'H');
    assert_eq!(buffer.get(4, 0).unwrap().char(), 'o');

    // Resize smaller (content preserved up to new size)
    buffer.resize(3, 3);
    assert_eq!(buffer.width(), 3);
    assert_eq!(buffer.height(), 3);
    assert_eq!(buffer.get(0, 0).unwrap().char(), 'H');
    assert_eq!(buffer.get(2, 0).unwrap().char(), 'l');
}

/// Test GPU buffer resize
#[test]
fn test_gpu_buffer_resize() {
    let mut gpu_buffer = CpuGpuBuffer::new(10, 10);

    {
        let cells = gpu_buffer.map_write();
        cells[0] = inky::render::gpu::GpuCell::new('X');
        cells[11] = inky::render::gpu::GpuCell::new('Y'); // (1, 1)
    }
    gpu_buffer.unmap();

    // Resize larger
    gpu_buffer.resize(20, 20);
    assert_eq!(gpu_buffer.width(), 20);
    assert_eq!(gpu_buffer.height(), 20);
    assert_eq!(gpu_buffer.get(0, 0).unwrap().char(), 'X');

    // Resize smaller
    gpu_buffer.resize(5, 5);
    assert_eq!(gpu_buffer.width(), 5);
    assert_eq!(gpu_buffer.height(), 5);
    assert_eq!(gpu_buffer.get(0, 0).unwrap().char(), 'X');
}

/// Test text with colors renders correctly
#[test]
fn test_colored_text() {
    // Use a border and flex direction to ensure text is positioned inside
    let node: Node = BoxNode::new()
        .width(30)
        .height(3)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Colored").color(Color::Red).bold())
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();

    let mut buffer = Buffer::new(80, 24);
    render_to_buffer(&node, &engine, &mut buffer);

    // Find the text in the buffer
    let text = buffer.to_text();
    assert!(text.contains("Colored"), "Buffer should contain 'Colored'");

    // Also verify border was rendered
    assert_eq!(buffer.get(0, 0).unwrap().char(), '┌');

    // Verify text position (inside border)
    assert_eq!(buffer.get(1, 1).unwrap().char(), 'C');
}

/// Test layout computation
#[test]
fn test_layout_computation() {
    let node: Node = BoxNode::new()
        .width(100)
        .height(50)
        .flex_direction(FlexDirection::Row)
        .child(BoxNode::new().flex_grow(1.0))
        .child(BoxNode::new().flex_grow(2.0))
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(100, 50).unwrap();

    let layout = engine.get(node.id()).unwrap();
    assert_eq!(layout.width, 100);
    assert_eq!(layout.height, 50);

    // Children should have flex-proportional widths
    // First child: 1/3 of 100 ≈ 33
    // Second child: 2/3 of 100 ≈ 67
    let children: Vec<_> = node.children().iter().collect();
    let child1_layout = engine.get(children[0].id()).unwrap();
    let child2_layout = engine.get(children[1].id()).unwrap();

    // Taffy may round differently, so check approximate
    assert!(child1_layout.width >= 32 && child1_layout.width <= 34);
    assert!(child2_layout.width >= 66 && child2_layout.width <= 68);
}

/// Test buffer to text conversion
#[test]
fn test_buffer_to_text() {
    let mut buffer = Buffer::new(10, 3);
    buffer.write_str(0, 0, "Line 1", Color::White, Color::Black);
    buffer.write_str(0, 1, "Line 2", Color::White, Color::Black);
    buffer.write_str(0, 2, "Line 3", Color::White, Color::Black);

    let text = buffer.to_text();
    assert!(text.contains("Line 1"));
    assert!(text.contains("Line 2"));
    assert!(text.contains("Line 3"));
}

/// Test macros produce valid nodes
#[test]
fn test_macro_node_creation() {
    use inky::{hbox, text, vbox};

    let node: Node = vbox![
        text!("Header").bold(),
        hbox![text!("Left"), text!("Right"),],
        text!("Footer"),
    ]
    .into();

    // Should have 3 children
    let children: Vec<_> = node.children().iter().collect();
    assert_eq!(children.len(), 3);

    // Middle child (hbox) should have 2 children
    let hbox_children: Vec<_> = children[1].children().iter().collect();
    assert_eq!(hbox_children.len(), 2);
}

/// Test GPU buffer map/unmap/submit cycle
#[test]
fn test_gpu_buffer_lifecycle() {
    let mut gpu_buffer = CpuGpuBuffer::new(10, 10);

    // Map, write, unmap, submit cycle
    {
        let cells = gpu_buffer.map_write();
        cells[0] = inky::render::gpu::GpuCell::new('A');
        cells[1] = inky::render::gpu::GpuCell::new('B');
    }
    gpu_buffer.unmap();
    gpu_buffer.submit();

    // Content should be preserved
    assert_eq!(gpu_buffer.get(0, 0).unwrap().char(), 'A');
    assert_eq!(gpu_buffer.get(1, 0).unwrap().char(), 'B');

    // Multiple cycles should work
    {
        let cells = gpu_buffer.map_write();
        cells[0] = inky::render::gpu::GpuCell::new('X');
    }
    gpu_buffer.unmap();
    gpu_buffer.submit();

    assert_eq!(gpu_buffer.get(0, 0).unwrap().char(), 'X');
    assert_eq!(gpu_buffer.get(1, 0).unwrap().char(), 'B'); // Unchanged
}
