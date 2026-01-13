#!/bin/bash
# Terminal Emulator Comparison Benchmark
#
# This script compares terminal rendering performance across different terminal
# emulators. It launches each terminal, runs benchmarks inside it, and collects
# timing data for comparison.
#
# Usage:
#   ./terminal_comparison_benchmark.sh [--quick|--full] [terminals...]
#
# Examples:
#   ./terminal_comparison_benchmark.sh                  # All detected terminals, quick mode
#   ./terminal_comparison_benchmark.sh --full           # All terminals, full mode
#   ./terminal_comparison_benchmark.sh iTerm Terminal   # Specific terminals only
#
# Author: DashTerm2 Project
# Created: Iteration #147

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results/comparison"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

mkdir -p "$RESULTS_DIR"

# Benchmark parameters
MODE="quick"
TERMINALS=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick)
            MODE="quick"
            shift
            ;;
        --full)
            MODE="full"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--quick|--full] [terminals...]"
            echo ""
            echo "Options:"
            echo "  --quick    Quick benchmark (~30s per terminal)"
            echo "  --full     Full benchmark (~2min per terminal)"
            echo ""
            echo "Detected terminals:"
            for app in "iTerm" "Terminal" "Alacritty" "kitty" "WezTerm" "Hyper"; do
                if [[ -d "/Applications/${app}.app" ]] || command -v "$app" &>/dev/null; then
                    echo "  - $app"
                fi
            done
            echo ""
            echo "Examples:"
            echo "  $0                          # All detected terminals"
            echo "  $0 iTerm Terminal           # Specific terminals"
            echo "  $0 --full iTerm             # Full benchmark on iTerm only"
            exit 0
            ;;
        *)
            TERMINALS+=("$1")
            shift
            ;;
    esac
done

# Terminal detection and app path mapping
declare -A TERMINAL_APPS=(
    ["iTerm"]="/Applications/iTerm.app"
    ["Terminal"]="/System/Applications/Utilities/Terminal.app"
    ["Alacritty"]="/Applications/Alacritty.app"
    ["kitty"]="/Applications/kitty.app"
    ["WezTerm"]="/Applications/WezTerm.app"
    ["Hyper"]="/Applications/Hyper.app"
)

# Detect available terminals if none specified
if [[ ${#TERMINALS[@]} -eq 0 ]]; then
    for name in "${!TERMINAL_APPS[@]}"; do
        app_path="${TERMINAL_APPS[$name]}"
        if [[ -d "$app_path" ]]; then
            TERMINALS+=("$name")
        fi
    done
fi

if [[ ${#TERMINALS[@]} -eq 0 ]]; then
    echo "Error: No terminals found or specified"
    exit 1
fi

# Sort terminals for consistent output
IFS=$'\n' TERMINALS=($(sort <<<"${TERMINALS[*]}")); unset IFS

echo "=============================================="
echo "DashTerm2 Terminal Comparison Benchmark"
echo "Mode: $MODE"
echo "Timestamp: $TIMESTAMP"
echo "Terminals: ${TERMINALS[*]}"
echo "=============================================="
echo ""

# Set benchmark parameters based on mode
case "$MODE" in
    quick)
        LONG_LINE_WIDTHS="100,500,1000,2000"
        LONG_LINE_COUNTS="500,1000"
        ITERATIONS=3
        YES_LINES=100000
        SEQ_COUNT=50000
        COLOR_ITERATIONS=10
        ;;
    full)
        LONG_LINE_WIDTHS="100,500,1000,2000,5000"
        LONG_LINE_COUNTS="500,1000,5000"
        ITERATIONS=5
        YES_LINES=500000
        SEQ_COUNT=200000
        COLOR_ITERATIONS=50
        ;;
esac

# Benchmark script to run inside each terminal
BENCHMARK_SCRIPT="$RESULTS_DIR/run_benchmark_${TIMESTAMP}.sh"

cat > "$BENCHMARK_SCRIPT" << 'BENCHMARK_EOF'
#!/bin/bash
# Internal benchmark script - runs inside the terminal being tested
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PARENT_DIR="$(dirname "$SCRIPT_DIR")"
OUTPUT_FILE="$1"
TERMINAL_NAME="$2"

# Source parameters from environment or use defaults
YES_LINES="${YES_LINES:-100000}"
SEQ_COUNT="${SEQ_COUNT:-50000}"
COLOR_ITERATIONS="${COLOR_ITERATIONS:-10}"
LONG_LINE_WIDTH="${LONG_LINE_WIDTH:-500}"
LONG_LINE_COUNT="${LONG_LINE_COUNT:-1000}"
ITERATIONS="${BENCHMARK_ITERATIONS:-3}"

LONG_LINE_GENERATOR="$PARENT_DIR/Sources/generate_long_lines.py"

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

# Run benchmark with multiple iterations
run_benchmark() {
    local name="$1"
    local cmd="$2"
    local rounds="$3"

    # Warmup
    eval "$cmd" >/dev/null 2>&1 || true

    local sum=0
    local min=999999999
    local max=0
    local times=""

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

    echo "{\"name\":\"$name\",\"mean_ms\":$mean,\"stddev_ms\":$stddev,\"min_ms\":$min,\"max_ms\":$max}"
}

# Get terminal info
TERM_NAME="${TERM_PROGRAM:-$TERMINAL_NAME}"
TERM_VERSION="${TERM_PROGRAM_VERSION:-unknown}"

# System info
CHIP=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
CORES=$(sysctl -n hw.ncpu 2>/dev/null || echo "0")
MEMSIZE=$(sysctl -n hw.memsize 2>/dev/null || echo "0")
MEMORY_GB=$((MEMSIZE / 1024 / 1024 / 1024))

echo "Running benchmarks in $TERM_NAME..."

# Collect results
RESULTS=""

# Raw throughput
echo "  Testing: yes_lines_$YES_LINES"
RESULT=$(run_benchmark "yes_lines_$YES_LINES" "yes | head -$YES_LINES" "$ITERATIONS")
RESULTS="$RESULT"

echo "  Testing: seq_$SEQ_COUNT"
RESULT=$(run_benchmark "seq_$SEQ_COUNT" "seq 1 $SEQ_COUNT" "$ITERATIONS")
RESULTS="$RESULTS,$RESULT"

# ANSI colors
echo "  Testing: 256color_x$COLOR_ITERATIONS"
RESULT=$(run_benchmark "256color_x$COLOR_ITERATIONS" \
    "for i in \$(seq 0 $COLOR_ITERATIONS); do for c in \$(seq 0 255); do printf '\033[38;5;%sm#' \$c; done; echo; done" \
    "$ITERATIONS")
RESULTS="$RESULTS,$RESULT"

# Long lines
echo "  Testing: long_lines_${LONG_LINE_WIDTH}x${LONG_LINE_COUNT}"
RESULT=$(run_benchmark "long_lines_${LONG_LINE_WIDTH}x${LONG_LINE_COUNT}" \
    "python3 \"$LONG_LINE_GENERATOR\" --lines $LONG_LINE_COUNT --columns $LONG_LINE_WIDTH" \
    "$ITERATIONS")
RESULTS="$RESULTS,$RESULT"

# Unicode
echo "  Testing: unicode_cjk"
RESULT=$(run_benchmark "unicode_cjk" \
    "for i in \$(seq 1 5); do for j in \$(seq 1 100); do echo '漢字テスト한국어測試'; done; done" \
    "$ITERATIONS")
RESULTS="$RESULTS,$RESULT"

# Emoji
echo "  Testing: emoji"
RESULT=$(run_benchmark "emoji" \
    "for i in \$(seq 1 5); do for j in \$(seq 1 100); do echo '🎉🚀💻🔥✨🌟🎯🏆💡🔧'; done; done" \
    "$ITERATIONS")
RESULTS="$RESULTS,$RESULT"

# Write JSON output
cat > "$OUTPUT_FILE" << EOF
{
  "benchmark_type": "terminal_comparison",
  "timestamp": "$(date +%Y%m%dT%H%M%S)",
  "terminal": {"name": "$TERM_NAME", "version": "$TERM_VERSION"},
  "system": {"chip": "$CHIP", "cores": $CORES, "memory": "${MEMORY_GB}GB"},
  "parameters": {
    "yes_lines": $YES_LINES,
    "seq_count": $SEQ_COUNT,
    "color_iterations": $COLOR_ITERATIONS,
    "long_line_width": $LONG_LINE_WIDTH,
    "long_line_count": $LONG_LINE_COUNT,
    "iterations": $ITERATIONS
  },
  "results": [$RESULTS]
}
EOF

echo "Results saved to: $OUTPUT_FILE"
BENCHMARK_EOF

chmod +x "$BENCHMARK_SCRIPT"

# Results collection
declare -A TERMINAL_RESULTS

echo "Starting terminal benchmarks..."
echo ""

for terminal in "${TERMINALS[@]}"; do
    app_path="${TERMINAL_APPS[$terminal]:-}"

    if [[ ! -d "$app_path" ]]; then
        echo "Skipping $terminal (not installed at $app_path)"
        continue
    fi

    OUTPUT_FILE="$RESULTS_DIR/${terminal}_${TIMESTAMP}.json"

    echo "=============================================="
    echo "Benchmarking: $terminal"
    echo "App: $app_path"
    echo "=============================================="

    # Export parameters for the benchmark script
    export YES_LINES SEQ_COUNT COLOR_ITERATIONS
    export LONG_LINE_WIDTH="${LONG_LINE_WIDTHS%%,*}"
    export LONG_LINE_COUNT="${LONG_LINE_COUNTS%%,*}"
    export BENCHMARK_ITERATIONS="$ITERATIONS"

    # Run benchmark based on terminal type
    case "$terminal" in
        "iTerm")
            # Use osascript to run in iTerm
            osascript << EOF
tell application "iTerm"
    activate
    set newWindow to (create window with default profile)
    tell current session of newWindow
        write text "export YES_LINES=$YES_LINES SEQ_COUNT=$SEQ_COUNT COLOR_ITERATIONS=$COLOR_ITERATIONS"
        write text "export LONG_LINE_WIDTH=$LONG_LINE_WIDTH LONG_LINE_COUNT=$LONG_LINE_COUNT BENCHMARK_ITERATIONS=$ITERATIONS"
        write text "\"$BENCHMARK_SCRIPT\" \"$OUTPUT_FILE\" \"$terminal\""
        write text "exit"
    end tell
end tell
EOF
            ;;
        "Terminal")
            # Use osascript to run in Terminal.app
            osascript << EOF
tell application "Terminal"
    activate
    do script "export YES_LINES=$YES_LINES SEQ_COUNT=$SEQ_COUNT COLOR_ITERATIONS=$COLOR_ITERATIONS && export LONG_LINE_WIDTH=$LONG_LINE_WIDTH LONG_LINE_COUNT=$LONG_LINE_COUNT BENCHMARK_ITERATIONS=$ITERATIONS && \"$BENCHMARK_SCRIPT\" \"$OUTPUT_FILE\" \"$terminal\" && exit"
end tell
EOF
            ;;
        *)
            # For other terminals, try generic approach
            echo "  Note: $terminal may require manual benchmark execution"
            open -a "$app_path"
            echo "  Run this command in $terminal:"
            echo "    $BENCHMARK_SCRIPT \"$OUTPUT_FILE\" \"$terminal\""
            ;;
    esac

    # Wait for benchmark to complete
    echo "  Waiting for benchmark to complete..."
    timeout=300
    waited=0
    while [[ ! -f "$OUTPUT_FILE" ]] && [[ $waited -lt $timeout ]]; do
        sleep 2
        ((waited+=2))
        echo -n "."
    done
    echo ""

    if [[ -f "$OUTPUT_FILE" ]]; then
        TERMINAL_RESULTS[$terminal]="$OUTPUT_FILE"
        echo "  Completed: $OUTPUT_FILE"
    else
        echo "  Warning: Benchmark did not complete within ${timeout}s"
    fi

    echo ""
    sleep 2  # Brief pause between terminals
done

# Generate comparison report
REPORT_FILE="$RESULTS_DIR/comparison_${TIMESTAMP}.md"

echo "=============================================="
echo "Generating comparison report..."
echo "=============================================="

cat > "$REPORT_FILE" << EOF
# Terminal Emulator Comparison Report

**Generated:** $(date)
**Mode:** $MODE

## System Information

$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "Unknown CPU")
$(sysctl -n hw.ncpu 2>/dev/null || echo "?") cores
$(($(sysctl -n hw.memsize 2>/dev/null || echo "0") / 1024 / 1024 / 1024))GB memory

## Benchmark Parameters

| Parameter | Value |
|-----------|-------|
| yes_lines | $YES_LINES |
| seq_count | $SEQ_COUNT |
| color_iterations | $COLOR_ITERATIONS |
| long_line_width | ${LONG_LINE_WIDTH:-500} |
| long_line_count | ${LONG_LINE_COUNT:-1000} |
| iterations | $ITERATIONS |

## Results Summary

EOF

# Parse results and create comparison table
if command -v jq &>/dev/null; then
    echo "| Terminal | yes_lines (ms) | seq (ms) | 256color (ms) | long_lines (ms) | unicode (ms) | emoji (ms) |" >> "$REPORT_FILE"
    echo "|----------|----------------|----------|---------------|-----------------|--------------|------------|" >> "$REPORT_FILE"

    for terminal in "${TERMINALS[@]}"; do
        result_file="${TERMINAL_RESULTS[$terminal]:-}"
        if [[ -f "$result_file" ]]; then
            yes_ms=$(jq -r '.results[] | select(.name | startswith("yes_lines")) | .mean_ms' "$result_file" 2>/dev/null || echo "N/A")
            seq_ms=$(jq -r '.results[] | select(.name | startswith("seq_")) | .mean_ms' "$result_file" 2>/dev/null || echo "N/A")
            color_ms=$(jq -r '.results[] | select(.name | startswith("256color")) | .mean_ms' "$result_file" 2>/dev/null || echo "N/A")
            long_ms=$(jq -r '.results[] | select(.name | startswith("long_lines")) | .mean_ms' "$result_file" 2>/dev/null || echo "N/A")
            unicode_ms=$(jq -r '.results[] | select(.name | startswith("unicode")) | .mean_ms' "$result_file" 2>/dev/null || echo "N/A")
            emoji_ms=$(jq -r '.results[] | select(.name | startswith("emoji")) | .mean_ms' "$result_file" 2>/dev/null || echo "N/A")

            echo "| $terminal | $yes_ms | $seq_ms | $color_ms | $long_ms | $unicode_ms | $emoji_ms |" >> "$REPORT_FILE"
        else
            echo "| $terminal | N/A | N/A | N/A | N/A | N/A | N/A |" >> "$REPORT_FILE"
        fi
    done
else
    echo "*jq not installed - detailed results in individual JSON files*" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" << EOF

## Individual Results

EOF

for terminal in "${TERMINALS[@]}"; do
    result_file="${TERMINAL_RESULTS[$terminal]:-}"
    if [[ -f "$result_file" ]]; then
        echo "### $terminal" >> "$REPORT_FILE"
        echo '```json' >> "$REPORT_FILE"
        cat "$result_file" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
    fi
done

echo "=============================================="
echo "Benchmark complete!"
echo ""
echo "Results directory: $RESULTS_DIR"
echo "Comparison report: $REPORT_FILE"
echo ""
echo "Individual results:"
for terminal in "${TERMINALS[@]}"; do
    result_file="${TERMINAL_RESULTS[$terminal]:-}"
    if [[ -f "$result_file" ]]; then
        echo "  $terminal: $result_file"
    fi
done
echo "=============================================="
