#!/bin/bash
# Test DashTerm2 startup time
# Measures time from launch to window visible

set -euo pipefail

APP_PATH="$(find ~/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development -name "DashTerm2.app" -print -quit 2>/dev/null || echo "")"

if [[ -z "$APP_PATH" ]]; then
    echo "ERROR: DashTerm2.app not found. Run build first."
    exit 1
fi

echo "Testing startup time for: $APP_PATH"
echo ""

# Kill any existing instances
pkill -9 DashTerm2 2>/dev/null || true
sleep 0.5

# Run 3 tests and average
total_ms=0
NUM_TESTS=3

for i in $(seq 1 $NUM_TESTS); do
    echo "Test $i/$NUM_TESTS..."

    # Time the launch
    start_time=$(python3 -c 'import time; print(int(time.time() * 1000))')

    # Launch the app
    open -a "$APP_PATH" &

    # Wait for window to appear (poll every 10ms)
    window_appeared=false
    for j in $(seq 1 500); do
        if osascript -e 'tell application "System Events" to exists (window 1 of process "DashTerm2")' 2>/dev/null | grep -q true; then
            window_appeared=true
            break
        fi
        sleep 0.01
    done

    end_time=$(python3 -c 'import time; print(int(time.time() * 1000))')
    elapsed=$((end_time - start_time))

    if [[ "$window_appeared" == "true" ]]; then
        echo "  Window visible in: ${elapsed}ms"
        total_ms=$((total_ms + elapsed))
    else
        echo "  TIMEOUT: Window did not appear within 5 seconds"
        total_ms=$((total_ms + 5000))
    fi

    # Close the app
    pkill -9 DashTerm2 2>/dev/null || true
    sleep 0.5
done

avg_ms=$((total_ms / NUM_TESTS))
echo ""
echo "========================================="
echo "Average startup time: ${avg_ms}ms"
if [[ $avg_ms -lt 500 ]]; then
    echo "STATUS: PASS (target: < 500ms)"
else
    echo "STATUS: FAIL (target: < 500ms)"
fi
echo "========================================="
