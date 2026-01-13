# DashFlow Benchmark Runbook

**Last Updated:** 2025-12-22 (Worker #1416 - M-394 Benchmark suite)
**Purpose:** Performance regression detection for hot paths

---

## Overview

This runbook documents how to run benchmarks, interpret results, and detect performance regressions for DashFlow's hot paths:

1. **Graph Executor** - State graph compilation and execution
2. **Streaming Codec** - Protobuf encode/decode and compression
3. **Retrievers/Vector Stores** - Document storage and similarity search
4. **Registry Client** - Content hashing, serialization, compression

---

## Quick Start

```bash
# Run all hot path benchmarks (~15 minutes)
./scripts/run_hot_path_benchmarks.sh

# Run individual benchmark suites
cargo bench -p dashflow --bench graph_benchmarks
cargo bench -p dashflow-streaming --bench codec_benchmarks
cargo bench -p dashflow-benchmarks --bench vectorstore_benchmarks
cargo bench -p dashflow-benchmarks --bench registry_benchmarks
```

---

## Regression Thresholds

### Definition of Regression

A **regression** is detected when benchmark performance degrades beyond the threshold.
Criterion uses statistical analysis to determine if changes are significant.

| Severity | Threshold | Action Required |
|----------|-----------|-----------------|
| **Critical** | >20% slower | Block merge, investigate immediately |
| **Warning** | 10-20% slower | Investigate before merge |
| **Minor** | 5-10% slower | Document, consider if intentional |
| **Noise** | <5% | Normal variance, ignore |

### Per-Component Thresholds

#### Graph Executor (Critical Path)

| Benchmark | Baseline | Regression Threshold | Critical Threshold |
|-----------|----------|---------------------|-------------------|
| `compilation/simple_graph_3_nodes` | <1ms | +15% | +25% |
| `compilation/complex_graph_10_nodes` | <5ms | +15% | +25% |
| `sequential_execution/3_nodes_simple` | <1ms | +10% | +20% |
| `sequential_execution/10_nodes_stress` | <10ms | +10% | +20% |
| `parallel_execution/fanout_5_workers` | <5ms | +15% | +30% |
| `checkpointing/memory_checkpoint_5_nodes` | <5ms | +10% | +20% |
| `checkpointing/file_checkpoint_5_nodes` | <50ms | +20% | +40% |
| `stress_tests/large_graph_100_nodes` | <100ms | +20% | +40% |

#### Streaming Codec (High Throughput Path)

| Benchmark | Baseline | Regression Threshold | Critical Threshold |
|-----------|----------|---------------------|-------------------|
| `encode/event` | <10μs | +10% | +20% |
| `decode/event` | <10μs | +10% | +20% |
| `encode/state_diff_large` | <100μs | +15% | +30% |
| `compression/zstd_level_3` | <100μs/KB | +15% | +30% |
| `roundtrip/event` | <20μs | +10% | +20% |

#### Vector Store / Retrievers

| Benchmark | Baseline | Regression Threshold | Critical Threshold |
|-----------|----------|---------------------|-------------------|
| `vectorstore_add/1000` | <100ms | +20% | +40% |
| `similarity_search/search_1000_docs_k5` | <10ms | +15% | +30% |
| `similarity_search/search_10000_docs_k5` | <50ms | +15% | +30% |
| `mmr_search/mmr_1000_docs_default` | <20ms | +15% | +30% |

#### Registry Client

| Benchmark | Baseline | Regression Threshold | Critical Threshold |
|-----------|----------|---------------------|-------------------|
| `registry_hashing/sha256_1mb` | <5ms | +10% | +20% |
| `registry_hashing/sha256_10mb` | <50ms | +10% | +20% |
| `registry_manifest_serialize/large_to_json` | <1ms | +15% | +30% |
| `registry_manifest_serialize/large_from_json` | <1ms | +15% | +30% |
| `registry_compression/gzip_compress_1mb` | <50ms | +20% | +40% |

---

## Running Benchmarks

### Prerequisites

```bash
# Ensure you're in release mode
cargo build --release

# Clean previous results (optional, for fresh baseline)
rm -rf target/criterion/
```

### Full Benchmark Suite

```bash
# All graph benchmarks (~5 minutes)
cargo bench -p dashflow --bench graph_benchmarks

# Streaming codec benchmarks (~2 minutes)
cargo bench -p dashflow-streaming --bench codec_benchmarks
cargo bench -p dashflow-streaming --bench diff_benchmarks

# Vector store benchmarks (~3 minutes)
cargo bench -p dashflow-benchmarks --bench vectorstore_benchmarks

# Registry benchmarks (~2 minutes)
cargo bench -p dashflow-benchmarks --bench registry_benchmarks

# Memory benchmarks (~2 minutes)
cargo bench -p dashflow-memory --bench memory_benchmarks
```

### Specific Benchmark Group

```bash
# Run only compilation benchmarks
cargo bench -p dashflow --bench graph_benchmarks compilation

# Run only parallel execution
cargo bench -p dashflow --bench graph_benchmarks parallel_execution

# Run only encode benchmarks
cargo bench -p dashflow-streaming --bench codec_benchmarks encode
```

### Single Benchmark

```bash
cargo bench -p dashflow --bench graph_benchmarks "simple_graph_3_nodes"
```

---

## Interpreting Results

### Criterion Output

Criterion provides statistical analysis:

```
sequential_execution/3_nodes_simple
                        time:   [234.45 µs 238.12 µs 242.01 µs]
                        change: [-2.1234% +0.5678% +3.2109%] (p = 0.12 > 0.05)
                        No change in performance detected.
```

Key fields:
- **time**: [lower bound, estimate, upper bound] at 95% confidence
- **change**: [lower bound, estimate, upper bound] vs previous run
- **p-value**: Statistical significance (p < 0.05 = significant change)

### Change Interpretation

| Message | Meaning | Action |
|---------|---------|--------|
| `No change in performance detected` | Within noise | None |
| `Performance has improved` | Faster than baseline | Document improvement |
| `Performance has regressed` | Slower than baseline | **Investigate** |

### HTML Reports

Criterion generates detailed HTML reports:

```bash
# Open the report
open target/criterion/report/index.html

# Or specific benchmark group
open target/criterion/compilation/report/index.html
```

Reports include:
- Statistical distribution plots
- Comparison vs previous runs
- Outlier detection
- Iteration time over time

---

## Regression Investigation

### Step 1: Reproduce

```bash
# Run the specific benchmark 3 times
cargo bench -p <package> --bench <bench> "<benchmark_name>"
cargo bench -p <package> --bench <bench> "<benchmark_name>"
cargo bench -p <package> --bench <bench> "<benchmark_name>"
```

If regression is inconsistent, it may be environmental noise.

### Step 2: Bisect

```bash
# Find the commit that introduced regression
git bisect start
git bisect bad HEAD
git bisect good <known-good-commit>

# At each step:
cargo bench -p <package> --bench <bench> "<benchmark_name>" 2>&1 | grep "change:"
git bisect good  # or git bisect bad
```

### Step 3: Profile

```bash
# Install flamegraph
cargo install flamegraph

# Generate flame graph
cargo flamegraph -p <package> --bench <bench> -- --bench "<benchmark_name>"

# Or use perf directly (Linux)
perf record cargo bench -p <package> --bench <bench> -- --bench "<benchmark_name>"
perf report
```

### Step 4: Fix or Document

If regression is:
- **Unintentional**: Fix the code, re-run benchmarks to verify
- **Intentional** (e.g., added validation): Document in commit message and update baseline

---

## Baseline Management

### Creating a Baseline

```bash
# Run all benchmarks to establish baseline
./scripts/run_hot_path_benchmarks.sh

# Save baseline report
cp -r target/criterion/ benchmarks/baseline_$(date +%Y%m%d)/
```

### Comparing Against Baseline

Criterion automatically compares against the previous run in `target/criterion/`.

To compare against a specific baseline:

```bash
# Restore baseline
cp -r benchmarks/baseline_20251222/ target/criterion/

# Run current benchmarks (will compare against restored baseline)
cargo bench -p dashflow --bench graph_benchmarks
```

### Updating Baseline After Intentional Changes

```bash
# Document the change
echo "Baseline updated: <reason>" >> benchmarks/BASELINE_HISTORY.md
echo "Date: $(date)" >> benchmarks/BASELINE_HISTORY.md
echo "Commit: $(git rev-parse HEAD)" >> benchmarks/BASELINE_HISTORY.md
echo "---" >> benchmarks/BASELINE_HISTORY.md

# Run benchmarks to set new baseline
cargo bench -p dashflow --bench graph_benchmarks
```

---

## Environment Considerations

### For Reliable Results

1. **Close background applications** - Especially browsers, IDEs
2. **Plug in laptop** - Avoid CPU throttling on battery
3. **Use consistent hardware** - Compare results from same machine
4. **Run multiple times** - Variance <5% indicates stable measurements
5. **Disable CPU scaling** (Linux):
   ```bash
   sudo cpupower frequency-set --governor performance
   ```

### Environment Variables

```bash
# Increase sample size for more accurate results (slower)
export CRITERION_SAMPLE_SIZE=100

# Reduce sample size for faster iteration (less accurate)
export CRITERION_SAMPLE_SIZE=10

# Disable colored output for CI
export CRITERION_NO_COLORS=1
```

---

## CI Integration

Since DashFlow uses internal CI (not GitHub Actions), benchmark regression detection should be integrated into the internal CI pipeline.

### Suggested CI Steps

```yaml
# Example CI configuration (adapt to internal CI system)
benchmark-regression:
  script:
    - cargo bench -p dashflow --bench graph_benchmarks -- --noplot
    - cargo bench -p dashflow-streaming --bench codec_benchmarks -- --noplot
    - cargo bench -p dashflow-benchmarks --bench vectorstore_benchmarks -- --noplot
    - cargo bench -p dashflow-benchmarks --bench registry_benchmarks -- --noplot
  artifacts:
    paths:
      - target/criterion/
  # Fail on regression (requires custom script to parse Criterion output)
  allow_failure: false
```

### Parsing Criterion for CI

```bash
# Check for regressions in Criterion output
cargo bench -p dashflow --bench graph_benchmarks 2>&1 | \
  grep -E "Performance has regressed" && exit 1 || exit 0
```

---

## Benchmark Index

### Graph Executor (`crates/dashflow/benches/graph_benchmarks.rs`)

| Group | Benchmarks | Purpose |
|-------|------------|---------|
| `compilation` | 3 | Graph build time |
| `sequential_execution` | 3 | Linear workflow execution |
| `conditional_branching` | 3 | Dynamic routing |
| `parallel_execution` | 3 | Concurrent execution |
| `checkpointing` | 5 | State persistence |
| `event_streaming` | 4 | Real-time events |
| `state_cloning` | 13 | Memory operations |
| `stress_tests` | 4 | Large workloads |
| `real_world_scenarios` | 3 | Realistic workflows |
| `tracing_overhead` | 3 | Instrumentation cost |

### Streaming Codec (`crates/dashflow-streaming/benches/codec_benchmarks.rs`)

| Group | Benchmarks | Purpose |
|-------|------------|---------|
| `encode` | 2 | Protobuf serialization |
| `decode` | 2 | Protobuf deserialization |
| `roundtrip` | 1 | End-to-end latency |
| `compression` | 5 | Zstd compress/decompress |
| `encode_with_compression` | 3 | Combined encoding |

### Vector Store (`crates/dashflow-benchmarks/benches/vectorstore_benchmarks.rs`)

| Group | Benchmarks | Purpose |
|-------|------------|---------|
| `vectorstore_add` | 4 | Document insertion |
| `similarity_search` | 6 | Vector search |
| `similarity_search_with_score` | 1 | Scored search |
| `mmr_search` | 3 | Diversity search |

### Registry (`crates/dashflow-benchmarks/benches/registry_benchmarks.rs`)

| Group | Benchmarks | Purpose |
|-------|------------|---------|
| `registry_hashing` | 4 | Content hashing |
| `registry_manifest_serialize` | 6 | JSON serialization |
| `registry_search_serialize` | 8 | Search result handling |
| `registry_compression` | 5 | Package compression |

---

## Troubleshooting

### "No matching benchmarks found"

- Check benchmark name spelling
- Use quotes around names with special characters
- List available benchmarks: `cargo bench -p <package> --bench <bench> -- --list`

### High Variance (>10%)

- Close background applications
- Increase sample size: `--sample-size 200`
- Run on dedicated hardware
- Check for thermal throttling

### Criterion Errors

```bash
# Clear cached data
rm -rf target/criterion/

# Update Criterion
cargo update -p criterion
```

### Benchmark Runs Too Long

```bash
# Reduce iterations
cargo bench -- --measurement-time 5 --warm-up-time 1

# Or set via environment
export CRITERION_MEASUREMENT_TIME=5
export CRITERION_WARM_UP_TIME=1
```

---

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [DashFlow BENCHMARKING_GUIDE.md](../benchmarks/BENCHMARKING_GUIDE.md)
- [DashFlow PERFORMANCE.md](PERFORMANCE.md)

---

**Maintainer:** DashFlow Team
**Last Reviewed:** 2025-12-22
