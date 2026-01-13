//! Comparison benchmark module coordination.
//!
//! This module coordinates benchmarks across different TUI frameworks,
//! using standard scenarios for fair comparison.

mod inky_benches;
mod scenarios;

#[cfg(feature = "compat-ratatui")]
mod ratatui_benches;

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::{render_to_buffer, Buffer};
use inky::style::FlexDirection;

/// Terminal sizes for benchmarks (width, height, label)
pub const SIZES: [(u16, u16, &str); 3] =
    [(80, 24, "80x24"), (120, 40, "120x40"), (200, 50, "200x50")];

/// Benchmark empty terminal creation (startup cost)
pub fn bench_empty_terminal(c: &mut Criterion) {
    let mut group = c.benchmark_group("empty_terminal");

    for (width, height, label) in SIZES {
        let cells = width as u64 * height as u64;
        group.throughput(Throughput::Elements(cells));

        // Inky benchmarks
        group.bench_with_input(
            BenchmarkId::new("inky", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| black_box(inky_benches::create_empty_buffer(w, h)));
            },
        );

        // Ratatui benchmarks (when feature is enabled)
        #[cfg(feature = "compat-ratatui")]
        group.bench_with_input(
            BenchmarkId::new("ratatui", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| black_box(ratatui_benches::create_empty_buffer(w, h)));
            },
        );
    }

    group.finish();
}

/// Benchmark text grid rendering (layout + render)
pub fn bench_text_grid(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_grid");

    for (rows, cols) in [(10, 10), (50, 50)] {
        let cells = (rows * cols) as u64;
        let label = format!("{}x{}", rows, cols);
        group.throughput(Throughput::Elements(cells));

        // Inky benchmarks (Taffy path)
        group.bench_with_input(
            BenchmarkId::new("inky_taffy", &label),
            &(rows, cols),
            |b, &(r, c)| {
                b.iter(|| black_box(inky_benches::render_text_grid(r, c)));
            },
        );

        // Inky benchmarks (SimpleLayout fast path)
        group.bench_with_input(
            BenchmarkId::new("inky_fast", &label),
            &(rows, cols),
            |b, &(r, c)| {
                b.iter(|| black_box(inky_benches::render_text_grid_fast(r, c)));
            },
        );

        // Ratatui benchmarks
        #[cfg(feature = "compat-ratatui")]
        group.bench_with_input(
            BenchmarkId::new("ratatui", &label),
            &(rows, cols),
            |b, &(r, c)| {
                b.iter(|| black_box(ratatui_benches::render_text_grid(r, c)));
            },
        );
    }

    group.finish();
}

/// Benchmark realistic chat UI rendering
pub fn bench_chat_ui(c: &mut Criterion) {
    let mut group = c.benchmark_group("chat_ui");

    for msg_count in [10, 100, 1000] {
        group.throughput(Throughput::Elements(msg_count as u64));

        // Inky benchmarks (Taffy path)
        group.bench_with_input(
            BenchmarkId::new("inky_taffy", format!("{}_msgs", msg_count)),
            &msg_count,
            |b, &count| {
                b.iter(|| black_box(inky_benches::render_chat_ui(count)));
            },
        );

        // Inky benchmarks (SimpleLayout fast path)
        group.bench_with_input(
            BenchmarkId::new("inky_fast", format!("{}_msgs", msg_count)),
            &msg_count,
            |b, &count| {
                b.iter(|| black_box(inky_benches::render_chat_ui_fast(count)));
            },
        );

        // Ratatui benchmarks
        #[cfg(feature = "compat-ratatui")]
        group.bench_with_input(
            BenchmarkId::new("ratatui", format!("{}_msgs", msg_count)),
            &msg_count,
            |b, &count| {
                b.iter(|| black_box(ratatui_benches::render_chat_ui(count)));
            },
        );
    }

    group.finish();
}

/// Benchmark full screen redraw (worst case)
pub fn bench_full_redraw(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_redraw");

    for (width, height, label) in SIZES {
        let cells = width as u64 * height as u64;
        group.throughput(Throughput::Elements(cells));

        // Inky benchmarks (Taffy path)
        group.bench_with_input(
            BenchmarkId::new("inky_taffy", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| black_box(inky_benches::full_redraw(w, h)));
            },
        );

        // Inky benchmarks (SimpleLayout fast path)
        group.bench_with_input(
            BenchmarkId::new("inky_fast", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| black_box(inky_benches::full_redraw_fast(w, h)));
            },
        );

        // Ratatui benchmarks
        #[cfg(feature = "compat-ratatui")]
        group.bench_with_input(
            BenchmarkId::new("ratatui", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| black_box(ratatui_benches::full_redraw(w, h)));
            },
        );
    }

    group.finish();
}

/// Benchmark incremental rendering (stable tree between frames).
///
/// This is the realistic scenario for most TUI apps where the tree structure
/// doesn't change between frames. Measures the benefit of layout caching.
pub fn bench_incremental_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_render");

    // Test with different message counts
    for msg_count in [10, 100] {
        let label = format!("{}_msgs", msg_count);

        // Build tree once outside the benchmark
        let messages = scenarios::generate_messages(msg_count);
        let header: Node = BoxNode::new()
            .height(1)
            .child(TextNode::new("Chat with Claude"))
            .into();

        let message_nodes: Vec<Node> = messages
            .iter()
            .map(|(role, content)| {
                BoxNode::new()
                    .flex_direction(FlexDirection::Column)
                    .child(TextNode::new(format!("{}:", role)))
                    .child(TextNode::new(content))
                    .into()
            })
            .collect();

        let message_list: Node = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .children(message_nodes)
            .into();

        let input: Node = BoxNode::new()
            .height(3)
            .child(TextNode::new("> Type your message here..."))
            .into();

        let root: Node = BoxNode::new()
            .width(120)
            .height(40)
            .flex_direction(FlexDirection::Column)
            .child(header)
            .child(message_list)
            .child(input)
            .into();

        // Build engine and compute layout once
        let mut engine = LayoutEngine::new();
        engine.build(&root).expect("layout build");
        engine.compute(120, 40).expect("layout compute");

        // Benchmark only the render phase (layout already cached)
        group.bench_function(BenchmarkId::new("inky_render_only", &label), |b| {
            b.iter(|| {
                let mut buffer = Buffer::new(120, 40);
                render_to_buffer(&root, &engine, &mut buffer);
                black_box(buffer)
            });
        });

        // Benchmark with layout caching (re-calling build/compute but tree unchanged)
        group.bench_function(BenchmarkId::new("inky_cached", &label), |b| {
            let mut engine = LayoutEngine::new();
            engine.build(&root).expect("layout build");
            engine.compute(120, 40).expect("layout compute");

            b.iter(|| {
                // Re-build with same tree (should hit cache)
                engine.build(&root).expect("layout build");
                engine.compute(120, 40).expect("layout compute");

                let mut buffer = Buffer::new(120, 40);
                render_to_buffer(&root, &engine, &mut buffer);
                black_box(buffer)
            });
        });
    }

    group.finish();
}
