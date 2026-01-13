#!/bin/bash
# DashTerm2 Smoke Test
# This script verifies the app can launch and stay running without crashing.
# CRITICAL: Run this before every commit to prevent shipping launch crashes.
#
# Test coverage:
# 1. App launches without crashing
# 2. App stays running for TIMEOUT_SECONDS
# 3. AppleScript accessibility (app responds to scripting)
# 4. Basic window creation (if --extended flag)

set -e

# Parse arguments
EXTENDED_TESTS=false
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --extended) EXTENDED_TESTS=true ;;
        --help) echo "Usage: $0 [--extended]"; exit 0 ;;
        *) echo "Unknown parameter: $1"; exit 1 ;;
    esac
    shift
done

PREFERRED_APP_NAMES=("DashTerm2" "iTerm2")
DERIVED_DATA_ROOT="$HOME/Library/Developer/Xcode/DerivedData"
APP_PATH=""
APP_PRODUCT_NAME=""

find_app_bundle() {
    local search_roots=()

    # Search both DashTerm2-* and iTerm2-* DerivedData folders for backwards compatibility
    for derived in "$DERIVED_DATA_ROOT"/DashTerm2-* "$DERIVED_DATA_ROOT"/iTerm2-*; do
        [[ -d "$derived" ]] || continue
        search_roots+=("$derived/Build/Products/Development")
    done
    search_roots+=("$PWD/build/Development")

    for root in "${search_roots[@]}"; do
        for name in "${PREFERRED_APP_NAMES[@]}"; do
            local candidate="$root/$name.app"
            if [[ -d "$candidate" ]]; then
                APP_PATH="$candidate"
                APP_PRODUCT_NAME="$name"
                return 0
            fi
        done
    done
    return 1
}

if ! find_app_bundle; then
    echo "ERROR: Could not find DashTerm2 or iTerm2 app bundle. Build the Development configuration first."
    echo "Run: xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build"
    exit 1
fi

INFO_PLIST="$APP_PATH/Contents/Info.plist"
if [[ -f "$INFO_PLIST" ]]; then
    CF_BUNDLE_NAME=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleName' "$INFO_PLIST" 2>/dev/null || true)
    if [[ -z "$CF_BUNDLE_NAME" ]]; then
        CF_BUNDLE_NAME=$(defaults read "$APP_PATH/Contents/Info" CFBundleName 2>/dev/null || true)
    fi
    CF_BUNDLE_EXECUTABLE=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$INFO_PLIST" 2>/dev/null || true)
    if [[ -z "$CF_BUNDLE_EXECUTABLE" ]]; then
        CF_BUNDLE_EXECUTABLE=$(defaults read "$APP_PATH/Contents/Info" CFBundleExecutable 2>/dev/null || true)
    fi
fi

OSA_APP_NAME="${CF_BUNDLE_NAME:-$APP_PRODUCT_NAME}"
APP_EXECUTABLE_NAME="${CF_BUNDLE_EXECUTABLE:-$APP_PRODUCT_NAME}"
EXECUTABLE="$APP_PATH/Contents/MacOS/$APP_EXECUTABLE_NAME"

if [[ ! -x "$EXECUTABLE" ]]; then
    echo "ERROR: Executable not found at $EXECUTABLE"
    exit 1
fi
TIMEOUT_SECONDS=10
PID_FILE="/tmp/dashterm2-smoke-test.pid"

# Track test results
TESTS_PASSED=0
TESTS_FAILED=0

pass_test() {
    echo "  [PASS] $1"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

fail_test() {
    echo "  [FAIL] $1"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

echo "=== DashTerm2 Smoke Test ==="
echo "App: $APP_PATH"
if $EXTENDED_TESTS; then
    echo "Mode: Extended (includes window tests)"
else
    echo "Mode: Basic (launch stability only)"
fi
echo ""

# Clean up any previous test
echo "[1/4] Cleaning up previous instances..."
pkill -9 -f "$APP_PATH" 2>/dev/null || true
sleep 1

# Launch the app
echo "[2/4] Launching app..."
"$EXECUTABLE" &
APP_PID=$!
echo $APP_PID > "$PID_FILE"
echo "  Launched with PID: $APP_PID"

# Wait and check if it stays alive
echo "[3/4] Testing launch stability for ${TIMEOUT_SECONDS} seconds..."
ELAPSED=0
while [[ $ELAPSED -lt $TIMEOUT_SECONDS ]]; do
    sleep 1
    ELAPSED=$((ELAPSED + 1))

    if ! kill -0 $APP_PID 2>/dev/null; then
        echo ""
        fail_test "Launch stability: crashed within ${ELAPSED} seconds"
        echo ""
        echo "Check crash logs at: ~/Library/Logs/DiagnosticReports/"
        echo "Most recent crash:"
        # Look for both DashTerm2 and iTerm2 crash logs
        ls -lt ~/Library/Logs/DiagnosticReports/DashTerm2* ~/Library/Logs/DiagnosticReports/iTerm2* 2>/dev/null | head -1
        rm -f "$PID_FILE"
        exit 1
    fi

    echo -n "."
done
echo ""
pass_test "Launch stability: survived ${TIMEOUT_SECONDS} seconds"

# Verify the process is actually responding (not hung)
echo "[4/4] Checking process health..."
if ps -p $APP_PID -o state= | grep -q "R\|S"; then
    pass_test "Process state: healthy (Running/Sleeping)"
else
    fail_test "Process state: abnormal"
fi

# Extended tests (optional, requires GUI access)
if $EXTENDED_TESTS; then
    echo ""
    echo "=== Extended Tests ==="

    # Test AppleScript accessibility
    echo "[E1] Testing AppleScript accessibility..."
    if osascript -e "tell application \"$OSA_APP_NAME\" to get version" >/dev/null 2>&1; then
        pass_test "AppleScript: app responds to version query"
    else
        fail_test "AppleScript: app did not respond (may need Accessibility permissions)"
    fi

    # Test window count
    echo "[E2] Testing window availability..."
    WINDOW_COUNT=$(osascript -e "tell application \"$OSA_APP_NAME\" to get count of windows" 2>/dev/null || echo "0")
    if [[ "$WINDOW_COUNT" -ge 1 ]]; then
        pass_test "Windows: $WINDOW_COUNT window(s) available"
    else
        # Try to create a window
        osascript -e "tell application \"$OSA_APP_NAME\" to create window with default profile" 2>/dev/null || true
        sleep 1
        WINDOW_COUNT=$(osascript -e "tell application \"$OSA_APP_NAME\" to get count of windows" 2>/dev/null || echo "0")
        if [[ "$WINDOW_COUNT" -ge 1 ]]; then
            pass_test "Windows: created new window successfully"
        else
            fail_test "Windows: could not create window"
        fi
    fi

    # Test basic terminal session
    echo "[E3] Testing terminal session..."
    SESSION_OUTPUT=$(osascript <<OSA 2>/dev/null || echo "")
tell application "$OSA_APP_NAME"
    tell current window
        tell current session
            write text "echo SMOKE_TEST_OK"
            delay 0.5
            return contents
        end tell
    end tell
end tell
OSA
    if [[ "$SESSION_OUTPUT" == *"SMOKE_TEST_OK"* ]]; then
        pass_test "Terminal session: echo command worked"
    else
        fail_test "Terminal session: could not verify output"
    fi
fi

# Clean up - terminate the app
echo ""
echo "Cleaning up..."
kill $APP_PID 2>/dev/null || true
rm -f "$PID_FILE"

# Summary
echo ""
echo "=== SMOKE TEST SUMMARY ==="
echo "Passed: $TESTS_PASSED"
echo "Failed: $TESTS_FAILED"

if [[ $TESTS_FAILED -gt 0 ]]; then
    echo ""
    echo "SMOKE TEST FAILED"
    exit 1
fi

echo ""
echo "=== SMOKE TEST PASSED ==="
exit 0
