#!/bin/bash
# DashTerm2 Competitive Benchmark Suite
# Run this script inside each terminal emulator

BENCHMARK_DIR="$(dirname "$0")"
RESULTS_FILE="$BENCHMARK_DIR/results_$(basename "$TERM_PROGRAM" .app 2>/dev/null || echo $TERM).txt"

echo "=== Terminal Benchmark Suite ===" | tee "$RESULTS_FILE"
echo "Terminal: ${TERM_PROGRAM:-$TERM}" | tee -a "$RESULTS_FILE"
echo "Date: $(date)" | tee -a "$RESULTS_FILE"
echo "Host: $(hostname)" | tee -a "$RESULTS_FILE"
echo "CPU: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Benchmark 1: Large file output (100MB random data)
echo "=== Benchmark 1: cat 100MB random file ===" | tee -a "$RESULTS_FILE"
if [[ -f "$BENCHMARK_DIR/100MB_random.txt" ]]; then
    START=$(python3 -c 'import time; print(time.time())')
    cat "$BENCHMARK_DIR/100MB_random.txt" > /dev/null 2>&1
    END=$(python3 -c 'import time; print(time.time())')
    ELAPSED=$(python3 -c "print(f'{$END - $START:.3f}')")
    echo "Time: ${ELAPSED}s" | tee -a "$RESULTS_FILE"
else
    echo "ERROR: 100MB_random.txt not found" | tee -a "$RESULTS_FILE"
fi
echo "" | tee -a "$RESULTS_FILE"

# Benchmark 2: Sustained throughput with yes
echo "=== Benchmark 2: yes | head -1000000 ===" | tee -a "$RESULTS_FILE"
START=$(python3 -c 'import time; print(time.time())')
yes | head -1000000 > /dev/null 2>&1
END=$(python3 -c 'import time; print(time.time())')
ELAPSED=$(python3 -c "print(f'{$END - $START:.3f}')")
echo "Time: ${ELAPSED}s" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Benchmark 3: seq for numeric output
echo "=== Benchmark 3: seq 1 1000000 ===" | tee -a "$RESULTS_FILE"
START=$(python3 -c 'import time; print(time.time())')
seq 1 1000000 > /dev/null 2>&1
END=$(python3 -c 'import time; print(time.time())')
ELAPSED=$(python3 -c "print(f'{$END - $START:.3f}')")
echo "Time: ${ELAPSED}s" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Benchmark 4: Memory usage (empty terminal)
echo "=== Benchmark 4: Memory usage ===" | tee -a "$RESULTS_FILE"
if [[ -n "$TERM_PROGRAM" ]]; then
    MEM=$(ps aux | grep -i "$(basename "$TERM_PROGRAM" .app)" | grep -v grep | head -1 | awk '{print $6}')
    echo "RSS (KB): ${MEM:-unknown}" | tee -a "$RESULTS_FILE"
else
    echo "RSS: unknown (TERM_PROGRAM not set)" | tee -a "$RESULTS_FILE"
fi
echo "" | tee -a "$RESULTS_FILE"

echo "=== Benchmark Complete ===" | tee -a "$RESULTS_FILE"
echo "Results saved to: $RESULTS_FILE"
