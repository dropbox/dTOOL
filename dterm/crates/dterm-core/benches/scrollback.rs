//! Scrollback benchmarks.
//!
//! Run with: cargo bench --package dterm-core --bench scrollback

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::scrollback::{Line, Scrollback};

/// Generate test lines of varying lengths.
#[allow(clippy::cast_possible_truncation)]
fn generate_lines(count: usize, avg_len: usize) -> Vec<Line> {
    (0..count)
        .map(|i| {
            let len = avg_len + (i % 20); // Vary length slightly
            let text: String = std::iter::repeat_with(|| {
                // SAFETY: i % 26 always produces 0-25, which fits in u8
                (b'a' + (i % 26) as u8) as char
            })
            .take(len)
            .collect();
            Line::from_str(&text)
        })
        .collect()
}

fn bench_scrollback_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrollback_push");

    // Test different hot/warm limits
    let configs = [
        ("small", 100, 1000),
        ("medium", 1000, 10_000),
        ("large", 5000, 50_000),
    ];

    for (name, hot_limit, warm_limit) in configs {
        let lines = generate_lines(10_000, 80);
        group.throughput(Throughput::Elements(lines.len() as u64));

        group.bench_with_input(BenchmarkId::new("sequential", name), &lines, |b, lines| {
            b.iter(|| {
                let mut sb = Scrollback::new(hot_limit, warm_limit, 100_000_000);
                for line in lines {
                    sb.push_line(black_box(line.clone()));
                }
                sb.line_count()
            });
        });
    }

    group.finish();
}

fn bench_scrollback_get_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrollback_get_line");

    // Pre-fill scrollback with different amounts
    let sizes = [1_000, 10_000, 100_000];

    for size in sizes {
        // Create and fill scrollback
        let mut sb = Scrollback::new(1000, 10_000, 100_000_000);
        for i in 0..size {
            sb.push_str(&format!("Line {i} with some content here"));
        }

        group.throughput(Throughput::Elements(1));

        // Benchmark getting from hot tier (recent lines)
        group.bench_with_input(BenchmarkId::new("hot_tier", size), &sb, |b, sb| {
            let idx = sb.line_count() - 1; // Most recent
            b.iter(|| sb.get_line(black_box(idx)));
        });

        // Benchmark getting from warm/cold tiers (older lines)
        if size > 1000 {
            group.bench_with_input(BenchmarkId::new("warm_tier", size), &sb, |b, sb| {
                let idx = sb.line_count() / 2; // Middle
                b.iter(|| sb.get_line(black_box(idx)));
            });

            group.bench_with_input(BenchmarkId::new("cold_tier", size), &sb, |b, sb| {
                let idx = 0; // Oldest
                b.iter(|| sb.get_line(black_box(idx)));
            });
        }

        // Benchmark reverse iteration (common for scrollback display)
        group.bench_with_input(BenchmarkId::new("get_line_rev", size), &sb, |b, sb| {
            b.iter(|| {
                for i in 0..100 {
                    black_box(sb.get_line_rev(i));
                }
            });
        });
    }

    group.finish();
}

fn bench_tier_promotion(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier_promotion");

    // Test promotion from hot to warm
    group.bench_function("hot_to_warm", |b| {
        b.iter(|| {
            // Small hot limit forces frequent promotion
            let mut sb = Scrollback::with_block_size(100, 10_000, 100_000_000, 50);
            for i in 0..1000 {
                sb.push_str(&format!("Line {i} with content"));
            }
            sb.warm_line_count()
        });
    });

    // Test promotion from warm to cold
    group.bench_function("warm_to_cold", |b| {
        b.iter(|| {
            // Small warm limit forces eviction to cold
            let mut sb = Scrollback::with_block_size(100, 500, 100_000_000, 50);
            for i in 0..2000 {
                sb.push_str(&format!("Line {i} with content"));
            }
            sb.cold_line_count()
        });
    });

    group.finish();
}

fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // Different content types
    let content_types = [
        (
            "ascii_text",
            "The quick brown fox jumps over the lazy dog. ",
        ),
        (
            "terminal_output",
            "drwxr-xr-x  5 user  staff  160 Dec 25 10:30 ",
        ),
        ("code", "fn main() { println!(\"Hello, world!\"); } "),
        (
            "escape_sequences",
            "\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m ",
        ),
    ];

    for (name, pattern) in content_types {
        group.bench_function(BenchmarkId::new("push_1000", name), |b| {
            b.iter(|| {
                let mut sb = Scrollback::with_block_size(100, 10_000, 100_000_000, 100);
                for i in 0..1000 {
                    let line = format!("{}{}", pattern, i);
                    sb.push_str(&line);
                }
                sb.memory_used()
            });
        });
    }

    group.finish();
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    group.sample_size(10); // Fewer samples for memory-heavy tests

    // Test memory usage at scale
    let sizes = [10_000, 100_000, 1_000_000];

    for size in sizes {
        group.throughput(Throughput::Elements(size));

        group.bench_with_input(BenchmarkId::new("fill", size), &size, |b, &size| {
            b.iter(|| {
                let mut sb = Scrollback::new(1000, 10_000, 500_000_000);
                for i in 0..size {
                    sb.push_str(&format!("Line {i}: Some typical terminal output"));
                }
                (sb.line_count(), sb.memory_used())
            });
        });
    }

    group.finish();
}

fn bench_iterator(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrollback_iterator");

    // Pre-fill scrollback
    let mut sb = Scrollback::new(1000, 10_000, 100_000_000);
    for i in 0..10_000 {
        sb.push_str(&format!("Line {i}"));
    }

    group.bench_function("iter_forward", |b| {
        b.iter(|| {
            let mut count = 0;
            for line in sb.iter() {
                black_box(line);
                count += 1;
            }
            count
        });
    });

    group.bench_function("iter_reverse", |b| {
        b.iter(|| {
            let mut count = 0;
            for line in sb.iter_rev() {
                black_box(line);
                count += 1;
            }
            count
        });
    });

    // Benchmark iterating only recent lines (common case)
    group.bench_function("iter_recent_100", |b| {
        b.iter(|| {
            let mut count = 0;
            for line in sb.iter_rev().take(100) {
                black_box(line);
                count += 1;
            }
            count
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scrollback_push,
    bench_scrollback_get_line,
    bench_tier_promotion,
    bench_compression_ratio,
    bench_memory_efficiency,
    bench_iterator,
);
criterion_main!(benches);
