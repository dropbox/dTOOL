#!/bin/bash
set -euo pipefail
# Check mutation testing progress
# Usage: ./scripts/check_mutation_progress.sh [logfile]
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

LOGFILE="${1:-mutants_text_splitters_run.log}"

if [ ! -f "$LOGFILE" ]; then
    echo "Log file not found: $LOGFILE"
    exit 1
fi

echo "=== Mutation Testing Progress ==="
echo ""

# Count total mutants
TOTAL=$(grep "Found .* mutants to test" "$LOGFILE" | sed 's/Found \([0-9]*\) mutants to test/\1/')
echo "Total mutants: $TOTAL"
echo ""

# Count processed mutants
CAUGHT=$(grep -c "^CAUGHT" "$LOGFILE")
MISSED=$(grep -c "^MISSED" "$LOGFILE")
TIMEOUT=$(grep -c "^TIMEOUT" "$LOGFILE")
UNVIABLE=$(grep -c "^UNVIABLE" "$LOGFILE")

PROCESSED=$((CAUGHT + MISSED + TIMEOUT + UNVIABLE))

echo "Processed: $PROCESSED / $TOTAL"
echo "  Caught:   $CAUGHT (tests detected mutation)"
echo "  Missed:   $MISSED (mutation survived)"
echo "  Timeout:  $TIMEOUT (test exceeded time limit)"
echo "  Unviable: $UNVIABLE (mutation didn't compile)"
echo ""

# Calculate percentages if any processed
if [ "$PROCESSED" -gt 0 ]; then
    TESTABLE=$((PROCESSED - UNVIABLE - TIMEOUT))
    if [ "$TESTABLE" -gt 0 ]; then
        SCORE=$((CAUGHT * 100 / TESTABLE))
        echo "Current mutation score: $SCORE% ($CAUGHT / $TESTABLE)"
        echo ""
    fi

    PERCENT=$((PROCESSED * 100 / TOTAL))
    echo "Progress: $PERCENT%"
    echo ""
fi

# Show recent missed mutants
echo "=== Recent Missed Mutants ==="
grep "^MISSED" "$LOGFILE" | tail -5
echo ""

# Estimate completion time
if [ "$PROCESSED" -gt 0 ]; then
    REMAINING=$((TOTAL - PROCESSED))
    AVG_TIME=30  # seconds per mutant (rough estimate)
    EST_SECONDS=$((REMAINING * AVG_TIME))
    EST_MINUTES=$((EST_SECONDS / 60))
    EST_HOURS=$((EST_MINUTES / 60))

    if [ "$EST_HOURS" -gt 0 ]; then
        echo "Estimated time remaining: ~${EST_HOURS}h ${EST_MINUTES}m"
    else
        echo "Estimated time remaining: ~${EST_MINUTES}m"
    fi
fi
