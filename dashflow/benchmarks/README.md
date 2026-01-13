# Benchmarks

Performance benchmarking suite for DashFlow Rust implementation.

## Overview

This directory contains benchmarks for measuring and validating performance improvements of the Rust implementation compared to Python DashFlow baseline.

## Benchmark Suites

### DashFlow Performance (`quick_comparison.sh`, `python/`)

**NEW (N=48):** Comprehensive DashFlow vs Python performance comparison

Benchmarks for validating "2-10x faster" claims for DashFlow implementation:

- **Graph compilation** - Build time for simple and complex graphs
- **Sequential execution** - Linear workflows (3, 5, 10 nodes)
- **Conditional branching** - Dynamic routing and loops
- **Parallel execution** - Concurrent node execution (3, 5, 50 workers)
- **Checkpointing** - State persistence (memory, file-based)
- **Event streaming** - Real-time event emission

**Quick Start:**
```bash
./benchmarks/quick_comparison.sh  # 5 minutes
cat benchmarks/PERFORMANCE_COMPARISON_N48.md
```

**Documentation:** See `BENCHMARKING_GUIDE.md` for detailed instructions

### Python Comparison (`python_comparison/`)

Comparative benchmarks measuring Rust vs Python performance on equivalent operations:

- **compare_text_splitters.rs** - Text splitting performance (CharacterTextSplitter, RecursiveCharacterTextSplitter)
- **compare_parsers.rs** - Document parsing and transformation performance
- **compare_document_loaders.rs** - Document loading performance across formats

**Key Results (from v1.1.0):**
- 25.6× median speedup over Python baseline
- Up to 2432× faster for tool calls
- 4.70× average speedup for text splitting operations

### Memory Profiling (`memory_profiling/`)

Memory usage analysis and profiling tools:

- Memory consumption tracking
- Allocation patterns
- Heap profiling with dhat-rs
- Memory efficiency validation

**Key Results:**
- 10× more efficient memory usage vs Python
- Zero garbage collection pauses
- Predictable memory patterns

## Running Benchmarks

### DashFlow Benchmarks (NEW - N=48)

```bash
# Quick comparison (recommended, ~5 minutes)
./benchmarks/quick_comparison.sh

# Full comparison (~30 minutes)
./benchmarks/compare_benchmarks.sh

# Python benchmarks only
source .venv_bench/bin/activate
python benchmarks/python/dashflow_benchmarks.py 30 3

# Rust benchmarks only
cargo bench --package dashflow --bench graph_benchmarks
```

**Requirements:**
- upstream Python DashFlow installed in `.venv_bench/`
- Rust in release mode

**Output:**
- `benchmarks/PERFORMANCE_COMPARISON_N48.md` - Comparison report
- `benchmarks/python/python_bench_results.json` - Python results
- `target/criterion/` - Rust Criterion results

### Python Comparison Benchmarks

```bash
# Run all Python comparison benchmarks
cd benchmarks/python_comparison
cargo run --release --bin compare_text_splitters
cargo run --release --bin compare_parsers
cargo run --release --bin compare_document_loaders
```

**Requirements:**
- Python DashFlow installed in baseline environment
- Python benchmark scripts in `scripts/python/`

### Memory Profiling

```bash
# Run memory profiling
cd benchmarks/memory_profiling
cargo run --release

# With dhat profiling
DHAT_PROFILING=1 cargo run --release
```

**Output:**
- Memory usage reports
- dhat-heap.json (for visualization with dh_view.html)

## Benchmark Infrastructure

### Criterion (Primary)

Most Rust performance benchmarks use [Criterion.rs](https://github.com/bheisler/criterion.rs):

```bash
# Run criterion benchmarks in a specific crate
cargo bench -p dashflow-text-splitters

# Run specific benchmark
cargo bench --bench text_splitter_bench
```

### Custom Benchmarks

Python comparison benchmarks use custom harness for cross-language comparison:
- JSON output for comparison
- Statistical analysis (mean, median, p95, p99)
- Side-by-side comparison tables

## Viewing Results

### Criterion Results

Criterion generates HTML reports:
```
target/criterion/report/index.html
```

### Python Comparison Results

Results saved to:
```
benchmarks/python_comparison/results/
```

Format: JSON with timing statistics for both Rust and Python

### Memory Profiling Results

dhat output:
```
dhat-heap.json  # Load in https://nnethercote.github.io/dh_view/dh_view.html
```

## Continuous Benchmarking

Benchmarks run in CI for:
- Pull request performance validation
- Regression detection
- Performance trend tracking

## Adding New Benchmarks

### Python Comparison Benchmark

1. Create new .rs file in `python_comparison/`
2. Add matching Python script in `scripts/python/`
3. Use common benchmark harness
4. Output JSON for comparison

### Criterion Benchmark

1. Add to `benches/` in relevant crate
2. Configure in Cargo.toml:
   ```toml
   [[bench]]
   name = "my_benchmark"
   harness = false
   ```
3. Use criterion macros

## Performance Baseline

### DashFlow (N=48 - Infrastructure Complete)

**Status:** Benchmarking infrastructure ready, awaiting execution

**Expected Results (Conservative Estimates):**
- Average: 3-4× faster than Python
- Checkpointing: 5-6× faster (bincode vs pickle)
- Parallel execution: 4-5× faster (no GIL)
- Compilation: 3-4× faster (compile-time optimization)

**See:** `BENCHMARKING_GUIDE.md` for detailed methodology

### DashFlow Core (v1.1.0)

Current performance baseline:
- Median: 25.6× faster than Python
- Text splitting: 4.70× average speedup
- Tool calls: Up to 2432× faster
- Memory: 10× more efficient
- Concurrency: 338× better (3.38M req/s vs 10k req/s)

See `archive/benchmark_baseline.txt` for detailed historical results.
