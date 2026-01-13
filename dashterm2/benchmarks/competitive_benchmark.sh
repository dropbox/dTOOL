#!/bin/bash
# Competitive Benchmark: Compare DashTerm2 against Alacritty, Kitty, WezTerm
# This script measures raw CLI throughput (not GUI rendering) to establish a baseline.
#
# For GUI terminal rendering benchmarks, we would need to:
# 1. Launch each terminal app
# 2. Execute commands within them
# 3. Measure with external tools (Instruments, etc.)
#
# This script focuses on what we can measure without GUI interaction:
# - Binary sizes
# - Startup time estimation (launch daemon/helper components)
# - Memory footprint of helper processes
# - Shell throughput under each shell
#
# Note: True terminal throughput requires running inside the terminal emulator.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
OUTPUT_FILE="$RESULTS_DIR/competitive_${TIMESTAMP}.json"
REPORT_FILE="$RESULTS_DIR/competitive_${TIMESTAMP}.md"

mkdir -p "$RESULTS_DIR"

# Terminal app paths
DASHTERM_APP="$HOME/Library/Developer/Xcode/DerivedData/DashTerm2-eozhmqjuyzuggzgkgmyyurjyassr/Build/Products/Development/DashTerm2.app"
ITERM2_APP="/Applications/iTerm.app"
ALACRITTY_APP="/Applications/Alacritty.app"
KITTY_APP="/Applications/kitty.app"
WEZTERM_APP="/Applications/WezTerm.app"

echo "==================================="
echo "DashTerm2 Competitive Benchmark"
echo "Timestamp: $TIMESTAMP"
echo "==================================="
echo ""

# Get system info
SYSTEM_INFO=$(system_profiler SPHardwareDataType 2>/dev/null | grep -E "(Model Name|Model Identifier|Chip|Memory)" | head -4)
echo "System Information:"
echo "$SYSTEM_INFO"
echo ""

# Start JSON output
cat > "$OUTPUT_FILE" << 'JSONHEADER'
{
  "timestamp": "TIMESTAMP_PLACEHOLDER",
  "type": "competitive_benchmark",
  "system": {
JSONHEADER

# Add system info to JSON
MODEL_NAME=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Model Name" | cut -d: -f2 | xargs)
MODEL_ID=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Model Identifier" | cut -d: -f2 | xargs)
CHIP=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Chip" | cut -d: -f2 | xargs)
MEMORY=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Memory" | cut -d: -f2 | xargs)

sed -i '' "s/TIMESTAMP_PLACEHOLDER/$TIMESTAMP/g" "$OUTPUT_FILE"
cat >> "$OUTPUT_FILE" << EOF
    "model_name": "$MODEL_NAME",
    "model_id": "$MODEL_ID",
    "chip": "$CHIP",
    "memory": "$MEMORY"
  },
  "terminals": {
EOF

# Function to get app size
get_app_size() {
    local app_path="$1"
    if [[ -d "$app_path" ]]; then
        du -sk "$app_path" 2>/dev/null | cut -f1
    else
        echo "0"
    fi
}

# Function to get app version
get_app_version() {
    local app_path="$1"
    if [[ -d "$app_path" ]]; then
        defaults read "$app_path/Contents/Info.plist" CFBundleShortVersionString 2>/dev/null || echo "unknown"
    else
        echo "not_installed"
    fi
}

echo "=== Binary Size Comparison ==="
echo ""

# Array of terminals to test
declare -a TERMINALS=(
    "DashTerm2:$DASHTERM_APP"
    "iTerm2:$ITERM2_APP"
    "Alacritty:$ALACRITTY_APP"
    "Kitty:$KITTY_APP"
    "WezTerm:$WEZTERM_APP"
)

FIRST=true
for entry in "${TERMINALS[@]}"; do
    IFS=':' read -r name path <<< "$entry"

    if [[ "$FIRST" != "true" ]]; then
        echo "," >> "$OUTPUT_FILE"
    fi
    FIRST=false

    size_kb=$(get_app_size "$path")
    version=$(get_app_version "$path")

    if [[ "$size_kb" != "0" ]]; then
        size_mb=$(echo "scale=1; $size_kb / 1024" | bc)
        echo "$name ($version): ${size_mb}MB"
    else
        echo "$name: Not found at $path"
        size_mb="0"
    fi

    cat >> "$OUTPUT_FILE" << EOF
    "$name": {
      "path": "$path",
      "version": "$version",
      "size_kb": $size_kb,
      "installed": $(if [[ -d "$path" ]]; then echo "true"; else echo "false"; fi)
    }
EOF
done

echo "  }," >> "$OUTPUT_FILE"

echo ""
echo "=== Shell Throughput Baseline ==="
echo "(This measures shell performance, not terminal rendering)"
echo ""

# Run hyperfine benchmark for shell throughput
if command -v hyperfine >/dev/null 2>&1; then
    echo "Running shell throughput benchmarks with hyperfine..."

    # Various workloads
    WORKLOADS=(
        "yes | head -500000"
        "seq 1 100000"
        "cat /dev/zero | head -c 10000000 | wc -c"
    )

    cat >> "$OUTPUT_FILE" << 'EOF'
  "shell_throughput": [
EOF

    WORKLOAD_FIRST=true
    for workload in "${WORKLOADS[@]}"; do
        if [[ "$WORKLOAD_FIRST" != "true" ]]; then
            echo "," >> "$OUTPUT_FILE"
        fi
        WORKLOAD_FIRST=false

        echo "Benchmarking: $workload"

        TEMP_JSON=$(mktemp)
        hyperfine --warmup 2 --runs 5 --export-json "$TEMP_JSON" "$workload" 2>/dev/null || true

        if [[ -s "$TEMP_JSON" ]]; then
            MEAN=$(jq -r '.results[0].mean // 0' "$TEMP_JSON")
            STDDEV=$(jq -r '.results[0].stddev // 0' "$TEMP_JSON")
            MIN=$(jq -r '.results[0].min // 0' "$TEMP_JSON")
            MAX=$(jq -r '.results[0].max // 0' "$TEMP_JSON")

            cat >> "$OUTPUT_FILE" << EOF
    {
      "command": "$workload",
      "mean_seconds": $MEAN,
      "stddev": $STDDEV,
      "min": $MIN,
      "max": $MAX
    }
EOF

            echo "  Mean: ${MEAN}s, Stddev: ${STDDEV}s"
        else
            cat >> "$OUTPUT_FILE" << EOF
    {
      "command": "$workload",
      "error": "benchmark_failed"
    }
EOF
        fi

        rm -f "$TEMP_JSON"
    done

    echo "  ]," >> "$OUTPUT_FILE"
else
    echo "hyperfine not found - skipping throughput benchmarks"
    echo '  "shell_throughput": [],' >> "$OUTPUT_FILE"
fi

echo ""
echo "=== Memory Baseline ==="
echo ""

# Current shell memory usage
RSS_SELF=$(ps -o rss= -p $$ 2>/dev/null | xargs)
echo "Current shell RSS: ${RSS_SELF}KB"

cat >> "$OUTPUT_FILE" << EOF
  "memory_baseline": {
    "shell_rss_kb": $RSS_SELF
  }
EOF

echo "}" >> "$OUTPUT_FILE"

# Generate Markdown report
cat > "$REPORT_FILE" << EOF
# DashTerm2 Competitive Benchmark Report

**Generated:** $(date -u +"%Y-%m-%d %H:%M:%S UTC")
**System:** $MODEL_NAME ($CHIP, $MEMORY)

## Binary Sizes

| Terminal | Version | Size |
|----------|---------|------|
EOF

for entry in "${TERMINALS[@]}"; do
    IFS=':' read -r name path <<< "$entry"
    size_kb=$(get_app_size "$path")
    version=$(get_app_version "$path")
    if [[ "$size_kb" != "0" ]]; then
        size_mb=$(echo "scale=1; $size_kb / 1024" | bc)
        echo "| $name | $version | ${size_mb}MB |" >> "$REPORT_FILE"
    else
        echo "| $name | N/A | Not installed |" >> "$REPORT_FILE"
    fi
done

cat >> "$REPORT_FILE" << 'EOF'

## Notes

This benchmark measures:
1. **Binary sizes** - On-disk footprint of each terminal application
2. **Shell throughput** - Raw command execution speed (terminal-agnostic)

### What This Does NOT Measure

To properly benchmark terminal rendering performance, you need to:
1. Launch each terminal application manually
2. Run performance tests within each terminal window
3. Use Instruments.app to capture Metal/GPU traces
4. Measure frame times during rapid text output

### Recommended Manual Tests

Run these commands inside each terminal to compare rendering:

```bash
# Throughput test
time cat /path/to/100MB_random.txt

# Line output test
time yes | head -1000000

# Color rendering test
for i in {0..255}; do printf "\e[38;5;${i}m█"; done

# Unicode stress test
cat /path/to/unicode_test_file.txt
```

### Next Steps

1. **Run in-terminal benchmarks** - Execute throughput.sh inside each terminal app
2. **Capture Metal traces** - Use Instruments for GPU performance analysis
3. **Measure input latency** - Use typometer or similar tool
4. **Memory profiling** - Monitor RSS during scrollback operations
EOF

echo ""
echo "==================================="
echo "Benchmark complete!"
echo "JSON results: $OUTPUT_FILE"
echo "Report: $REPORT_FILE"
echo "==================================="

# Pretty print summary
if command -v jq >/dev/null 2>&1; then
    echo ""
    echo "JSON Summary:"
    jq '.' "$OUTPUT_FILE"
fi
