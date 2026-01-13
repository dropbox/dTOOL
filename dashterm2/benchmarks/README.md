# DashTerm2 Performance Benchmark Suite

Comprehensive performance benchmarks for validating terminal rendering and buffer management optimizations.

## Overview

This benchmark suite measures key performance metrics across:
- **Text Rendering**: Line rendering with ASCII, Unicode, and emoji content
- **Screen Buffer**: VT100Screen operations (insertion, deletion, resize, history)
- **Memory**: Allocation patterns and footprint measurements
- **Metal Renderer**: GPU frame time and glyph cache performance (GUI required)

## Quick Start

```bash
# Run all benchmarks
./scripts/run-benchmarks.sh

# Run specific benchmark category
./scripts/run-benchmarks.sh --category text

# Compare against baseline
./scripts/run-benchmarks.sh --compare

# Generate JSON report
./scripts/run-benchmarks.sh --json results.json
```

## Requirements

- macOS 14.0+ (Sonoma or later)
- Xcode 16.0+
- DashTerm2 built in Development configuration

## Benchmark Categories

### Text Rendering Benchmarks
Measures time to process and prepare text for rendering.

| Benchmark | Description |
|-----------|-------------|
| `TextRender1K` | Render 1,000 lines of text |
| `TextRender10K` | Render 10,000 lines of text |
| `TextRender100K` | Render 100,000 lines of text |
| `UnicodeText` | Render text with Unicode characters |
| `EmojiText` | Render emoji-heavy content |
| `MixedContent` | Mixed ASCII/Unicode/emoji |

### Row Data Pool Benchmarks
Measures allocation performance for Metal row data with and without pooling.

| Benchmark | Description |
|-----------|-------------|
| `row_data_pool_benchmark` | Compares allocation speed with/without object pooling |

**Build and run:**
```bash
clang -framework Foundation -fobjc-arc -O3 \
  benchmarks/row_data_pool_benchmark.m -o benchmarks/row_data_pool_benchmark
./benchmarks/row_data_pool_benchmark
```

Example output on Apple M3 Max:
```
Configuration: 24x80 (standard) (24 rows x 80 columns)
  No Pooling:   0.0060 ms/frame
  With Pooling: 0.0048 ms/frame
  Speedup:      1.27x faster
  Allocations saved at 60fps: 5760/sec
```

### ASCII Glyph Processing Benchmarks
Measures the performance improvement from eliminating Objective-C dispatch overhead in the ASCII fast path.

| Benchmark | Description |
|-----------|-------------|
| `ascii_glyph_benchmark` | Compares Objective-C method dispatch vs inline C++ for ASCII glyph processing |

**Build and run:**
```bash
clang -framework Foundation -fobjc-arc -O3 \
  benchmarks/ascii_glyph_benchmark.m -o benchmarks/ascii_glyph_benchmark
./benchmarks/ascii_glyph_benchmark
```

Example output on Apple M3 Max:
```
Configuration: standard (24x80) (24 rows x 80 columns = 1920 glyphs)
  Objective-C dispatch: 0.0071 ms/frame
  Inline C++ path:      0.0028 ms/frame
  Speedup:              2.57x faster

Configuration: large (100x200) (100 rows x 200 columns = 20000 glyphs)
  Objective-C dispatch: 0.0769 ms/frame
  Inline C++ path:      0.0340 ms/frame
  Speedup:              2.26x faster
```

### VT100 Parser Benchmarks
Measures ASCII fast-path throughput inside `VT100StringParser`.

| Benchmark | Description |
|-----------|-------------|
| `vt100_parser_benchmark` | Compares scalar vs SIMD ASCII scanning in `DecodeMixedASCIIBytes` |

The benchmark supports NEON (Apple Silicon), AVX2 (Intel with AVX2), and SSE2 (older Intel) SIMD optimizations:

**Apple Silicon (NEON):**
```bash
clang -framework Foundation -fobjc-arc -O3 \
  benchmarks/vt100_parser_benchmark.m -o benchmarks/vt100_parser_benchmark
./benchmarks/vt100_parser_benchmark
```

**Intel (AVX2 - recommended for Haswell and newer):**
```bash
clang -framework Foundation -fobjc-arc -O3 -mavx2 \
  benchmarks/vt100_parser_benchmark.m -o benchmarks/vt100_parser_benchmark
./benchmarks/vt100_parser_benchmark
```

**Intel (SSE2 - for older CPUs):**
```bash
clang -framework Foundation -fobjc-arc -O3 -msse2 \
  benchmarks/vt100_parser_benchmark.m -o benchmarks/vt100_parser_benchmark
./benchmarks/vt100_parser_benchmark
```

Example output on Apple M3 Max:
```
VT100 Parser ASCII Fast-Path Benchmark
Payload Size: 8.00 MB
Iterations:   200

Scalar     Avg:    2.811 ms  Throughput: 2845.61 MB/s
NEON       Avg:    1.128 ms  Throughput: 7091.75 MB/s

Speedup: 2.49x faster ASCII scanning (NEON)
```

### Screen Buffer Benchmarks
Measures VT100Screen buffer operations.

| Benchmark | Description |
|-----------|-------------|
| `LineInsertion` | Insert lines at various positions |
| `LineDeletion` | Delete lines at various positions |
| `ScrollUp` | Scroll buffer up N lines |
| `ScrollDown` | Scroll buffer down N lines |
| `Resize` | Resize screen buffer |
| `HistoryAccess` | Access historical lines |

### ScreenCharArray Cache Benchmark
Measures the impact of caching `screenCharArrayForLine:` results when rendering repeated frames.

| Benchmark | Description |
|-----------|-------------|
| `screen_char_cache_benchmark` | Simulates viewport redraws with/without cached screen lines |

**Build and run:**
```bash
clang -O3 -fobjc-arc -framework Foundation -framework CoreFoundation \
  Benchmarks/screen_char_cache_benchmark.m -o /tmp/screen_char_cache_benchmark
/tmp/screen_char_cache_benchmark
```

Example output on Apple M3 Max:
```
DashTerm2 ScreenCharArray Cache Benchmark
==========================================
Viewport: 60 rows, width 120

Scenario: Static viewport (no mutations)
  Without cache:  1159.26 ms
  With cache:      475.27 ms
  Speedup:           2.44x

Scenario: Cursor updates every 60 frames
  Without cache:  1195.66 ms
  With cache:      473.65 ms
  Speedup:           2.52x

Scenario: Steady scroll (mutate & scroll)
  Without cache:  1191.11 ms
  With cache:      471.33 ms
  Speedup:           2.53x
```

### External Attribute Index Cache Benchmark (Negative Result)
Measures the potential impact of caching `externalAttributeIndexForLine:` results. This benchmark demonstrated that caching adds **overhead** rather than benefit.

| Benchmark | Description |
|-----------|-------------|
| `eaindex_cache_benchmark` | Compares EA index lookup with/without caching |

**Build and run:**
```bash
clang -O3 -fobjc-arc -framework Foundation -framework CoreFoundation \
  benchmarks/eaindex_cache_benchmark.m -o benchmarks/eaindex_cache_benchmark
./benchmarks/eaindex_cache_benchmark
```

Example output on Apple M3 Max:
```
DashTerm2 External Attribute Index Cache Benchmark
===================================================
Viewport: 60 rows, width 120, cache limit 256

Scenario: Static viewport
  EA Index density: 10%
  Without cache:   496.61 ms
  With cache:      708.95 ms
  Speedup:         0.70x  (SLOWER - cache overhead exceeds benefit)
```

**Key insight**: Unlike `screenCharArrayForLine:` which involves expensive LineBuffer traversal and memory allocation, `externalAttributeIndexForLine:` is a lightweight metadata lookup. The cache machinery overhead (NSDictionary lookup, LRU maintenance) exceeds the baseline cost. See `reports/main/PERFORMANCE_AUDIT_138.md` for detailed analysis.

### Texture Page Pruning Benchmark (Negative Result)
Evaluates optimizing the O(n log n) sort in texture page pruning. This benchmark demonstrated that optimizing pruning would **severely degrade** the more frequent `recordUse()` operation.

| Benchmark | Description |
|-----------|-------------|
| `texture_page_prune_benchmark` | Compares full sort vs linked list vs partial sort approaches |

**Build and run:**
```bash
clang++ -std=c++17 -framework Foundation -fobjc-arc -O3 \
  benchmarks/texture_page_prune_benchmark.mm -o benchmarks/texture_page_prune_benchmark
./benchmarks/texture_page_prune_benchmark
```

Example output on Apple M3 Max:
```
Texture Page Pruning Benchmark
===============================
Configuration: 4096 pages, prune 512, 200 uses/frame, 100 frames
  Current (full sort):    5.805 ms total, 0.0581 ms/prune
  Optimized (linked list): 2.254 ms total, 0.0225 ms/prune
  Partial sort (k=512):    3.795 ms total, 0.0380 ms/prune

  recordUse overhead comparison:
    Current recordUse:   5.494 ms for 2000000 calls
    Optimized recordUse: 107.387 ms for 2000000 calls (list maintenance)
```

**Key insight**: The linked list approach makes pruning ~3x faster but makes `recordUse()` **19x slower**. Since `recordUse()` is called thousands of times per frame and pruning only occurs when exceeding 64K unique non-ASCII glyphs (extremely rare), the current O(n log n) sort is optimal. See `reports/main/PERFORMANCE_AUDIT_139.md` for detailed analysis.

### Memory Benchmarks
Measures memory allocation and footprint.

| Benchmark | Description |
|-----------|-------------|
| `AllocationCount` | Count allocations during operations |
| `MemoryFootprint` | Measure memory for scrollback sizes |
| `PeakMemory` | Track peak memory during operations |

### Metal Renderer Benchmarks (GUI Required)
Measures GPU rendering performance.

| Benchmark | Description |
|-----------|-------------|
| `FrameTime` | GPU frame render time |
| `GlyphCacheHit` | Glyph cache hit rate |
| `TextureAtlas` | Texture atlas performance |

## Output Formats

### Console Output
```
DashTerm2 Performance Benchmarks
================================
Category: Text Rendering

  TextRender1K .................. 12.3ms (±1.2ms)
  TextRender10K ................. 98.7ms (±5.4ms)
  TextRender100K ................ 892.1ms (±23.1ms)
```

### JSON Output
```json
{
  "timestamp": "2025-12-17T12:00:00Z",
  "system": {
    "os": "macOS 15.2",
    "cpu": "Apple M2 Pro",
    "memory": "16GB"
  },
  "results": {
    "TextRender1K": {
      "mean_ms": 12.3,
      "stddev_ms": 1.2,
      "min_ms": 10.1,
      "max_ms": 15.2,
      "iterations": 100
    }
  }
}
```

## Baseline Management

Baselines are stored in `Benchmarks/baselines/` as JSON files.

```bash
# Save current results as new baseline
./scripts/run-benchmarks.sh --save-baseline

# Compare against baseline
./scripts/run-benchmarks.sh --compare

# Compare against specific baseline
./scripts/run-benchmarks.sh --compare baselines/baseline_20251217.json
```

## Adding New Benchmarks

1. Create a new benchmark class conforming to `Benchmark` protocol
2. Register in `BenchmarkRegistry.swift`
3. Run to verify functionality
4. Update this README with description

## Interpreting Results

### Variance
- **< 5%**: Excellent reproducibility
- **5-10%**: Acceptable variance
- **> 10%**: Results may be unreliable; close background apps

### Performance Targets
Based on optimization goals:
- Text rendering: < 1ms per 1000 lines
- Screen buffer ops: < 0.1ms per operation
- Memory: Linear scaling with content size

## Troubleshooting

### High Variance
1. Close all other applications
2. Disable Spotlight indexing temporarily
3. Run on AC power (not battery)
4. Increase warmup iterations

### Build Failures
```bash
# Rebuild benchmark target
xcodebuild -project DashTerm2.xcodeproj -scheme DashTermBenchmarks -configuration Development build
```

## CI Integration

The benchmark suite supports CI via JSON output and exit codes:
- Exit 0: All benchmarks passed
- Exit 1: Benchmark failure
- Exit 2: Regression detected (>10% slower than baseline)

```yaml
# GitHub Actions example
- name: Run Benchmarks
  run: |
    ./scripts/run-benchmarks.sh --json results.json --compare --threshold 10
```

### TaskNotifier I/O Polling Benchmark
Measures the performance improvement from using kqueue() instead of select() for terminal I/O event notification.

| Benchmark | Description |
|-----------|-------------|
| `tasknotifier_kqueue_benchmark` | Compares select() vs kqueue() for I/O polling with varying task counts |

**Build and run:**
```bash
clang -framework Foundation -fobjc-arc -O3 \
  benchmarks/tasknotifier_kqueue_benchmark.m -o benchmarks/tasknotifier_kqueue_benchmark
./benchmarks/tasknotifier_kqueue_benchmark
```

Example output on Apple M3 Max:
```
TaskNotifier I/O Polling Benchmark: select() vs kqueue()
=========================================================
Iterations per config: 10000
Warmup iterations: 1000

Tasks        select() (ms)   kqueue() (ms)    Speedup
---------- --------------- --------------- ----------
1                    3.285           2.294      1.43x
4                    5.827           2.628      2.22x
16                  14.555           2.634      5.53x
64                  47.471           2.701     17.57x
256                192.822           2.765     69.74x
---------- --------------- --------------- ----------
Total              263.960          13.021     20.27x
```

**Key insight**: select() scales O(n) with the number of file descriptors, while kqueue() provides O(1) event retrieval. For typical terminal usage (1-16 sessions), kqueue provides 1.4-5.5x speedup. For heavy users with many sessions, the improvement is dramatic (17-70x). Additionally, kqueue removes the FD_SETSIZE (1024) limitation that select() has.

See `reports/main/PERFORMANCE_AUDIT_140.md` for detailed analysis.

### PTYTask Write Buffer Benchmark (Low-Priority Positive Result)
Evaluates optimization potential for the PTYTask write buffer using a ring buffer with os_unfair_lock instead of the current NSLock+memmove approach.

| Benchmark | Description |
|-----------|-------------|
| `ptytask_writebuffer_benchmark` | Compares NSLock+memmove vs ring buffer+os_unfair_lock for keyboard input buffering |

**Build and run:**
```bash
clang -framework Foundation -fobjc-arc -O3 \
  benchmarks/ptytask_writebuffer_benchmark.m -o benchmarks/ptytask_writebuffer_benchmark
./benchmarks/ptytask_writebuffer_benchmark
```

Example output on Apple M3 Max:
```
Configuration: Single byte (typing) (1 bytes x 1 writes)
  Current (NSLock)      Append:    1.092 ms  Drain:    1.071 ms  Total:    2.164 ms
  Ring (unfair_lock)    Append:    0.370 ms  Drain:    0.275 ms  Total:    0.646 ms
  Speedup: 3.35x (ring buffer faster)

Configuration: Large paste (10KB) (1024 bytes x 10 writes)
  Current (NSLock)      Append:    6.939 ms  Drain:    8.077 ms  Total:   15.015 ms
  Ring (unfair_lock)    Append:    2.936 ms  Drain:    0.547 ms  Total:    3.482 ms
  Speedup: 4.31x (ring buffer faster)

Overall: Current=48.94 ms, Ring=15.43 ms, Speedup=3.17x
```

**Key insight**: While the ring buffer shows 3-4x speedup, the absolute time saved is minimal because the write buffer is used for keyboard input (low frequency). Normal typing (~10 chars/sec) saves only ~6.7 microseconds/sec. The optimization is **positive but low priority** - worth implementing only after higher-impact optimizations are exhausted.

See `reports/main/PERFORMANCE_AUDIT_141.md` for detailed analysis.

### VT100Grid Fill Benchmark (Implemented)
Measures the performance of filling screen character arrays with a constant value, comparing different implementations including NEON vectorization.

| Benchmark | Description |
|-----------|-------------|
| `grid_fill_benchmark` | Compares doubling memcpy vs simple loop vs unrolled loop vs NEON vectorized |

**Build and run:**
```bash
clang -framework Foundation -fobjc-arc -O3 \
  benchmarks/grid_fill_benchmark.m -o benchmarks/grid_fill_benchmark
./benchmarks/grid_fill_benchmark
```

Example output on Apple M3 Max:
```
Configuration             Current (doubling)        Simple loop  Pattern (4x init)      Unrolled (8x)    NEON vectorized       Best
------------------------- ------------------ ------------------ ------------------ ------------------ ------------------ ----------
Small (10 chars)                    3.215 ms           0.796 ms           0.697 ms           0.539 ms           0.712 ms    5.97x (Unrolled (8x))
Line clear (80 chars)               4.968 ms           3.856 ms           3.074 ms           3.193 ms           1.568 ms    3.17x (NEON vectorized)
Wide line (200 chars)               2.007 ms           2.905 ms           1.470 ms           2.376 ms           1.020 ms    1.97x (NEON vectorized)
Large clear (1000 chars)            1.762 ms           3.137 ms           1.522 ms           3.062 ms           1.193 ms    1.48x (NEON vectorized)
Full screen (24x80)                 1.853 ms           3.435 ms           1.475 ms           2.923 ms           1.176 ms    1.58x (NEON vectorized)
Large screen (60x200)               2.061 ms           5.278 ms           2.108 ms           3.737 ms           1.829 ms    1.13x (NEON vectorized)

TOTAL                              15.867 ms          19.409 ms          10.346 ms          15.829 ms           7.498 ms    2.12x (NEON vectorized)
```

**Key insight**: NEON vectorization provides **2.12x faster** fill operations overall on Apple Silicon. The optimization uses:
- Small fills (<8 chars): Unrolled loop for ~6x speedup
- Larger fills: NEON 96-byte bulk stores for 1.5-3x speedup

This optimization has been **implemented** in `sources/VT100Grid.m` (iteration #142).

See `reports/main/PERFORMANCE_AUDIT_142.md` for detailed analysis.

### End-to-End Workload Benchmark (NEW)
Measures actual terminal throughput for real-world scenarios by testing the full terminal pipeline from process output through Metal rendering.

| Benchmark | Description |
|-----------|-------------|
| `e2e_workload_benchmark.sh` | Full pipeline benchmark including raw throughput, ANSI colors, Unicode, and scrollback |

**Run the benchmark:**
```bash
# Quick benchmark (~30 seconds)
./Benchmarks/e2e_workload_benchmark.sh --quick

# Full benchmark (~2 minutes)
./Benchmarks/e2e_workload_benchmark.sh --full

# Stress test (~5 minutes)
./Benchmarks/e2e_workload_benchmark.sh --stress
```

Example output on Apple M3 Max (iTerm.app v3.6.6):
```
=== Raw Throughput ===
  yes_lines_100000: 21.398ms (stddev: 4.998ms)    -> ~4.6M lines/sec
  cat_zero_1000000B: 16.174ms (stddev: 0.281ms)   -> ~61 MB/sec

=== ANSI Escape Sequences ===
  256color_x10: 67.435ms (stddev: 3.028ms)
  truecolor_x10: 59.085ms (stddev: 1.779ms)

=== Unicode Rendering ===
  cjk_x5: 39.541ms (stddev: 1.049ms)
  emoji_x5: 45.196ms (stddev: 3.811ms)

=== Scrollback Stress ===
  long_lines_x1000: 61.870ms (stddev: 4.576ms)
  rapid_output_x5000: 32.228ms (stddev: 1.353ms)
```

**Key insight (Iteration #146 update)**: The earlier 4.5s measurement for `long_lines_x1000` was dominated by process-launch overhead from spawning `tr` 1,000 times, not terminal rendering. The benchmark now uses `Benchmarks/Sources/generate_long_lines.py` to stream wide lines without extra processes, so the test reflects true scrollback throughput (~62ms on M3 Max).

See `reports/main/E2E_WORKLOAD_BENCHMARK_145.md` for the initial study and `reports/main/LONG_LINE_STRESS_146.md` for the correction details.

### Long Line Generator with Sweep Mode (Iteration #147)
Enhanced `generate_long_lines.py` now supports sweep mode for testing multiple configurations:

| Feature | Description |
|---------|-------------|
| Single mode | Emit lines with fixed dimensions (default) |
| Sweep mode | Test multiple width/line combinations with timing |

**Usage:**
```bash
# Single mode (default)
python3 benchmarks/Sources/generate_long_lines.py --lines 1000 --columns 500

# Sweep mode - test multiple configurations
python3 benchmarks/Sources/generate_long_lines.py --sweep --widths 100,500,1000,2000 --line-counts 500,1000

# Range syntax (start:end:step)
python3 benchmarks/Sources/generate_long_lines.py --sweep --widths 100:1000:100 --line-counts 500:2000:500

# Output JSON for programmatic use
python3 benchmarks/Sources/generate_long_lines.py --sweep --json
```

### Quick Terminal Test (Iteration #147)
Fast benchmark script for comparing terminal emulator performance. Run this INSIDE each terminal you want to test.

**Usage:**
```bash
./benchmarks/quick_terminal_test.sh
```

Example output on iTerm.app 3.6.6 (Apple M3 Max):
```
=== Raw Throughput ===
  yes_100k: 16.585ms (stddev: 0.820ms)
  seq_50k: 21.649ms (stddev: 0.889ms)
  cat_1MB: 13.426ms (stddev: 2.209ms)

=== ANSI Escape Sequences ===
  256color: 65.255ms (stddev: 1.557ms)
  truecolor: 62.088ms (stddev: 1.607ms)

=== Unicode ===
  cjk: 38.290ms (stddev: 0.900ms)
  emoji: 37.948ms (stddev: 0.594ms)

=== Long Lines ===
  long_500x1000: 44.619ms (stddev: 1.046ms)
  long_2000x1000: 45.208ms (stddev: 0.556ms)
```

### Comparison Report Generator (Iteration #147)
Generate markdown comparison reports from multiple terminal benchmark results.

**Usage:**
```bash
# Generate report from results
python3 benchmarks/generate_comparison_report.py

# Save to file
python3 benchmarks/generate_comparison_report.py --output comparison_report.md

# Custom results directory
python3 benchmarks/generate_comparison_report.py /path/to/results --output report.md
```

### Terminal Comparison Benchmark (Iteration #147)
Automated benchmark runner for comparing multiple terminal emulators.

**Usage:**
```bash
# Quick benchmark all detected terminals
./benchmarks/terminal_comparison_benchmark.sh

# Full benchmark on specific terminals
./benchmarks/terminal_comparison_benchmark.sh --full iTerm Terminal
```

**Detected terminals:** iTerm, Terminal, Alacritty, kitty, WezTerm
