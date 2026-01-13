#!/bin/bash
set -euo pipefail

PASS_COUNT=0
FAIL_COUNT=0
RUNS="${1:-100}"

for i in $(seq 1 "$RUNS"); do
    OUTPUT="$(cargo test --package dashflow --lib --release 2>&1 || true)"
    if echo "$OUTPUT" | grep -q "test result: ok"; then
        echo "Run $i: PASS"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo "=== FAILURE on run $i ==="
        echo "$OUTPUT" | grep -A 10 "failures:" || true
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done

echo "========================================"
echo "Summary: $PASS_COUNT passed, $FAIL_COUNT failed out of $RUNS runs"
if [ "$FAIL_COUNT" -gt 0 ]; then
    FAILURE_RATE="$(awk -v fails="$FAIL_COUNT" -v runs="$RUNS" 'BEGIN { printf "%.1f", (fails * 100) / runs }')"
    echo "Failure rate: ${FAILURE_RATE}%"
else
    echo "Failure rate: 0%"
fi
