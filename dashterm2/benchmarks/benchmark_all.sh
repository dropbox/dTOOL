#!/bin/bash
# Run benchmarks across all terminals
# This script should be run from a regular terminal

BENCHMARK_DIR="$(dirname "$0")"
cd "$BENCHMARK_DIR"

# Get system info
echo "=== System Information ==="
echo "Host: $(hostname)"
echo "Date: $(date)"
echo "CPU: $(sysctl -n machdep.cpu.brand_string)"
echo "Memory: $(sysctl -n hw.memsize | awk '{print $0/1024/1024/1024 " GB"}')"
echo "macOS: $(sw_vers -productVersion)"
echo ""

# Function to measure command execution time in milliseconds
measure_time() {
    local start=$(python3 -c 'import time; print(int(time.time()*1000))')
    eval "$@" > /dev/null 2>&1
    local end=$(python3 -c 'import time; print(int(time.time()*1000))')
    echo $((end - start))
}

# Baseline benchmark (runs in current shell, no terminal rendering)
echo "=== Baseline (pipe to /dev/null, no rendering) ==="
echo "yes | head -1000000: $(measure_time 'yes | head -1000000')ms"
echo "seq 1 1000000: $(measure_time 'seq 1 1000000')ms"
echo "cat 100MB_random.txt: $(measure_time 'cat 100MB_random.txt')ms"
echo ""

# Memory baseline
echo "=== Memory Usage (RSS in KB) ==="
for app in "iTerm2" "Alacritty" "kitty" "WezTerm" "Terminal"; do
    MEM=$(ps aux 2>/dev/null | grep -i "$app" | grep -v grep | awk '{sum += $6} END {print sum}')
    if [[ -n "$MEM" && "$MEM" != "0" ]]; then
        echo "$app: ${MEM} KB ($(echo "scale=1; $MEM/1024" | bc) MB)"
    fi
done
echo ""

echo "=== Benchmark Notes ==="
echo "To measure actual terminal rendering performance:"
echo "1. Open each terminal application"
echo "2. Run: time (yes | head -1000000)"
echo "3. Run: time (seq 1 1000000)"
echo "4. Run: time cat $BENCHMARK_DIR/100MB_random.txt"
echo "5. Record the real time from each"
