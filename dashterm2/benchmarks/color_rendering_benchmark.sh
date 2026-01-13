#!/bin/bash
# Color Rendering Benchmark
#
# Measures terminal color escape sequence rendering performance by using
# pre-generated test files to eliminate shell/generator overhead.
#
# This provides a more accurate measurement of terminal rendering performance
# than running shell loops which are dominated by shell execution overhead.
#
# Usage:
#   ./color_rendering_benchmark.sh
#
# Author: DashTerm2 Project
# Created: Iteration #148

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
TEMP_DIR="/tmp/dashterm_color_bench"
mkdir -p "$RESULTS_DIR" "$TEMP_DIR"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
TERM_NAME="${TERM_PROGRAM:-unknown}"
TERM_VERSION="${TERM_PROGRAM_VERSION:-unknown}"
OUTPUT_FILE="$RESULTS_DIR/color_rendering_${TIMESTAMP}.json"

GENERATOR="$SCRIPT_DIR/Sources/generate_color_test.py"

echo "=============================================="
echo "Color Rendering Benchmark"
echo "Terminal: $TERM_NAME ($TERM_VERSION)"
echo "Timestamp: $TIMESTAMP"
echo "=============================================="
echo ""

# System info
CHIP=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
CORES=$(sysctl -n hw.ncpu 2>/dev/null || echo "0")
MEMSIZE=$(sysctl -n hw.memsize 2>/dev/null || echo "0")
MEMORY_GB=$((MEMSIZE / 1024 / 1024 / 1024))

echo "System: $CHIP ($CORES cores, ${MEMORY_GB}GB)"
echo ""

# High-precision timing
time_cmd() {
    perl -MTime::HiRes=time -e '
        my $cmd = shift;
        my $start = time;
        system($cmd);
        my $end = time;
        printf "%.3f", ($end - $start) * 1000;
    ' -- "$1"
}

run_bench() {
    local name="$1"
    local cmd="$2"
    local rounds=5

    echo -n "  $name: " >&2

    # Warmup
    eval "$cmd" >/dev/null 2>&1 || true

    local sum=0 min=999999999 max=0 times=""

    for ((i=0; i<rounds; i++)); do
        local ms=$(time_cmd "$cmd")
        times="$times $ms"
        sum=$(echo "$sum + $ms" | bc)
        if (( $(echo "$ms < $min" | bc -l) )); then min="$ms"; fi
        if (( $(echo "$ms > $max" | bc -l) )); then max="$ms"; fi
    done

    local mean=$(echo "scale=3; $sum / $rounds" | bc)

    local sq_sum=0
    for t in $times; do
        local diff=$(echo "$t - $mean" | bc)
        sq_sum=$(echo "$sq_sum + ($diff * $diff)" | bc)
    done
    local stddev=$(echo "scale=3; sqrt($sq_sum / $rounds)" | bc)

    echo "${mean}ms (stddev: ${stddev}ms)" >&2

    # Fix bc decimal format for JSON
    mean=$(echo "$mean" | sed 's/^\./0./')
    stddev=$(echo "$stddev" | sed 's/^\./0./')
    min=$(echo "$min" | sed 's/^\./0./')
    max=$(echo "$max" | sed 's/^\./0./')
    echo "{\"name\":\"$name\",\"mean_ms\":$mean,\"stddev_ms\":$stddev,\"min_ms\":$min,\"max_ms\":$max}"
}

# Generate test files
echo "Generating test files..." >&2
python3 "$GENERATOR" --mode 256 --lines 11 --file "$TEMP_DIR/color_256_11.txt"
python3 "$GENERATOR" --mode 24bit --lines 11 --file "$TEMP_DIR/color_24bit_11.txt"
python3 "$GENERATOR" --mode mixed --lines 11 --file "$TEMP_DIR/color_mixed_11.txt"
python3 "$GENERATOR" --mode 256 --lines 100 --file "$TEMP_DIR/color_256_100.txt"
python3 "$GENERATOR" --mode 24bit --lines 100 --file "$TEMP_DIR/color_24bit_100.txt"

# Also generate plain text baseline (no escape sequences)
head -c 32637 /dev/zero | tr '\0' '#' | fold -w 2963 > "$TEMP_DIR/plain_11.txt"
head -c 326370 /dev/zero | tr '\0' '#' | fold -w 2963 > "$TEMP_DIR/plain_100.txt"

echo "" >&2
echo "Running benchmarks..." >&2
echo "" >&2

RESULTS=""

# Baseline: plain text (no escape sequences)
echo "=== Baseline (Plain Text) ===" >&2
R=$(run_bench "plain_11_lines" "cat $TEMP_DIR/plain_11.txt")
RESULTS="$R"

R=$(run_bench "plain_100_lines" "cat $TEMP_DIR/plain_100.txt")
RESULTS="$RESULTS,$R"
echo "" >&2

# 256-color rendering
echo "=== 256-Color Rendering ===" >&2
R=$(run_bench "256color_11_lines" "cat $TEMP_DIR/color_256_11.txt")
RESULTS="$RESULTS,$R"

R=$(run_bench "256color_100_lines" "cat $TEMP_DIR/color_256_100.txt")
RESULTS="$RESULTS,$R"
echo "" >&2

# 24-bit color rendering
echo "=== 24-Bit Color Rendering ===" >&2
R=$(run_bench "24bit_11_lines" "cat $TEMP_DIR/color_24bit_11.txt")
RESULTS="$RESULTS,$R"

R=$(run_bench "24bit_100_lines" "cat $TEMP_DIR/color_24bit_100.txt")
RESULTS="$RESULTS,$R"
echo "" >&2

# Mixed color rendering
echo "=== Mixed Color Rendering ===" >&2
R=$(run_bench "mixed_11_lines" "cat $TEMP_DIR/color_mixed_11.txt")
RESULTS="$RESULTS,$R"
echo "" >&2

# Compare with generator overhead
echo "=== Generator Overhead Comparison ===" >&2
R=$(run_bench "gen_256_11" "python3 $GENERATOR --mode 256 --lines 11")
RESULTS="$RESULTS,$R"

R=$(run_bench "gen_24bit_11" "python3 $GENERATOR --mode 24bit --lines 11")
RESULTS="$RESULTS,$R"

R=$(run_bench "shell_256_11" "for i in \$(seq 0 10); do for c in \$(seq 0 255); do printf '\033[38;5;%sm#' \$c; done; echo; done")
RESULTS="$RESULTS,$R"
echo "" >&2

# Write JSON
cat > "$OUTPUT_FILE" << EOF
{
  "benchmark_type": "color_rendering",
  "timestamp": "$TIMESTAMP",
  "terminal": {"name": "$TERM_NAME", "version": "$TERM_VERSION"},
  "system": {"chip": "$CHIP", "cores": $CORES, "memory": "${MEMORY_GB}GB"},
  "results": [$RESULTS]
}
EOF

# Calculate and display analysis
echo "=== Analysis ===" >&2

# Extract values for analysis using grep/sed
plain_11=$(grep -o '"plain_11_lines".*"mean_ms":[0-9.]*' "$OUTPUT_FILE" | grep -o 'mean_ms":[0-9.]*' | cut -d: -f2)
color_256_11=$(grep -o '"256color_11_lines".*"mean_ms":[0-9.]*' "$OUTPUT_FILE" | grep -o 'mean_ms":[0-9.]*' | cut -d: -f2)
color_24bit_11=$(grep -o '"24bit_11_lines".*"mean_ms":[0-9.]*' "$OUTPUT_FILE" | grep -o 'mean_ms":[0-9.]*' | cut -d: -f2)

echo "Plain text baseline (11 lines): ${plain_11}ms" >&2
echo "256-color rendering (11 lines): ${color_256_11}ms" >&2
echo "24-bit color rendering (11 lines): ${color_24bit_11}ms" >&2

if [[ -n "$plain_11" && -n "$color_256_11" ]]; then
    overhead=$(echo "scale=2; $color_256_11 - $plain_11" | bc)
    echo "Color escape overhead: ${overhead}ms" >&2
fi

echo "" >&2
echo "=============================================="
echo "Complete! Results: $OUTPUT_FILE"
echo "=============================================="

# Cleanup
rm -rf "$TEMP_DIR"
