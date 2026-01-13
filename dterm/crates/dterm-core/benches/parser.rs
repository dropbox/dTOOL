//! Parser benchmarks.
//!
//! Run with: cargo bench --package dterm-core --bench parser

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::parser::{ActionSink, BatchActionSink, NullSink, Parser};

/// Generate test data: pure ASCII text (best case for fast path)
fn generate_ascii_text(size: usize) -> Vec<u8> {
    let pattern = b"Hello, World! This is a test of the terminal parser. ";
    pattern.iter().cycle().take(size).copied().collect()
}

/// Generate test data: ASCII with occasional escape sequences
fn generate_mixed_terminal(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let text = b"Line of text here";
    let colors = [
        b"\x1b[31m".as_slice(), // Red
        b"\x1b[32m",            // Green
        b"\x1b[33m",            // Yellow
        b"\x1b[0m",             // Reset
    ];

    let mut i = 0;
    while data.len() < size {
        // Add a color code every ~80 chars
        if i % 4 == 0 && !data.is_empty() {
            data.extend_from_slice(colors[i % colors.len()]);
        }
        data.extend_from_slice(text);
        data.push(b'\n');
        i += 1;
    }
    data.truncate(size);
    data
}

/// Generate test data: heavy escape sequences (worst case)
fn generate_heavy_escapes(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let sequences = [
        b"\x1b[38;5;196m".as_slice(), // 256-color foreground
        b"\x1b[48;5;21m",             // 256-color background
        b"\x1b[1;4;5m",               // Bold, underline, blink
        b"\x1b[0m",                   // Reset
        b"\x1b[H",                    // Home
        b"\x1b[2J",                   // Clear screen
        b"\x1b]0;Title\x07",          // OSC title
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

/// Counting sink for verification (may be used for debugging benchmarks)
#[allow(dead_code)]
#[derive(Default)]
struct CountingSink {
    chars: usize,
    controls: usize,
    sequences: usize,
}

impl ActionSink for CountingSink {
    fn print(&mut self, _: char) {
        self.chars += 1;
    }
    fn execute(&mut self, _: u8) {
        self.controls += 1;
    }
    fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) {
        self.sequences += 1;
    }
    fn esc_dispatch(&mut self, _: &[u8], _: u8) {
        self.sequences += 1;
    }
    fn osc_dispatch(&mut self, _: &[&[u8]]) {
        self.sequences += 1;
    }
    fn dcs_hook(&mut self, _: &[u16], _: &[u8], _: u8) {
        self.sequences += 1;
    }
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _: u8) {}
    fn apc_end(&mut self) {}
}

impl BatchActionSink for CountingSink {
    fn print_str(&mut self, s: &str) {
        self.chars += s.len();
    }
}

fn bench_parser(c: &mut Criterion) {
    let sizes = [1024, 64 * 1024, 1024 * 1024]; // 1KB, 64KB, 1MB

    // === ASCII Text Benchmarks ===
    let mut group = c.benchmark_group("parser_ascii");
    for size in sizes {
        let data = generate_ascii_text(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("advance", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("advance_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("advance_batch", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_batch(black_box(data), &mut sink);
            });
        });
    }
    group.finish();

    // === Mixed Terminal Output Benchmarks ===
    let mut group = c.benchmark_group("parser_mixed");
    for size in sizes {
        let data = generate_mixed_terminal(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("advance", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("advance_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("advance_batch", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_batch(black_box(data), &mut sink);
            });
        });
    }
    group.finish();

    // === Heavy Escapes Benchmarks ===
    let mut group = c.benchmark_group("parser_escapes");
    for size in sizes {
        let data = generate_heavy_escapes(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("advance", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance(black_box(data), &mut sink);
            });
        });

        group.bench_with_input(BenchmarkId::new("advance_fast", size), &data, |b, data| {
            b.iter(|| {
                let mut parser = Parser::new();
                let mut sink = NullSink;
                parser.advance_fast(black_box(data), &mut sink);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parser);
criterion_main!(benches);
