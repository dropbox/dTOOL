#!/bin/bash
# DashFlow Verification with Checkpoint
# Runs cargo check/test with timeouts and records success
#
# Usage: ./scripts/verify_and_checkpoint.sh [--quick|--full] [--allow-warnings]
#
# --quick: Just cargo check (300s timeout for 108-crate workspace)
# --full:  cargo check + cargo test/nextest (timeouts applied)
# --allow-warnings: Record verification even if warnings exist (NOT recommended)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

MODE="--quick"
ALLOW_WARNINGS=0

for arg in "$@"; do
    case "$arg" in
        --quick|--full)
            MODE="$arg"
            ;;
        --allow-warnings)
            ALLOW_WARNINGS=1
            ;;
        -h|--help)
            echo "Usage: $0 [--quick|--full] [--allow-warnings]"
            exit 0
            ;;
        *)
            echo "ERROR: Unknown argument: $arg"
            echo "Usage: $0 [--quick|--full] [--allow-warnings]"
            exit 2
            ;;
    esac
done

HEAD_SHA=$(git rev-parse HEAD)
VERIFIED_FILE="$REPO_ROOT/.dashflow/verified_commits"

echo "=== DashFlow Verification ==="
echo "Mode: $MODE"
echo "HEAD: $HEAD_SHA"
echo "Allow warnings: $ALLOW_WARNINGS"
echo ""

# Check if already verified
if [ -f "$VERIFIED_FILE" ] && grep -q "$HEAD_SHA" "$VERIFIED_FILE" 2>/dev/null; then
    echo "HEAD is already verified. Skipping."
    echo "To force re-verification, remove $HEAD_SHA from $VERIFIED_FILE"
    exit 0
fi

# Run cargo check with timeout (108-crate workspace can be slow on cold caches)
CHECK_TIMEOUT_SECS="${DASHFLOW_CARGO_CHECK_TIMEOUT_SECS:-600}"
echo "Running cargo check (timeout: ${CHECK_TIMEOUT_SECS}s for 108 crates)..."
CHECK_LOG="$(mktemp -t dashflow-cargo-check.XXXXXX)"
trap 'rm -f "$CHECK_LOG"' EXIT

set +e
CARGO_TERM_COLOR=never timeout "${CHECK_TIMEOUT_SECS}" cargo check --color never >"$CHECK_LOG" 2>&1
CHECK_EXIT=$?
set -e

if [ $CHECK_EXIT -eq 124 ]; then
    echo "ERROR: cargo check timed out after ${CHECK_TIMEOUT_SECS}s"
    echo "Last output:"
    tail -20 "$CHECK_LOG"
    exit 1
fi

if [ $CHECK_EXIT -ne 0 ]; then
    echo "ERROR: cargo check failed (exit: $CHECK_EXIT)"
    echo "First errors:"
    grep -nE '^(error\\[|error:)' "$CHECK_LOG" | head -50 || true
    echo ""
    echo "Last output:"
    tail -50 "$CHECK_LOG"
    exit 1
fi

WARNING_COUNT="$(awk 'BEGIN{c=0} /^warning:/{c++} END{print c}' "$CHECK_LOG")"
if [ "$WARNING_COUNT" -gt 0 ]; then
    if [ "$ALLOW_WARNINGS" -eq 1 ]; then
        echo "cargo check: PASSED (warnings: $WARNING_COUNT)"
    else
        echo "ERROR: cargo check produced warnings ($WARNING_COUNT)."
        echo "Fix warnings before recording verification, or pass --allow-warnings."
        echo ""
        echo "First warnings:"
        grep -nE '^warning:' "$CHECK_LOG" | head -50 || true
        exit 1
    fi
else
    echo "cargo check: PASSED (no warnings)"
fi

if [ "$MODE" = "--full" ]; then
    echo ""
    # 108-crate workspace + cold cache can exceed 8 minutes just compiling tests.
    # Keep a hard timeout, but make it large enough to be usable on fresh machines.
    TEST_TIMEOUT_SECS="${DASHFLOW_CARGO_TEST_TIMEOUT_SECS:-3600}"
    echo "Running cargo test (timeout: ${TEST_TIMEOUT_SECS}s)..."
    # Use nextest if available, fallback to cargo test
    if command -v cargo-nextest &> /dev/null; then
        if ! timeout "${TEST_TIMEOUT_SECS}" cargo nextest run 2>&1; then
            echo "ERROR: cargo nextest failed or timed out"
            exit 1
        fi
    else
        if ! timeout "${TEST_TIMEOUT_SECS}" cargo test 2>&1; then
            echo "ERROR: cargo test failed or timed out"
            exit 1
        fi
    fi
    echo "cargo test: PASSED"
fi

# Record verification
mkdir -p "$(dirname "$VERIFIED_FILE")"
echo "$HEAD_SHA $(date -Iseconds) $MODE" >> "$VERIFIED_FILE"
echo ""
echo "=== Verification Complete ==="
echo "Recorded: $HEAD_SHA verified at $(date)"

# Keep only last 50 verified commits
if [ -f "$VERIFIED_FILE" ]; then
    tail -50 "$VERIFIED_FILE" > "$VERIFIED_FILE.tmp" && mv "$VERIFIED_FILE.tmp" "$VERIFIED_FILE"
fi
