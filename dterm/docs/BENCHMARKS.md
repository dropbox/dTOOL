# dterm-core Benchmark Results

**Last Updated:** 2026-01-02
**Platform:** macOS (Apple Silicon)

---

## IMPORTANT: Scope of These Benchmarks

**dterm-core is NOT a complete terminal.** It is a library providing:
- VT parser
- Terminal grid and state machine
- Scrollback storage
- Search indexing

**dterm-core does NOT include:**
- GPU rendering
- Window management
- Platform integration

dterm-core integrates into **DashTerm2** (an iTerm2 fork), which provides the rendering layer.

---

## Fair Comparisons

| Component | dterm-core | vte crate | Full Terminals |
|-----------|------------|-----------|----------------|
| VT Parser | ✅ | ✅ | ✅ |
| Grid/State | ✅ | ❌ | ✅ |
| Scrollback | ✅ | ❌ | ✅ |
| GPU Rendering | ❌ | ❌ | ✅ |
| Complete App | ❌ | ❌ | ✅ |

**We can only fairly compare:**
- dterm parser vs vte parser (both parser-only)
- dterm parser+grid vs vte parser (dterm does more work)

**We CANNOT fairly compare:**
- dterm-core vs Ghostty/Kitty/Alacritty (they include rendering)

---

## Parser Benchmarks (Fair Comparison)

### dterm-core vs vte (Alacritty's parser)

Both measured as parser-only with no-op callbacks. 65KB workload.

| Workload | dterm-core | vte | dterm speedup |
|----------|------------|-----|---------------|
| Pure ASCII | 3.45 GiB/s | 376 MiB/s | **9.4x faster** |
| Mixed terminal | 2.22 GiB/s | 385 MiB/s | **5.9x faster** |
| Heavy escapes | 931 MiB/s | 385 MiB/s | **2.4x faster** |

### Why dterm parser is faster

1. **SIMD fast path** - Scans ASCII runs without per-byte state machine
2. **Batch callbacks** - `print_str()` instead of per-character `print()`
3. **Table-driven state machine** - Optimized transition tables

---

## Full Terminal Processing (dterm_basic)

This measures parser + terminal state machine + grid updates (but NO rendering).

| Workload | dterm_basic | vte (parser only) |
|----------|-------------|-------------------|
| ASCII | 595 MiB/s | 376 MiB/s |

**Note:** dterm_basic does MORE work than vte (grid updates, state tracking) but is still 58% faster.

---

## What We Cannot Claim

The following comparison would be **misleading and unfair**:

| Terminal | Throughput | What it measures |
|----------|------------|------------------|
| dterm-core | 3.5 GiB/s | Parser only, NO rendering |
| Ghostty | ~600 MB/s | Full terminal WITH rendering |
| Kitty | ~500 MB/s | Full terminal WITH rendering |
| Alacritty | ~400 MB/s | Full terminal WITH rendering |

Ghostty/Kitty/Alacritty numbers include GPU rendering overhead. Comparing them to dterm-core's parser-only numbers is meaningless.

---

## Performance Gate (Pre-commit)

Every commit must pass these minimum thresholds:

| Metric | Minimum | Typical |
|--------|---------|---------|
| ASCII throughput | 300 MB/s | ~600 MB/s |
| Mixed throughput | 250 MB/s | ~2000 MB/s |
| Escape throughput | 150 MB/s | ~930 MB/s |

---

## Other Benchmarks

### Scrollback (Tiered Storage)

| Operation | Throughput |
|-----------|------------|
| Hot tier write | ~500K lines/sec |
| Hot→Warm promotion | ~100K lines/sec |
| Search (trigram) | O(1) lookup |

### Grid Operations

| Operation | Time |
|-----------|------|
| Resize (80x24 → 120x40) | ~50 µs |
| Clear screen | ~10 µs |
| Scroll region | ~5 µs |

---

## Running Benchmarks

```bash
# Parser comparison
cargo bench --package dterm-core --bench comparative

# All benchmarks
cargo bench --package dterm-core

# Quick performance gate
./scripts/perf-gate.sh --quick

# Full performance gate
./scripts/perf-gate.sh --full
```

---

## Future: Full Terminal Benchmarks

Once DashTerm2 integration is complete, we will add end-to-end benchmarks measuring:
- Input-to-screen latency
- Frame rendering time
- Full terminal throughput (parser + grid + GPU)

Only then can we fairly compare to other complete terminals.
