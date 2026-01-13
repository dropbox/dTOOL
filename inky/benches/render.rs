#![allow(clippy::unwrap_used)]
//! Performance benchmarks for inky rendering pipeline.
//!
//! Run with: `cargo bench --bench render`
//!
//! ## Benchmark Scenarios
//!
//! - **Buffer creation**: Allocate a terminal buffer
//! - **Cell write**: Write individual cells to buffer
//! - **String write**: Write strings to buffer
//! - **Buffer to GPU**: Convert CPU buffer to GPU format
//! - **GPU buffer copy**: Copy data into GPU buffer
//! - **Full render cycle**: Layout + paint + convert

use std::hint::black_box;
use std::time::Instant;

use inky::diff::{apply_changes, Change, Differ};
use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::gpu::{
    buffer_to_gpu_cells, copy_buffer_to_gpu, CpuGpuBuffer, GpuBuffer, GpuCell,
};
use inky::render::render_to_buffer;
use inky::render::{Buffer, Cell};
use inky::style::{Color, FlexDirection};

const WARM_UP_ITERS: u32 = 100;
const BENCH_ITERS: u32 = 1000;

fn bench<F: FnMut()>(name: &str, mut f: F) {
    // Warm up
    for _ in 0..WARM_UP_ITERS {
        f();
    }

    // Benchmark
    let start = Instant::now();
    for _ in 0..BENCH_ITERS {
        f();
    }
    let elapsed = start.elapsed();

    let per_iter = elapsed / BENCH_ITERS;
    let iters_per_sec = BENCH_ITERS as f64 / elapsed.as_secs_f64();

    println!(
        "{:40} {:>10.2?} / iter  ({:.0} iter/s)",
        name, per_iter, iters_per_sec
    );
}

fn main() {
    println!("=== inky Render Benchmarks ===\n");

    // Terminal sizes to test
    let sizes = [
        (80, 24, "80x24 (standard)"),
        (120, 40, "120x40 (large)"),
        (200, 50, "200x50 (xl)"),
    ];

    for (width, height, label) in sizes {
        println!("\n--- {} ({} cells) ---\n", label, width * height);

        // Buffer creation
        bench(&format!("Buffer::new({})", label), || {
            black_box(Buffer::new(width, height));
        });

        // CpuGpuBuffer creation
        bench(&format!("CpuGpuBuffer::new({})", label), || {
            black_box(CpuGpuBuffer::new(width, height));
        });

        // GpuCell creation
        bench("GpuCell::blank()", || {
            black_box(GpuCell::blank());
        });

        // Cell write
        let mut buffer = Buffer::new(width, height);
        bench("Cell write (single)", || {
            buffer.set(0, 0, Cell::new('X'));
        });

        // String write
        bench("String write (10 chars)", || {
            buffer.write_str(0, 0, "Hello Inky", Color::White, Color::Black);
        });

        // Full buffer write
        let text = "X".repeat(width as usize);
        bench(&format!("Full row write ({} chars)", width), || {
            buffer.write_str(0, 0, &text, Color::White, Color::Black);
        });

        // Buffer fill (optimized with slice access)
        bench(&format!("Buffer::fill(10x10) ({})", label), || {
            buffer.fill(0, 0, 10, 10, Cell::new('X'));
        });

        // Full buffer fill
        bench(&format!("Buffer::fill(full) ({})", label), || {
            buffer.fill(0, 0, width, height, Cell::new('X'));
        });

        // Buffer to GPU cells conversion
        let buffer = Buffer::new(width, height);
        bench("buffer_to_gpu_cells()", || {
            black_box(buffer_to_gpu_cells(&buffer));
        });

        // GPU buffer copy
        let mut gpu_buffer = CpuGpuBuffer::new(width, height);
        bench("copy_buffer_to_gpu()", || {
            copy_buffer_to_gpu(&buffer, &mut gpu_buffer);
        });

        // GPU buffer map/write/unmap cycle
        bench("GPU map/write/unmap cycle", || {
            let cells = gpu_buffer.map_write();
            cells[0] = GpuCell::new('Y');
            gpu_buffer.unmap();
        });

        // GPU buffer submit
        bench("GPU submit()", || {
            gpu_buffer.submit();
        });

        // Full GPU render cycle: create, copy, submit
        bench("Full GPU cycle (create+copy+submit)", || {
            let buffer = Buffer::new(width, height);
            let mut gpu_buffer = CpuGpuBuffer::new(width, height);
            copy_buffer_to_gpu(&buffer, &mut gpu_buffer);
            gpu_buffer.submit();
        });
    }

    println!("\n=== Memory Benchmarks ===\n");

    // Memory sizes
    println!("Cell size:      {} bytes", std::mem::size_of::<Cell>());
    println!("GpuCell size:   {} bytes", std::mem::size_of::<GpuCell>());

    let _buffer_80x24 = Buffer::new(80, 24);
    let _gpu_buffer_80x24 = CpuGpuBuffer::new(80, 24);
    println!(
        "Buffer 80x24:   {} bytes (cells only)",
        80 * 24 * std::mem::size_of::<Cell>()
    );
    println!(
        "GpuBuffer 80x24: {} bytes (cells only)",
        80 * 24 * std::mem::size_of::<GpuCell>()
    );

    let _buffer_200x50 = Buffer::new(200, 50);
    let _gpu_buffer_200x50 = CpuGpuBuffer::new(200, 50);
    println!(
        "Buffer 200x50:  {} bytes (cells only)",
        200 * 50 * std::mem::size_of::<Cell>()
    );
    println!(
        "GpuBuffer 200x50: {} bytes (cells only)",
        200 * 50 * std::mem::size_of::<GpuCell>()
    );

    println!("\n=== Throughput Benchmarks ===\n");

    // Streaming text (LLM token simulation)
    let mut buffer = Buffer::new(80, 24);
    let chars: Vec<char> = "The quick brown fox jumps over the lazy dog. "
        .chars()
        .collect();

    let start = Instant::now();
    for pos in 0..10_000 {
        let x = (pos % 80) as u16;
        let y = ((pos / 80) % 24) as u16;
        buffer.set(x, y, Cell::new(chars[pos % chars.len()]));
    }
    let elapsed = start.elapsed();
    let chars_per_sec = 10_000.0 / elapsed.as_secs_f64();
    println!(
        "Streaming char write:   {:.0} chars/s ({:.2?} for 10k chars)",
        chars_per_sec, elapsed
    );

    // Bulk write throughput
    let mut buffer = Buffer::new(200, 50);
    let text = "X".repeat(200);
    let start = Instant::now();
    for _ in 0..1_000 {
        for y in 0..50 {
            buffer.write_str(0, y, &text, Color::White, Color::Black);
        }
    }
    let elapsed = start.elapsed();
    let cells_per_sec = (200 * 50 * 1_000) as f64 / elapsed.as_secs_f64();
    println!(
        "Bulk write throughput:  {:.0} cells/s ({:.2?} for 1k full screens)",
        cells_per_sec, elapsed
    );

    // GPU conversion throughput
    let buffer = Buffer::new(200, 50);
    let start = Instant::now();
    for _ in 0..1_000 {
        black_box(buffer_to_gpu_cells(&buffer));
    }
    let elapsed = start.elapsed();
    let cells_per_sec = (200 * 50 * 1_000) as f64 / elapsed.as_secs_f64();
    println!(
        "GPU conversion:         {:.0} cells/s ({:.2?} for 1k conversions)",
        cells_per_sec, elapsed
    );

    println!("\n=== Diff Benchmarks (Double-Buffering) ===\n");

    // Benchmark new double-buffering diff (no clone)
    for (width, height, label) in sizes {
        println!("\n--- {} ({} cells) ---\n", label, width * height);

        // New: diff_and_swap (no buffer clone)
        let mut differ = Differ::with_size(width, height);
        bench(&format!("Differ::diff_and_swap ({})", label), || {
            let buffer = differ.current_buffer();
            buffer.write_str(0, 0, "Test", Color::White, Color::Black);
            black_box(differ.diff_and_swap());
        });

        // Legacy: diff with external buffer (requires copy)
        let mut differ = Differ::new();
        let buffer = Buffer::new(width, height);
        bench(&format!("Differ::diff legacy ({})", label), || {
            black_box(differ.diff(&buffer));
        });

        // Memory allocation comparison
        let buffer_size = width as usize * height as usize * std::mem::size_of::<Cell>();
        println!(
            "  Buffer clone avoided: {} bytes/frame ({:.1} KB)",
            buffer_size,
            buffer_size as f64 / 1024.0
        );
    }

    println!("\n=== Terminal Write Benchmarks (Batching) ===\n");

    // Benchmark apply_changes with batched writes
    for (width, height, label) in sizes {
        println!("\n--- {} ({} cells) ---\n", label, width * height);

        // Create test changes (full screen write)
        let mut buffer = Buffer::new(width, height);
        for y in 0..height {
            let text = "X".repeat(width as usize);
            buffer.write_str(0, y, &text, Color::White, Color::Black);
        }

        // Collect cells for the changes
        let changes: Vec<Change> = (0..height)
            .map(|y| {
                let cells: inky::diff::CellVec = (0..width)
                    .filter_map(|x| buffer.get(x, y).cloned())
                    .collect();
                Change::WriteCells { cells }
            })
            .collect();

        // Benchmark writing to a Vec<u8> (avoids syscall overhead in benchmark)
        let mut output = Vec::with_capacity(width as usize * height as usize * 30);
        bench(&format!("apply_changes batched ({})", label), || {
            output.clear();
            apply_changes(&mut output, &changes).unwrap();
            black_box(&output);
        });

        println!(
            "  Output buffer size: {} bytes ({:.1} KB)",
            output.len(),
            output.len() as f64 / 1024.0
        );
    }

    println!("\n=== Porter-Requested Benchmarks ===\n");

    // These benchmarks were requested by codex_inky porter for validation
    // before removing ratatui dependency. See docs/CODEX_PORTER_RESPONSE.md Q2.

    // Benchmark: Render 100 TextNodes
    println!("\n--- TextNode Render Scaling ---\n");
    for count in [100, 1000] {
        let nodes: Vec<Node> = (0..count)
            .map(|i| TextNode::new(format!("Message line {}", i)).into())
            .collect();
        let root: Node = BoxNode::new()
            .width(200)
            .height(50)
            .flex_direction(FlexDirection::Column)
            .children(nodes)
            .into();

        let mut engine = LayoutEngine::new();
        engine.build(&root).expect("layout build");
        engine.compute(200, 50).expect("layout compute");
        let mut buffer = Buffer::new(200, 50);

        bench(&format!("Render {} TextNodes (full frame)", count), || {
            render_to_buffer(&root, &engine, &mut buffer);
            black_box(&buffer);
        });
    }

    // Benchmark: Layout 1000 BoxNodes (nested)
    println!("\n--- Layout 1000 Nested BoxNodes ---\n");
    let nested_root: Node = {
        // Create 10 rows with 100 columns each = 1000 leaf boxes + 10 row boxes + 1 root = 1011 nodes
        let rows: Vec<Node> = (0..10)
            .map(|_| {
                let cols: Vec<Node> = (0..100)
                    .map(|_| BoxNode::new().width(2).height(5).into())
                    .collect();
                BoxNode::new()
                    .flex_direction(FlexDirection::Row)
                    .children(cols)
                    .into()
            })
            .collect();
        BoxNode::new()
            .width(200)
            .height(50)
            .flex_direction(FlexDirection::Column)
            .children(rows)
            .into()
    };

    bench("Layout build 1011 BoxNodes", || {
        let mut engine = LayoutEngine::new();
        engine.build(&nested_root).expect("layout build");
        black_box(&engine);
    });

    bench("Layout compute 1011 BoxNodes (200x50)", || {
        let mut engine = LayoutEngine::new();
        engine.build(&nested_root).expect("layout build");
        engine.compute(200, 50).expect("layout compute");
        black_box(&engine);
    });

    // Benchmark: Realistic Chat UI (what codex will use)
    println!("\n--- Realistic Chat UI (100 messages) ---\n");
    let chat_ui: Node = {
        // Header
        let header = BoxNode::new()
            .height(1)
            .child(TextNode::new("Chat with Claude"));

        // Message list with 100 messages alternating user/assistant
        let messages: Vec<Node> = (0..100)
            .map(|i| {
                let role = if i % 2 == 0 { "user" } else { "assistant" };
                let content = format!("[{}] Message number {} with some typical content that might appear in a chat...", role, i);
                BoxNode::new()
                    .flex_direction(FlexDirection::Column)
                    .child(TextNode::new(format!("{}:", role)))
                    .child(TextNode::new(content))
                    .into()
            })
            .collect();
        let message_list = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .children(messages);

        // Input area
        let input = BoxNode::new()
            .height(3)
            .child(TextNode::new("> Type your message here..."));

        // Full layout
        BoxNode::new()
            .width(120)
            .height(40)
            .flex_direction(FlexDirection::Column)
            .child(header)
            .child(message_list)
            .child(input)
            .into()
    };

    bench("Chat UI: build + compute + render (100 msgs)", || {
        let mut engine = LayoutEngine::new();
        engine.build(&chat_ui).expect("layout build");
        engine.compute(120, 40).expect("layout compute");
        let mut buffer = Buffer::new(120, 40);
        render_to_buffer(&chat_ui, &engine, &mut buffer);
        black_box(&buffer);
    });

    // Break down the components
    let mut engine = LayoutEngine::new();
    engine.build(&chat_ui).expect("layout build");

    bench("Chat UI: layout build only", || {
        let mut engine = LayoutEngine::new();
        engine.build(&chat_ui).expect("layout build");
        black_box(&engine);
    });

    engine.compute(120, 40).expect("layout compute");

    bench("Chat UI: layout compute only", || {
        let mut engine = LayoutEngine::new();
        engine.build(&chat_ui).expect("layout build");
        engine.compute(120, 40).expect("layout compute");
        black_box(&engine);
    });

    bench("Chat UI: render only (pre-computed layout)", || {
        let mut buffer = Buffer::new(120, 40);
        render_to_buffer(&chat_ui, &engine, &mut buffer);
        black_box(&buffer);
    });

    // Memory usage per 1000 nodes
    println!("\n--- Memory Usage per 1000 Nodes ---\n");
    let text_node_size = std::mem::size_of::<TextNode>();
    let box_node_size = std::mem::size_of::<BoxNode>();
    let node_size = std::mem::size_of::<Node>();
    println!("TextNode size:   {} bytes", text_node_size);
    println!("BoxNode size:    {} bytes", box_node_size);
    println!("Node enum size:  {} bytes", node_size);
    println!(
        "1000 TextNodes:  {:.1} KB",
        (text_node_size * 1000) as f64 / 1024.0
    );
    println!(
        "1000 BoxNodes:   {:.1} KB",
        (box_node_size * 1000) as f64 / 1024.0
    );
    println!(
        "1000 Node enums: {:.1} KB",
        (node_size * 1000) as f64 / 1024.0
    );

    // Taffy node overhead
    let taffy_node_size = std::mem::size_of::<taffy::NodeId>();
    let taffy_style_size = std::mem::size_of::<taffy::Style>();
    println!("\nTaffy overhead per node:");
    println!("  NodeId size:   {} bytes", taffy_node_size);
    println!("  Style size:    {} bytes", taffy_style_size);

    println!("\n=== Layout Caching Benchmarks ===\n");

    // Benchmark layout tree caching
    let root = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .child(BoxNode::new().width(100).height(10))
        .child(BoxNode::new().width(100).height(10))
        .child(BoxNode::new().width(100).height(10))
        .into();

    // Benchmark first build (must build)
    let mut engine = LayoutEngine::new();
    bench("LayoutEngine::build (first)", || {
        engine.invalidate();
        engine.build(&root).unwrap();
        black_box(&engine);
    });

    // Benchmark cached build (should skip)
    let mut engine = LayoutEngine::new();
    engine.build(&root).unwrap(); // First build
    bench("LayoutEngine::build (cached)", || {
        engine.build(&root).unwrap(); // Cached
        black_box(&engine);
    });

    // Benchmark full layout cycle
    let mut engine = LayoutEngine::new();
    bench("LayoutEngine full cycle (cached)", || {
        engine.build(&root).unwrap();
        engine.compute(200, 50).unwrap();
        black_box(&engine);
    });

    println!("\n=== Summary ===\n");
    println!("Performance targets (from ARCHITECTURE_PLAN.md):");
    println!("  - Frame (no change): <0.1ms");
    println!("  - Frame (1 cell): <0.1ms");
    println!("  - Frame (full 200x50): <4ms (Tier 2), <1ms (Tier 3)");
    println!("  - Streaming tokens: >10K chars/sec");
    println!();
}
