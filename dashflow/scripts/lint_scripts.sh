#!/bin/bash
# scripts/lint_scripts.sh - Lint shell scripts for style compliance
#
# Usage:
#   ./scripts/lint_scripts.sh           # Lint all scripts
#   ./scripts/lint_scripts.sh --fix     # Show fix suggestions
#   ./scripts/lint_scripts.sh FILE...   # Lint specific files
#
# Checks:
#   1. Shebang line (#!/bin/bash)
#   2. Strict mode (set -euo pipefail or set -e)
#   3. Python3 vs python usage
#
# See docs/SCRIPT_STYLE.md for style guide.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

SHOW_FIX=false
FILES=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --help|-h)
            echo "Usage: $0 [--fix] [FILE...]"
            echo ""
            echo "Options:"
            echo "  --fix    Show fix suggestions"
            echo "  --help   Show this help"
            echo ""
            echo "Lint all scripts in scripts/ or specific files."
            exit 0
            ;;
        --fix|-f)
            SHOW_FIX=true
            shift
            ;;
        *)
            FILES+=("$1")
            shift
            ;;
    esac
done

# Default to all scripts
if [ ${#FILES[@]} -eq 0 ]; then
    shopt -s nullglob
    FILES=(scripts/*.sh ./*.sh)
fi

ERRORS=0
WARNINGS=0

lint_file() {
    local file="$1"
    local filename
    filename=$(basename "$file")
    local has_error=false

    # Skip self
    if [ "$filename" = "lint_scripts.sh" ]; then
        return
    fi

    # Check shebang (accept both #!/bin/bash and #!/usr/bin/env bash)
    if ! head -1 "$file" | grep -qE '^#!(\/bin\/bash|\/usr\/bin\/env bash)'; then
        echo "ERROR: $file: Missing bash shebang (use #!/bin/bash or #!/usr/bin/env bash)"
        ERRORS=$((ERRORS + 1))
        has_error=true
    fi

    # Check for strict mode (prefer pipefail, but accept set -e)
    if grep -q "set -euo pipefail" "$file"; then
        : # Best practice
    elif grep -q "set -e" "$file"; then
        echo "WARN:  $file: Uses 'set -e' but not 'set -euo pipefail'"
        WARNINGS=$((WARNINGS + 1))
        if [ "$SHOW_FIX" = true ]; then
            echo "  FIX: Replace 'set -e' with 'set -euo pipefail'"
        fi
    else
        echo "ERROR: $file: Missing 'set -e' or 'set -euo pipefail'"
        ERRORS=$((ERRORS + 1))
        has_error=true
        if [ "$SHOW_FIX" = true ]; then
            echo "  FIX: Add 'set -euo pipefail' after shebang"
        fi
    fi

    # Check for python vs python3 (detect python command invocation, not paths like scripts/python/*)
    if grep -E '(^|[^[:alnum:]_./-])python([^[:alnum:]_]|$)' "$file" | grep -qv "^#"; then
        echo "WARN:  $file: Uses 'python' instead of 'python3'"
        WARNINGS=$((WARNINGS + 1))
        if [ "$SHOW_FIX" = true ]; then
            echo "  FIX: Replace 'python ' with 'python3 '"
        fi
    fi

    # Check for negative PID in kill (process group kill)
    # Matches: kill -TERM -$PID, kill -9 -1234 (negative PID = process group)
    # Does NOT match: kill -0 $PID (signal 0 = check if process exists)
    if grep -E 'kill\s+(-[A-Z]+|-[0-9]+)\s+-[0-9\$]' "$file" | grep -qv "^#"; then
        echo "ERROR: $file: Uses negative PID with kill (process group) - dangerous"
        ERRORS=$((ERRORS + 1))
        has_error=true
        if [ "$SHOW_FIX" = true ]; then
            echo "  FIX: Kill individual processes, not process groups"
        fi
    fi

    if [ "$has_error" = false ] && [ $WARNINGS -eq 0 ]; then
        : # Don't print OK for clean files to reduce noise
    fi
}

echo "=== Script Style Linter ==="
echo "Checking ${#FILES[@]} files..."
echo ""

for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        lint_file "$file"
    fi
done

echo ""
echo "=== Summary ==="
echo "Errors:   $ERRORS"
echo "Warnings: $WARNINGS"

if [ $ERRORS -gt 0 ]; then
    echo ""
    echo "See docs/SCRIPT_STYLE.md for style guidelines."
    exit 1
fi

if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo "All scripts pass lint checks!"
fi

exit 0
