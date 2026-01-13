#!/bin/bash
# scripts/check_ignore_reasons.sh - M-27: Require justification strings for ignored tests
#
# Usage:
#   ./scripts/check_ignore_reasons.sh           # Check all Rust files
#   ./scripts/check_ignore_reasons.sh FILE...   # Check specific files
#   ./scripts/check_ignore_reasons.sh --staged  # Check only staged files (for pre-commit)
#
# This script enforces that all #[ignore] attributes have reason strings:
#   GOOD: #[ignore = "requires external service"]
#   BAD:  #[ignore]
#
# Part of the "LITERALLY PERFECT" quality goal - tests must be traceable.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Pattern for bare #[ignore] without reason string
# Matches: #[ignore] but NOT #[ignore = "..."]
# Must be at start of line (after optional whitespace) to avoid matching comments
BARE_IGNORE_PATTERN='^\s*#\[ignore\s*\]'

check_file() {
    local file="$1"
    local filepath="$file"

    # Make path absolute if relative
    if [[ ! "$filepath" = /* ]]; then
        filepath="$REPO_ROOT/$file"
    fi

    if [ ! -f "$filepath" ]; then
        return 0
    fi

    # Only check Rust files
    if [[ ! "$filepath" =~ \.rs$ ]]; then
        return 0
    fi

    # Find bare #[ignore] without reason strings
    local matches
    matches=$(grep -nE "$BARE_IGNORE_PATTERN" "$filepath" 2>/dev/null || true)

    if [ -n "$matches" ]; then
        echo "ERROR: Bare #[ignore] found in $file (M-27 violation)"
        echo "$matches" | while read -r line; do
            echo "  $line"
        done
        return 1
    fi
    return 0
}

# Parse arguments
STAGED_MODE=false
FILES=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --staged)
            STAGED_MODE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--staged] [FILE...]"
            echo ""
            echo "Check that all #[ignore] test attributes have reason strings."
            echo ""
            echo "Options:"
            echo "  --staged    Check only git staged files"
            echo "  FILE...     Check specific files"
            echo ""
            echo "Examples:"
            echo "  $0                          # Check all crates/"
            echo "  $0 --staged                 # Check staged files (for pre-commit)"
            echo "  $0 crates/dashflow/src/     # Check specific path"
            exit 0
            ;;
        *)
            FILES+=("$1")
            shift
            ;;
    esac
done

echo "=== M-27: Ignored Test Justification Check ==="
echo ""

ERRORS=0

if [ "$STAGED_MODE" = true ]; then
    # Check only staged Rust files
    STAGED_FILES=$(git diff --cached --name-only 2>/dev/null | grep '\.rs$' || true)

    if [ -z "$STAGED_FILES" ]; then
        echo "No staged Rust files to check."
        exit 0
    fi

    for file in $STAGED_FILES; do
        if ! check_file "$file"; then
            ERRORS=$((ERRORS + 1))
        fi
    done
elif [ ${#FILES[@]} -gt 0 ]; then
    # Check specified files/directories
    for file in "${FILES[@]}"; do
        if [ -d "$file" ]; then
            while IFS= read -r -d '' rsfile; do
                if ! check_file "$rsfile"; then
                    ERRORS=$((ERRORS + 1))
                fi
            done < <(find "$file" -name "*.rs" -print0 2>/dev/null)
        else
            if ! check_file "$file"; then
                ERRORS=$((ERRORS + 1))
            fi
        fi
    done
else
    # Check all Rust files in crates/
    while IFS= read -r -d '' file; do
        if ! check_file "$file"; then
            ERRORS=$((ERRORS + 1))
        fi
    done < <(find "$REPO_ROOT/crates" -name "*.rs" -print0 2>/dev/null)
fi

echo ""
if [ $ERRORS -eq 0 ]; then
    echo "All #[ignore] attributes have reason strings."
    exit 0
else
    echo "Found $ERRORS file(s) with bare #[ignore] attributes."
    echo ""
    echo "To fix, add a reason string:"
    echo '  #[ignore = "requires external service"]'
    echo '  #[ignore = "requires API_KEY environment variable"]'
    echo '  #[ignore = "requires Docker for testcontainers"]'
    echo ""
    echo "Standard reasons:"
    echo '  - "requires X server/service"'
    echo '  - "requires X_API_KEY environment variable"'
    echo '  - "requires Docker for testcontainers"'
    echo '  - "flaky test - see issue #NNN"'
    exit 1
fi
