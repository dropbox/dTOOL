#!/bin/bash
# Lint for performance anti-patterns
# Add to pre-commit hook to catch issues before they land

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

ERRORS=0
WARNINGS=0

echo "=== Performance Lint ==="

# Get list of staged .m, .swift files
if [ -n "$1" ]; then
    # Files passed as arguments
    FILES="$@"
else
    # Check all source files
    FILES=$(find "$PROJECT_DIR/sources" -name "*.m" -o -name "*.swift" 2>/dev/null)
fi

for FILE in $FILES; do
    [ -f "$FILE" ] || continue
    BASENAME=$(basename "$FILE")

    # ---------------------------------------------------------------------
    # ERROR: Default YES for comparison/validation modes
    # These double the work for no benefit in production
    # ---------------------------------------------------------------------
    if grep -qE "ComparisonEnabled.*YES|ValidationEnabled.*YES" "$FILE" 2>/dev/null; then
        if grep -qE "DEFINE_BOOL.*ComparisonEnabled.*YES|DEFINE_BOOL.*ValidationEnabled.*YES" "$FILE"; then
            echo -e "${RED}ERROR${NC}: $BASENAME - Comparison/Validation mode defaults to YES"
            echo "       This doubles processing overhead. Use NO for production."
            ERRORS=$((ERRORS + 1))
        fi
    fi

    # ---------------------------------------------------------------------
    # ERROR: Synchronous file I/O in application delegate startup
    # ---------------------------------------------------------------------
    if [[ "$BASENAME" == "iTermApplicationDelegate.m" ]]; then
        # Check applicationWillFinishLaunching for synchronous operations
        if grep -A 200 "applicationWillFinishLaunching:" "$FILE" | \
           grep -B 200 "^}" | \
           grep -qE "contentsOfFile:|stringWithContentsOfFile:|dataWithContentsOfFile:" 2>/dev/null; then
            echo -e "${RED}ERROR${NC}: $BASENAME - Synchronous file I/O in startup path"
            ERRORS=$((ERRORS + 1))
        fi
    fi

    # ---------------------------------------------------------------------
    # WARNING: Comments indicating dead code
    # ---------------------------------------------------------------------
    DEAD_CODE=$(grep -n "never called\|not called\|currently not called" "$FILE" 2>/dev/null | head -3)
    if [ -n "$DEAD_CODE" ]; then
        echo -e "${YELLOW}WARNING${NC}: $BASENAME - 'Never called' comments found:"
        echo "$DEAD_CODE" | sed 's/^/         /'
        WARNINGS=$((WARNINGS + 1))
    fi

    # ---------------------------------------------------------------------
    # WARNING: dispatch_sync on main queue (potential deadlock/blocking)
    # ---------------------------------------------------------------------
    if grep -qE "dispatch_sync.*dispatch_get_main_queue" "$FILE" 2>/dev/null; then
        echo -e "${YELLOW}WARNING${NC}: $BASENAME - dispatch_sync to main queue (can block)"
        WARNINGS=$((WARNINGS + 1))
    fi

    # ---------------------------------------------------------------------
    # WARNING: NSUserDefaults synchronize (deprecated, blocks)
    # ---------------------------------------------------------------------
    if grep -qE "\[.*synchronize\]" "$FILE" 2>/dev/null; then
        if ! grep -qE "// lint-ignore: synchronize" "$FILE"; then
            echo -e "${YELLOW}WARNING${NC}: $BASENAME - NSUserDefaults synchronize (deprecated, blocks)"
            WARNINGS=$((WARNINGS + 1))
        fi
    fi

    # ---------------------------------------------------------------------
    # WARNING: Running both old and new systems simultaneously
    # ---------------------------------------------------------------------
    if grep -qE "// Run both|// Execute both|// Compare both" "$FILE" 2>/dev/null; then
        echo -e "${YELLOW}WARNING${NC}: $BASENAME - Code running duplicate systems"
        WARNINGS=$((WARNINGS + 1))
    fi

done

echo ""
if [ $ERRORS -gt 0 ]; then
    echo -e "${RED}FAILED${NC}: $ERRORS errors, $WARNINGS warnings"
    exit 1
elif [ $WARNINGS -gt 0 ]; then
    echo -e "${YELLOW}PASSED${NC} with $WARNINGS warnings"
    exit 0
else
    echo -e "${GREEN}PASSED${NC}: No performance issues found"
    exit 0
fi
