#!/bin/bash
# Long Line Sweep Benchmark
#
# Measures terminal rendering performance across different line widths and counts.
# Run this INSIDE the terminal emulator you want to benchmark.
#
# Usage:
#   ./long_line_sweep_benchmark.sh [--quick|--full|--stress]
#
# Author: DashTerm2 Project
# Created: Iteration #147

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

mkdir -p "$RESULTS_DIR"

# Parse mode
MODE="${1:---quick}"

case "$MODE" in
    --quick|quick)
        MODE="quick"
        WIDTHS="100,500,1000,2000"
        LINE_COUNTS="500,1000"
        ITERATIONS=3
        ;;
    --full|full)
        MODE="full"
        WIDTHS="100,500,1000,2000,5000"
        LINE_COUNTS="500,1000,5000,10000"
        ITERATIONS=5
        ;;
    --stress|stress)
        MODE="stress"
        WIDTHS="100,500,1000,2000,5000,10000"
        LINE_COUNTS="1000,5000,10000,50000"
        ITERATIONS=5
        ;;
    --help|-h)
        echo "Usage: $0 [--quick|--full|--stress]"
        echo ""
        echo "Modes:"
        echo "  --quick   Quick sweep (~1 minute)"
        echo "  --full    Full sweep (~5 minutes)"
        echo "  --stress  Stress test (~15 minutes)"
        exit 0
        ;;
    *)
        echo "Unknown mode: $MODE"
        exit 1
        ;;
esac

OUTPUT_FILE="$RESULTS_DIR/long_line_sweep_${TIMESTAMP}.json"
LONG_LINE_GENERATOR="$SCRIPT_DIR/Sources/generate_long_lines.py"

if [[ ! -f "$LONG_LINE_GENERATOR" ]]; then
    echo "Error: Missing generator script: $LONG_LINE_GENERATOR"
    exit 1
fi

# Terminal info
TERM_NAME="${TERM_PROGRAM:-unknown}"
TERM_VERSION="${TERM_PROGRAM_VERSION:-unknown}"

# System info
CHIP=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
CORES=$(sysctl -n hw.ncpu 2>/dev/null || echo "0")
MEMSIZE=$(sysctl -n hw.memsize 2>/dev/null || echo "0")
MEMORY_GB=$((MEMSIZE / 1024 / 1024 / 1024))

echo "=============================================="
echo "DashTerm2 Long Line Sweep Benchmark"
echo "Mode: $MODE"
echo "Timestamp: $TIMESTAMP"
echo "Terminal: $TERM_NAME ($TERM_VERSION)"
echo "System: $CHIP ($CORES cores, ${MEMORY_GB}GB)"
echo "=============================================="
echo ""
echo "Widths: $WIDTHS"
echo "Line counts: $LINE_COUNTS"
echo "Iterations: $ITERATIONS"
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

# Parse comma-separated list
IFS=',' read -ra WIDTH_ARR <<< "$WIDTHS"
IFS=',' read -ra COUNT_ARR <<< "$LINE_COUNTS"

# Initialize JSON
cat > "$OUTPUT_FILE" << EOF
{
  "benchmark_type": "long_line_sweep",
  "timestamp": "$TIMESTAMP",
  "mode": "$MODE",
  "terminal": {"name": "$TERM_NAME", "version": "$TERM_VERSION"},
  "system": {"chip": "$CHIP", "cores": $CORES, "memory": "${MEMORY_GB}GB"},
  "parameters": {"widths": [$WIDTHS], "line_counts": [$LINE_COUNTS], "iterations": $ITERATIONS},
  "results": [
EOF

FIRST_RESULT=true

echo "Running benchmark sweep..."
echo ""
printf "%-12s %-12s %12s %12s %12s %12s\n" "Width" "Lines" "Mean (ms)" "Stddev" "Min" "Max"
echo "------------------------------------------------------------------------"

for width in "${WIDTH_ARR[@]}"; do
    for line_count in "${COUNT_ARR[@]}"; do
        # Run benchmark
        times=""
        sum=0
        min=999999999
        max=0

        # Warmup - output to terminal (what we want to measure)
        python3 "$LONG_LINE_GENERATOR" --lines 100 --columns 100 >/dev/tty 2>/dev/null || true

        for ((i=0; i<ITERATIONS; i++)); do
            # Output to terminal via /dev/tty to measure actual rendering
            ms=$(time_cmd "python3 \"$LONG_LINE_GENERATOR\" --lines $line_count --columns $width >/dev/tty 2>/dev/null")
            times="$times $ms"
            sum=$(echo "$sum + $ms" | bc)
            if (( $(echo "$ms < $min" | bc -l) )); then min="$ms"; fi
            if (( $(echo "$ms > $max" | bc -l) )); then max="$ms"; fi
        done

        mean=$(echo "scale=3; $sum / $ITERATIONS" | bc)

        sq_sum=0
        for t in $times; do
            diff=$(echo "$t - $mean" | bc)
            sq_sum=$(echo "$sq_sum + ($diff * $diff)" | bc)
        done
        stddev=$(echo "scale=3; sqrt($sq_sum / $ITERATIONS)" | bc)

        printf "%-12s %-12s %12.3f %12.3f %12.3f %12.3f\n" "$width" "$line_count" "$mean" "$stddev" "$min" "$max"

        # Append to JSON
        if [[ "$FIRST_RESULT" == "true" ]]; then
            FIRST_RESULT=false
        else
            echo "," >> "$OUTPUT_FILE"
        fi

        total_bytes=$((width * line_count))
        throughput_mbs=$(echo "scale=1; ($total_bytes / 1000000) / ($mean / 1000)" | bc 2>/dev/null || echo "0")

        cat >> "$OUTPUT_FILE" << EOF
    {"width": $width, "lines": $line_count, "mean_ms": $mean, "stddev_ms": $stddev, "min_ms": $min, "max_ms": $max, "total_bytes": $total_bytes, "throughput_mbs": $throughput_mbs}
EOF
    done
done

# Close JSON
cat >> "$OUTPUT_FILE" << EOF

  ]
}
EOF

echo ""
echo "=============================================="
echo "Benchmark complete!"
echo "Results: $OUTPUT_FILE"
echo "=============================================="

# Show summary
echo ""
echo "Summary: Throughput by configuration"
echo ""
if command -v jq &>/dev/null; then
    jq -r '.results[] | "\(.width)x\(.lines): \(.throughput_mbs) MB/s (\(.mean_ms)ms)"' "$OUTPUT_FILE" 2>/dev/null || cat "$OUTPUT_FILE"
fi
