# DashFlow Performance Comparison: Rust vs Python

**Generated:** 2025-11-16 04:49:11
**Branch:** main
**Commit:** N=48 (Performance benchmarking)

## Executive Summary

- **Average Speedup:** 584.76x
- **Min Speedup:** 163.77x
- **Max Speedup:** 1054.71x
- **Benchmarks Compared:** 8

✅ **Performance Goal ACHIEVED**: Rust is 2x+ faster on average

## Detailed Results

| Benchmark | Python (ms) | Rust (ms) | Speedup | Status |
|-----------|-------------|-----------|---------|--------|
| compilation/simple_graph_3_nodes | 0.429 | 0.000 | 1054.71x | ✅ |
| conditional_branching/binary_conditional | 1.020 | 0.001 | 926.56x | ✅ |
| sequential_execution/3_nodes_simple | 0.765 | 0.001 | 924.99x | ✅ |
| sequential_execution/5_nodes_complex | 1.912 | 0.003 | 570.07x | ✅ |
| checkpointing/memory_checkpoint_3_nodes | 1.574 | 0.003 | 525.59x | ✅ |
| conditional_branching/loop_with_exit_condition | 0.947 | 0.003 | 334.81x | ✅ |
| checkpointing/memory_checkpoint_5_nodes | 2.775 | 0.016 | 177.59x | ✅ |
| parallel_execution/fanout_3_workers | 1.666 | 0.010 | 163.77x | ✅ |

## Interpretation

- ✅ **2x+ faster**: Meets or exceeds performance goals
- ⚠️ **1-2x faster**: Modest improvement, within expected range
- ❌ **<1x**: Rust slower than Python (investigate)

## Python Benchmark Details

| Benchmark | Mean (ms) | Median (ms) | Std Dev (ms) |
|-----------|-----------|-------------|--------------|
| compilation/simple_graph_3_nodes | 0.429 | 0.417 | 0.035 |
| compilation/complex_graph_10_nodes | 2.178 | 2.143 | 0.158 |
| sequential_execution/3_nodes_simple | 0.765 | 0.733 | 0.119 |
| sequential_execution/5_nodes_complex | 1.912 | 1.891 | 0.064 |
| conditional_branching/binary_conditional | 1.020 | 1.015 | 0.019 |
| conditional_branching/loop_with_exit_condition | 0.947 | 0.925 | 0.085 |
| parallel_execution/fanout_3_workers | 1.666 | 1.612 | 0.130 |
| checkpointing/memory_checkpoint_3_nodes | 1.574 | 1.546 | 0.097 |
| checkpointing/memory_checkpoint_5_nodes | 2.775 | 2.722 | 0.135 |

## Notes

- Python benchmarks: 20 iterations, 3 warmup (simple timing)
- Rust benchmarks: Criterion default (statistical analysis)
- Both run in release/optimized mode
- Results may vary based on system load and hardware
