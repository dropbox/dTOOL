#!/bin/bash
# End-to-End Real Workload Benchmark for Terminal Emulators
#
# This script measures actual terminal throughput for realistic workloads.
# Run this INSIDE the terminal emulator you want to benchmark.
#
# Usage:
#   ./e2e_workload_benchmark.sh [--quick|--full|--stress]
#
# Author: DashTerm2 Project
# Created: Iteration #145

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
mkdir -p "$RESULTS_DIR"

# Benchmark parameters
MODE="${1:-quick}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
OUTPUT_FILE="$RESULTS_DIR/e2e_workload_${TIMESTAMP}.json"

# Test file paths
TEST_100MB="$SCRIPT_DIR/100MB_random.txt"
LONG_LINE_GENERATOR="$SCRIPT_DIR/Sources/generate_long_lines.py"
LONG_LINE_COUNT=1000
LONG_LINE_WIDTH=500

if [[ ! -f "$LONG_LINE_GENERATOR" ]]; then
    echo "Missing long line generator: $LONG_LINE_GENERATOR" >&2
    exit 1
fi

# Set parameters based on mode
case "$MODE" in
    --quick|quick)
        MODE="quick"
        YES_LINES=100000
        SEQ_COUNT=50000
        CAT_BYTES=1000000
        COLOR_ITERATIONS=10
        UNICODE_ITERATIONS=5
        TEST_ROUNDS=3
        ;;
    --full|full)
        MODE="full"
        YES_LINES=500000
        SEQ_COUNT=200000
        CAT_BYTES=10000000
        COLOR_ITERATIONS=50
        UNICODE_ITERATIONS=20
        TEST_ROUNDS=5
        ;;
    --stress|stress)
        MODE="stress"
        YES_LINES=2000000
        SEQ_COUNT=1000000
        CAT_BYTES=100000000
        COLOR_ITERATIONS=200
        UNICODE_ITERATIONS=100
        TEST_ROUNDS=10
        ;;
    --help|-h)
        echo "Usage: $0 [--quick|--full|--stress]"
        echo ""
        echo "Modes:"
        echo "  --quick   Fast benchmark (~30 seconds)"
        echo "  --full    Standard benchmark (~2 minutes)"
        echo "  --stress  Extended stress test (~5 minutes)"
        exit 0
        ;;
    *)
        MODE="quick"
        YES_LINES=100000
        SEQ_COUNT=50000
        CAT_BYTES=1000000
        COLOR_ITERATIONS=10
        UNICODE_ITERATIONS=5
        TEST_ROUNDS=3
        ;;
esac

echo "=========================================="
echo "DashTerm2 End-to-End Workload Benchmark"
echo "Mode: $MODE"
echo "Timestamp: $TIMESTAMP"
echo "=========================================="
echo ""

# Detect terminal
TERM_NAME="${TERM_PROGRAM:-unknown}"
TERM_VERSION="${TERM_PROGRAM_VERSION:-unknown}"
echo "Terminal: $TERM_NAME (v$TERM_VERSION)"
echo ""

# Get system info
CHIP=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
CORES=$(sysctl -n hw.ncpu 2>/dev/null || echo "0")
MEMSIZE=$(sysctl -n hw.memsize 2>/dev/null || echo "0")
MEMORY_GB=$((MEMSIZE / 1024 / 1024 / 1024))
echo "System: $CHIP ($CORES cores, ${MEMORY_GB}GB)"
echo ""

# Simple timing function using perl
time_cmd() {
    perl -MTime::HiRes=time -e '
        my $cmd = shift;
        my $start = time;
        system($cmd);
        my $end = time;
        printf "%.3f", ($end - $start) * 1000;
    ' -- "$1"
}

# Run a single benchmark and output result
run_single_benchmark() {
    local name="$1"
    local cmd="$2"
    local rounds="$3"

    echo -n "  $name: "

    # Run warmup
    eval "$cmd" >/dev/null 2>&1 || true

    # Collect times
    local sum=0
    local min=999999999
    local max=0
    local times=""

    for ((i=0; i<rounds; i++)); do
        local ms=$(time_cmd "$cmd >/dev/null 2>&1")
        times="$times $ms"
        # Use awk for floating point comparison
        sum=$(echo "$sum + $ms" | bc)
        if (( $(echo "$ms < $min" | bc -l) )); then min="$ms"; fi
        if (( $(echo "$ms > $max" | bc -l) )); then max="$ms"; fi
    done

    local mean=$(echo "scale=3; $sum / $rounds" | bc)

    # Calculate stddev
    local sq_sum=0
    for t in $times; do
        local diff=$(echo "$t - $mean" | bc)
        sq_sum=$(echo "$sq_sum + ($diff * $diff)" | bc)
    done
    local stddev=$(echo "scale=3; sqrt($sq_sum / $rounds)" | bc)

    echo "${mean}ms (stddev: ${stddev}ms)"

    # Return JSON fragment
    echo "{\"name\": \"$name\", \"mean_ms\": $mean, \"stddev_ms\": $stddev, \"min_ms\": $min, \"max_ms\": $max}" >> "$OUTPUT_FILE.tmp"
}

# Initialize JSON
echo "{" > "$OUTPUT_FILE"
echo "  \"benchmark_type\": \"e2e_workload\"," >> "$OUTPUT_FILE"
echo "  \"timestamp\": \"$TIMESTAMP\"," >> "$OUTPUT_FILE"
echo "  \"mode\": \"$MODE\"," >> "$OUTPUT_FILE"
echo "  \"terminal\": {\"name\": \"$TERM_NAME\", \"version\": \"$TERM_VERSION\"}," >> "$OUTPUT_FILE"
echo "  \"system\": {\"chip\": \"$CHIP\", \"cores\": $CORES, \"memory\": \"${MEMORY_GB}GB\"}," >> "$OUTPUT_FILE"
echo "  \"results\": [" >> "$OUTPUT_FILE"

rm -f "$OUTPUT_FILE.tmp"

echo "Running benchmarks..."
echo ""

# ============================================
# 1. RAW THROUGHPUT TESTS
# ============================================
echo "=== Raw Throughput ==="

run_single_benchmark "yes_lines_$YES_LINES" "yes | head -$YES_LINES" "$TEST_ROUNDS"

run_single_benchmark "seq_$SEQ_COUNT" "seq 1 $SEQ_COUNT" "$TEST_ROUNDS"

run_single_benchmark "cat_zero_${CAT_BYTES}B" "head -c $CAT_BYTES /dev/zero" "$TEST_ROUNDS"

# Test: cat random text file (if available)
if [[ -f "$TEST_100MB" ]]; then
    if [[ "$MODE" == "stress" ]]; then
        run_single_benchmark "cat_100MB_random" "cat '$TEST_100MB'" "2"
    else
        run_single_benchmark "cat_random_1MB" "head -c 1000000 '$TEST_100MB'" "$TEST_ROUNDS"
    fi
fi

echo ""

# ============================================
# 2. COLOR/ANSI ESCAPE SEQUENCE TESTS
# ============================================
echo "=== ANSI Escape Sequences ==="

run_single_benchmark "256color_x$COLOR_ITERATIONS" \
    "for i in \$(seq 0 $COLOR_ITERATIONS); do for c in \$(seq 0 255); do printf '\033[38;5;%sm#' \$c; done; echo; done" \
    "$TEST_ROUNDS"

run_single_benchmark "truecolor_x$COLOR_ITERATIONS" \
    "for i in \$(seq 0 $COLOR_ITERATIONS); do for r in \$(seq 0 8 255); do printf '\033[38;2;%s;0;0m#' \$r; done; printf '\033[0m\n'; done" \
    "$TEST_ROUNDS"

echo ""

# ============================================
# 3. UNICODE RENDERING TESTS
# ============================================
echo "=== Unicode Rendering ==="

run_single_benchmark "cjk_x$UNICODE_ITERATIONS" \
    "for i in \$(seq 1 $UNICODE_ITERATIONS); do for j in \$(seq 1 100); do echo '漢字テスト한국어測試'; done; done" \
    "$TEST_ROUNDS"

run_single_benchmark "emoji_x$UNICODE_ITERATIONS" \
    "for i in \$(seq 1 $UNICODE_ITERATIONS); do for j in \$(seq 1 100); do echo '🎉🚀💻🔥✨🌟🎯🏆💡🔧'; done; done" \
    "$TEST_ROUNDS"

run_single_benchmark "boxdraw_x$UNICODE_ITERATIONS" \
    "for i in \$(seq 1 $UNICODE_ITERATIONS); do for j in \$(seq 1 100); do echo '╔═══════════════════════╗║│├┼┤└┴┘'; done; done" \
    "$TEST_ROUNDS"

echo ""

# ============================================
# 4. SCROLLBACK STRESS TESTS
# ============================================
echo "=== Scrollback Stress ==="

run_single_benchmark "long_lines_x$LONG_LINE_COUNT" \
    "python3 \"$LONG_LINE_GENERATOR\" --lines $LONG_LINE_COUNT --columns $LONG_LINE_WIDTH" \
    "$TEST_ROUNDS"

run_single_benchmark "rapid_output_x5000" \
    "for i in \$(seq 1 5000); do echo 'Line number \$i with some padding text here'; done" \
    "$TEST_ROUNDS"

echo ""

# ============================================
# FINALIZE JSON
# ============================================

# Combine results into JSON array
if [[ -f "$OUTPUT_FILE.tmp" ]]; then
    FIRST=true
    while IFS= read -r line; do
        if [[ "$FIRST" == "true" ]]; then
            echo "    $line" >> "$OUTPUT_FILE"
            FIRST=false
        else
            echo "    ,$line" >> "$OUTPUT_FILE"
        fi
    done < "$OUTPUT_FILE.tmp"
    rm -f "$OUTPUT_FILE.tmp"
fi

echo "  ]" >> "$OUTPUT_FILE"
echo "}" >> "$OUTPUT_FILE"

# Compute summary statistics
echo "=========================================="
echo "Benchmark complete!"
echo "Results: $OUTPUT_FILE"
echo ""

# Show key metrics
if command -v jq >/dev/null 2>&1; then
    echo "Summary (mean times):"
    jq -r '.results[] | "  \(.name): \(.mean_ms)ms"' "$OUTPUT_FILE" 2>/dev/null || cat "$OUTPUT_FILE"
fi

echo "=========================================="
