#!/bin/bash
# Terminal rendering benchmark - run this INSIDE each terminal
# Usage: ./terminal_benchmark.sh [output_suffix]

SUFFIX="${1:-$(date +%H%M%S)}"
BENCHMARK_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTPUT_FILE="$BENCHMARK_DIR/results_${SUFFIX}.txt"

echo "=== Terminal Rendering Benchmark ===" | tee "$OUTPUT_FILE"
echo "Terminal: ${TERM_PROGRAM:-$TERM}" | tee -a "$OUTPUT_FILE"
echo "Date: $(date)" | tee -a "$OUTPUT_FILE"
echo "" | tee -a "$OUTPUT_FILE"

# Test 1: yes throughput with actual rendering
echo "--- Test 1: yes | head -1000000 (rendered) ---" | tee -a "$OUTPUT_FILE"
START=$(python3 -c 'import time; print(time.time())')
yes | head -1000000
END=$(python3 -c 'import time; print(time.time())')
ELAPSED=$(python3 -c "print(f'{$END - $START:.3f}')")
echo "Time: ${ELAPSED}s" | tee -a "$OUTPUT_FILE"
echo "" | tee -a "$OUTPUT_FILE"

# Test 2: seq throughput
echo "--- Test 2: seq 1 500000 (rendered) ---" | tee -a "$OUTPUT_FILE"
START=$(python3 -c 'import time; print(time.time())')
seq 1 500000
END=$(python3 -c 'import time; print(time.time())')
ELAPSED=$(python3 -c "print(f'{$END - $START:.3f}')")
echo "Time: ${ELAPSED}s" | tee -a "$OUTPUT_FILE"
echo "" | tee -a "$OUTPUT_FILE"

# Test 3: Memory after scrollback generation
echo "--- Test 3: Memory Usage ---" | tee -a "$OUTPUT_FILE"
if [[ -n "$TERM_PROGRAM" ]]; then
    APPNAME=$(basename "$TERM_PROGRAM" .app)
    MEM=$(ps aux | grep -i "$APPNAME" | grep -v grep | awk '{sum += $6} END {print sum}')
    echo "RSS: ${MEM:-unknown} KB" | tee -a "$OUTPUT_FILE"
fi
echo "" | tee -a "$OUTPUT_FILE"

echo "=== Complete ===" | tee -a "$OUTPUT_FILE"
echo "Results saved to: $OUTPUT_FILE"
