#!/bin/bash
# run-tests-with-crash-capture.sh - Run tests and automatically capture any crashes
#
# This wraps xcodebuild test commands to:
# 1. Record the start time
# 2. Run the tests
# 3. Capture any new crash reports after tests complete
# 4. Output a summary if crashes occurred
#
# Usage:
#   ./scripts/run-tests-with-crash-capture.sh [xcodebuild args...]
#   ./scripts/run-tests-with-crash-capture.sh -scheme DashTerm2Tests
#
# If no args provided, runs the default test suite

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CAPTURE_SCRIPT="$SCRIPT_DIR/capture-crashes.sh"

cd "$PROJECT_DIR"

# Fix UITests-Runner code signing issues
# The UITests-Runner can get corrupted signatures when building with CODE_SIGNING_ALLOWED=NO
# This clears quarantine attributes (no password needed) to prevent "damaged" alerts
fix_uitests_runner_signing() {
    # Clear quarantine attributes from DerivedData - this doesn't need a password
    xattr -cr ~/Library/Developer/Xcode/DerivedData/DashTerm2* 2>/dev/null || true
}

# Record start time for crash detection
START_TIME=$(date +%s)

echo "========================================"
echo "Running tests with crash capture"
echo "Started: $(date)"
echo "========================================"
echo ""

# Default test command if no arguments
# Aggressively disable all code signing to prevent keychain/password prompts
if [[ $# -eq 0 ]]; then
    TEST_ARGS=(
        -project DashTerm2.xcodeproj
        -scheme DashTerm2Tests
        -destination 'platform=macOS'
        CODE_SIGNING_ALLOWED=NO
        CODE_SIGNING_REQUIRED=NO
        CODE_SIGN_IDENTITY=""
        CODE_SIGN_ENTITLEMENTS=""
        DEVELOPMENT_TEAM=""
        PROVISIONING_PROFILE_SPECIFIER=""
    )
else
    TEST_ARGS=("$@")
fi

# Fix UITests-Runner signing before running tests
fix_uitests_runner_signing

# Run tests, capturing exit code
set +e
xcodebuild test "${TEST_ARGS[@]}"
TEST_EXIT_CODE=$?
set -e

echo ""
echo "========================================"
echo "Tests completed with exit code: $TEST_EXIT_CODE"
echo "========================================"

# Give macOS a moment to write crash reports
sleep 2

# Check for crashes that occurred since we started
echo ""
echo "Checking for crash reports..."
"$CAPTURE_SCRIPT" --since "$(date -r $START_TIME '+%Y-%m-%d %H:%M:%S')" || true

# If tests failed and we found crashes, provide helpful output
if [[ $TEST_EXIT_CODE -ne 0 ]]; then
    CRASH_SUMMARY="$PROJECT_DIR/worker_logs/app_crashes.summary"
    if [[ -f "$CRASH_SUMMARY" ]]; then
        echo ""
        echo "========================================"
        echo "CRASH REPORTS CAPTURED"
        echo "========================================"
        echo ""
        echo "Crash summary available at: $CRASH_SUMMARY"
        echo "Individual reports in: $PROJECT_DIR/worker_logs/app_crashes/"
        echo ""
        echo "To view summary:"
        echo "  cat $CRASH_SUMMARY"
        echo ""
    fi
fi

exit $TEST_EXIT_CODE
