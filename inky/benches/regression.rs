#![allow(clippy::unwrap_used)]
#![allow(missing_docs)] // criterion_group! macro generates undocumented functions
//! Criterion benchmarks for performance regression detection.
//!
//! Run with: `cargo bench --bench regression`
//!
//! ## Baseline comparison
//!
//! Save a baseline:
//! ```bash
//! cargo bench --bench regression -- --save-baseline main
//! ```
//!
//! Compare against baseline:
//! ```bash
//! cargo bench --bench regression -- --baseline main
//! ```
//!
//! ## Tracked metrics
//!
//! - Buffer operations (creation, fill, write)
//! - Layout computation (build, compute)
//! - Diff algorithm (change detection)
//! - Full render cycle

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use inky::diff::Differ;
use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::{Buffer, Cell};
use inky::style::{Color, FlexDirection};

/// Buffer sizes to benchmark (width, height, label)
const SIZES: [(u16, u16, &str); 3] = [(80, 24, "80x24"), (120, 40, "120x40"), (200, 50, "200x50")];

fn bench_buffer_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_creation");

    for (width, height, label) in SIZES {
        let cells = width as u64 * height as u64;
        group.throughput(Throughput::Elements(cells));
        group.bench_with_input(
            BenchmarkId::new("new", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| black_box(Buffer::new(w, h)));
            },
        );
    }

    group.finish();
}

fn bench_buffer_fill(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_fill");

    for (width, height, label) in SIZES {
        let cells = width as u64 * height as u64;
        group.throughput(Throughput::Elements(cells));
        let mut buffer = Buffer::new(width, height);
        group.bench_with_input(
            BenchmarkId::new("full", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| {
                    buffer.fill(0, 0, w, h, Cell::new('X'));
                });
            },
        );
    }

    group.finish();
}

fn bench_buffer_write_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_write_str");

    for (width, height, label) in SIZES {
        let text = "X".repeat(width as usize);
        let mut buffer = Buffer::new(width, height);
        group.throughput(Throughput::Bytes(width as u64));
        group.bench_with_input(BenchmarkId::new("row", label), &(), |b, &()| {
            b.iter(|| {
                buffer.write_str(0, 0, &text, Color::White, Color::Black);
            });
        });
    }

    group.finish();
}

fn bench_layout_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_build");

    // Simple tree: single box
    let simple = BoxNode::new().width(100).height(50).into();
    group.bench_function("simple_1_node", |b| {
        let mut engine = LayoutEngine::new();
        b.iter(|| {
            engine.invalidate();
            engine.build(&simple).unwrap();
        });
    });

    // Medium tree: 10 children
    let medium = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .children((0..10).map(|_| Node::from(BoxNode::new().height(5))))
        .into();
    group.bench_function("medium_11_nodes", |b| {
        let mut engine = LayoutEngine::new();
        b.iter(|| {
            engine.invalidate();
            engine.build(&medium).unwrap();
        });
    });

    // Complex tree: nested hierarchy
    let complex = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .children((0..5).map(|_| {
            Node::from(
                BoxNode::new()
                    .flex_direction(FlexDirection::Row)
                    .children((0..10).map(|_| Node::from(BoxNode::new().width(20).height(10)))),
            )
        }))
        .into();
    group.bench_function("complex_56_nodes", |b| {
        let mut engine = LayoutEngine::new();
        b.iter(|| {
            engine.invalidate();
            engine.build(&complex).unwrap();
        });
    });

    group.finish();
}

fn bench_layout_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_compute");

    let node = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .children((0..10).map(|_| Node::from(BoxNode::new().height(5))))
        .into();

    for (width, height, label) in SIZES {
        let mut engine = LayoutEngine::new();
        engine.build(&node).unwrap();
        group.bench_with_input(
            BenchmarkId::new("compute", label),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| {
                    engine.compute(w, h).unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_layout_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_cached");

    let node = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .children((0..10).map(|_| Node::from(BoxNode::new().height(5))))
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(200, 50).unwrap();

    group.bench_function("build_cached", |b| {
        b.iter(|| {
            engine.build(&node).unwrap();
        });
    });

    group.finish();
}

fn bench_diff_no_changes(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_no_changes");

    for (width, height, label) in SIZES {
        let cells = width as u64 * height as u64;
        group.throughput(Throughput::Elements(cells));

        let mut differ = Differ::with_size(width, height);
        // Initialize both buffers with same content
        {
            let buffer = differ.current_buffer();
            buffer.fill(0, 0, width, height, Cell::new('X'));
        }
        let _ = differ.diff_and_swap();
        {
            let buffer = differ.current_buffer();
            buffer.fill(0, 0, width, height, Cell::new('X'));
        }

        group.bench_with_input(BenchmarkId::new("swap", label), &(), |b, &()| {
            b.iter(|| {
                let _ = black_box(differ.diff_and_swap());
            });
        });
    }

    group.finish();
}

fn bench_diff_full_change(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_full_change");

    for (width, height, label) in SIZES {
        let cells = width as u64 * height as u64;
        group.throughput(Throughput::Elements(cells));

        let mut differ = Differ::with_size(width, height);
        let mut toggle = true;

        group.bench_with_input(BenchmarkId::new("swap", label), &(), |b, &()| {
            b.iter(|| {
                let buffer = differ.current_buffer();
                let ch = if toggle { 'X' } else { 'O' };
                buffer.fill(0, 0, width, height, Cell::new(ch));
                toggle = !toggle;
                let _ = black_box(differ.diff_and_swap());
            });
        });
    }

    group.finish();
}

fn bench_text_node_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_node");

    // Short text (fits in SmartString inline storage)
    group.bench_function("short_15_chars", |b| {
        b.iter(|| black_box(TextNode::new("Hello, world!!")));
    });

    // Long text (heap allocated)
    let long_text = "X".repeat(100);
    group.bench_function("long_100_chars", |b| {
        b.iter(|| black_box(TextNode::new(&long_text)));
    });

    group.finish();
}

fn bench_streaming_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming");
    group.throughput(Throughput::Elements(1000));

    let chars: Vec<char> = "The quick brown fox jumps over the lazy dog. "
        .chars()
        .collect();

    let mut buffer = Buffer::new(80, 24);
    let mut pos = 0usize;

    group.bench_function("1000_chars", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let x = (pos % 80) as u16;
                let y = ((pos / 80) % 24) as u16;
                buffer.set(x, y, Cell::new(chars[pos % chars.len()]));
                pos = pos.wrapping_add(1);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_creation,
    bench_buffer_fill,
    bench_buffer_write_str,
    bench_layout_build,
    bench_layout_compute,
    bench_layout_cached,
    bench_diff_no_changes,
    bench_diff_full_change,
    bench_text_node_creation,
    bench_streaming_write,
);

criterion_main!(benches);
