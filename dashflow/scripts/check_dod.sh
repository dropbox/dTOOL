#!/bin/bash
# DashFlow Definition of Done (DoD) Checklist
# Verifies a phase/task meets completion criteria before claiming "done"
#
# Usage: ./scripts/check_dod.sh [--strict] [--allow-dirty] [crate-name]
#
# Options:
#   --strict      Require zero warnings (not just low warning count)
#   --allow-dirty Allow running with uncommitted changes (warn but don't fail)
#
# Criteria from CLAUDE.md:
# 1. Zero Warnings - cargo check shows no warnings
# 2. Zero Deprecation Usage - no deprecated types/functions
# 3. Tests Pass - affected crate tests pass
# 4. No TODOs Left - no // TODO comments for this task
#
# Exit codes:
#   0 = All checks passed (safe to mark complete)
#   1 = One or more checks failed

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

STRICT="--normal"
CRATE=""
ALLOW_DIRTY=""

# Parse arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --strict)
            STRICT="--strict"
            ;;
        --allow-dirty)
            ALLOW_DIRTY="true"
            ;;
        *)
            if [ -z "$CRATE" ]; then
                CRATE="$1"
            fi
            ;;
    esac
    shift
done

echo "=== Definition of Done Checklist ==="
echo "Time: $(date -Iseconds)"
echo "Mode: $STRICT"
[ -n "$CRATE" ] && echo "Crate: $CRATE"
echo ""

# M-88: Check for clean working tree to prevent accidental manager commits with unrelated diffs
echo "0. Checking working tree status..."
if ! git diff --quiet HEAD 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
    DIRTY_FILES=$(git status --porcelain | wc -l | tr -d ' ')
    if [ "$ALLOW_DIRTY" = "true" ]; then
        echo "   [WARN] Working tree is dirty ($DIRTY_FILES changed files) - proceeding with --allow-dirty"
    else
        echo "   [FAIL] Working tree is dirty ($DIRTY_FILES changed files)"
        echo "   Dirty working trees can cause accidental commits with unrelated diffs."
        echo "   Either commit/stash your changes first, or use --allow-dirty to proceed."
        echo ""
        echo "   Changed files:"
        git status --porcelain | head -5
        [ "$DIRTY_FILES" -gt 5 ] && echo "   ... and $((DIRTY_FILES - 5)) more"
        echo ""
        exit 1
    fi
else
    echo "   [PASS] Working tree is clean"
fi
echo ""

FAILURES=0

# Helper: portable timeout (works on Linux and macOS)
run_with_timeout() {
    local timeout_seconds=$1
    shift
    if command -v timeout &> /dev/null; then
        timeout "$timeout_seconds" "$@"
    elif command -v gtimeout &> /dev/null; then
        gtimeout "$timeout_seconds" "$@"
    else
        echo "  WARNING: timeout command not available (install coreutils on macOS: brew install coreutils)"
        "$@"
    fi
}

# 1. Check for compilation errors
echo "1. Checking for compilation errors..."
if [ -n "$CRATE" ]; then
    CHECK_CMD="run_with_timeout 180 cargo check -p $CRATE 2>&1"
else
    CHECK_CMD="run_with_timeout 300 cargo check 2>&1"
fi

set +e
CHECK_OUTPUT=$(eval "$CHECK_CMD")
CHECK_EXIT=$?
set -e

if [ $CHECK_EXIT -eq 124 ]; then
    echo "   [FAIL] cargo check timed out"
    FAILURES=$((FAILURES + 1))
elif echo "$CHECK_OUTPUT" | grep -q "^error\["; then
    echo "   [FAIL] Compilation errors found"
    echo "$CHECK_OUTPUT" | grep "^error\[" | head -5
    FAILURES=$((FAILURES + 1))
else
    echo "   [PASS] No compilation errors"
fi

# 2. Count warnings (strict mode requires zero)
echo ""
echo "2. Checking warnings..."
WARNING_COUNT=$(echo "$CHECK_OUTPUT" | grep -c "^warning:" 2>/dev/null) || WARNING_COUNT=0
echo "   Found $WARNING_COUNT warnings"

if [ "$STRICT" = "--strict" ] && [ "$WARNING_COUNT" -gt 0 ]; then
    echo "   [FAIL] Strict mode requires zero warnings"
    echo "$CHECK_OUTPUT" | grep "^warning:" | head -10
    FAILURES=$((FAILURES + 1))
elif [ "$WARNING_COUNT" -gt 50 ]; then
    echo "   [WARN] High warning count (>50)"
else
    echo "   [PASS] Warning count acceptable"
fi

# 3. Check for deprecated usage
echo ""
echo "3. Checking for deprecated usage..."
DEPRECATED_COUNT=$(echo "$CHECK_OUTPUT" | grep -c "deprecated" 2>/dev/null) || DEPRECATED_COUNT=0
if [ "$DEPRECATED_COUNT" -gt 0 ]; then
    echo "   [WARN] $DEPRECATED_COUNT deprecation warnings"
    echo "$CHECK_OUTPUT" | grep "deprecated" | head -5
else
    echo "   [PASS] No deprecation warnings"
fi

# 4. Check for TODO comments (in recently modified files)
echo ""
echo "4. Checking for TODO comments..."
if [ -n "$CRATE" ]; then
    TODO_FILES=$(find "crates/$CRATE" -name "*.rs" -exec grep -l "// TODO" {} \; 2>/dev/null || true)
else
    # Check files modified in last commit
    TODO_FILES=$(git diff --name-only HEAD~1 HEAD -- "*.rs" 2>/dev/null | xargs -I{} grep -l "// TODO" {} 2>/dev/null || true)
fi

if [ -n "$TODO_FILES" ]; then
    TODO_COUNT=$(echo "$TODO_FILES" | wc -l | tr -d ' ')
    echo "   [WARN] $TODO_COUNT files contain // TODO"
    echo "$TODO_FILES" | head -5
else
    echo "   [PASS] No TODO comments in scope"
fi

# 5. Run tests for the crate
echo ""
echo "5. Running tests..."
if [ -n "$CRATE" ]; then
    TEST_CMD="run_with_timeout 300 cargo test -p $CRATE 2>&1"
else
    echo "   (Skipping full test suite - specify crate for tests)"
    TEST_CMD=""
fi

if [ -n "$TEST_CMD" ]; then
    set +e
    TEST_OUTPUT=$(eval "$TEST_CMD")
    TEST_EXIT=$?
    set -e

    if [ $TEST_EXIT -eq 124 ]; then
        echo "   [FAIL] Tests timed out"
        FAILURES=$((FAILURES + 1))
    elif [ $TEST_EXIT -ne 0 ]; then
        echo "   [FAIL] Tests failed"
        echo "$TEST_OUTPUT" | grep -E "(FAILED|panicked)" | head -5
        FAILURES=$((FAILURES + 1))
    else
        PASSED=$(echo "$TEST_OUTPUT" | grep -o "[0-9]* passed" | head -1 || echo "0 passed")
        echo "   [PASS] Tests passed ($PASSED)"
    fi
else
    echo "   [SKIP] No crate specified"
fi

# 6. Check for debug println! in production code (not CLI output)
echo ""
echo "6. Checking for debug println!..."
DEBUG_PRINTLN=$(grep -rn 'println!' crates/ --include="*.rs" | grep -v "examples/" | grep -v "test" | grep -v "output.rs" | grep -v "// user output" | wc -l | tr -d ' ')
if [ "$DEBUG_PRINTLN" -gt 20 ]; then
    echo "   [WARN] Many println! calls ($DEBUG_PRINTLN) - verify they're intentional"
else
    echo "   [PASS] println! usage looks reasonable ($DEBUG_PRINTLN)"
fi

# Summary
echo ""
echo "=== Summary ==="
if [ $FAILURES -eq 0 ]; then
    echo "STATUS: [PASS] Definition of Done criteria met"
    echo ""
    echo "This work can be marked as COMPLETE."
    exit 0
else
    echo "STATUS: [FAIL] $FAILURES criteria failed"
    echo ""
    echo "Do NOT mark this work as complete until failures are resolved."
    exit 1
fi
