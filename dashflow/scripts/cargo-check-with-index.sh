#!/bin/bash
# DashFlow Cargo Check with Type Index Regeneration (Phase 967)
#
# Wrapper around `cargo check` that automatically regenerates the type index
# when Rust source files have been modified since the last index generation.
#
# Usage:
#   ./scripts/cargo-check-with-index.sh [cargo-check-args...]
#   ./scripts/cargo-check-with-index.sh -p dashflow
#   ./scripts/cargo-check-with-index.sh --all-targets
#
# Environment:
#   DASHFLOW_INDEX_SKIP=1   Skip index regeneration
#   DASHFLOW_INDEX_FORCE=1  Force index regeneration regardless of staleness

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

INDEX_PATH=".dashflow/index/types.json"
SKIP_INDEX="${DASHFLOW_INDEX_SKIP:-0}"
FORCE_INDEX="${DASHFLOW_INDEX_FORCE:-0}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Find dashflow binary
find_dashflow() {
    if [ -x "$REPO_ROOT/target/release/dashflow" ]; then
        echo "$REPO_ROOT/target/release/dashflow"
    elif [ -x "$REPO_ROOT/target/debug/dashflow" ]; then
        echo "$REPO_ROOT/target/debug/dashflow"
    elif command -v dashflow &>/dev/null; then
        command -v dashflow
    else
        echo ""
    fi
}

# Check if index is stale
is_index_stale() {
    if [ ! -f "$INDEX_PATH" ]; then
        return 0  # Missing = stale
    fi

    # Compare index mtime with most recent .rs file
    for dir in crates/*/src; do
        if [ -d "$dir" ]; then
            if find "$dir" -name "*.rs" -newer "$INDEX_PATH" 2>/dev/null | head -1 | grep -q .; then
                return 0  # Found newer file = stale
            fi
        fi
    done

    return 1  # Not stale
}

# Regenerate index
regenerate_index() {
    local dashflow_bin
    dashflow_bin=$(find_dashflow)

    if [ -z "$dashflow_bin" ]; then
        echo -e "${YELLOW}Warning: dashflow binary not found, cannot regenerate index${NC}"
        echo -e "${YELLOW}Build with: cargo build --release -p dashflow-cli${NC}"
        return 1
    fi

    echo -e "${CYAN}Regenerating type index...${NC}"
    if "$dashflow_bin" introspect index --rebuild; then
        echo -e "${GREEN}Type index regenerated successfully${NC}"
        return 0
    else
        echo -e "${RED}Failed to regenerate type index${NC}"
        return 1
    fi
}

# Main execution
main() {
    # Run cargo check first
    echo -e "${CYAN}Running cargo check...${NC}"
    if ! cargo check "$@"; then
        echo -e "${RED}cargo check failed${NC}"
        exit 1
    fi
    echo -e "${GREEN}cargo check passed${NC}"

    # Skip index regeneration if requested
    if [ "$SKIP_INDEX" = "1" ]; then
        echo -e "${YELLOW}Skipping index regeneration (DASHFLOW_INDEX_SKIP=1)${NC}"
        exit 0
    fi

    # Check if index needs regeneration
    if [ "$FORCE_INDEX" = "1" ]; then
        echo -e "${YELLOW}Forcing index regeneration (DASHFLOW_INDEX_FORCE=1)${NC}"
        regenerate_index || true
    elif is_index_stale; then
        echo -e "${YELLOW}Type index is stale, regenerating...${NC}"
        regenerate_index || true
    else
        echo -e "${GREEN}Type index is up to date${NC}"
    fi
}

main "$@"
