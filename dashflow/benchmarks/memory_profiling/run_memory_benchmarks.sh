#!/bin/bash
# Run memory benchmarks for Rust and Python DashFlow implementations
# Portable across macOS and Linux
#
# Prerequisites:
#   - macOS: Built-in /usr/bin/time supports -l flag
#   - Linux: GNU time (install: apt-get install time OR yum install time)
#   - Python 3 with langchain installed (for Python benchmark)
#   - bc (for ratio calculation; optional)
#
# Usage: ./run_memory_benchmarks.sh [--skip-python]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RESULTS_FILE="$SCRIPT_DIR/memory_results.json"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin) PLATFORM="macos" ;;
    Linux)  PLATFORM="linux" ;;
    *)      echo "ERROR: Unsupported platform: $OS"; exit 1 ;;
esac

echo "=== DashFlow Memory Benchmark Runner ==="
echo "Platform: $PLATFORM"
echo "Project root: $PROJECT_ROOT"
echo "Results file: $RESULTS_FILE"
echo ""

# Verify prerequisites
verify_prerequisites() {
    local errors=0

    # Check for time command
    if [[ "$PLATFORM" == "macos" ]]; then
        if [[ ! -x /usr/bin/time ]]; then
            echo "ERROR: /usr/bin/time not found (required for macOS memory stats)"
            errors=$((errors + 1))
        fi
    else
        # Linux: Need GNU time, not bash builtin
        if ! command -v /usr/bin/time &>/dev/null; then
            echo "ERROR: GNU time not found. Install with: apt-get install time (Debian/Ubuntu) or yum install time (RHEL/CentOS)"
            errors=$((errors + 1))
        fi
    fi

    # Check for Python
    if ! command -v python3 &>/dev/null; then
        echo "WARNING: python3 not found. Python benchmarks will be skipped."
    fi

    # Check for bc (optional)
    if ! command -v bc &>/dev/null; then
        echo "WARNING: bc not found. Ratio calculation will use integer division."
    fi

    if [[ $errors -gt 0 ]]; then
        exit 1
    fi
}

verify_prerequisites

# Clean up previous results
rm -f "$RESULTS_FILE"

# Function to extract memory stats from /usr/bin/time output
# Returns memory in MB
extract_memory_stats() {
    local time_output="$1"
    local peak_mem_mb=0

    if [[ "$PLATFORM" == "macos" ]]; then
        # macOS: "maximum resident set size" is in bytes
        local max_rss=$(echo "$time_output" | grep "maximum resident set size" | awk '{print $1}')
        if [[ -n "$max_rss" && "$max_rss" -gt 0 ]]; then
            peak_mem_mb=$((max_rss / 1024 / 1024))
        fi
    else
        # Linux: "Maximum resident set size (kbytes)" is in KB
        local max_rss=$(echo "$time_output" | grep -i "maximum resident set size" | awk '{print $NF}')
        if [[ -n "$max_rss" && "$max_rss" -gt 0 ]]; then
            peak_mem_mb=$((max_rss / 1024))
        fi
    fi

    echo "$peak_mem_mb"
}

# Get time command with appropriate flags
get_time_cmd() {
    if [[ "$PLATFORM" == "macos" ]]; then
        echo "/usr/bin/time -l"
    else
        echo "/usr/bin/time -v"
    fi
}

# Build Rust benchmark (release mode for realistic memory usage)
echo "=== Building Rust Memory Benchmark (release mode) ==="
cd "$PROJECT_ROOT"
cargo build --release --bin memory_bench 2>&1 | tail -5
RUST_BIN="$PROJECT_ROOT/target/release/memory_bench"

if [ ! -f "$RUST_BIN" ]; then
    echo "ERROR: Rust binary not found at $RUST_BIN"
    echo "Creating standalone binary..."
    cd "$SCRIPT_DIR"

    # Create a temporary Cargo project for the standalone binary
    mkdir -p temp_rust_bench
    cd temp_rust_bench

    if [ ! -f "Cargo.toml" ]; then
        cargo init --bin --name memory_bench

        # Add dependencies
        cat >> Cargo.toml <<EOF

[dependencies]
dashflow = { path = "$PROJECT_ROOT/crates/dashflow" }
dashflow-text-splitters = { path = "$PROJECT_ROOT/crates/dashflow-text-splitters" }
serde_json = "1.0"
tokio = { version = "1.0", features = ["rt", "macros"] }
EOF
    fi

    # Copy the benchmark source
    cp "$SCRIPT_DIR/memory_bench_rust.rs" src/main.rs

    # Build release binary
    cargo build --release
    RUST_BIN="./target/release/memory_bench"
fi

echo ""

# Run Rust benchmark
TIME_CMD=$(get_time_cmd)
echo "=== Running Rust Memory Benchmark (3 iterations) ==="
RUST_MEMORY_TOTAL=0
for i in {1..3}; do
    echo "Iteration $i/3..."
    RUST_OUTPUT=$($TIME_CMD "$RUST_BIN" 2>&1)
    RUST_MEMORY=$(extract_memory_stats "$RUST_OUTPUT")
    RUST_MEMORY_TOTAL=$((RUST_MEMORY_TOTAL + RUST_MEMORY))
    echo "  Peak memory: ${RUST_MEMORY} MB"
done
RUST_MEMORY_AVG=$((RUST_MEMORY_TOTAL / 3))
echo "Average Rust memory: ${RUST_MEMORY_AVG} MB"
echo ""

# Run Python benchmark (skip if python3 not available or --skip-python flag)
SKIP_PYTHON=false
if [[ "$1" == "--skip-python" ]] || ! command -v python3 &>/dev/null; then
    SKIP_PYTHON=true
fi

PYTHON_MEMORY_AVG=0
if [[ "$SKIP_PYTHON" == "false" ]]; then
    echo "=== Running Python Memory Benchmark (3 iterations) ==="
    PYTHON_MEMORY_TOTAL=0
    for i in {1..3}; do
        echo "Iteration $i/3..."
        PYTHON_OUTPUT=$($TIME_CMD python3 "$SCRIPT_DIR/memory_bench_python.py" 2>&1)
        PYTHON_MEMORY=$(extract_memory_stats "$PYTHON_OUTPUT")
        PYTHON_MEMORY_TOTAL=$((PYTHON_MEMORY_TOTAL + PYTHON_MEMORY))
        echo "  Peak memory: ${PYTHON_MEMORY} MB"
    done
    PYTHON_MEMORY_AVG=$((PYTHON_MEMORY_TOTAL / 3))
    echo "Average Python memory: ${PYTHON_MEMORY_AVG} MB"
    echo ""
else
    echo "=== Skipping Python Memory Benchmark ==="
    echo ""
fi

# Calculate improvement (with bc fallback for decimal ratios)
calculate_ratio() {
    local python_mem="$1"
    local rust_mem="$2"
    if command -v bc &>/dev/null && [[ "$rust_mem" -gt 0 ]]; then
        echo "scale=2; $python_mem / $rust_mem" | bc
    elif [[ "$rust_mem" -gt 0 ]]; then
        echo "$((python_mem / rust_mem))"
    else
        echo "N/A"
    fi
}

if [[ "$PYTHON_MEMORY_AVG" -gt 0 && "$RUST_MEMORY_AVG" -gt 0 ]]; then
    IMPROVEMENT=$((100 - (RUST_MEMORY_AVG * 100 / PYTHON_MEMORY_AVG)))
    RATIO=$(calculate_ratio "$PYTHON_MEMORY_AVG" "$RUST_MEMORY_AVG")
else
    IMPROVEMENT="N/A"
    RATIO="N/A"
fi

# Save results to JSON
cat > "$RESULTS_FILE" <<EOF
{
  "rust_memory_mb": $RUST_MEMORY_AVG,
  "python_memory_mb": $PYTHON_MEMORY_AVG,
  "improvement_percent": $IMPROVEMENT,
  "python_to_rust_ratio": "$RATIO",
  "benchmark_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "operations": {
    "messages_created": 1000,
    "messages_cloned": 1000,
    "serializations": 1000,
    "template_renders": 1000,
    "text_splits": "~100 documents",
    "runnable_invocations": 1000,
    "tool_calls": 1000
  }
}
EOF

# Print summary
echo "=== Memory Benchmark Results ==="
echo "Rust memory:   ${RUST_MEMORY_AVG} MB"
echo "Python memory: ${PYTHON_MEMORY_AVG} MB"
echo "Improvement:   ${IMPROVEMENT}% (${RATIO}× less memory)"
echo ""
echo "Results saved to: $RESULTS_FILE"
