#!/bin/bash
# DashFlow Pre-commit Hook: Type Index Freshness Check
#
# This hook checks if the type index is stale and optionally rebuilds it.
# Install by copying to .git/hooks/pre-commit or by sourcing from the hook.
#
# Usage:
#   ./scripts/pre-commit-type-index.sh           # Check only
#   ./scripts/pre-commit-type-index.sh --rebuild # Rebuild if stale

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

INDEX_PATH=".dashflow/index/types.json"
REBUILD_FLAG="${1:-}"

# Check if dashflow binary exists
DASHFLOW_BIN="$REPO_ROOT/target/release/dashflow"
if [ ! -x "$DASHFLOW_BIN" ]; then
    DASHFLOW_BIN="$REPO_ROOT/target/debug/dashflow"
fi

if [ ! -x "$DASHFLOW_BIN" ]; then
    # Try to find it in PATH
    DASHFLOW_BIN="$(which dashflow 2>/dev/null || true)"
fi

# Function to check index staleness manually (fallback if binary not available)
check_stale_manual() {
    if [ ! -f "$INDEX_PATH" ]; then
        echo "missing"
        return
    fi

    # Get index file modification time
    INDEX_MTIME=$(stat -f %m "$INDEX_PATH" 2>/dev/null || stat -c %Y "$INDEX_PATH" 2>/dev/null)

    # Check key source directories for newer files
    for dir in crates/dashflow/src crates/dashflow-opensearch/src crates/dashflow-openai/src; do
        if [ -d "$dir" ]; then
            NEWEST=$(find "$dir" -name "*.rs" -newer "$INDEX_PATH" 2>/dev/null | head -1)
            if [ -n "$NEWEST" ]; then
                echo "stale"
                return
            fi
        fi
    done

    echo "fresh"
}

# Main logic
if [ -x "$DASHFLOW_BIN" ]; then
    # Use dashflow binary for authoritative check
    STATUS=$("$DASHFLOW_BIN" introspect index --json 2>/dev/null | grep -o '"status":\s*"[^"]*"' | cut -d'"' -f4 || echo "unknown")
else
    # Fallback to manual check
    STATUS=$(check_stale_manual)
fi

case "$STATUS" in
    "fresh")
        echo "Type index is fresh."
        exit 0
        ;;
    "stale"|"missing")
        if [ "$REBUILD_FLAG" = "--rebuild" ]; then
            echo "Type index is $STATUS. Rebuilding..."
            if [ -x "$DASHFLOW_BIN" ]; then
                "$DASHFLOW_BIN" introspect index --rebuild
            else
                echo "ERROR: dashflow binary not found. Build with 'cargo build --release -p dashflow-cli'"
                exit 1
            fi
        else
            echo "WARNING: Type index is $STATUS."
            echo "Run 'dashflow introspect index --rebuild' to update."
            # Don't block commit, just warn
            exit 0
        fi
        ;;
    *)
        echo "Could not determine index status: $STATUS"
        exit 0
        ;;
esac
