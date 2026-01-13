#!/bin/bash
# Run memory benchmarks for Rust and Python DashFlow implementations
# Portable across macOS and Linux
#
# Prerequisites:
#   - macOS: Built-in /usr/bin/time supports -l flag
#   - Linux: GNU time (install: apt-get install time OR yum install time)
#   - bc (optional, for decimal ratio calculation)
#
# Usage: ./run_benchmarks.sh [--skip-python]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin) PLATFORM="macos" ;;
    Linux)  PLATFORM="linux" ;;
    *)      echo "ERROR: Unsupported platform: $OS"; exit 1 ;;
esac

echo "=== DashFlow Memory Benchmark Runner ==="
echo "Platform: $PLATFORM"
echo ""

# Get time command with appropriate flags
get_time_cmd() {
    if [[ "$PLATFORM" == "macos" ]]; then
        echo "/usr/bin/time -l"
    else
        echo "/usr/bin/time -v"
    fi
}

# Extract memory in MB from time output
extract_memory_mb() {
    local time_output="$1"
    if [[ "$PLATFORM" == "macos" ]]; then
        # macOS: "maximum resident set size" is in bytes
        local max_rss=$(echo "$time_output" | grep "maximum resident set size" | awk '{print $1}')
        echo "$((max_rss / 1024 / 1024))"
    else
        # Linux: "Maximum resident set size (kbytes)" is in KB
        local max_rss=$(echo "$time_output" | grep -i "maximum resident set size" | awk '{print $NF}')
        echo "$((max_rss / 1024))"
    fi
}

TIME_CMD=$(get_time_cmd)

# Run Rust benchmark (3 times)
echo "=== Running Rust Memory Benchmark (3 iterations) ==="
RUST_BIN="$PROJECT_ROOT/target/release/memory_bench"
if [ ! -f "$RUST_BIN" ]; then
    echo "ERROR: Rust binary not found. Run: cargo build --release -p memory_bench"
    exit 1
fi

RUST_TOTAL=0
for i in {1..3}; do
    echo "Iteration $i/3..."
    OUTPUT=$($TIME_CMD "$RUST_BIN" 2>&1)
    MEM_MB=$(extract_memory_mb "$OUTPUT")
    echo "  Peak memory: ${MEM_MB} MB"
    RUST_TOTAL=$((RUST_TOTAL + MEM_MB))
done
RUST_AVG=$((RUST_TOTAL / 3))
echo "Average Rust memory: ${RUST_AVG} MB"
echo ""

# Run Python benchmark (3 times) - skip if --skip-python or python3 unavailable
SKIP_PYTHON=false
if [[ "$1" == "--skip-python" ]] || ! command -v python3 &>/dev/null; then
    SKIP_PYTHON=true
fi

PYTHON_AVG=0
if [[ "$SKIP_PYTHON" == "false" ]]; then
    echo "=== Running Python Memory Benchmark (3 iterations) ==="
    PYTHON_TOTAL=0
    for i in {1..3}; do
        echo "Iteration $i/3..."
        OUTPUT=$($TIME_CMD python3 "$SCRIPT_DIR/memory_bench_python.py" 2>&1)
        MEM_MB=$(extract_memory_mb "$OUTPUT")
        echo "  Peak memory: ${MEM_MB} MB"
        PYTHON_TOTAL=$((PYTHON_TOTAL + MEM_MB))
    done
    PYTHON_AVG=$((PYTHON_TOTAL / 3))
    echo "Average Python memory: ${PYTHON_AVG} MB"
    echo ""
else
    echo "=== Skipping Python Memory Benchmark ==="
    echo ""
fi

# Calculate improvement
if [[ "$PYTHON_AVG" -gt 0 && "$RUST_AVG" -gt 0 ]]; then
    IMPROVEMENT=$((100 - (RUST_AVG * 100 / PYTHON_AVG)))
    if command -v bc &>/dev/null; then
        RATIO=$(echo "scale=2; $PYTHON_AVG / $RUST_AVG" | bc)
    else
        RATIO="$((PYTHON_AVG / RUST_AVG))"
    fi
else
    IMPROVEMENT="N/A"
    RATIO="N/A"
fi

# Print summary
echo "=== Memory Benchmark Results ==="
echo "Rust memory:   ${RUST_AVG} MB"
if [[ "$SKIP_PYTHON" == "false" ]]; then
    echo "Python memory: ${PYTHON_AVG} MB"
    echo "Improvement:   ${IMPROVEMENT}% reduction (${RATIO}× less memory)"
fi
