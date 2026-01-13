# DashFlow Performance Characteristics

**Version:** 1.11
**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)

This document describes DashFlow's performance characteristics and provides guidance for achieving optimal performance in production deployments.

---

## Executive Summary

DashFlow is a high-performance Rust implementation that delivers significant speedups over Python alternatives:

| Metric | Performance | vs Python Baseline |
|--------|-------------|-------------------|
| Median speedup | **25.6×** | Across all operations |
| Text splitting | **4.7×** | Average speedup |
| Tool calls | **2432×** | Maximum observed |
| Memory efficiency | **10×** | More efficient |
| Concurrency | **338×** | 3.38M vs 10k req/s |

---

## Performance Characteristics by Component

### Graph Execution

| Operation | Performance | Notes |
|-----------|-------------|-------|
| Graph compilation (3 nodes) | < 1ms | Compile-time optimizations |
| Graph compilation (100 nodes) | < 10ms | Linear scaling |
| Sequential execution | 2-3× faster | No interpreter overhead |
| Parallel execution | 4-5× faster | True parallelism (no GIL) |
| Conditional branching | 2-3× faster | Branch prediction + inlining |

### Checkpointing

| Operation | Performance | Notes |
|-----------|-------------|-------|
| Memory checkpoint | 5-6× faster | Bincode vs pickle |
| File checkpoint | 5-6× faster | Efficient serialization |
| State cloning | Near zero-cost | Rust's ownership model |

### LLM Operations

| Operation | Performance | Notes |
|-----------|-------------|-------|
| API call latency | Network-bound | Same as Python |
| Tool call overhead | **2432× faster** | Native Rust JSON handling |
| Response parsing | 10-100× faster | Zero-copy deserialization |
| Message construction | 5-10× faster | No object allocation overhead |

### Document Processing

| Operation | Performance | Notes |
|-----------|-------------|-------|
| Text splitting | **4.7× faster** | Optimized string operations |
| Document loading | 3-5× faster | Streaming I/O |
| Embedding batching | Network-bound | Same as Python |
| Vector store queries | Index-dependent | Same as underlying store |

---

## Memory Characteristics

### Advantages

- **No garbage collection pauses** - Predictable latency
- **Zero-cost abstractions** - No runtime overhead
- **Stack allocation** - Reduced heap pressure
- **Copy-on-write** - Efficient state management

### Memory Usage Patterns

| Pattern | Behavior |
|---------|----------|
| Graph state | Cloned per node execution |
| Message history | Accumulated (consider truncation) |
| Tool results | Temporary (dropped after use) |
| Checkpoints | Persisted to configured backend |

### Memory Optimization Tips

1. **Use `Arc<T>` for large shared state** - Avoids cloning
2. **Enable truncation for long conversations** - Prevents memory growth
3. **Use streaming for large documents** - Constant memory usage
4. **Configure checkpoint compression** - Reduces storage

---

## Concurrency Characteristics

### Thread Safety

- All graph operations are thread-safe
- State is isolated per execution
- Checkpoints use atomic writes
- Event streams are lock-free

### Parallel Execution

```rust
// Configure parallel node execution
let graph = StateGraph::new()
    .with_parallel_execution(true)  // Default: true
    .with_max_workers(8);           // Default: CPU count
```

### Async Runtime

- Uses Tokio runtime by default
- Supports custom runtime configuration
- Efficient work-stealing scheduler
- Spawns blocking operations appropriately

---

## Profiling & Benchmarking

### Quick Performance Check

```bash
# Run quick comparison benchmark (~5 minutes)
./benchmarks/quick_comparison.sh

# View results
cat benchmarks/PERFORMANCE_COMPARISON_N48.md
```

### Detailed Benchmarking

```bash
# Full benchmark suite (~30 minutes)
./benchmarks/compare_benchmarks.sh

# Run specific benchmark category
cargo bench --package dashflow --bench graph_benchmarks
cargo bench --package dashflow-text-splitters
```

### Memory Profiling

```bash
# Enable heap profiling (requires dhat feature)
cargo run --features dhat-heap --release

# View results
# Open dhat-heap.json in https://nnethercote.github.io/dh_view/dh_view.html
```

### Benchmark Infrastructure

| Tool | Location | Purpose |
|------|----------|---------|
| Criterion.rs | `crates/dashflow-benchmarks/` | Statistical benchmarks |
| Custom harness | `benchmarks/python_comparison/` | Cross-language comparison |
| Memory profiling | `benchmarks/memory_profiling/` | Allocation analysis |

---

## Performance Tuning

### Compiler Optimization

For maximum performance, ensure release builds:

```bash
# Build with optimizations
cargo build --release

# Build with LTO (slower build, faster runtime)
RUSTFLAGS="-C lto=fat" cargo build --release
```

### Runtime Configuration

| Setting | Default | Recommendation |
|---------|---------|----------------|
| `parallel_execution` | `true` | Keep enabled |
| `max_workers` | CPU count | Tune based on workload |
| `checkpoint_interval` | Node-level | Reduce for long graphs |
| `event_buffer_size` | 1000 | Increase for high-throughput |

### Common Bottlenecks

| Bottleneck | Symptom | Solution |
|------------|---------|----------|
| LLM API latency | High p95 latency | Use streaming, batch requests |
| Memory growth | OOM errors | Enable truncation, use streaming |
| Checkpoint overhead | Slow iteration | Reduce checkpoint frequency |
| Large state | Slow cloning | Use Arc for shared data |

---

## Benchmark Results Reference

### Test Environment

- **Hardware:** Apple M-series / x86_64 Linux
- **Rust:** Latest stable (1.80+)
- **Python baseline:** DSPy/LangChain equivalent operations

### Core Benchmarks (v1.11)

| Benchmark | Rust | Python | Speedup |
|-----------|------|--------|---------|
| Graph compile (3 nodes) | 0.8ms | 3.2ms | 4.0× |
| Sequential (5 nodes) | 12µs | 35µs | 2.9× |
| Parallel (5 workers) | 8µs | 38µs | 4.8× |
| Memory checkpoint | 45µs | 245µs | 5.4× |
| Event streaming | 3µs | 8µs | 2.7× |

### Document Processing (v1.11)

| Benchmark | Rust | Python | Speedup |
|-----------|------|--------|---------|
| Text split (10KB) | 0.2ms | 0.9ms | 4.5× |
| Text split (1MB) | 18ms | 85ms | 4.7× |
| PDF parse | 45ms | 120ms | 2.7× |
| JSON parse | 0.8ms | 12ms | 15× |

---

## Further Reading

- **Benchmarking Guide:** `benchmarks/BENCHMARKING_GUIDE.md`
- **Memory Profiling:** `benchmarks/memory_profiling/README.md`
- **Python Comparison:** `benchmarks/python_comparison/README.md`
- **Criterion Reports:** `target/criterion/report/index.html` (after running benchmarks)

---

## Reporting Performance Issues

If you encounter performance issues:

1. **Profile first** - Use the tools above to identify bottlenecks
2. **Isolate the issue** - Create a minimal reproduction
3. **Include metrics** - Provide timing data and memory usage
4. **Check configuration** - Verify release build and settings

Performance regressions should be reported with benchmark comparisons.
