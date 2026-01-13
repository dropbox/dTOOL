//! RLE Compression Benchmarks
//!
//! Measures compression ratios for RLE-encoded cell attributes in scrollback.
//!
//! The target is 10-30x compression for styled terminal output:
//! - Plain text (single style): 80 cells → 1 run = 80x compression
//! - Normal prompt output (3-5 styles): 80 cells → 3-5 runs = 16-26x compression
//! - Heavy styling (every word different): 80 cells → ~40 runs = 2x compression
//!
//! Run with: cargo bench --package dterm-core --bench rle_compression

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::grid::Grid;
use dterm_core::rle::Rle;
use dterm_core::scrollback::{CellAttrs, Line};

// ============================================================================
// Test Data Generation
// ============================================================================

/// Create lines simulating plain ASCII text with default attributes.
fn generate_plain_lines(count: usize, cols: usize) -> Vec<Line> {
    (0..count)
        .map(|i| {
            // Plain text line - all default attributes
            let text: String = (0..cols)
                .map(|j| (b'a' + ((i + j) % 26) as u8) as char)
                .collect();
            Line::from_str(&text)
        })
        .collect()
}

/// Create lines simulating a typical prompt + command.
/// Structure: "[green]user@host[default]:[blue]/path[default]$ [default]command"
fn generate_prompt_lines(count: usize, cols: usize) -> Vec<Line> {
    // Color constants (using true color format: 0x01_RRGGBB)
    let green_fg = 0x01_00FF00u32;
    let blue_fg = 0x01_0000FFu32;
    let default_fg = 0xFF_FF_FF_FFu32;
    let default_bg = 0xFF_00_00_00u32;
    let bold_flags = 0x01u16;

    // Prompt structure (proportional to cols):
    // - 20% green (user@host)
    // - 5% default (:)
    // - 30% blue (path)
    // - 45% default ($ and command)
    let green_end = cols / 5;
    let colon_end = green_end + cols / 20;
    let blue_end = colon_end + (3 * cols) / 10;

    (0..count)
        .map(|i| {
            let text: String = (0..cols)
                .map(|j| (b'a' + ((i + j) % 26) as u8) as char)
                .collect();

            let mut rle: Rle<CellAttrs> = Rle::new();
            for pos in 0..cols {
                let (fg, flags) = if pos < green_end {
                    (green_fg, bold_flags)
                } else if pos < colon_end {
                    (default_fg, 0u16)
                } else if pos < blue_end {
                    (blue_fg, 0u16)
                } else {
                    // Covers both dollar_end range and rest of line
                    (default_fg, 0u16)
                };
                rle.push(CellAttrs::from_raw(fg, default_bg, flags));
            }

            Line::with_attrs(&text, rle)
        })
        .collect()
}

/// Create lines with syntax-highlighted code (many style changes).
/// Simulates: keywords, strings, comments, numbers with different colors.
fn generate_code_lines(count: usize, cols: usize) -> Vec<Line> {
    // Color palette for syntax highlighting
    let colors = [
        0x01_FF79C6u32,   // pink - keywords
        0x01_F1FA8Cu32,   // yellow - strings
        0x01_6272A4u32,   // gray - comments
        0x01_BD93F9u32,   // purple - numbers
        0x01_50FA7Bu32,   // green - types
        0x01_FFB86Cu32,   // orange - operators
        0x01_8BE9FDu32,   // cyan - functions
        0xFF_FF_FF_FFu32, // default
    ];
    let default_bg = 0xFF_00_00_00u32;
    let token_len = 3; // Average token length

    (0..count)
        .map(|i| {
            let text: String = (0..cols)
                .map(|j| (b'a' + ((i + j) % 26) as u8) as char)
                .collect();

            let mut rle: Rle<CellAttrs> = Rle::new();
            let mut color_idx = 0;
            let mut chars_in_token = 0;

            for _pos in 0..cols {
                if chars_in_token >= token_len {
                    color_idx = (color_idx + 1) % colors.len();
                    chars_in_token = 0;
                }
                chars_in_token += 1;
                rle.push(CellAttrs::from_raw(colors[color_idx], default_bg, 0));
            }

            Line::with_attrs(&text, rle)
        })
        .collect()
}

/// Create lines with per-character colors (worst case for RLE).
fn generate_rainbow_lines(count: usize, cols: usize) -> Vec<Line> {
    let default_bg = 0xFF_00_00_00u32;

    (0..count)
        .map(|i| {
            let text: String = (0..cols)
                .map(|j| (b'a' + ((i + j) % 26) as u8) as char)
                .collect();

            let mut rle: Rle<CellAttrs> = Rle::new();
            for pos in 0..cols {
                // Create unique color for each character (RGB from position)
                let r = ((pos * 3) % 256) as u32;
                let g = ((pos * 5) % 256) as u32;
                let b = ((pos * 7) % 256) as u32;
                let fg = 0x01_00_00_00 | (r << 16) | (g << 8) | b;
                rle.push(CellAttrs::from_raw(fg, default_bg, 0));
            }

            Line::with_attrs(&text, rle)
        })
        .collect()
}

/// Create lines simulating ls -la output (alternating colors for permissions, sizes, names).
fn generate_ls_output_lines(count: usize, cols: usize) -> Vec<Line> {
    // ls -la typically has:
    // - permissions (10 chars, green)
    // - user (8 chars, yellow)
    // - group (8 chars, yellow)
    // - size (8 chars, green)
    // - date (12 chars, blue)
    // - filename (rest, white)
    let perms_end = 10.min(cols);
    let user_end = (perms_end + 9).min(cols);
    let group_end = (user_end + 9).min(cols);
    let size_end = (group_end + 9).min(cols);
    let date_end = (size_end + 13).min(cols);

    let green = 0x01_00FF00u32;
    let blue = 0x01_6666FFu32;
    let yellow = 0x01_FFFF00u32;
    let white = 0xFF_FF_FF_FFu32;
    let default_bg = 0xFF_00_00_00u32;

    (0..count)
        .map(|i| {
            let text: String = (0..cols)
                .map(|j| (b'a' + ((i + j) % 26) as u8) as char)
                .collect();

            let mut rle: Rle<CellAttrs> = Rle::new();
            for pos in 0..cols {
                let fg = if pos < perms_end {
                    green
                } else if pos < user_end || pos < group_end {
                    yellow
                } else if pos < size_end {
                    green
                } else if pos < date_end {
                    blue
                } else {
                    white
                };
                rle.push(CellAttrs::from_raw(fg, default_bg, 0));
            }

            Line::with_attrs(&text, rle)
        })
        .collect()
}

/// Create lines with bold/italic text formatting (flags variation).
fn generate_formatted_lines(count: usize, cols: usize) -> Vec<Line> {
    let default_fg = 0xFF_FF_FF_FFu32;
    let default_bg = 0xFF_00_00_00u32;

    // Alternate between different formatting styles
    // Bold = 0x01, Italic = 0x04, Underline = 0x08, Bold+Italic = 0x05
    let flag_patterns: [u16; 5] = [0x00, 0x01, 0x04, 0x08, 0x05];

    (0..count)
        .map(|i| {
            let text: String = (0..cols)
                .map(|j| (b'a' + ((i + j) % 26) as u8) as char)
                .collect();

            let segment_len = cols / flag_patterns.len();
            let mut rle: Rle<CellAttrs> = Rle::new();

            for pos in 0..cols {
                let flag_idx = pos / segment_len.max(1);
                let flags = flag_patterns.get(flag_idx).copied().unwrap_or(0);
                rle.push(CellAttrs::from_raw(default_fg, default_bg, flags));
            }

            Line::with_attrs(&text, rle)
        })
        .collect()
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate uncompressed size (bytes) for storing attrs per cell.
fn uncompressed_size(num_chars: usize) -> usize {
    // Each cell would store: fg (4) + bg (4) + flags (2) = 10 bytes
    num_chars * 10
}

/// Calculate RLE compressed size for a line.
fn rle_size(line: &Line) -> usize {
    // RLE stores: run_count runs × (value:10 + length:4) = 14 bytes per run
    // Plus overhead: Option discriminant, Vec header
    const RUN_SIZE: usize = 14;
    const OVERHEAD: usize = 24; // Option + Vec

    let run_count = line.attr_run_count();
    if run_count == 0 {
        // No attrs stored (all default)
        0
    } else {
        OVERHEAD + run_count * RUN_SIZE
    }
}

/// Calculate compression ratio.
fn compression_ratio(uncompressed: usize, compressed: usize) -> f64 {
    if compressed == 0 {
        // All default attrs - effectively infinite compression
        // Return a large but sensible number
        f64::MAX.min(uncompressed as f64)
    } else {
        uncompressed as f64 / compressed as f64
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_compression_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_compression_ratio");

    let cols = 80; // Standard terminal width

    // Test different content types
    let test_cases: Vec<(&str, Vec<Line>)> = vec![
        ("plain_text", generate_plain_lines(1, cols)),
        ("prompt", generate_prompt_lines(1, cols)),
        ("code_syntax", generate_code_lines(1, cols)),
        ("rainbow_worst", generate_rainbow_lines(1, cols)),
        ("ls_output", generate_ls_output_lines(1, cols)),
        ("formatted", generate_formatted_lines(1, cols)),
    ];

    // Report compression ratios and benchmark conversion
    println!("\n=== RLE Compression Ratios (80 columns) ===");
    for (name, lines) in &test_cases {
        let line = &lines[0];
        let char_count = line.as_bytes().len();
        let uncompressed = uncompressed_size(char_count);
        let compressed = rle_size(line);
        let ratio = compression_ratio(uncompressed, compressed);
        let run_count = line.attr_run_count();

        println!(
            "{:15} | chars: {:3} | runs: {:3} | uncompressed: {:4}B | compressed: {:4}B | ratio: {:6.1}x",
            name, char_count, run_count, uncompressed, compressed, ratio
        );
    }
    println!();

    // Benchmark line creation with attributes
    for (name, _) in &test_cases {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("create", name),
            &cols,
            |b, &cols| match *name {
                "plain_text" => b.iter(|| generate_plain_lines(1, black_box(cols))),
                "prompt" => b.iter(|| generate_prompt_lines(1, black_box(cols))),
                "code_syntax" => b.iter(|| generate_code_lines(1, black_box(cols))),
                "rainbow_worst" => b.iter(|| generate_rainbow_lines(1, black_box(cols))),
                "ls_output" => b.iter(|| generate_ls_output_lines(1, black_box(cols))),
                "formatted" => b.iter(|| generate_formatted_lines(1, black_box(cols))),
                _ => {}
            },
        );
    }

    group.finish();
}

fn bench_line_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_serialization");

    let cols = 80;
    let test_cases: Vec<(&str, Vec<Line>)> = vec![
        ("plain_text", generate_plain_lines(1, cols)),
        ("prompt", generate_prompt_lines(1, cols)),
        ("code_syntax", generate_code_lines(1, cols)),
    ];

    // Benchmark serialization
    for (name, lines) in &test_cases {
        let line = &lines[0];

        group.throughput(Throughput::Bytes(line.as_bytes().len() as u64));

        group.bench_with_input(BenchmarkId::new("serialize", name), line, |b, line| {
            b.iter(|| line.serialize())
        });

        let serialized = line.serialize();
        group.bench_with_input(
            BenchmarkId::new("deserialize", name),
            &serialized,
            |b, data| b.iter(|| Line::deserialize(black_box(data))),
        );
    }

    group.finish();
}

fn bench_bulk_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_bulk");
    group.sample_size(50);

    let cols = 80;
    let line_counts = [100, 1000, 10_000];

    for count in line_counts {
        group.throughput(Throughput::Elements(count));

        // Mixed content (typical terminal session)
        group.bench_with_input(
            BenchmarkId::new("create_mixed", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut lines = Vec::with_capacity(count as usize);
                    for i in 0..count as usize {
                        let line = match i % 5 {
                            0 => generate_prompt_lines(1, cols).pop().unwrap(),
                            1 | 2 => generate_plain_lines(1, cols).pop().unwrap(), // Most output is plain
                            3 => generate_ls_output_lines(1, cols).pop().unwrap(),
                            _ => generate_code_lines(1, cols).pop().unwrap(),
                        };
                        lines.push(line);
                    }
                    black_box(lines.len())
                })
            },
        );
    }

    group.finish();
}

fn bench_memory_savings(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_memory");
    group.sample_size(10);

    let cols = 80;

    // Simulate 10K lines of mixed terminal output
    println!("\n=== Memory Savings for 10K Lines ===");

    let mixed_lines: Vec<Line> = (0..10_000)
        .map(|i| match i % 10 {
            0 => generate_prompt_lines(1, cols).pop().unwrap(),
            1..=5 => generate_plain_lines(1, cols).pop().unwrap(), // 50% plain text
            6 | 7 => generate_ls_output_lines(1, cols).pop().unwrap(),
            8 => generate_code_lines(1, cols).pop().unwrap(),
            _ => generate_formatted_lines(1, cols).pop().unwrap(),
        })
        .collect();

    // Calculate total memory
    let total_chars: usize = mixed_lines.iter().map(|l| l.as_bytes().len()).sum();
    let total_runs: usize = mixed_lines.iter().map(|l| l.attr_run_count()).sum();
    let uncompressed_total = uncompressed_size(total_chars);
    let compressed_total: usize = mixed_lines.iter().map(rle_size).sum();
    let overall_ratio = compression_ratio(uncompressed_total, compressed_total);

    println!("Lines: 10,000 (mixed content)");
    println!("Total characters: {}", total_chars);
    println!("Total attribute runs: {}", total_runs);
    println!(
        "Uncompressed attrs: {} bytes ({:.1} MB)",
        uncompressed_total,
        uncompressed_total as f64 / 1_000_000.0
    );
    println!(
        "Compressed attrs: {} bytes ({:.1} KB)",
        compressed_total,
        compressed_total as f64 / 1_000.0
    );
    println!("Overall compression ratio: {:.1}x", overall_ratio);
    println!(
        "Memory saved: {:.1} MB",
        (uncompressed_total - compressed_total) as f64 / 1_000_000.0
    );
    println!();

    // Benchmark serialization of all lines
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("serialize_10k_mixed", |b| {
        b.iter(|| {
            let total_size: usize = mixed_lines.iter().map(|l| l.serialize().len()).sum();
            black_box(total_size)
        })
    });

    group.finish();
}

fn bench_attr_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_access");

    let cols = 80;

    // Test attribute lookup performance
    let test_cases: Vec<(&str, Vec<Line>)> = vec![
        ("plain_text", generate_plain_lines(1, cols)),
        ("prompt", generate_prompt_lines(1, cols)),
        ("code_syntax", generate_code_lines(1, cols)),
    ];

    for (name, lines) in &test_cases {
        let line = &lines[0];

        // Random access pattern
        group.bench_with_input(BenchmarkId::new("random_access", name), line, |b, line| {
            let indices: Vec<usize> = (0..80).collect();
            b.iter(|| {
                for &idx in &indices {
                    black_box(line.get_attr(idx));
                }
            })
        });

        // Sequential access pattern
        group.bench_with_input(
            BenchmarkId::new("sequential_access", name),
            line,
            |b, line| {
                b.iter(|| {
                    for idx in 0..80 {
                        black_box(line.get_attr(idx));
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_wide_columns(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_wide_columns");

    // Test various column widths
    let col_widths = [80, 132, 200, 400];

    println!("\n=== Compression at Different Column Widths ===");
    for cols in col_widths {
        let prompt_lines = generate_prompt_lines(1, cols);
        let prompt_line = &prompt_lines[0];
        let uncompressed = uncompressed_size(cols);
        let compressed = rle_size(prompt_line);
        let ratio = compression_ratio(uncompressed, compressed);

        println!(
            "Prompt {:3} cols | runs: {:2} | ratio: {:5.1}x",
            cols,
            prompt_line.attr_run_count(),
            ratio
        );

        group.throughput(Throughput::Elements(cols as u64));
        group.bench_with_input(
            BenchmarkId::new("create_prompt", cols),
            &cols,
            |b, &cols| b.iter(|| generate_prompt_lines(1, black_box(cols))),
        );
    }
    println!();

    group.finish();
}

fn bench_grid_scrollback_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("rle_grid_integration");
    group.sample_size(20);

    // Test actual grid scrollback with styled content
    let rows = 24;
    let cols = 80;

    println!("\n=== Grid Scrollback Integration ===");

    // Write styled content to grid and let it scroll into scrollback
    let mut grid = Grid::with_scrollback(rows, cols, 10_000);

    // Write 1000 lines of mixed content
    for i in 0..1000 {
        // Simulate styled prompt
        for c in "[user@host ~/code]$ ".chars() {
            grid.write_char(c);
        }
        // Command
        for c in format!("echo line {}", i).chars() {
            grid.write_char(c);
        }
        grid.line_feed();
        grid.carriage_return();

        // Output line
        for c in format!("line {}", i).chars() {
            grid.write_char(c);
        }
        grid.line_feed();
        grid.carriage_return();
    }

    let scrollback_lines = grid.scrollback_lines();
    println!("Scrollback lines: {}", scrollback_lines);

    // Benchmark scrolling more content
    group.throughput(Throughput::Elements(100));
    group.bench_function("scroll_100_lines", |b| {
        let mut grid = Grid::with_scrollback(rows, cols, 10_000);
        // Pre-fill
        for i in 0..500 {
            for c in format!("prefill {}", i).chars() {
                grid.write_char(c);
            }
            grid.line_feed();
            grid.carriage_return();
        }

        b.iter(|| {
            for i in 0..100 {
                for c in format!("line {}", i).chars() {
                    grid.write_char(c);
                }
                grid.line_feed();
                grid.carriage_return();
            }
            grid.scrollback_lines()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_compression_ratios,
    bench_line_serialization,
    bench_bulk_creation,
    bench_memory_savings,
    bench_attr_access,
    bench_wide_columns,
    bench_grid_scrollback_integration,
);
criterion_main!(benches);
