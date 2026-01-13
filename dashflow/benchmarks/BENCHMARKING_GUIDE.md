# DashFlow Benchmarking Guide

**Created:** November 16, 2025 (N=48)
**Purpose:** Validate "2-10x faster" performance claims with empirical data

---

## Overview

This directory contains performance benchmarking infrastructure to compare DashFlow against upstream Python DashFlow. The goal is to validate the claim that the Rust implementation is **2-10x faster** than the Python baseline.

---

## Benchmark Infrastructure

### Rust Benchmarks

**Location:** `crates/dashflow/benches/graph_benchmarks.rs`

**Tool:** [Criterion.rs](https://github.com/bheisler/criterion.rs) (statistical benchmarking)

**Coverage:**
- Graph compilation (3, 10, 100 nodes)
- Sequential execution (3, 5, 10 nodes)
- Conditional branching (binary, multi-way, loops)
- Parallel execution (3, 5, 50 workers)
- Checkpointing (memory, file-based)
- Event streaming (values, updates, events)
- State cloning (small, medium, large)
- Stress tests (100 nodes, deep nesting, wide fanout)
- Real-world scenarios (customer service, batch processing, financial analysis)
- Tracing overhead

**Total:** 35+ benchmarks across 10 categories

**Run Command:**
```bash
cargo bench --package dashflow --bench graph_benchmarks
```

**Output:** HTML reports in `target/criterion/` with statistical analysis

### Python Benchmarks

**Location:** `benchmarks/python/dashflow_benchmarks.py`

**Tool:** `time.perf_counter()` with statistical analysis

**Coverage:** Mirrors core Rust benchmarks for apples-to-apples comparison
- Graph compilation (3, 10 nodes)
- Sequential execution (3, 5 nodes)
- Conditional branching (binary, loop)
- Parallel execution (3 workers)
- Checkpointing (memory-based)

**Total:** 9 benchmarks matching Rust equivalents

**Run Command:**
```bash
source .venv_bench/bin/activate
python benchmarks/python/dashflow_benchmarks.py [iterations] [warmup]
```

**Output:** JSON results in `benchmarks/python/python_bench_results.json`

### Comparative Benchmarking

**Scripts:**
- `benchmarks/quick_comparison.sh` - Quick comparison (8 core benchmarks, ~5 minutes)
- `benchmarks/compare_benchmarks.sh` - Full comparison (all benchmarks, ~30 minutes)

**Run Command:**
```bash
./benchmarks/quick_comparison.sh
```

**Output:** Markdown report in `benchmarks/PERFORMANCE_COMPARISON_N48.md`

---

## Expected Performance Characteristics

Based on Rust's advantages:

### Where Rust Should Excel (5-10x)

1. **State Cloning:** Zero-cost abstractions vs Python object copying
2. **Checkpointing:** Bincode serialization vs pickle
3. **Parallel Execution:** True parallelism vs GIL-constrained threading
4. **Graph Compilation:** Compile-time optimizations vs runtime overhead
5. **Memory Management:** Stack allocation vs heap allocation + GC

### Where Rust Should Match (2-3x)

1. **Sequential Execution:** CPU-bound logic benefits from compiled code
2. **Conditional Branching:** Branch prediction + inlining
3. **Event Streaming:** Iterator optimizations

### Where Differences May Be Small (1-2x)

1. **I/O-bound operations:** Network/disk speed dominates
2. **External LLM calls:** API latency dominates

---

## Performance Goals

### Minimum (MUST ACHIEVE)

- **Average Speedup:** ≥2.0x across all benchmarks
- **No Regressions:** 0 benchmarks slower than Python
- **Consistency:** <30% variance in speedups across categories

### Target (SHOULD ACHIEVE)

- **Average Speedup:** ≥3.0x across all benchmarks
- **Checkpointing:** ≥5.0x faster (bincode vs pickle)
- **Parallel Execution:** ≥3.0x faster (no GIL)

### Stretch (NICE TO HAVE)

- **Average Speedup:** ≥5.0x across all benchmarks
- **State Cloning:** ≥10.0x faster (zero-cost abstractions)
- **Compilation:** ≥10.0x faster (compile-time optimization)

---

## Initial Performance Estimates

Based on Rust's architectural advantages and Python benchmark results from N=48:

| Benchmark | Python (ms) | Rust Estimate (ms) | Expected Speedup |
|-----------|-------------|-------------------|------------------|
| **Compilation** |
| simple_graph_3_nodes | 0.440 | 0.150 | 2.9x |
| complex_graph_10_nodes | 2.138 | 0.600 | 3.6x |
| **Sequential Execution** |
| 3_nodes_simple | 0.818 | 0.300 | 2.7x |
| 5_nodes_complex | 1.924 | 0.650 | 3.0x |
| **Conditional Branching** |
| binary_conditional | 1.025 | 0.400 | 2.6x |
| loop_with_exit_condition | 0.915 | 0.350 | 2.6x |
| **Parallel Execution** |
| fanout_3_workers | 1.908 | 0.450 | 4.2x |
| **Checkpointing** |
| memory_checkpoint_3_nodes | 2.310 | 0.400 | 5.8x |
| memory_checkpoint_5_nodes | 2.946 | 0.600 | 4.9x |

**Estimated Average Speedup: 3.6x**

These are conservative estimates based on:
- Rust's compile-time optimizations
- Zero-cost abstractions
- No GIL for parallelism
- Bincode vs pickle for serialization
- Stack allocation vs heap + GC

---

## Running Benchmarks

### Quick Comparison (Recommended for N=48)

Run core benchmarks only (~5 minutes):

```bash
./benchmarks/quick_comparison.sh
```

This runs:
- Python benchmarks: 20 iterations, 3 warmup
- Rust benchmarks: 8 core benchmarks with reduced sample size
- Generates comparison report

### Full Comparison (Comprehensive)

Run all benchmarks (~30 minutes):

```bash
./benchmarks/compare_benchmarks.sh [iterations] [warmup]
```

This runs:
- Python benchmarks: All 9 benchmarks
- Rust benchmarks: All 35+ benchmarks
- Statistical analysis with Criterion
- Detailed HTML reports

### Python Only

```bash
source .venv_bench/bin/activate
python benchmarks/python/dashflow_benchmarks.py 30 3
```

### Rust Only

```bash
cargo bench --package dashflow --bench graph_benchmarks
```

---

## Interpreting Results

### Speedup Categories

- **✅ 2x+ faster:** Meets or exceeds performance goals
- **⚠️ 1-2x faster:** Modest improvement, acceptable
- **❌ <1x:** Rust slower than Python (**INVESTIGATE**)

### What to Check If Goals Not Met

1. **Build Configuration:**
   ```bash
   cargo bench --release  # Ensure release mode
   ```

2. **Python Virtual Environment:**
   ```bash
   python --version  # Should be 3.10+
   pip list | grep dashflow  # Check version
   ```

3. **System Load:**
   ```bash
   top  # Check CPU usage
   ```

4. **Benchmark Configuration:**
   - Python: Increase iterations for more stable results
   - Rust: Check Criterion sample size

### Common Issues

1. **"No matching benchmarks found"**
   - Criterion output format changed
   - Update parsing logic in comparison script

2. **High variance in Python results**
   - Increase warmup iterations
   - Close background applications

3. **Rust slower than expected**
   - Check debug vs release build
   - Profile with `cargo flamegraph`

---

## Next Steps (Post N=48)

1. **Run Quick Comparison:**
   ```bash
   ./benchmarks/quick_comparison.sh
   ```

2. **Analyze Results:**
   - Review `benchmarks/PERFORMANCE_COMPARISON_N48.md`
   - Verify ≥2x average speedup

3. **If Goals Not Met:**
   - Profile slow benchmarks
   - Optimize hot paths
   - Re-run benchmarks

4. **If Goals Met:**
   - Update README with performance data
   - Create release notes highlighting performance
   - Consider publishing benchmarks to docs

---

## Benchmarking Best Practices

### For Reliable Results

1. **Close background applications** (browsers, IDEs, etc.)
2. **Plug in laptop** (avoid battery throttling)
3. **Run multiple times** and compare (variance < 10% is good)
4. **Use release builds** for both Rust and Python
5. **Profile outliers** to understand variance

### For Fair Comparison

1. **Same hardware** for Python and Rust benchmarks
2. **Same data** (initial state, graph structure)
3. **Same operations** (node logic, checkpointing)
4. **Same concurrency** (3 workers = 3 workers)

---

## File Structure

```
benchmarks/
├── BENCHMARKING_GUIDE.md              # This file
├── quick_comparison.sh                # Quick benchmark runner
├── compare_benchmarks.sh              # Full benchmark runner
├── python/
│   ├── dashflow_benchmarks.py        # Python benchmarks
│   └── python_bench_results.json      # Python results (generated)
├── rust_bench_results.json            # Rust results (generated)
└── PERFORMANCE_COMPARISON_N48.md      # Comparison report (generated)
```

---

## References

- **Rust Benchmarks:** `crates/dashflow/benches/graph_benchmarks.rs`
- **Criterion Docs:** https://bheisler.github.io/criterion.rs/book/
- **Python Baseline:** upstream Python DashFlow 0.2.x
- **Hardware:** [Fill in after running benchmarks]

---

**Status:** Infrastructure complete, ready for benchmark execution
**Worker:** N=48
**Date:** November 16, 2025
**Estimated Time to Run:** 5 minutes (quick) or 30 minutes (full)
