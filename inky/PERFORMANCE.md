# Performance Baselines

Measured on: MacBook Pro M2, 16GB RAM, macOS 14.0
inky-tui version: 0.1.0
Rust version: 1.83.0

## Quick Summary

| Operation | Target | Actual |
|-----------|--------|--------|
| Frame (no change) | <0.1ms | ~0.6µs |
| Frame (1 cell) | <0.1ms | ~0.6µs |
| Full redraw (200x50) | <4ms | 66.7µs |
| Streaming tokens | >10K/s | 674M chars/s |

All performance targets exceeded by 10-100x.

## Cell Operations

| Operation | Time | Throughput |
|-----------|------|------------|
| Cell::blank() | ~0.3ns | 3B cells/s |
| Cell write (single) | ~0.3ns | 3B cells/s |
| String write (10 chars) | 20ns | 500M chars/s |
| Full row write (200 chars) | 291ns | 687M chars/s |

## Buffer Operations

| Operation | Size | Time |
|-----------|------|------|
| Buffer::fill | 10x10 | 12ns |
| Buffer::fill | 80x24 | ~180ns |
| Buffer::fill | 200x50 | 738ns |

## Diff Performance (Double-Buffering)

The `Differ::diff_and_swap` method provides zero-allocation diffing:

| Size | Cells | diff_and_swap | Legacy diff | Improvement |
|------|-------|---------------|-------------|-------------|
| 80x24 (standard) | 1,920 | 624ns | 908ns | 31% faster |
| 120x40 (large) | 4,800 | 1.46µs | 1.90µs | 23% faster |
| 200x50 (xl) | 10,000 | 3.41µs | 4.48µs | 24% faster |

Memory savings per frame:
- 80x24: 15.0 KB avoided
- 200x50: 78.1 KB avoided

## Terminal Write Performance

| Size | Time | Output Size |
|------|------|-------------|
| 80x24 | 12.97µs | 1.9 KB |
| 120x40 | 32.02µs | 4.8 KB |
| 200x50 | 66.66µs | 9.8 KB |

## Layout Performance

| Operation | Time |
|-----------|------|
| Layout build (first) | 661ns |
| Layout build (cached) | 400ns |
| Full layout cycle (cached) | 393ns |

Cache hit provides 40% speedup.

## GPU Buffer Operations (Tier 3)

| Operation | Time | Throughput |
|-----------|------|------------|
| buffer_to_gpu_cells | 40.62µs | 246K/s |
| copy_buffer_to_gpu | 39.93µs | 250K/s |
| Full GPU cycle | 42.13µs | 23.7K frames/s |
| GPU conversion throughput | - | 254M cells/s |

## Memory Usage

| Item | Size |
|------|------|
| Cell struct | 8 bytes |
| GpuCell struct | 8 bytes |
| Buffer 80x24 | 15.0 KB |
| Buffer 200x50 | 78.1 KB |

## Throughput Benchmarks

| Metric | Value |
|--------|-------|
| Streaming char write | 674M chars/s |
| Bulk write throughput | 716M cells/s |
| GPU conversion | 254M cells/s |

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run render benchmarks only
cargo bench --bench render

# Run with baseline comparison
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

## Benchmark Files

- `benches/render.rs` - Buffer, diff, GPU benchmarks
- `benches/regression.rs` - Criterion benchmarks for CI

## Performance Regression Detection

The CI pipeline runs benchmarks and fails if performance regresses by more than 10%:

```bash
cargo bench -- --baseline main --threshold 10
```
