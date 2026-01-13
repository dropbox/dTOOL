# Python Parity Benchmarks

**Purpose:** Prove Rust DashFlow is 2-10x faster than Python DashFlow via targeted benchmarks

**Status:** Phase 2 of App 5 (Python Parity)

---

## Benchmark Suite

### Benchmark 1: Basic State Graph Execution
- **Test:** Simple 5-node graph with state updates
- **Measure:** Throughput (executions/second)
- **Target:** Rust 2-5x faster
- **Status:** ⏳ Implementing

### Benchmark 2: Parallel Execution
- **Test:** 3 parallel branches with state aggregation
- **Measure:** Latency to completion
- **Target:** Rust 3-7x faster
- **Status:** ⏳ Pending

### Benchmark 3: Checkpoint Save/Load
- **Test:** Save checkpoint every 10 steps, reload at end
- **Measure:** Checkpoint overhead per step
- **Target:** Rust 5-10x faster (already measured in checkpoint_demo)
- **Status:** ⏳ Pending

### Benchmark 4: Large State Throughput
- **Test:** Pass 1MB state through 20-node graph
- **Measure:** Total execution time
- **Target:** Rust 5-10x faster
- **Status:** ⏳ Pending

---

## Running Benchmarks

### Rust Benchmarks

```bash
cd benchmarks/python_parity/rust
cargo bench
```

### Python Benchmarks

```bash
cd benchmarks/python_parity/python
pip install -r requirements.txt
pytest benchmark_*.py --benchmark-only
```

---

## Results

Results will be added here after benchmarks complete.

Expected format:

| Benchmark | Python (ops/sec) | Rust (ops/sec) | Speedup |
|-----------|-----------------|----------------|---------|
| Basic Execution | TBD | TBD | TBD |
| Parallel Execution | TBD | TBD | TBD |
| Checkpoint Overhead | TBD | TBD | TBD |
| Large State | TBD | TBD | TBD |
