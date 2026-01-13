# Performance Baseline - DashFlow

**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)

**Date**: 2025-11-08
**Commit**: #1046
**Phase**: v1.6.0 Phase 4 (Performance & Optimization)
**Status**: Production-Ready Performance ✅

## Overview

This document establishes performance baselines for core DashFlow operations. All benchmarks run on release builds with LTO enabled.

**Current Assessment**: Performance is excellent across all operations. Our overhead is <0.001% of typical LLM request time, making further optimization unnecessary for production use.

## Benchmark Configuration

- **Tool**: Criterion.rs 0.5
- **Build Profile**: Release (opt-level=3, lto=true, codegen-units=1)
- **Hardware**: Darwin 24.6.0 (macOS, Apple Silicon)
- **Iterations**: 100 samples, auto-determined iteration counts
- **Baseline Date**: 2025-11-08 (Commit #1046)

## Executive Summary

| Category | Time Range | Status | Notes |
|----------|------------|--------|-------|
| Message Construction | 16-32 ns | ✅ Excellent | Blazing fast |
| Chat Model Invoke | 140-286 ns | ✅ Excellent | Sub-microsecond |
| Streaming | 556 ns - 8.1 µs | ✅ Excellent | Very fast even for 200 tokens |
| Serialization | 98-164 ns | ✅ Excellent | Efficient encoding |
| Config Operations | 67-131 ns | ✅ Excellent | Lightweight |
| Prompt Templates | 101-385 ns | ✅ Excellent | Fast rendering |

**Bottom Line**: Total overhead for typical LLM request is <0.01ms, vs. 500-5000ms for actual LLM inference. Our code is not the bottleneck.

## Baseline Results

### Message Construction (nanoseconds)

| Operation | Time (ns) | Throughput | Change from Previous |
|-----------|-----------|------------|----------------------|
| Human message simple | 16 ns | ~62M ops/sec | Stable |
| AI message simple | 31 ns | ~32M ops/sec | Stable |
| Convert to base message | 14 ns | ~71M ops/sec | +1.7% |
| Batch create 10 messages | 273 ns | ~27 ns/msg | +2.7% |

**Analysis**: Message construction is extremely fast. Batch operations show excellent efficiency (27ns per message vs 16ns for single = 69% efficiency, good for batch overhead).

**Trends**: Minor regressions (1-3%) are within noise threshold. Absolute performance remains excellent.

### Chat Model Operations (nanoseconds)

| Operation | Time (ns) | Notes | Change from Previous |
|-----------|-----------|-------|----------------------|
| Single message invoke | 141 ns | Sub-microsecond | Stable (0%) |
| Multi-turn conversation | 167 ns | Very fast | Improved (-2.9%) |
| Long message 1KB | 147 ns | Linear scaling | Improved (-1.3%) |
| Long message 10KB | 272 ns | ~27ns per KB | Improved (-2.2%) |

**Analysis**: Chat model invocation is extremely fast. Size scaling is linear and efficient (~27ns per KB). Recent improvements in multi-turn and long message paths.

### Streaming Operations (microseconds)

| Operation | Time (µs) | Tokens/sec | Change from Previous |
|-----------|-----------|------------|----------------------|
| Stream 10 tokens | 0.556 µs | ~56 ns/token | +1.1% |
| Stream 50 tokens | 2.20 µs | ~44 ns/token | -0.4% (improved) |
| Stream 200 tokens | 8.11 µs | ~41 ns/token | -0.6% (improved) |

**Analysis**: Streaming performance is excellent. Token throughput improves with batch size (56ns → 41ns per token), showing good amortization of overhead. Previous 22% regression concern resolved (now stable to slightly improved).

**Context**: Streaming 200 tokens in 8.11µs is extremely fast. Real bottleneck is network (50-200ms) and LLM generation (500-5000ms).

### Batch Operations (microseconds)

| Operation | Time (µs) | Per-message | Change from Previous |
|-----------|-----------|-------------|----------------------|
| Batch 5 messages | 1.26 µs | ~252 ns/msg | +2.9% |
| Batch 10 messages | 2.48 µs | ~248 ns/msg | +4.6% |

**Analysis**: Batch operations show good efficiency (248-252ns per message). Recent regressions (3-5%) noted but absolute performance remains excellent.

### Config Operations (nanoseconds)

| Operation | Time (ns) | Notes | Change from Previous |
|-----------|-----------|-------|----------------------|
| Create config with tags | 67 ns | Efficient builder | Stable |
| Create config with metadata | 131 ns | Includes JSON | Stable |
| Clone config | 54 ns | Arc-based, cheap | Stable |
| Invoke with no config | 135 ns | Overhead measurement | +11% ⚠️ |
| Invoke with config | 128 ns | Config passing cost | +6% ⚠️ |

**Analysis**: Config operations remain fast. Recent regressions in config overhead (6-11%) are concerning but absolute times remain excellent (<150ns). Monitoring recommended.

**Note**: Config overhead regression is largest observed (11%). While still fast in absolute terms, worth investigating in future profiling.

### Message Serialization (nanoseconds)

| Operation | Time (ns) | Throughput | Change from Previous |
|-----------|-----------|------------|----------------------|
| Serialize human message | 98 ns | ~10.2M ops/sec | Stable |
| Deserialize human message | 126 ns | ~7.9M ops/sec | Stable |
| Serialize AI message | 164 ns | ~6.1M ops/sec | Improved (-3.9%) |
| Serialize batch (10 msgs) | 1,208 µs | ~121 ns/msg | Stable |

**Analysis**: Serialization performance is excellent. Recent improvements in AI message serialization. Batch efficiency good (121ns per message).

### Prompt Template Performance (nanoseconds)

| Operation | Time (ns) | Notes | Change from Previous |
|-----------|-----------|-------|----------------------|
| Render simple FString | 101 ns | Single variable | Stable |
| Render complex (4 vars) | 385 ns | ~96 ns per variable | Stable |
| Render long content | 369 ns | 2,800 char string | Improved (-5.0%) |

**Analysis**: Template rendering remains fast. Long content rendering improved significantly. No optimization needed.

### Runnable Operations (microseconds)

| Operation | Time (µs) | Notes | Change from Previous |
|-----------|-----------|-------|----------------------|
| Lambda runnable | 1.27 µs | Simple function | Improved (-4.4%) |
| Passthrough runnable | 1.46 µs | Identity operation | Stable |
| Batch 10 operations | 13.0 µs | ~1.3 µs per op | Improved (-5.9%) |

**Analysis**: Runnable operations showing improvements (4-6%). Excellent performance for chain abstractions.

### Tool Operations (nanoseconds)

| Operation | Time (ns) | Notes | Change from Previous |
|-----------|-----------|-------|----------------------|
| Simple tool call | 55 ns | Minimal overhead | Improved (-2.5%) |
| Tool with processing | 71 ns | Includes logic | Improved (-3.8%) |
| Schema access | 307 ns | Reflection overhead | Improved (-2.0%) |

**Analysis**: Tool operations are very fast. Recent improvements across all operations. Schema access overhead minimal.

### Output Parsers (nanoseconds to microseconds)

| Parser | Time | Notes |
|--------|------|-------|
| String parser (simple) | 11 ns | Passthrough |
| String parser (long) | 41 ns | No parsing overhead |
| JSON parser (simple) | 147 ns | serde_json |
| JSON parser (complex) | 1.31 µs | Nested objects |
| XML parser (simple) | 894 ns | DOM parsing |
| YAML parser (simple) | 1.85 µs | YAML overhead |

**Analysis**: Parser performance ranges from nanoseconds (string) to microseconds (structured). All well within acceptable ranges for production use.

## Real-World Performance Context

### Typical LLM Request Breakdown

```
Total Time: 1500ms (typical GPT-4 request)

Components:
- Network latency:        100ms (6.7%)
- LLM inference:       1,399.99ms (93.3%)
- Our code overhead:      0.01ms (0.0007%)
                         ^^^^^^
                         16 ns - 8 µs depending on operation
```

### Why We Don't Optimize Further

**Current Performance:**
- Message construction: 16-32 nanoseconds
- Chat invocation: 140-286 nanoseconds
- Streaming 200 tokens: 8.11 microseconds
- **Total overhead: <0.01 milliseconds**

**Real Bottlenecks:**
- Network latency: 50-200 milliseconds (5,000-20,000× our overhead)
- LLM inference: 500-5,000 milliseconds (50,000-500,000× our overhead)

**Optimization Impact Example:**
- If we reduce our 8.11µs to 4µs (50% improvement, very hard):
  - Request time: 1500ms → 1499.996ms
  - User perception: **ZERO** (humans can't perceive <10ms differences)
  - ROI: Time better spent on documentation and features

### Performance Budget & Monitoring

**Acceptance Criteria** (Current: ✅ ALL PASS):
- ✅ Absolute performance < 1ms total overhead
- ✅ Streaming < 100µs for 200 tokens (current: 8.11µs)
- ✅ Message construction < 100ns (current: 16-32ns)
- ✅ Invoke operations < 1µs (current: 140-286ns)

**Alert Thresholds** (for future commits):
- ⚠️ >50% regression in any benchmark (investigate)
- 🚨 >1ms absolute time for any operation (optimize immediately)

**Monitoring Schedule**:
- Run benchmarks every 50 commits
- Document regressions >50% in commit messages
- Investigate only if absolute performance degrades to >1ms

## Performance vs. Python DashFlow

### Estimated Comparison

**DashFlow:**
- Message operations: 16-32 nanoseconds
- Chat invocation: 140-286 nanoseconds
- Total framework overhead: <0.01 milliseconds

**Python DashFlow (estimated):**
- Message operations: ~1-5 microseconds (interpreter overhead)
- Chat invocation: ~10-50 microseconds (dynamic dispatch)
- Total framework overhead: ~1-5 milliseconds

**Performance Advantage**: 100-500× faster than Python
**Practical Impact**: Negligible - both are dominated by network/LLM time

**Conclusion**: While Rust is dramatically faster than Python, neither framework is the bottleneck in real LLM applications. The advantage is more about resource efficiency (memory, CPU) than user-perceived latency.

## Performance Trends

### Observations from N=1045 → N=1046

**Improvements:**
- ✅ Multi-turn conversation: -2.9%
- ✅ Long messages: -1.3% to -2.2%
- ✅ Streaming: -0.4% to -0.6% (resolved previous 22% regression)
- ✅ Runnable operations: -4.4% to -5.9%
- ✅ Tool operations: -2.0% to -3.8%
- ✅ JSON parsing: -3.6%
- ✅ Template rendering (long): -5.0%

**Regressions:**
- ⚠️ Config overhead: +6% to +11% (worth monitoring)
- ⚠️ Batch operations: +3% to +5%
- ⚠️ Message batch creation: +2.7%
- ⚠️ Convert to base message: +1.7%

**Analysis**: Mixed results with more improvements than regressions. Streaming regression concern from N=1045 (22%) fully resolved. Config overhead regression largest at 11% but absolute time still excellent.

**Verdict**: Continue monitoring, no optimization needed unless absolute performance degrades.

## Optimization History

### Historical Optimizations

**Commit #112 (2025-10-28): Template Rendering**
- Before: 23-93µs per template render
- After: 130-457ns per template render
- Improvement: 177-203× faster (replaced regex with manual parsing)

**Commit #804: Text Splitter Pre-allocation**
- Improved memory efficiency in text splitting
- Reduced allocations in hot paths

**v1.6.0 Phase 4 (Commits #1045-1046): Performance Analysis**
- Established comprehensive baseline
- Documented decision to defer further optimization
- Focused on monitoring vs. premature optimization

### Future Optimization Candidates

**If Needed (Not Currently):**

1. **Config Overhead** (11% regression observed)
   - Current: 135ns invoke with no config
   - Target: <120ns (only if regression continues)
   - Method: Profile config passing mechanism

2. **Batch Operations** (3-5% regression)
   - Current: 248-252ns per message in batch
   - Target: <230ns (only if user-reported issues)
   - Method: Review batch allocation patterns

**Not Needed:**
- ❌ Streaming optimization (8.11µs for 200 tokens is excellent)
- ❌ Message construction (16-32ns is blazing fast)
- ❌ Chat invocation (140-286ns is sub-microsecond)

## Benchmark Reproducibility

To reproduce these benchmarks:

```bash
# Run all benchmarks
cargo bench --package dashflow-benchmarks

# Save baseline for comparison
cargo bench --package dashflow-benchmarks > reports/benchmark_baseline_$(date +%Y-%m-%d).txt

# Compare with previous baseline
diff reports/benchmark_baseline_2025-11-08.txt reports/benchmark_baseline_$(date +%Y-%m-%d).txt

# Run specific benchmark suite
cargo bench --package dashflow-benchmarks --bench chat_model_benchmarks
cargo bench --package dashflow-benchmarks --bench core_benchmarks
```

Results are stored in `target/criterion/` with historical comparison.

### Available Benchmark Suites

1. **chat_model_benchmarks.rs**: Chat model invocation, streaming, batching
2. **core_benchmarks.rs**: Messages, config, templates, runnables, tools
3. **embeddings_benchmarks.rs**: Embedding operations
4. **vectorstore_benchmarks.rs**: Vector search and storage
5. **text_splitter_benchmarks.rs**: Text splitting algorithms
6. **loader_benchmarks.rs**: Document loading

## Profiling Tools

### Available Tools

**cargo-bench (Criterion.rs)** - Statistical benchmarking
```bash
cargo bench --package dashflow-benchmarks
```

**cargo-flamegraph** - CPU profiling (not used, not needed)
```bash
# If investigating hotspots (currently not necessary)
cargo flamegraph --bench chat_model_benchmarks -p dashflow-benchmarks
open flamegraph.svg
```

**Instruments.app (macOS)** - System-level profiling
```bash
# macOS alternative to perf (not currently needed)
instruments -t "Time Profiler" target/release/examples/example_name
```

### When to Profile

**Profile when:**
- ✅ User reports performance issues
- ✅ Benchmark shows >1s operations
- ✅ Profiling reveals easy wins

**Don't profile when:**
- ❌ Benchmarks show <1ms operations (current state)
- ❌ Micro-optimizing <0.001% overhead
- ❌ No user-reported issues

## Notes

- All timings are median values from 100 samples
- Outliers (typically <10%) are automatically detected and reported
- Benchmarks use black-box evaluation to prevent compiler optimizations
- Statistical significance determined via Criterion (p < 0.05)
- Regressions noted but accepted if absolute performance remains excellent

## References

- **Previous Baseline**: docs/PERFORMANCE_BASELINE.md (commit #112, 2025-10-28)
