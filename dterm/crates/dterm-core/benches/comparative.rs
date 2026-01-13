//! Comparative benchmarks against other terminal parsers.
//!
//! Run with: cargo bench --package dterm-core --bench comparative
//!
//! ## IMPORTANT: What This Benchmark Measures
//!
//! This benchmark compares **parser-only** performance:
//! - dterm-core parser vs vte crate (used by Alacritty)
//!
//! **This is NOT a full terminal comparison.** dterm-core is a library that
//! provides parser + grid + state machine, but NO rendering. It integrates
//! into DashTerm2 (an iTerm2 fork) which provides the rendering layer.
//!
//! ## Fair Comparisons
//!
//! | Component        | dterm-core | vte   | Full terminals (Ghostty/Kitty/etc) |
//! |------------------|------------|-------|-------------------------------------|
//! | VT Parser        | Yes        | Yes   | Yes                                 |
//! | Grid/State       | Yes        | No    | Yes                                 |
//! | GPU Rendering    | No         | No    | Yes                                 |
//!
//! **Fair:** dterm parser vs vte parser (both parser-only)
//! **Unfair:** dterm parser vs "Ghostty 600 MB/s" (Ghostty includes rendering)
//!
//! ## Methodology
//!
//! All parsers process identical test data through their parsing pipeline.
//! The benchmark measures throughput including:
//! - State machine transitions
//! - UTF-8 decoding
//! - Callback dispatch (using minimal no-op sinks)
//!
//! ## Test Corpus
//!
//! 1. **Pure ASCII** - Best case, fast path should dominate
//! 2. **Mixed terminal output** - Typical shell session (text + occasional SGR)
//! 3. **Heavy escapes** - Worst case, dense escape sequences
//! 4. **UTF-8 content** - Multi-byte characters (CJK, emoji)
//! 5. **vttest-style** - Sequences from terminal compliance tests
//!
//! ## Results (Parser vs Parser - Fair Comparison)
//!
//! | Workload      | dterm parser | vte parser | Speedup |
//! |---------------|--------------|------------|---------|
//! | ASCII         | ~3.5 GiB/s   | ~376 MiB/s | ~9.4x   |
//! | Mixed         | ~2.2 GiB/s   | ~385 MiB/s | ~5.9x   |
//! | Heavy escapes | ~931 MiB/s   | ~385 MiB/s | ~2.4x   |
//!
//! ## What We Cannot Claim
//!
//! We cannot compare dterm-core to full terminals (Ghostty, Kitty, Alacritty)
//! because dterm-core does not include rendering. Such comparisons would be
//! misleading. Full terminal benchmarks require DashTerm2 integration.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::parser::{ActionSink, BatchActionSink, NullSink, Parser};
use dterm_core::prelude::Terminal;

// === vte crate types ===

/// No-op performer for vte benchmarks
struct VteNullPerformer;

impl vte::Perform for VteNullPerformer {
    fn print(&mut self, _c: char) {}
    fn execute(&mut self, _byte: u8) {}
    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
    }
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn csi_dispatch(
        &mut self,
        _params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

/// Counting performer for vte (for verification)
#[derive(Default)]
struct VteCountingPerformer {
    prints: usize,
    executes: usize,
    csi: usize,
    esc: usize,
    osc: usize,
}

impl vte::Perform for VteCountingPerformer {
    fn print(&mut self, _c: char) {
        self.prints += 1;
    }
    fn execute(&mut self, _byte: u8) {
        self.executes += 1;
    }
    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
    }
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        self.osc += 1;
    }
    fn csi_dispatch(
        &mut self,
        _params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        self.csi += 1;
    }
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        self.esc += 1;
    }
}

// === dterm counting sink (for verification) ===

#[derive(Default)]
struct DtermCountingSink {
    prints: usize,
    executes: usize,
    csi: usize,
    esc: usize,
    osc: usize,
}

impl ActionSink for DtermCountingSink {
    fn print(&mut self, _: char) {
        self.prints += 1;
    }
    fn execute(&mut self, _: u8) {
        self.executes += 1;
    }
    fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) {
        self.csi += 1;
    }
    fn esc_dispatch(&mut self, _: &[u8], _: u8) {
        self.esc += 1;
    }
    fn osc_dispatch(&mut self, _: &[&[u8]]) {
        self.osc += 1;
    }
    fn dcs_hook(&mut self, _: &[u16], _: &[u8], _: u8) {}
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _: u8) {}
    fn apc_end(&mut self) {}
}

impl BatchActionSink for DtermCountingSink {
    fn print_str(&mut self, s: &str) {
        self.prints += s.chars().count();
    }
}

// === Test Data Generators ===

/// Pure ASCII text - best case for SIMD fast path
fn generate_ascii(size: usize) -> Vec<u8> {
    let pattern = b"Hello, World! This is a test of the terminal parser. ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 ";
    pattern.iter().cycle().take(size).copied().collect()
}

/// Mixed terminal output - typical shell session
fn generate_mixed_terminal(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let text = b"Line of text here with some content";
    let colors = [
        b"\x1b[31m".as_slice(), // Red
        b"\x1b[32m",            // Green
        b"\x1b[33m",            // Yellow
        b"\x1b[0m",             // Reset
        b"\x1b[1m",             // Bold
        b"\x1b[4m",             // Underline
    ];

    let mut i = 0;
    while data.len() < size {
        // Color code every ~60 chars
        if i % 5 == 0 && !data.is_empty() {
            data.extend_from_slice(colors[i % colors.len()]);
        }
        data.extend_from_slice(text);
        data.push(b'\n');
        i += 1;
    }
    data.truncate(size);
    data
}

/// Heavy escape sequences - worst case for parser
fn generate_heavy_escapes(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let sequences = [
        b"\x1b[38;5;196m".as_slice(), // 256-color foreground
        b"\x1b[48;5;21m",             // 256-color background
        b"\x1b[38;2;255;128;64m",     // RGB foreground
        b"\x1b[1;4;5m",               // Bold, underline, blink
        b"\x1b[0m",                   // Reset
        b"\x1b[H",                    // Home
        b"\x1b[2J",                   // Clear screen
        b"\x1b[10;20H",               // Move cursor
        b"\x1b[?25h",                 // Show cursor
        b"\x1b[?25l",                 // Hide cursor
    ];

    let mut i = 0;
    while data.len() < size {
        data.extend_from_slice(sequences[i % sequences.len()]);
        data.extend_from_slice(b"X"); // Single char between escapes
        i += 1;
    }
    data.truncate(size);
    data
}

/// UTF-8 content - multi-byte characters
fn generate_utf8(size: usize) -> Vec<u8> {
    let patterns = [
        "Hello, World! ".as_bytes(),
        "中文测试 ".as_bytes(),
        "日本語テスト ".as_bytes(),
        "한글 테스트 ".as_bytes(),
        "\n".as_bytes(),
    ];

    let mut data = Vec::with_capacity(size);
    let mut i = 0;
    while data.len() < size {
        data.extend_from_slice(patterns[i % patterns.len()]);
        i += 1;
    }
    data.truncate(size);
    data
}

/// vttest-style sequences - terminal compliance tests
fn generate_vttest_style(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);

    // Sequences commonly used in vttest
    let sequences = [
        // DECSTBM - Set scrolling region
        b"\x1b[1;24r".as_slice(),
        // SGR with multiple params
        b"\x1b[0;1;4;38;5;196m",
        // Cursor position
        b"\x1b[12;40H",
        // DECSET/DECRST
        b"\x1b[?7h\x1b[?7l",
        // Text
        b"Test line content here",
        b"\r\n",
        // Erase sequences
        b"\x1b[K\x1b[2K",
        // Insert/delete line
        b"\x1b[L\x1b[M",
        // Scroll up/down
        b"\x1b[S\x1b[T",
        // Save/restore cursor
        b"\x1b7\x1b8",
        // Character set selection
        b"\x1b(0\x1b(B",
    ];

    let mut i = 0;
    while data.len() < size {
        data.extend_from_slice(sequences[i % sequences.len()]);
        i += 1;
    }
    data.truncate(size);
    data
}

/// Real terminal output simulation (ls -la, git log, etc.)
fn generate_realistic(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);

    // Simulate git log output
    for i in 0..(size / 200).max(10) {
        // Yellow commit hash
        data.extend_from_slice(b"\x1b[33mcommit ");
        data.extend_from_slice(format!("abc123{:04x}", i).as_bytes());
        data.extend_from_slice(b"\x1b[0m\n");

        // Author line
        data.extend_from_slice(b"Author: Developer <dev@example.com>\n");

        // Date line
        data.extend_from_slice(b"Date:   Mon Dec 28 12:00:00 2025 +0000\n\n");

        // Commit message
        data.extend_from_slice(b"    Add feature ");
        data.extend_from_slice(format!("{}", i).as_bytes());
        data.extend_from_slice(b"\n\n");
    }

    data.truncate(size);
    data
}

// === Benchmark Functions ===

fn bench_comparative_parser(c: &mut Criterion) {
    let sizes = [1024, 64 * 1024, 1024 * 1024]; // 1KB, 64KB, 1MB

    // === Pure ASCII ===
    let mut group = c.benchmark_group("comparative/ascii");
    for size in sizes {
        let data = generate_ascii(size);
        group.throughput(Throughput::Bytes(size as u64));

        // dterm-core advance_fast (optimized)
        group.bench_with_input(BenchmarkId::new("dterm_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        // dterm-core advance_batch (batch printing)
        group.bench_with_input(BenchmarkId::new("dterm_batch", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_batch(black_box(data), &mut sink);
            });
        });

        // dterm-core advance (basic)
        group.bench_with_input(BenchmarkId::new("dterm_basic", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance(black_box(data), &mut sink);
            });
        });

        // vte crate
        group.bench_with_input(BenchmarkId::new("vte", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = vte::Parser::new();
                let mut performer = VteNullPerformer;
                parser.advance(&mut performer, black_box(data));
            });
        });
    }
    group.finish();

    // === Mixed Terminal Output ===
    let mut group = c.benchmark_group("comparative/mixed");
    for size in sizes {
        let data = generate_mixed_terminal(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("dterm_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("vte", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = vte::Parser::new();
                let mut performer = VteNullPerformer;
                parser.advance(&mut performer, black_box(data));
            });
        });
    }
    group.finish();

    // === Heavy Escapes ===
    let mut group = c.benchmark_group("comparative/escapes");
    for size in sizes {
        let data = generate_heavy_escapes(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("dterm_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("vte", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = vte::Parser::new();
                let mut performer = VteNullPerformer;
                parser.advance(&mut performer, black_box(data));
            });
        });
    }
    group.finish();

    // === UTF-8 Content ===
    let mut group = c.benchmark_group("comparative/utf8");
    for size in sizes {
        let data = generate_utf8(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("dterm_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("vte", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = vte::Parser::new();
                let mut performer = VteNullPerformer;
                parser.advance(&mut performer, black_box(data));
            });
        });
    }
    group.finish();

    // === VTTEST-style ===
    let mut group = c.benchmark_group("comparative/vttest");
    for size in sizes {
        let data = generate_vttest_style(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("dterm_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("vte", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = vte::Parser::new();
                let mut performer = VteNullPerformer;
                parser.advance(&mut performer, black_box(data));
            });
        });
    }
    group.finish();

    // === Realistic Output ===
    let mut group = c.benchmark_group("comparative/realistic");
    for size in sizes {
        let data = generate_realistic(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("dterm_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("vte", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = vte::Parser::new();
                let mut performer = VteNullPerformer;
                parser.advance(&mut performer, black_box(data));
            });
        });
    }
    group.finish();
}

/// Verify that both parsers produce similar action counts
fn bench_correctness_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparative/verify");
    group.sample_size(10);

    let test_cases: Vec<(&str, Vec<u8>)> = vec![
        ("ascii", generate_ascii(1024)),
        ("mixed", generate_mixed_terminal(1024)),
        ("escapes", generate_heavy_escapes(1024)),
        ("utf8", generate_utf8(1024)),
    ];

    group.bench_function("action_count_comparison", |b| {
        b.iter(|| {
            for (name, data) in &test_cases {
                // dterm counts
                let mut dterm_parser = Parser::new();
                let mut dterm_sink = DtermCountingSink::default();
                dterm_parser.advance_fast(data, &mut dterm_sink);

                // vte counts
                let mut vte_parser = vte::Parser::new();
                let mut vte_performer = VteCountingPerformer::default();
                vte_parser.advance(&mut vte_performer, data);

                // Basic sanity check: print counts should be similar
                // (Not exact match due to API differences in how prints are batched)
                let dterm_total =
                    dterm_sink.prints + dterm_sink.csi + dterm_sink.esc + dterm_sink.osc;
                let vte_total = vte_performer.prints
                    + vte_performer.csi
                    + vte_performer.esc
                    + vte_performer.osc;

                // Allow 5% variance due to implementation differences
                // Use absolute difference via max/min to avoid signed arithmetic
                let diff = dterm_total.max(vte_total) - dterm_total.min(vte_total);
                let max_allowed = (dterm_total.max(vte_total) / 20).max(10);

                assert!(
                    diff <= max_allowed,
                    "{}: dterm={} vs vte={} (diff={})",
                    name,
                    dterm_total,
                    vte_total,
                    diff
                );
            }
        });
    });
    group.finish();
}

/// Throughput summary at 1MB size for quick comparison
fn bench_throughput_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_summary_1mb");
    let size = 1024 * 1024; // 1MB
    group.throughput(Throughput::Bytes(size as u64));

    // ASCII - best case
    let ascii = generate_ascii(size);
    group.bench_with_input(BenchmarkId::new("dterm", "ascii"), &ascii, |b, data| {
        b.iter(|| {
            let mut parser = Parser::new();
            let mut sink = NullSink;
            parser.advance_fast(black_box(data), &mut sink);
        });
    });
    group.bench_with_input(BenchmarkId::new("vte", "ascii"), &ascii, |b, data| {
        b.iter(|| {
            let mut parser = vte::Parser::new();
            let mut performer = VteNullPerformer;
            parser.advance(&mut performer, black_box(data));
        });
    });

    // Mixed - typical case
    let mixed = generate_mixed_terminal(size);
    group.bench_with_input(BenchmarkId::new("dterm", "mixed"), &mixed, |b, data| {
        b.iter(|| {
            let mut parser = Parser::new();
            let mut sink = NullSink;
            parser.advance_fast(black_box(data), &mut sink);
        });
    });
    group.bench_with_input(BenchmarkId::new("vte", "mixed"), &mixed, |b, data| {
        b.iter(|| {
            let mut parser = vte::Parser::new();
            let mut performer = VteNullPerformer;
            parser.advance(&mut performer, black_box(data));
        });
    });

    // Escapes - worst case
    let escapes = generate_heavy_escapes(size);
    group.bench_with_input(BenchmarkId::new("dterm", "escapes"), &escapes, |b, data| {
        b.iter(|| {
            let mut parser = Parser::new();
            let mut sink = NullSink;
            parser.advance_fast(black_box(data), &mut sink);
        });
    });
    group.bench_with_input(BenchmarkId::new("vte", "escapes"), &escapes, |b, data| {
        b.iter(|| {
            let mut parser = vte::Parser::new();
            let mut performer = VteNullPerformer;
            parser.advance(&mut performer, black_box(data));
        });
    });

    group.finish();
}

/// Full terminal processing: parser + state updates
///
/// This benchmark measures end-to-end throughput including:
/// - Parsing escape sequences
/// - Updating terminal grid state
/// - Cursor movement
/// - SGR attribute changes
///
/// This is more realistic than parser-only benchmarks.
fn bench_terminal_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("terminal_processing");
    let sizes = [1024, 64 * 1024, 256 * 1024]; // 1KB, 64KB, 256KB

    // Use 80x24 terminal (standard size)
    let rows = 24;
    let cols = 80;

    // === ASCII (typical command output) ===
    for size in sizes {
        let data = generate_ascii(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("ascii", size), &data, |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        });
    }

    // === Mixed (git log, ls with colors) ===
    for size in sizes {
        let data = generate_mixed_terminal(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("mixed", size), &data, |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        });
    }

    // === Heavy escapes (vim, htop, ncurses apps) ===
    for size in sizes {
        let data = generate_heavy_escapes(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("escapes", size), &data, |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        });
    }

    // === Realistic (git log output) ===
    for size in sizes {
        let data = generate_realistic(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("realistic", size), &data, |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        });
    }

    // === UTF-8 (internationalized content) ===
    for size in sizes {
        let data = generate_utf8(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("utf8", size), &data, |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        });
    }

    group.finish();
}

/// Summary of terminal processing at 256KB
fn bench_terminal_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("terminal_summary_256kb");
    let size = 256 * 1024;
    group.throughput(Throughput::Bytes(size as u64));

    let rows = 24;
    let cols = 80;

    let ascii = generate_ascii(size);
    group.bench_with_input(BenchmarkId::new("process", "ascii"), &ascii, |b, data| {
        b.iter(|| {
            let mut term = Terminal::new(rows, cols);
            term.process(black_box(data));
            term.cursor().row
        });
    });

    let mixed = generate_mixed_terminal(size);
    group.bench_with_input(BenchmarkId::new("process", "mixed"), &mixed, |b, data| {
        b.iter(|| {
            let mut term = Terminal::new(rows, cols);
            term.process(black_box(data));
            term.cursor().row
        });
    });

    let realistic = generate_realistic(size);
    group.bench_with_input(
        BenchmarkId::new("process", "realistic"),
        &realistic,
        |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        },
    );

    let escapes = generate_heavy_escapes(size);
    group.bench_with_input(
        BenchmarkId::new("process", "escapes"),
        &escapes,
        |b, data| {
            b.iter(|| {
                let mut term = Terminal::new(rows, cols);
                term.process(black_box(data));
                term.cursor().row
            });
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_comparative_parser,
    bench_correctness_verification,
    bench_throughput_summary,
    bench_terminal_processing,
    bench_terminal_summary,
);
criterion_main!(benches);
