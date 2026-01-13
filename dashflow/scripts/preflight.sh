#!/bin/bash
# DashFlow Pre-flight Check
# Run BEFORE starting work to avoid wasted time
#
# Usage: ./scripts/preflight.sh
#
# This script:
# 1. Checks for stale cargo locks and cleans them
# 2. Verifies no hanging test processes
# 3. Checks if HEAD is already verified (skip re-verification)
# 4. Verifies proto schema is in sync (M-117)
# 5. Sets up isolated build environment

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== DashFlow Pre-flight Check ==="
echo "Repository: $REPO_ROOT"
echo "HEAD: $(git rev-parse --short HEAD)"
echo ""

# 1. Kill any stale cargo/rustc processes older than 10 minutes
echo "1. Checking for stale cargo processes..."
STALE_PIDS=$(ps aux | grep -E 'cargo|rustc' | grep -v grep | awk '{
    cmd="ps -o etime= -p "$2
    cmd | getline elapsed
    close(cmd)
    if (elapsed ~ /:/ && elapsed !~ /-/) {
        split(elapsed, a, ":")
        if (length(a) == 3 && a[1] >= 10) print $2
        if (length(a) == 2 && a[1] >= 10) print $2
    }
}' || true)

if [ -n "$STALE_PIDS" ]; then
    echo "   Found stale processes: $STALE_PIDS"
    echo "   Killing..."
    echo "$STALE_PIDS" | xargs kill 2>/dev/null || true
    echo "   Done."
else
    echo "   No stale processes found."
fi

# 2. Check for cargo lock files and clean if stale
echo ""
echo "2. Checking cargo locks..."
LOCK_FILE="$HOME/.cargo/.package-cache"
if [ -f "$LOCK_FILE" ]; then
    LOCK_AGE=$(($(date +%s) - $(stat -f %m "$LOCK_FILE" 2>/dev/null || stat -c %Y "$LOCK_FILE" 2>/dev/null)))
    if [ "$LOCK_AGE" -gt 600 ]; then
        echo "   Stale cargo lock (${LOCK_AGE}s old). Removing..."
        rm -f "$LOCK_FILE"
    else
        echo "   Cargo lock exists but recent (${LOCK_AGE}s old)."
    fi
else
    echo "   No cargo lock file."
fi

# 3. Check if HEAD is already verified
echo ""
echo "3. Checking verification status..."
VERIFIED_FILE="$REPO_ROOT/.dashflow/verified_commits"
HEAD_SHA=$(git rev-parse HEAD)

if [ -f "$VERIFIED_FILE" ] && grep -q "$HEAD_SHA" "$VERIFIED_FILE" 2>/dev/null; then
    echo "   HEAD $HEAD_SHA is ALREADY VERIFIED."
    echo "   Skip re-running cargo check/test unless you changed code."
    export DASHFLOW_SKIP_VERIFICATION=1
else
    echo "   HEAD $HEAD_SHA not yet verified."
    echo "   Run verification after your changes."
fi

# 4. Check proto schema sync (M-117)
echo ""
echo "4. Checking proto schema sync..."
if [ -f "$REPO_ROOT/scripts/verify_proto_schema.sh" ]; then
    if "$REPO_ROOT/scripts/verify_proto_schema.sh" > /dev/null 2>&1; then
        echo "   Proto schema is in sync."
    else
        echo "   WARNING: Proto schema out of sync!"
        echo "   Run: ./scripts/verify_proto_schema.sh --fix"
        # Don't fail preflight, just warn - schema sync is fixable
    fi
else
    echo "   Proto verification script not found. Skipping."
fi

# 5. Set up isolated target directory to avoid lock contention
echo ""
echo "5. Setting up isolated build environment..."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
echo "   CARGO_TARGET_DIR=$CARGO_TARGET_DIR"

# 6. Summary
echo ""
echo "=== Pre-flight Complete ==="
if [ "${DASHFLOW_SKIP_VERIFICATION:-0}" = "1" ]; then
    echo "STATUS: HEAD already verified. Focus on your task, not verification."
else
    echo "STATUS: Ready for work. Verify after making changes."
fi
echo ""
echo "Tips for workers:"
echo "  - Don't run 'cargo check' in loops - run ONCE after changes"
echo "  - Use 'timeout 300 cargo test' to prevent hangs"
echo "  - If build seems stuck, run this script again"
echo ""
