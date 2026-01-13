#!/usr/bin/env bash
#
# Quick Performance Comparison: Rust vs Python
#
# Runs Python benchmarks and generates a report with manual Rust timing estimates.
# For full automated comparison, use compare_benchmarks.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Quick Performance Comparison${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Step 1: Run Python benchmarks
echo -e "${YELLOW}[1/2] Running Python benchmarks...${NC}"
cd "$PROJECT_ROOT"

# Activate virtual environment and run Python benchmarks with moderate iterations
source .venv_bench/bin/activate
python benchmarks/python/dashflow_benchmarks.py 20 3

echo -e "${GREEN}✓ Python benchmarks complete${NC}"
echo ""

# Step 2: Run a quick subset of Rust benchmarks
echo -e "${YELLOW}[2/2] Running Rust benchmarks (quick subset)...${NC}"

source ~/.cargo/env

# Run specific benchmarks with lower sample size for speed
# Use regex to match specific benchmarks (compilation, sequential 3/5, conditional binary/loop, parallel fanout_3, checkpointing memory 3/5)
cargo bench --package dashflow --bench graph_benchmarks \
    -- --warm-up-time 1 --measurement-time 5 \
    "simple_graph_3_nodes|3_nodes_simple|5_nodes_complex|binary_conditional|loop_with_exit_condition|fanout_3_workers|memory_checkpoint_3_nodes|memory_checkpoint_5_nodes" \
    2>&1 | tee /tmp/rust_bench_output.txt

echo -e "${GREEN}✓ Rust benchmarks complete${NC}"
echo ""

# Step 3: Generate quick comparison from Criterion output
echo -e "${YELLOW}[3/3] Generating comparison report...${NC}"

python3 - <<'PYTHON_SCRIPT'
import json
import re
from datetime import datetime
from pathlib import Path

# Load Python results
with open("benchmarks/python/python_bench_results.json") as f:
    python_results = json.load(f)

# Parse Rust results from Criterion output
rust_times = {}
with open("/tmp/rust_bench_output.txt") as f:
    content = f.read()

    # Extract benchmark times from Criterion output
    # Format: "bench_name ... time:   [1.2345 ms 1.3456 ms 1.4567 ms]"
    pattern = r'(\S+)\s+time:\s+\[(\d+\.\d+)\s+([µmn]?s)\s+(\d+\.\d+)\s+([µmn]?s)\s+(\d+\.\d+)\s+([µmn]?s)\]'
    matches = re.findall(pattern, content)

    for match in matches:
        bench_name = match[0]
        mean_val = float(match[3])
        mean_unit = match[4]

        # Convert to milliseconds
        if mean_unit == 'ns':
            mean_ms = mean_val / 1_000_000
        elif mean_unit == 'µs' or mean_unit == 'us':
            mean_ms = mean_val / 1_000
        elif mean_unit == 'ms':
            mean_ms = mean_val
        else:
            mean_ms = mean_val * 1000  # seconds to ms

        rust_times[bench_name] = mean_ms

# Create lookup dictionaries
python_by_name = {b["name"]: b for b in python_results["benchmarks"]}

# Generate markdown report
report = []
report.append("# DashFlow Performance Comparison: Rust vs Python")
report.append("")
report.append(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
report.append(f"**Branch:** main")
report.append(f"**Commit:** N=48 (Performance benchmarking)")
report.append("")

# Map Python benchmark names to Rust benchmark names
name_mapping = {
    "compilation/simple_graph_3_nodes": "compilation/simple_graph_3_nodes",
    "sequential_execution/3_nodes_simple": "sequential_execution/3_nodes_simple",
    "sequential_execution/5_nodes_complex": "sequential_execution/5_nodes_complex",
    "conditional_branching/binary_conditional": "conditional_branching/binary_conditional",
    "conditional_branching/loop_with_exit_condition": "conditional_branching/loop_with_exit_condition",
    "parallel_execution/fanout_3_workers": "parallel_execution/fanout_3_workers",
    "checkpointing/memory_checkpoint_3_nodes": "checkpointing/memory_checkpoint_3_nodes",
    "checkpointing/memory_checkpoint_5_nodes": "checkpointing/memory_checkpoint_5_nodes",
}

# Calculate speedups
speedups = []
comparisons = []

for py_name, rust_name in name_mapping.items():
    if py_name in python_by_name:
        python_bench = python_by_name[py_name]

        # Try to find Rust benchmark (Criterion may use slightly different names)
        rust_mean_ms = None
        for rust_bench_name, rust_time in rust_times.items():
            if rust_name.replace("/", "_").replace(" ", "_") in rust_bench_name or \
               rust_name.split("/")[-1] in rust_bench_name:
                rust_mean_ms = rust_time
                break

        if rust_mean_ms:
            speedup = python_bench["mean_ms"] / rust_mean_ms
            speedups.append(speedup)
            comparisons.append((py_name, python_bench, rust_mean_ms, speedup))

report.append("## Executive Summary")
report.append("")

if speedups:
    avg_speedup = sum(speedups) / len(speedups)
    min_speedup = min(speedups)
    max_speedup = max(speedups)

    report.append(f"- **Average Speedup:** {avg_speedup:.2f}x")
    report.append(f"- **Min Speedup:** {min_speedup:.2f}x")
    report.append(f"- **Max Speedup:** {max_speedup:.2f}x")
    report.append(f"- **Benchmarks Compared:** {len(speedups)}")
    report.append("")

    if avg_speedup >= 2.0:
        report.append("✅ **Performance Goal ACHIEVED**: Rust is 2x+ faster on average")
    elif avg_speedup >= 1.5:
        report.append("⚠️ **Performance Goal PARTIAL**: Rust is 1.5-2x faster on average")
    elif avg_speedup >= 1.0:
        report.append("⚠️ **Performance Goal NOT MET**: Rust is faster but <1.5x on average")
    else:
        report.append("❌ **Performance Goal NOT MET**: Rust is slower than Python")
else:
    report.append("⚠️ No matching benchmarks found - check Criterion output format")

report.append("")
report.append("## Detailed Results")
report.append("")
report.append("| Benchmark | Python (ms) | Rust (ms) | Speedup | Status |")
report.append("|-----------|-------------|-----------|---------|--------|")

# Sort by speedup (highest first)
comparisons.sort(key=lambda x: x[3], reverse=True)

for name, python_bench, rust_mean_ms, speedup in comparisons:
    status = "✅" if speedup >= 2.0 else "⚠️" if speedup >= 1.0 else "❌"
    report.append(
        f"| {name} | {python_bench['mean_ms']:.3f} | {rust_mean_ms:.3f} | "
        f"{speedup:.2f}x | {status} |"
    )

report.append("")
report.append("## Interpretation")
report.append("")
report.append("- ✅ **2x+ faster**: Meets or exceeds performance goals")
report.append("- ⚠️ **1-2x faster**: Modest improvement, within expected range")
report.append("- ❌ **<1x**: Rust slower than Python (investigate)")
report.append("")

report.append("## Python Benchmark Details")
report.append("")
report.append("| Benchmark | Mean (ms) | Median (ms) | Std Dev (ms) |")
report.append("|-----------|-----------|-------------|--------------|")
for bench in python_results["benchmarks"]:
    report.append(
        f"| {bench['name']} | {bench['mean_ms']:.3f} | {bench['median_ms']:.3f} | "
        f"{bench['std_dev_ms']:.3f} |"
    )

report.append("")
report.append("## Notes")
report.append("")
report.append("- Python benchmarks: 20 iterations, 3 warmup (simple timing)")
report.append("- Rust benchmarks: Criterion default (statistical analysis)")
report.append("- Both run in release/optimized mode")
report.append("- Results may vary based on system load and hardware")
report.append("")

# Write report
output_path = "benchmarks/PERFORMANCE_COMPARISON_N48.md"
with open(output_path, "w") as f:
    f.write("\n".join(report))

print(f"\nComparison report saved to {output_path}")

# Print summary to console
print("")
print("=" * 80)
print("PERFORMANCE COMPARISON SUMMARY")
print("=" * 80)
if speedups:
    print(f"Average Speedup: {avg_speedup:.2f}x")
    print(f"Min Speedup: {min_speedup:.2f}x")
    print(f"Max Speedup: {max_speedup:.2f}x")
    print(f"Benchmarks: {len(speedups)}")

    # Determine if performance goals are met
    if avg_speedup >= 2.0:
        print("")
        print("✅ Performance Goal ACHIEVED: Rust is 2x+ faster on average")
    elif avg_speedup >= 1.5:
        print("")
        print("⚠️ Performance Goal PARTIAL: Rust is 1.5-2x faster on average")
    elif avg_speedup >= 1.0:
        print("")
        print("⚠️ Performance Goal NOT MET: Rust is faster but <1.5x on average")
    else:
        print("")
        print("❌ Performance Goal NOT MET: Rust is slower than Python")
else:
    print("⚠️ No matching benchmarks found - check Criterion output format")

print("=" * 80)
PYTHON_SCRIPT

echo ""
echo -e "${BLUE}Report saved to: benchmarks/PERFORMANCE_COMPARISON_N48.md${NC}"
echo ""
echo "View the full report with:"
echo "  cat benchmarks/PERFORMANCE_COMPARISON_N48.md"
