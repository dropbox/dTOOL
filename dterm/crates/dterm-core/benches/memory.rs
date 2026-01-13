//! Memory usage benchmarks for measuring footprint characteristics.
//!
//! Run with: cargo bench --package dterm-core --bench memory
//!
//! ## Purpose
//!
//! This benchmark suite measures memory usage characteristics of the terminal.
//! It tracks allocations and memory footprint at various states.
//!
//! ## Metrics
//!
//! 1. **Empty terminal** - Baseline memory usage
//!    Target: <30 MB (match foot)
//!
//! 2. **100K scrollback** - Moderate history
//!    Target: <50 MB
//!
//! 3. **1M scrollback** - Heavy history
//!    Target: <100 MB
//!
//! 4. **Memory per 1K lines** - Efficiency ratio
//!    Target: <50 KB per 1K lines (with compression)
//!
//! ## Reference
//!
//! - foot terminal: ~30 MB empty
//! - Ghostty: Uses style deduplication for 12x savings
//! - Alacritty: Ring buffer with fixed scrollback
//!
//! ## Note
//!
//! This benchmark measures in-process memory structures. Actual RSS
//! will be higher due to allocator overhead, shared libraries, etc.
//! The values here represent the "controllable" memory footprint.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::grid::Grid;
use dterm_core::prelude::Terminal;
use dterm_core::scrollback::Scrollback;
use std::time::Duration;

/// Configure memory benchmarks.
fn memory_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(3))
        .sample_size(50)
        .warm_up_time(Duration::from_secs(1))
}

/// Baseline memory: empty terminal creation.
fn bench_empty_terminal(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/empty");
    group.sample_size(50);

    // Standard terminal sizes
    let sizes = [
        ("24x80", 24u16, 80u16),
        ("50x132", 50, 132),
        ("100x200", 100, 200),
    ];

    for (name, rows, cols) in sizes {
        group.bench_function(BenchmarkId::new("create", name), |b| {
            b.iter(|| {
                let term = Terminal::new(black_box(rows), black_box(cols));
                // Access to prevent optimization
                black_box(term.cursor())
            });
        });
    }

    group.finish();
}

/// Grid memory allocation patterns.
fn bench_grid_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/grid");
    group.sample_size(50);

    // Measure grid creation at various sizes
    let sizes = [
        ("small_24x80", 24u16, 80u16),
        ("medium_50x132", 50, 132),
        ("large_100x200", 100, 200),
        ("huge_150x300", 150, 300),
    ];

    for (name, rows, cols) in sizes {
        let cells = (rows as u64) * (cols as u64);
        group.throughput(Throughput::Elements(cells));

        group.bench_function(BenchmarkId::new("create", name), |b| {
            b.iter(|| {
                let grid = Grid::new(black_box(rows), black_box(cols));
                black_box(grid.rows())
            });
        });
    }

    group.finish();
}

/// Scrollback memory scaling.
///
/// Measures memory growth as scrollback increases.
fn bench_scrollback_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/scrollback");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(5));

    // Test scrollback sizes (in lines)
    let scrollback_sizes = [100, 1_000, 10_000, 100_000];

    for scrollback in scrollback_sizes {
        let name = format!("{}_lines", scrollback);
        group.throughput(Throughput::Elements(scrollback as u64));

        group.bench_function(BenchmarkId::new("fill", &name), |b| {
            b.iter(|| {
                let mut grid = Grid::with_scrollback(24, 80, scrollback);

                // Fill scrollback
                for i in 0..scrollback {
                    // Write a line with some content
                    for c in format!("Line {}: content here", i).chars() {
                        grid.write_char(c);
                    }
                    grid.line_feed();
                    grid.carriage_return();
                }

                // Return metrics
                (grid.scrollback_lines(), grid.ring_buffer_scrollback())
            });
        });
    }

    group.finish();
}

/// Tiered scrollback memory efficiency.
///
/// Tests the compression ratio of tiered scrollback.
fn bench_tiered_scrollback_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/tiered_scrollback");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Different compression configurations
    let configs = [
        ("small_ring", 100, 1_000, 10_000_000),
        ("medium_ring", 500, 10_000, 100_000_000),
        ("large_ring", 1_000, 100_000, 1_000_000_000),
    ];

    for (name, ring_size, hot_limit, max_bytes) in configs {
        group.bench_function(BenchmarkId::new("fill_10k", name), |b| {
            b.iter(|| {
                let scrollback = Scrollback::new(ring_size, hot_limit, max_bytes);
                let mut grid = Grid::with_tiered_scrollback(24, 80, ring_size, scrollback);

                // Fill with 10K lines of varied content
                for i in 0..10_000 {
                    // Vary content to test compression
                    if i % 10 == 0 {
                        // Colored line
                        for c in format!("\x1b[31mRed line {}\x1b[0m", i).chars() {
                            grid.write_char(c);
                        }
                    } else {
                        // Plain line
                        for c in format!("Plain content line number {}", i).chars() {
                            grid.write_char(c);
                        }
                    }
                    grid.line_feed();
                    grid.carriage_return();
                }

                (
                    grid.ring_buffer_scrollback(),
                    grid.tiered_scrollback_lines(),
                    grid.history_line_count(),
                )
            });
        });
    }

    group.finish();
}

/// Line content memory patterns.
///
/// Tests memory usage for different types of line content.
fn bench_line_content_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/line_content");
    group.sample_size(30);

    let line_count = 1000;

    // Plain ASCII lines
    group.bench_function("plain_ascii", |b| {
        b.iter(|| {
            let mut grid = Grid::with_scrollback(24, 80, line_count);
            for _ in 0..line_count {
                for c in "Plain ASCII line with some content here".chars() {
                    grid.write_char(c);
                }
                grid.line_feed();
                grid.carriage_return();
            }
            grid.scrollback_lines()
        });
    });

    // Lines with SGR attributes (colors, bold, etc.)
    group.bench_function("styled_lines", |b| {
        b.iter(|| {
            let mut term = Terminal::new(24, 80);
            for i in 0..line_count {
                term.process(
                    format!(
                        "\x1b[{}mStyled line {} with colors\x1b[0m\r\n",
                        31 + (i % 6),
                        i
                    )
                    .as_bytes(),
                );
            }
            term.cursor().row
        });
    });

    // Lines with UTF-8 content
    group.bench_function("utf8_cjk", |b| {
        b.iter(|| {
            let mut grid = Grid::with_scrollback(24, 80, line_count);
            for _ in 0..line_count {
                for c in "中文内容 日本語 한글 emoji 😀".chars() {
                    grid.write_char(c);
                }
                grid.line_feed();
                grid.carriage_return();
            }
            grid.scrollback_lines()
        });
    });

    // Lines with hyperlinks
    group.bench_function("with_hyperlinks", |b| {
        b.iter(|| {
            let mut term = Terminal::new(24, 80);
            for i in 0..line_count {
                term.process(
                    format!(
                        "\x1b]8;;https://example.com/{}\x07Link {}\x1b]8;;\x07\r\n",
                        i, i
                    )
                    .as_bytes(),
                );
            }
            term.cursor().row
        });
    });

    group.finish();
}

/// Terminal resize memory behavior.
///
/// Tests memory behavior during resize operations.
fn bench_resize_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/resize");
    group.sample_size(50);

    // Grow operation
    group.bench_function("grow_24x80_to_50x132", |b| {
        b.iter(|| {
            let mut grid = Grid::new(24, 80);
            // Fill with content
            for row in 0..24 {
                grid.set_cursor(row, 0);
                for _ in 0..80 {
                    grid.write_char('X');
                }
            }
            grid.resize(black_box(50), black_box(132));
            (grid.rows(), grid.cols())
        });
    });

    // Shrink operation
    group.bench_function("shrink_50x132_to_24x80", |b| {
        b.iter(|| {
            let mut grid = Grid::new(50, 132);
            // Fill with content
            for row in 0..50 {
                grid.set_cursor(row, 0);
                for _ in 0..132 {
                    grid.write_char('X');
                }
            }
            grid.resize(black_box(24), black_box(80));
            (grid.rows(), grid.cols())
        });
    });

    // Repeated resize (worst case)
    group.bench_function("repeated_resize_cycle", |b| {
        b.iter(|| {
            let mut grid = Grid::new(24, 80);
            for _ in 0..10 {
                grid.resize(50, 132);
                grid.resize(24, 80);
            }
            (grid.rows(), grid.cols())
        });
    });

    group.finish();
}

/// Cell and row structure sizes.
///
/// Documents the memory cost of core data structures.
fn bench_structure_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/structure_sizes");
    group.sample_size(10);

    // This benchmark mainly documents sizes; timing is less important
    group.bench_function("cell_allocation_1000", |b| {
        b.iter(|| {
            let mut grid = Grid::new(24, 80);
            // Write 1000 unique characters to allocate cells
            for i in 0u32..1000 {
                let c = char::from_u32(0x4E00 + (i % 0x5000)).unwrap_or('X');
                grid.write_char(c);
            }
            grid.cursor_col()
        });
    });

    group.bench_function("row_allocation_100", |b| {
        b.iter(|| {
            let mut grid = Grid::with_scrollback(24, 80, 100);
            for _ in 0..100 {
                for c in "Content for this row".chars() {
                    grid.write_char(c);
                }
                grid.line_feed();
                grid.carriage_return();
            }
            grid.scrollback_lines()
        });
    });

    group.finish();
}

/// Alternate screen memory.
///
/// Tests memory behavior when switching to/from alternate screen.
fn bench_alternate_screen_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/alternate_screen");
    group.sample_size(50);

    group.bench_function("switch_to_alt", |b| {
        b.iter(|| {
            let mut term = Terminal::new(24, 80);
            // Fill main screen
            term.process(b"Content on main screen\r\n");
            // Switch to alternate
            term.process(b"\x1b[?1049h");
            term.cursor().row
        });
    });

    group.bench_function("switch_back_from_alt", |b| {
        b.iter(|| {
            let mut term = Terminal::new(24, 80);
            term.process(b"Content on main screen\r\n");
            term.process(b"\x1b[?1049h");
            term.process(b"Content on alternate screen\r\n");
            term.process(b"\x1b[?1049l");
            term.cursor().row
        });
    });

    group.bench_function("repeated_switches", |b| {
        b.iter(|| {
            let mut term = Terminal::new(24, 80);
            for i in 0..10 {
                term.process(format!("Main {}\r\n", i).as_bytes());
                term.process(b"\x1b[?1049h");
                term.process(format!("Alt {}\r\n", i).as_bytes());
                term.process(b"\x1b[?1049l");
            }
            term.cursor().row
        });
    });

    group.finish();
}

/// Summary: memory efficiency at scale.
///
/// Tests that simulate real-world memory usage patterns.
fn bench_memory_efficiency_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/efficiency_summary");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Simulate 10K lines of typical terminal output
    group.bench_function("10k_lines_mixed", |b| {
        b.iter(|| {
            let mut term = Terminal::new(24, 80);
            for i in 0..10_000 {
                if i % 100 == 0 {
                    // Colored header
                    term.process(
                        format!("\x1b[1;33m=== Section {} ===\x1b[0m\r\n", i / 100).as_bytes(),
                    );
                } else if i % 10 == 0 {
                    // Git-like colored line
                    term.process(format!("\x1b[32m+\x1b[0m Added line {}\r\n", i).as_bytes());
                } else {
                    // Plain content
                    term.process(format!("  Plain content line {}\r\n", i).as_bytes());
                }
            }
            term.cursor().row
        });
    });

    // Simulate 100K lines (heavy usage)
    group.bench_function("100k_lines_plain", |b| {
        b.iter(|| {
            let mut grid = Grid::with_scrollback(24, 80, 100_000);
            for i in 0..100_000 {
                for c in format!("Line {}: typical content", i).chars() {
                    grid.write_char(c);
                }
                grid.line_feed();
                grid.carriage_return();
            }
            grid.scrollback_lines()
        });
    });

    group.finish();
}

/// Style deduplication benchmarks.
///
/// Measures the efficiency of style interning (Ghostty pattern).
/// This demonstrates memory savings potential when cells share styles.
fn bench_style_deduplication(c: &mut Criterion) {
    use dterm_core::grid::{Color, Style, StyleAttrs, StyleTable, GRID_DEFAULT_STYLE_ID};

    let mut group = c.benchmark_group("memory/style_dedup");
    group.sample_size(100);

    // Benchmark: style interning throughput
    group.bench_function("intern_unique_styles", |b| {
        b.iter(|| {
            let mut table = StyleTable::new();
            // Simulate 100 unique styles (typical real-world usage)
            for i in 0u8..100 {
                let style = Style::new(
                    Color::new(i, i.wrapping_mul(2), i.wrapping_mul(3)),
                    Color::DEFAULT_BG,
                    StyleAttrs::empty(),
                );
                black_box(table.intern(style));
            }
            table.len()
        });
    });

    // Benchmark: repeated intern (cache hit)
    group.bench_function("intern_same_style_repeated", |b| {
        let mut table = StyleTable::new();
        let style = Style::new(Color::new(255, 0, 0), Color::DEFAULT_BG, StyleAttrs::BOLD);
        b.iter(|| black_box(table.intern(style)));
    });

    // Benchmark: style lookup by ID
    group.bench_function("get_style_by_id", |b| {
        let mut table = StyleTable::new();
        let style = Style::new(Color::new(255, 0, 0), Color::DEFAULT_BG, StyleAttrs::BOLD);
        let id = table.intern(style);
        b.iter(|| black_box(table.get(id)));
    });

    // Benchmark: default style access (common case)
    group.bench_function("get_default_style", |b| {
        let table = StyleTable::new();
        b.iter(|| black_box(table.get(GRID_DEFAULT_STYLE_ID)));
    });

    // Memory savings simulation: count unique styles in typical output
    group.bench_function("typical_terminal_style_count", |b| {
        b.iter(|| {
            let mut table = StyleTable::new();
            // Simulate typical terminal output patterns
            // Most text: default style
            for _ in 0..1000 {
                table.intern(Style::DEFAULT);
            }
            // Prompts: bold + color
            for _ in 0..50 {
                table.intern(Style::new(
                    Color::new(0, 255, 0),
                    Color::DEFAULT_BG,
                    StyleAttrs::BOLD,
                ));
            }
            // Errors: red
            for _ in 0..20 {
                table.intern(Style::new(
                    Color::new(255, 0, 0),
                    Color::DEFAULT_BG,
                    StyleAttrs::empty(),
                ));
            }
            // Warnings: yellow
            for _ in 0..30 {
                table.intern(Style::new(
                    Color::new(255, 255, 0),
                    Color::DEFAULT_BG,
                    StyleAttrs::empty(),
                ));
            }
            // Links: blue underline
            for _ in 0..10 {
                table.intern(Style::new(
                    Color::new(0, 0, 255),
                    Color::DEFAULT_BG,
                    StyleAttrs::UNDERLINE,
                ));
            }
            // Headers: bold
            for _ in 0..40 {
                table.intern(Style::new(
                    Color::DEFAULT_FG,
                    Color::DEFAULT_BG,
                    StyleAttrs::BOLD,
                ));
            }
            // Result: should be ~6 unique styles, not 1150
            (table.len(), table.stats().total_refs)
        });
    });

    // Grid style API integration
    group.bench_function("grid_intern_style", |b| {
        let mut grid = Grid::new(24, 80);
        let style = Style::new(
            Color::new(255, 128, 0),
            Color::DEFAULT_BG,
            StyleAttrs::ITALIC,
        );
        b.iter(|| black_box(grid.intern_style(style)));
    });

    // Memory calculation: style table overhead
    group.bench_function("style_table_memory_1000_styles", |b| {
        b.iter(|| {
            let mut table = StyleTable::with_capacity(1000);
            for i in 0u16..1000 {
                let style = Style::new(
                    Color::new(
                        (i % 256) as u8,
                        ((i >> 4) % 256) as u8,
                        ((i >> 8) % 256) as u8,
                    ),
                    Color::DEFAULT_BG,
                    StyleAttrs::from_bits_truncate(i & 0x3FF),
                );
                table.intern(style);
            }
            (table.len(), table.memory_used())
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = memory_criterion();
    targets = bench_empty_terminal,
              bench_grid_memory,
              bench_scrollback_memory,
              bench_tiered_scrollback_memory,
              bench_line_content_memory,
              bench_resize_memory,
              bench_structure_sizes,
              bench_alternate_screen_memory,
              bench_memory_efficiency_summary,
              bench_style_deduplication,
}

criterion_main!(benches);
