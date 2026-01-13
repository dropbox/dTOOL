#!/usr/bin/env bash
#
# Compare Python and Rust DashFlow Performance
# Portable across macOS and Linux
#
# This script runs both Python and Rust benchmarks and generates a comparison report.
#
# Prerequisites:
#   - Python 3 with langchain installed
#   - Virtual environment at .venv_bench/ OR system Python with langchain
#   - Rust toolchain with cargo
#
# Usage: ./benchmarks/compare_benchmarks.sh [iterations] [warmup]
#   iterations: Number of benchmark iterations (default: 30)
#   warmup: Number of warmup iterations (default: 3)

set -euo pipefail

ITERATIONS=${1:-30}
WARMUP=${2:-3}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PYTHON_BENCH_DIR="$SCRIPT_DIR/python"
PYTHON_RESULTS="$PYTHON_BENCH_DIR/python_bench_results.json"
RUST_RESULTS="$SCRIPT_DIR/rust_bench_results.json"
COMPARISON_REPORT="$SCRIPT_DIR/performance_comparison.md"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin) PLATFORM="macos" ;;
    Linux)  PLATFORM="linux" ;;
    *)      PLATFORM="unknown" ;;
esac

# Colors for output (check if terminal supports colors)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    BLUE='\033[0;34m'
    YELLOW='\033[1;33m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    BLUE=''
    YELLOW=''
    NC=''
fi

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}DashFlow Performance Comparison${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Platform: $PLATFORM"
echo "Configuration:"
echo "  Iterations: $ITERATIONS"
echo "  Warmup: $WARMUP"
echo ""

# Step 1: Run Python benchmarks
echo -e "${YELLOW}[1/3] Running Python benchmarks...${NC}"
cd "$PROJECT_ROOT"

# Try to activate virtual environment (multiple common locations)
VENV_ACTIVATED=false
for venv_path in ".venv_bench" ".venv" "venv" ".env"; do
    if [[ -f "$PROJECT_ROOT/$venv_path/bin/activate" ]]; then
        source "$PROJECT_ROOT/$venv_path/bin/activate"
        VENV_ACTIVATED=true
        echo "Using virtual environment: $venv_path"
        break
    fi
done

if [[ "$VENV_ACTIVATED" == "false" ]]; then
    echo "WARNING: No virtual environment found. Using system Python."
    echo "         Create one with: python3 -m venv .venv_bench && source .venv_bench/bin/activate && pip install langchain"
fi

# Check for python3
if ! command -v python3 &>/dev/null && ! command -v python &>/dev/null; then
    echo -e "${RED}Error: Python not found. Install Python 3 to run benchmarks.${NC}"
    exit 1
fi

PYTHON_CMD="python3"
if ! command -v python3 &>/dev/null; then
    PYTHON_CMD="python"
fi

$PYTHON_CMD "$PYTHON_BENCH_DIR/dashflow_benchmarks.py" "$ITERATIONS" "$WARMUP"

if [ ! -f "$PYTHON_RESULTS" ]; then
    echo -e "${RED}Error: Python benchmark results not found at $PYTHON_RESULTS${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Python benchmarks complete${NC}"
echo ""

# Step 2: Run Rust benchmarks
echo -e "${YELLOW}[2/3] Running Rust benchmarks...${NC}"

# Build Rust benchmarks in release mode
cargo build --release --package dashflow --benches

# Run criterion benchmarks and extract results
# Note: Criterion doesn't output JSON by default, so we'll parse the output
cargo bench --package dashflow --bench graph_benchmarks -- --save-baseline comparison 2>&1 | tee /tmp/rust_bench_output.txt

# Parse Criterion output and create JSON
# This is a simplified extraction - Criterion stores detailed results in target/criterion/
python3 - <<'PYTHON_SCRIPT'
import json
import re
import os
from pathlib import Path

# Read Criterion's JSON output from target/criterion/
criterion_dir = Path("target/criterion")
results = {"benchmarks": []}

# Find all benchmark directories
if criterion_dir.exists():
    for bench_group in criterion_dir.iterdir():
        if not bench_group.is_dir() or bench_group.name in ["report", ".gitignore"]:
            continue

        # Look for estimates.json in subdirectories
        for bench_dir in bench_group.iterdir():
            if not bench_dir.is_dir():
                continue

            estimates_file = bench_dir / "comparison" / "estimates.json"
            if not estimates_file.exists():
                estimates_file = bench_dir / "new" / "estimates.json"

            if estimates_file.exists():
                with open(estimates_file) as f:
                    data = json.load(f)

                # Extract mean time (in nanoseconds, convert to milliseconds)
                mean_ns = data.get("mean", {}).get("point_estimate", 0)
                std_dev_ns = data.get("std_dev", {}).get("point_estimate", 0)
                median_ns = data.get("median", {}).get("point_estimate", 0)

                # Construct benchmark name
                bench_name = f"{bench_group.name}/{bench_dir.name}"

                results["benchmarks"].append({
                    "name": bench_name,
                    "mean_ms": mean_ns / 1_000_000.0,  # ns to ms
                    "median_ms": median_ns / 1_000_000.0,
                    "std_dev_ms": std_dev_ns / 1_000_000.0,
                    "iterations": 100,  # Criterion default
                })

# Save results
output_path = "benchmarks/rust_bench_results.json"
os.makedirs(os.path.dirname(output_path), exist_ok=True)
with open(output_path, "w") as f:
    json.dump(results, f, indent=2)

print(f"Extracted {len(results['benchmarks'])} Rust benchmark results")
PYTHON_SCRIPT

if [ ! -f "$RUST_RESULTS" ]; then
    echo -e "${RED}Error: Rust benchmark results not found at $RUST_RESULTS${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Rust benchmarks complete${NC}"
echo ""

# Step 3: Generate comparison report
echo -e "${YELLOW}[3/3] Generating comparison report...${NC}"

python3 - <<'PYTHON_SCRIPT'
import json
import sys
from datetime import datetime
from pathlib import Path

# Load results
with open("benchmarks/python/python_bench_results.json") as f:
    python_results = json.load(f)

with open("benchmarks/rust_bench_results.json") as f:
    rust_results = json.load(f)

# Create lookup dictionaries
python_by_name = {b["name"]: b for b in python_results["benchmarks"]}
rust_by_name = {b["name"]: b for b in rust_results["benchmarks"]}

# Generate markdown report
report = []
report.append("# DashFlow Performance Comparison: Rust vs Python")
report.append("")
report.append(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
report.append("")
report.append("## Executive Summary")
report.append("")

# Calculate aggregate statistics
speedups = []
for name, python_bench in python_by_name.items():
    if name in rust_by_name:
        rust_bench = rust_by_name[name]
        speedup = python_bench["mean_ms"] / rust_bench["mean_ms"]
        speedups.append(speedup)

if speedups:
    avg_speedup = sum(speedups) / len(speedups)
    min_speedup = min(speedups)
    max_speedup = max(speedups)

    report.append(f"- **Average Speedup:** {avg_speedup:.2f}x")
    report.append(f"- **Min Speedup:** {min_speedup:.2f}x")
    report.append(f"- **Max Speedup:** {max_speedup:.2f}x")
    report.append(f"- **Benchmarks Compared:** {len(speedups)}")
else:
    report.append("- No matching benchmarks found for comparison")

report.append("")
report.append("## Detailed Results")
report.append("")
report.append("| Benchmark | Python (ms) | Rust (ms) | Speedup | Status |")
report.append("|-----------|-------------|-----------|---------|--------|")

# Sort by speedup (highest first)
comparisons = []
for name, python_bench in python_by_name.items():
    if name in rust_by_name:
        rust_bench = rust_by_name[name]
        speedup = python_bench["mean_ms"] / rust_bench["mean_ms"]
        comparisons.append((name, python_bench, rust_bench, speedup))

comparisons.sort(key=lambda x: x[3], reverse=True)

for name, python_bench, rust_bench, speedup in comparisons:
    status = "✅" if speedup >= 2.0 else "⚠️" if speedup >= 1.0 else "❌"
    report.append(
        f"| {name} | {python_bench['mean_ms']:.3f} | {rust_bench['mean_ms']:.3f} | "
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
report.append("## Rust Benchmark Details")
report.append("")
report.append("| Benchmark | Mean (ms) | Median (ms) | Std Dev (ms) |")
report.append("|-----------|-----------|-------------|--------------|")
for bench in rust_results["benchmarks"]:
    report.append(
        f"| {bench['name']} | {bench['mean_ms']:.3f} | {bench['median_ms']:.3f} | "
        f"{bench['std_dev_ms']:.3f} |"
    )

report.append("")
report.append("## Notes")
report.append("")
report.append("- Python benchmarks use simple timing (time.perf_counter)")
report.append("- Rust benchmarks use Criterion (statistical analysis)")
report.append("- Both run in release/optimized mode")
report.append("- Results may vary based on system load and hardware")
report.append("")

# Write report
output_path = "benchmarks/performance_comparison.md"
with open(output_path, "w") as f:
    f.write("\n".join(report))

print(f"Comparison report saved to {output_path}")

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
    elif avg_speedup >= 1.0:
        print("")
        print("⚠️ Performance Goal PARTIAL: Rust is faster but <2x on average")
    else:
        print("")
        print("❌ Performance Goal NOT MET: Rust is slower than Python")

print("=" * 80)
PYTHON_SCRIPT

echo -e "${GREEN}✓ Comparison report generated${NC}"
echo ""
echo -e "${BLUE}Report saved to: $COMPARISON_REPORT${NC}"
echo ""
echo "View the full report with:"
echo "  cat $COMPARISON_REPORT"
