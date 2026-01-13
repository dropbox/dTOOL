#!/bin/bash
# Quick Terminal Performance Test
#
# Run this script inside any terminal emulator to measure its rendering performance.
# Results are saved to a JSON file that can be compared across terminals.
#
# Usage:
#   ./quick_terminal_test.sh
#
# Author: DashTerm2 Project
# Created: Iteration #147

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results/comparison"
mkdir -p "$RESULTS_DIR"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
TERM_NAME="${TERM_PROGRAM:-unknown}"
TERM_VERSION="${TERM_PROGRAM_VERSION:-unknown}"
OUTPUT_FILE="$RESULTS_DIR/${TERM_NAME}_quick_${TIMESTAMP}.json"

LONG_LINE_GENERATOR="$SCRIPT_DIR/Sources/generate_long_lines.py"

echo "=============================================="
echo "Quick Terminal Performance Test"
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
    local rounds=3

    echo -n "  $name: " >&2

    # Warmup
    eval "$cmd" >/dev/null 2>&1 || true

    local sum=0 min=999999999 max=0 times=""

    for ((i=0; i<rounds; i++)); do
        local ms=$(time_cmd "$cmd >/dev/null 2>&1")
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
    # Return JSON only on stdout - ensure proper decimal format
    # bc outputs ".123" for values < 1, we need "0.123" for valid JSON
    mean=$(echo "$mean" | sed 's/^\./0./')
    stddev=$(echo "$stddev" | sed 's/^\./0./')
    min=$(echo "$min" | sed 's/^\./0./')
    max=$(echo "$max" | sed 's/^\./0./')
    echo "{\"name\":\"$name\",\"mean_ms\":$mean,\"stddev_ms\":$stddev,\"min_ms\":$min,\"max_ms\":$max}"
}

echo "Running benchmarks..." >&2
echo "" >&2

RESULTS=""

# Raw throughput
echo "=== Raw Throughput ===" >&2
R=$(run_bench "yes_100k" "yes | head -100000")
RESULTS="$R"

R=$(run_bench "seq_50k" "seq 1 50000")
RESULTS="$RESULTS,$R"

R=$(run_bench "cat_1MB" "head -c 1000000 /dev/zero")
RESULTS="$RESULTS,$R"

echo "" >&2

# ANSI colors
echo "=== ANSI Escape Sequences ===" >&2
R=$(run_bench "256color" "for i in \$(seq 0 10); do for c in \$(seq 0 255); do printf '\033[38;5;%sm#' \$c; done; echo; done")
RESULTS="$RESULTS,$R"

R=$(run_bench "truecolor" "for i in \$(seq 0 10); do for r in \$(seq 0 8 255); do printf '\033[38;2;%s;0;0m#' \$r; done; printf '\033[0m\n'; done")
RESULTS="$RESULTS,$R"

echo "" >&2

# Unicode
echo "=== Unicode ===" >&2
R=$(run_bench "cjk" "for i in \$(seq 1 5); do for j in \$(seq 1 100); do echo '漢字テスト한국어測試'; done; done")
RESULTS="$RESULTS,$R"

R=$(run_bench "emoji" "for i in \$(seq 1 5); do for j in \$(seq 1 100); do echo '🎉🚀💻🔥✨🌟🎯🏆💡🔧'; done; done")
RESULTS="$RESULTS,$R"

echo "" >&2

# Long lines (if generator available)
if [[ -f "$LONG_LINE_GENERATOR" ]]; then
    echo "=== Long Lines ===" >&2
    R=$(run_bench "long_500x1000" "python3 \"$LONG_LINE_GENERATOR\" --lines 1000 --columns 500")
    RESULTS="$RESULTS,$R"

    R=$(run_bench "long_2000x1000" "python3 \"$LONG_LINE_GENERATOR\" --lines 1000 --columns 2000")
    RESULTS="$RESULTS,$R"
    echo "" >&2
fi

# Write JSON
cat > "$OUTPUT_FILE" << EOF
{
  "benchmark_type": "quick_terminal_test",
  "timestamp": "$TIMESTAMP",
  "terminal": {"name": "$TERM_NAME", "version": "$TERM_VERSION"},
  "system": {"chip": "$CHIP", "cores": $CORES, "memory": "${MEMORY_GB}GB"},
  "results": [$RESULTS]
}
EOF

echo "=============================================="
echo "Complete! Results: $OUTPUT_FILE"
echo "=============================================="
