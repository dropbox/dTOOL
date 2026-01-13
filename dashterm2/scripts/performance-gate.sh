#!/bin/bash
# Performance gate - fails CI if key metrics regress
# Run after every build to catch performance bugs early

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
APP_PATH="$HOME/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development/DashTerm2.app"

# Expand glob
APP_PATH=$(echo $APP_PATH)

if [ ! -d "$APP_PATH" ]; then
    echo "ERROR: DashTerm2.app not found at $APP_PATH"
    echo "Build the app first with: xcodebuild -scheme DashTerm2 -configuration Development build"
    exit 1
fi

echo "=== DashTerm2 Performance Gate ==="
echo "App: $APP_PATH"
echo ""

# -----------------------------------------------------------------------------
# 1. Startup Time Check
# -----------------------------------------------------------------------------
echo "[1/4] Checking startup time..."

# Kill any existing instances
pkill -9 DashTerm2 2>/dev/null || true
sleep 1

# Launch and measure time to window
START_TIME=$(python3 -c "import time; print(time.time())")

# Launch app in background
open -a "$APP_PATH" &
APP_PID=$!

# Wait for window to appear (check for DashTerm2 window)
TIMEOUT=10
ELAPSED=0
while [ $ELAPSED -lt $TIMEOUT ]; do
    # Check if any window exists
    WINDOW_COUNT=$(osascript -e 'tell application "System Events" to count windows of process "DashTerm2"' 2>/dev/null || echo "0")
    if [ "$WINDOW_COUNT" -gt 0 ]; then
        break
    fi
    sleep 0.1
    ELAPSED=$((ELAPSED + 1))
done

END_TIME=$(python3 -c "import time; print(time.time())")
STARTUP_TIME=$(python3 -c "print(f'{$END_TIME - $START_TIME:.3f}')")

# Kill the app
pkill -9 DashTerm2 2>/dev/null || true

# Check threshold
STARTUP_THRESHOLD="1.0"  # 1 second max
if python3 -c "exit(0 if $STARTUP_TIME < $STARTUP_THRESHOLD else 1)"; then
    echo "✓ Startup time: ${STARTUP_TIME}s (threshold: ${STARTUP_THRESHOLD}s)"
else
    echo "✗ FAIL: Startup time ${STARTUP_TIME}s exceeds threshold ${STARTUP_THRESHOLD}s"
    exit 1
fi

# -----------------------------------------------------------------------------
# 2. Parser Configuration Check
# -----------------------------------------------------------------------------
echo ""
echo "[2/4] Checking parser configuration..."

# Read defaults to check parser settings
COMPARISON_ENABLED=$(defaults read com.dashterm.dashterm2 DtermCoreParserComparisonEnabled 2>/dev/null || echo "not set")

if [ "$COMPARISON_ENABLED" = "1" ]; then
    echo "✗ FAIL: dtermCoreParserComparisonEnabled is ON - doubles parsing overhead!"
    echo "  This should only be enabled for debugging."
    exit 1
else
    echo "✓ Parser comparison disabled (good for performance)"
fi

# -----------------------------------------------------------------------------
# 3. Check for Performance Anti-patterns in Code
# -----------------------------------------------------------------------------
echo ""
echo "[3/4] Checking for performance anti-patterns..."

ANTIPATTERNS=0

# Check for synchronous disk I/O on main thread indicators
if grep -r "NSUserDefaults.*synchronize\]" "$PROJECT_DIR/sources/" 2>/dev/null | grep -v "\.h:" | head -1; then
    echo "⚠ WARNING: Found synchronize call (blocks main thread)"
    ANTIPATTERNS=$((ANTIPATTERNS + 1))
fi

# Check for parallel comparison mode being enabled by default
if grep -E "dtermCoreParserComparisonEnabled.*YES" "$PROJECT_DIR/sources/iTermAdvancedSettingsModel.m" 2>/dev/null; then
    echo "✗ FAIL: dtermCoreParserComparisonEnabled defaults to YES"
    exit 1
fi

# Check for blocking calls in applicationWillFinishLaunching
# (These should be async)
BLOCKING_IN_STARTUP=$(grep -A 100 "applicationWillFinishLaunching:" "$PROJECT_DIR/sources/iTermApplicationDelegate.m" | \
    grep -E "\[.*sharedInstance\]|\[.*synchronize\]|dispatch_sync" | \
    grep -v "dispatch_async" | head -5)

if [ -n "$BLOCKING_IN_STARTUP" ]; then
    echo "⚠ WARNING: Potential blocking calls in startup path:"
    echo "$BLOCKING_IN_STARTUP"
    ANTIPATTERNS=$((ANTIPATTERNS + 1))
fi

if [ $ANTIPATTERNS -eq 0 ]; then
    echo "✓ No performance anti-patterns found"
else
    echo "⚠ Found $ANTIPATTERNS potential issues (warnings only)"
fi

# -----------------------------------------------------------------------------
# 4. Check for Dead Code Indicators
# -----------------------------------------------------------------------------
echo ""
echo "[4/4] Checking for dead code indicators..."

DEAD_CODE=$(grep -rn "never called\|not called\|currently not called" "$PROJECT_DIR/sources/" 2>/dev/null | \
    grep -v "\.h:" | grep -v "Test" | head -10)

if [ -n "$DEAD_CODE" ]; then
    echo "⚠ WARNING: Found 'never called' comments - potential dead code or bugs:"
    echo "$DEAD_CODE"
fi

echo ""
echo "=== Performance Gate PASSED ==="
