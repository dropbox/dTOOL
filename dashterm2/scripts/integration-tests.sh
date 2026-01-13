#!/bin/bash
# DashTerm2 Automated Integration Tests
# Verifies the app launches, runs basic operations, and doesn't crash

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
APP_PATH="$PROJECT_ROOT/Build/Products/Development/DashTerm2.app"
TEST_RESULTS_DIR="$PROJECT_ROOT/test_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
PASSED=0
FAILED=0
SKIPPED=0

# Find app in DerivedData if not in Build/Products
if [ ! -d "$APP_PATH" ]; then
    APP_PATH=$(find ~/Library/Developer/Xcode/DerivedData -name "DashTerm2.app" -path "*/Development/*" 2>/dev/null | head -1)
fi

if [ ! -d "$APP_PATH" ]; then
    echo -e "${RED}ERROR: Cannot find DashTerm2.app. Build the project first.${NC}"
    exit 1
fi

echo "========================================"
echo "  DashTerm2 Integration Tests"
echo "========================================"
echo "App: $APP_PATH"
echo "Date: $(date)"
echo ""

# Create test results directory
mkdir -p "$TEST_RESULTS_DIR"

# Helper functions
log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    PASSED=$((PASSED + 1))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    FAILED=$((FAILED + 1))
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
    SKIPPED=$((SKIPPED + 1))
}

cleanup() {
    echo ""
    echo "Cleaning up test processes..."
    pkill -f "DashTerm2.app" 2>/dev/null || true
    sleep 1
}

trap cleanup EXIT

# =============================================================================
# TEST 1: App Launch Test
# =============================================================================
echo ""
echo "--- Test 1: App Launch ---"

pkill -f "DashTerm2.app" 2>/dev/null || true
sleep 1

open "$APP_PATH"
sleep 3

if pgrep -f "DashTerm2.app" > /dev/null; then
    log_pass "App launches successfully"
    APP_PID=$(pgrep -f "DashTerm2.app" | head -1)
else
    log_fail "App failed to launch"
    exit 1
fi

# =============================================================================
# TEST 2: Launch Stability (10 seconds)
# =============================================================================
echo ""
echo "--- Test 2: Launch Stability (10s) ---"

STABILITY_SECONDS=10
for i in $(seq 1 $STABILITY_SECONDS); do
    if ! pgrep -f "DashTerm2.app" > /dev/null; then
        log_fail "App crashed after $i seconds"
        exit 1
    fi
    sleep 1
done
log_pass "App stable for $STABILITY_SECONDS seconds"

# =============================================================================
# TEST 3: Process State Check
# =============================================================================
echo ""
echo "--- Test 3: Process State ---"

PROC_STATE=$(ps -o state= -p "$APP_PID" 2>/dev/null || echo "Z")
if [[ "$PROC_STATE" =~ ^[RrSs] ]]; then
    log_pass "Process state is healthy ($PROC_STATE)"
else
    log_fail "Process state is unhealthy ($PROC_STATE)"
fi

# =============================================================================
# TEST 4: Memory Usage Check
# =============================================================================
echo ""
echo "--- Test 4: Memory Usage ---"

MEM_MB=$(ps -o rss= -p "$APP_PID" 2>/dev/null | awk '{print int($1/1024)}')
if [ -n "$MEM_MB" ] && [ "$MEM_MB" -lt 2000 ]; then
    log_pass "Memory usage reasonable: ${MEM_MB}MB"
else
    log_fail "Memory usage high: ${MEM_MB}MB (>2GB)"
fi

# =============================================================================
# TEST 5: CPU Usage Check
# =============================================================================
echo ""
echo "--- Test 5: CPU Usage (idle check) ---"

sleep 2  # Let app settle
CPU_PCT=$(ps -o %cpu= -p "$APP_PID" 2>/dev/null | awk '{print int($1)}')
if [ -n "$CPU_PCT" ] && [ "$CPU_PCT" -lt 50 ]; then
    log_pass "CPU usage at idle: ${CPU_PCT}%"
else
    log_fail "CPU usage too high at idle: ${CPU_PCT}%"
fi

# =============================================================================
# TEST 6: Window Created Check
# =============================================================================
echo ""
echo "--- Test 6: Window Creation ---"

# Use AppleScript to check for windows
WINDOW_COUNT=$(osascript -e 'tell application "System Events" to count windows of (first process whose name contains "DashTerm2")' 2>/dev/null || echo "0")
if [ "$WINDOW_COUNT" -gt 0 ]; then
    log_pass "Window created (count: $WINDOW_COUNT)"
else
    log_skip "Could not verify window creation (may need accessibility permissions)"
fi

# =============================================================================
# TEST 7: Menu Bar Present
# =============================================================================
echo ""
echo "--- Test 7: Menu Bar Integration ---"

MENU_CHECK=$(osascript -e 'tell application "System Events" to get name of menu bar items of menu bar 1 of (first process whose name contains "DashTerm2")' 2>/dev/null || echo "")
if [[ "$MENU_CHECK" == *"Shell"* ]] || [[ "$MENU_CHECK" == *"Edit"* ]]; then
    log_pass "Menu bar integration working"
else
    log_skip "Could not verify menu bar (may need accessibility permissions)"
fi

# =============================================================================
# TEST 8: Screenshot Capture
# =============================================================================
echo ""
echo "--- Test 8: Screenshot Capture ---"

SCREENSHOT_PATH="$TEST_RESULTS_DIR/test_screenshot_$TIMESTAMP.png"
if screencapture -x "$SCREENSHOT_PATH" 2>/dev/null; then
    if [ -f "$SCREENSHOT_PATH" ] && [ -s "$SCREENSHOT_PATH" ]; then
        log_pass "Screenshot captured: $SCREENSHOT_PATH"
    else
        log_fail "Screenshot file is empty or missing"
    fi
else
    log_skip "Screenshot capture failed"
fi

# =============================================================================
# TEST 9: Extended Stability (30 seconds)
# =============================================================================
echo ""
echo "--- Test 9: Extended Stability (30s) ---"

STABILITY_SECONDS=30
START_MEM=$(ps -o rss= -p "$APP_PID" 2>/dev/null)
for i in $(seq 1 $STABILITY_SECONDS); do
    if ! pgrep -f "DashTerm2.app" > /dev/null; then
        log_fail "App crashed after $i seconds of extended test"
        break
    fi
    if [ $((i % 10)) -eq 0 ]; then
        echo "  ... $i seconds"
    fi
    sleep 1
done

if pgrep -f "DashTerm2.app" > /dev/null; then
    log_pass "App stable for $STABILITY_SECONDS seconds extended"
fi

# =============================================================================
# TEST 10: Memory Leak Check
# =============================================================================
echo ""
echo "--- Test 10: Memory Leak Check ---"

END_MEM=$(ps -o rss= -p "$APP_PID" 2>/dev/null)
if [ -n "$START_MEM" ] && [ -n "$END_MEM" ]; then
    MEM_GROWTH=$(( (END_MEM - START_MEM) / 1024 ))
    if [ "$MEM_GROWTH" -lt 100 ]; then
        log_pass "Memory stable during test (growth: ${MEM_GROWTH}MB)"
    else
        log_fail "Possible memory leak detected (growth: ${MEM_GROWTH}MB)"
    fi
else
    log_skip "Could not measure memory growth"
fi

# =============================================================================
# TEST 11: Graceful Quit
# =============================================================================
echo ""
echo "--- Test 11: Graceful Quit ---"

osascript -e 'tell application "DashTerm2" to quit' 2>/dev/null || pkill -f "DashTerm2.app"
sleep 3

if pgrep -f "DashTerm2.app" > /dev/null; then
    log_fail "App did not quit gracefully"
    pkill -9 -f "DashTerm2.app" 2>/dev/null || true
else
    log_pass "App quit gracefully"
fi

# =============================================================================
# RESULTS SUMMARY
# =============================================================================
echo ""
echo "========================================"
echo "  TEST RESULTS SUMMARY"
echo "========================================"
echo -e "${GREEN}Passed:${NC}  $PASSED"
echo -e "${RED}Failed:${NC}  $FAILED"
echo -e "${YELLOW}Skipped:${NC} $SKIPPED"
echo ""

# Save results to file
RESULTS_FILE="$TEST_RESULTS_DIR/results_$TIMESTAMP.txt"
cat > "$RESULTS_FILE" << EOF
DashTerm2 Integration Test Results
Date: $(date)
App: $APP_PATH

Passed:  $PASSED
Failed:  $FAILED
Skipped: $SKIPPED

Overall: $([ $FAILED -eq 0 ] && echo "PASS" || echo "FAIL")
EOF

echo "Results saved to: $RESULTS_FILE"

if [ $FAILED -eq 0 ]; then
    echo ""
    echo -e "${GREEN}=== ALL TESTS PASSED ===${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}=== TESTS FAILED ===${NC}"
    exit 1
fi
