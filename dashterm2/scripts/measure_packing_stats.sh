#!/bin/bash
#
# measure_packing_stats.sh - Measure scrollback memory efficiency
#
# This script helps verify that the LineBlockPacked memory optimization
# is working correctly by comparing reported stats with actual memory usage.
#
# Usage:
#   ./scripts/measure_packing_stats.sh [lines] [output_file]
#
# Requirements:
#   - DashTerm2 must be running
#   - AppleScript automation enabled for DashTerm2/DashTerm2
#
# Example workflow:
#   1. Open a fresh DashTerm2 terminal
#   2. Run this script: ./scripts/measure_packing_stats.sh 100000
#   3. Check Debug > Show Scrollback Packing Stats
#   4. Compare with this script's output

set -e

LINES=${1:-50000}
OUTPUT_FILE=${2:-"reports/main/packing_measurement_$(date +%Y%m%d_%H%M%S).md"}

echo "╔═══════════════════════════════════════════════════════════════════╗"
echo "║         Scrollback Packing Measurement Script                      ║"
echo "╚═══════════════════════════════════════════════════════════════════╝"
echo ""
echo "Configuration:"
echo "  Lines to generate: $LINES"
echo "  Output file: $OUTPUT_FILE"
echo ""

# Find DashTerm2/DashTerm2 process
ITERM_PID=$(pgrep -x DashTerm2 | head -1)
if [ -z "$ITERM_PID" ]; then
    echo "Error: DashTerm2/DashTerm2 is not running"
    echo "Please start DashTerm2 and try again"
    exit 1
fi

echo "Found DashTerm2/DashTerm2 process: PID $ITERM_PID"
echo ""

# Get initial memory usage
get_memory_mb() {
    local pid=$1
    # Use footprint for accurate resident memory on macOS
    if command -v footprint &> /dev/null; then
        footprint -j "$pid" 2>/dev/null | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    # Look for dirty memory
    for entry in data:
        if 'dirty_size' in entry:
            print(f\"{entry['dirty_size'] / 1024 / 1024:.1f}\")
            sys.exit(0)
except:
    pass
print('0')
" || echo "0"
    else
        # Fallback to ps
        ps -o rss= -p "$pid" 2>/dev/null | awk '{printf "%.1f\n", $1/1024}' || echo "0"
    fi
}

# Use ps for reliable memory reading
get_rss_mb() {
    local pid=$1
    ps -o rss= -p "$pid" 2>/dev/null | awk '{printf "%.1f\n", $1/1024}' || echo "0"
}

BEFORE_MEM=$(get_rss_mb "$ITERM_PID")
echo "Initial memory (RSS): ${BEFORE_MEM} MB"
echo ""

# Create the markdown report
mkdir -p "$(dirname "$OUTPUT_FILE")"

cat > "$OUTPUT_FILE" << EOF
# Scrollback Packing Measurement Report

**Date:** $(date +"%Y-%m-%d %H:%M:%S")
**Lines Generated:** $LINES
**Terminal PID:** $ITERM_PID

---

## Initial State

| Metric | Value |
|--------|-------|
| Process RSS | ${BEFORE_MEM} MB |

---

## Instructions

To complete this measurement:

1. **Generate scrollback** in the terminal by running:
   \`\`\`bash
   seq 1 $LINES | while read n; do echo "Line \$n: $(head -c 60 /dev/urandom | base64 | head -c 60)"; done
   \`\`\`

   Or for faster generation:
   \`\`\`bash
   python3 -c "for i in range($LINES): print(f'Line {i}: ' + 'x' * 60)"
   \`\`\`

2. **Wait for packing** - blocks are packed when new ones are created, so generate a bit more output after the main bulk to trigger packing of earlier blocks.

3. **Check Debug menu** - Go to DashTerm2 > Debug > Show Scrollback Packing Stats and record the values.

4. **Measure final memory**:
   \`\`\`bash
   ps -o rss= -p $ITERM_PID | awk '{printf "%.1f MB\n", \$1/1024}'
   \`\`\`

5. **Calculate efficiency**:
   - Expected savings for $LINES lines at 80 chars = ~33% of character storage
   - Character storage = $LINES × 80 × 12 bytes (unpacked) ≈ $((LINES * 80 * 12 / 1024 / 1024)) MB unpacked
   - Packed storage = $LINES × 80 × 8 bytes ≈ $((LINES * 80 * 8 / 1024 / 1024)) MB packed
   - Expected savings ≈ $((LINES * 80 * 4 / 1024 / 1024)) MB

---

## Expected Values (Theoretical)

| Metric | Unpacked | Packed | Savings |
|--------|----------|--------|---------|
| Character bytes | $((LINES * 80 * 12)) | $((LINES * 80 * 8)) | $((LINES * 80 * 4)) |
| Character MB | $((LINES * 80 * 12 / 1024 / 1024)) | $((LINES * 80 * 8 / 1024 / 1024)) | $((LINES * 80 * 4 / 1024 / 1024)) |

---

## Recording Your Results

After following the instructions above, fill in this section:

### Debug Menu Stats (from Show Scrollback Packing Stats)

| Metric | Value |
|--------|-------|
| Total Blocks | |
| Packed Blocks | |
| Total Raw Lines | |
| Packed Raw Lines | |
| Current Memory | |
| Without Packing | |
| Saved | |
| Savings % | |

### Actual Memory Measurement

| Metric | Value |
|--------|-------|
| Final Process RSS | |
| Memory Increase | |

### Analysis

| Check | Pass/Fail | Notes |
|-------|-----------|-------|
| Savings % ≈ 33% | | |
| Most blocks packed | | |
| Memory increase reasonable | | |

EOF

echo "Report template created: $OUTPUT_FILE"
echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "QUICK START: Copy and paste this command into your terminal:"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "python3 -c \"for i in range($LINES): print(f'Line {i}: ' + 'x' * 60)\""
echo ""
echo "Then go to: DashTerm2 > Debug > Show Scrollback Packing Stats"
echo ""
echo "Final memory check: ps -o rss= -p $ITERM_PID | awk '{printf \"%.1f MB\\n\", \$1/1024}'"
echo ""
