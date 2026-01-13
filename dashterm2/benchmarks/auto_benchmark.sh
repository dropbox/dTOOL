#!/bin/bash
# Automated terminal benchmark runner using osascript
# Runs a simple timing benchmark in each terminal

BENCHMARK_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_FILE="$BENCHMARK_DIR/competitive_results.txt"

echo "=== Automated Competitive Benchmark ===" | tee "$RESULTS_FILE"
echo "Date: $(date)" | tee -a "$RESULTS_FILE"
echo "Host: $(hostname)" | tee -a "$RESULTS_FILE"
echo "CPU: $(sysctl -n machdep.cpu.brand_string)" | tee -a "$RESULTS_FILE"
echo "Memory: $(sysctl -n hw.memsize | awk '{print $0/1024/1024/1024 " GB"}')" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Simple shell-only benchmark for baseline comparison
run_benchmark() {
    local name="$1"
    local result_file="$BENCHMARK_DIR/result_${name}.txt"

    # Run time command and capture output
    { time (yes | head -500000 > /dev/null) ; } 2>&1 | grep real | awk '{print $2}'
}

echo "=== Shell Baseline (no terminal rendering) ===" | tee -a "$RESULTS_FILE"
BASELINE=$({ time (yes | head -500000 > /dev/null) ; } 2>&1 | grep real)
echo "yes | head -500000 -> /dev/null: $BASELINE" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# For actual terminal benchmarks, we need to measure with rendering
# This requires manual execution or AppleScript automation

echo "=== Memory Comparison ===" | tee -a "$RESULTS_FILE"
echo "(Empty or freshly launched terminal)" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Get memory for each running terminal
for app in "iTerm2" "Alacritty" "kitty" "WezTerm" "Terminal"; do
    # Try to get PID and memory
    PID=$(pgrep -x "$app" 2>/dev/null | head -1)
    if [[ -n "$PID" ]]; then
        MEM=$(ps -p "$PID" -o rss= 2>/dev/null)
        if [[ -n "$MEM" ]]; then
            echo "$app: ${MEM} KB ($(echo "scale=1; $MEM/1024" | bc) MB)" | tee -a "$RESULTS_FILE"
        fi
    else
        # Try case-insensitive search
        MEM=$(ps aux | grep -i "$app" | grep -v grep | head -1 | awk '{print $6}')
        if [[ -n "$MEM" && "$MEM" != "" ]]; then
            echo "$app: ${MEM} KB ($(echo "scale=1; $MEM/1024" | bc) MB)" | tee -a "$RESULTS_FILE"
        fi
    fi
done

echo "" | tee -a "$RESULTS_FILE"
echo "=== Notes ===" | tee -a "$RESULTS_FILE"
echo "For accurate rendering benchmarks, run manually in each terminal:" | tee -a "$RESULTS_FILE"
echo "  time (yes | head -1000000)" | tee -a "$RESULTS_FILE"
echo "  time (seq 1 1000000)" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Results saved to: $RESULTS_FILE"
