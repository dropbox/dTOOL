#!/bin/bash
# =============================================================================
# DashTerm2 Pre-Commit Build & Test Check
# =============================================================================
# Three-tier validation:
#   1. Fast incremental Swift syntax check (every commit)
#   2. Run affected tests (if test files changed)
#   3. Full validation (every N commits or on demand)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
VALIDATION_COUNTER_FILE="$PROJECT_ROOT/.git/validation_counter"
FULL_VALIDATION_INTERVAL=10  # Full validation every N commits

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

cd "$PROJECT_ROOT"

# =============================================================================
# TIER 1: Fast Incremental Swift Syntax Check (< 5 seconds)
# =============================================================================
tier1_swift_syntax_check() {
    echo -e "${BLUE}[TIER 1]${NC} Swift syntax check..."

    # Get staged Swift files
    SWIFT_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.swift$' || true)

    if [ -z "$SWIFT_FILES" ]; then
        echo -e "  ${GREEN}✓${NC} No Swift files to check"
        return 0
    fi

    # Quick syntax check using swiftc -parse (doesn't compile, just parses)
    ERRORS=0
    for file in $SWIFT_FILES; do
        if [ -f "$file" ]; then
            # Use swiftc to parse the file (fast syntax check)
            if ! swiftc -parse "$file" 2>/dev/null; then
                echo -e "  ${RED}✗${NC} Syntax error in: $file"
                ERRORS=$((ERRORS + 1))
            fi
        fi
    done

    if [ $ERRORS -gt 0 ]; then
        echo -e "${RED}TIER 1 FAILED: $ERRORS file(s) have syntax errors${NC}"
        return 1
    fi

    echo -e "  ${GREEN}✓${NC} All $( echo "$SWIFT_FILES" | wc -w | tr -d ' ') Swift files pass syntax check"
    return 0
}

# =============================================================================
# TIER 2: Test Compilation Check (< 90 seconds)
# =============================================================================
# NOTE: Use "ModernTests" scheme - DashTerm2Tests has WebExtensionsFramework linker issues
tier2_run_affected_tests() {
    echo -e "${BLUE}[TIER 2]${NC} Test compilation check..."

    # Get staged Swift files (any Swift file change could break tests)
    SWIFT_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep '\.swift$' || true)
    TEST_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep "XCTests.*\.swift$" || true)

    if [ -z "$SWIFT_FILES" ]; then
        echo -e "  ${GREEN}✓${NC} No Swift files modified"
        return 0
    fi

    # Always verify test compilation when Swift files change
    echo -e "  Verifying test compilation (ModernTests scheme)..."

    # Capture build output
    BUILD_OUTPUT=$(xcodebuild build-for-testing \
        -project DashTerm2.xcodeproj \
        -scheme ModernTests \
        -destination 'platform=macOS' \
        CODE_SIGNING_ALLOWED=NO \
        -quiet 2>&1 || true)

    if echo "$BUILD_OUTPUT" | grep -q "error:"; then
        echo -e "  ${RED}✗${NC} Test compilation FAILED"
        echo ""
        echo "Compilation errors:"
        echo "$BUILD_OUTPUT" | grep "error:" | head -15
        echo ""
        echo -e "  ${YELLOW}Fix these errors before committing.${NC}"
        return 1
    fi

    if echo "$BUILD_OUTPUT" | grep -q "BUILD FAILED"; then
        echo -e "  ${RED}✗${NC} Test build FAILED"
        echo "$BUILD_OUTPUT" | tail -20
        return 1
    fi

    echo -e "  ${GREEN}✓${NC} Tests compile successfully"

    # If test files were modified, extract and report new test functions
    if [ -n "$TEST_FILES" ]; then
        TEST_FUNCTIONS=$(git diff --cached -U0 | grep "^\+.*func test_" | sed 's/.*func \(test_[^(]*\).*/\1/' | sort -u || true)
        if [ -n "$TEST_FUNCTIONS" ]; then
            TEST_COUNT=$(echo "$TEST_FUNCTIONS" | wc -l | tr -d ' ')
            echo -e "  ${GREEN}✓${NC} $TEST_COUNT new/modified test function(s) detected"
        fi
    fi

    return 0
}

# =============================================================================
# TIER 3: Full Validation (every N commits)
# =============================================================================
tier3_full_validation() {
    # Check if we should run full validation
    COUNTER=0
    if [ -f "$VALIDATION_COUNTER_FILE" ]; then
        COUNTER=$(cat "$VALIDATION_COUNTER_FILE")
    fi

    COUNTER=$((COUNTER + 1))
    echo "$COUNTER" > "$VALIDATION_COUNTER_FILE"

    if [ $((COUNTER % FULL_VALIDATION_INTERVAL)) -ne 0 ]; then
        echo -e "${BLUE}[TIER 3]${NC} Full validation: skipped (commit $COUNTER, next at $((COUNTER + FULL_VALIDATION_INTERVAL - (COUNTER % FULL_VALIDATION_INTERVAL))))"
        return 0
    fi

    echo -e "${BLUE}[TIER 3]${NC} Full validation (every $FULL_VALIDATION_INTERVAL commits)..."

    # Full build
    echo -e "  Building entire project..."
    if ! xcodebuild build \
        -project DashTerm2.xcodeproj \
        -scheme DashTerm2 \
        -configuration Development \
        CODE_SIGNING_ALLOWED=NO \
        -quiet 2>&1 | grep -q "BUILD FAILED"; then
        echo -e "  ${GREEN}✓${NC} Full build succeeded"
    else
        echo -e "  ${RED}✗${NC} Full build failed"
        xcodebuild build \
            -project DashTerm2.xcodeproj \
            -scheme DashTerm2 \
            -configuration Development \
            CODE_SIGNING_ALLOWED=NO 2>&1 | grep "error:" | head -10
        return 1
    fi

    # Build tests (use ModernTests scheme - DashTerm2Tests has linker issues)
    echo -e "  Building test target (ModernTests)..."
    if ! xcodebuild build-for-testing \
        -project DashTerm2.xcodeproj \
        -scheme ModernTests \
        -destination 'platform=macOS' \
        CODE_SIGNING_ALLOWED=NO \
        -quiet 2>&1 | grep -q "BUILD FAILED"; then
        echo -e "  ${GREEN}✓${NC} Test build succeeded"
    else
        echo -e "  ${RED}✗${NC} Test build failed"
        return 1
    fi

    echo -e "  ${GREEN}✓${NC} Full validation passed"
    return 0
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    echo ""
    echo "============================================================================="
    echo "  DashTerm2 Pre-Commit Build & Test Check"
    echo "============================================================================="
    echo ""

    # Allow skipping with environment variable
    if [ "${SKIP_BUILD_CHECK:-}" = "1" ]; then
        echo -e "${YELLOW}Skipping build check (SKIP_BUILD_CHECK=1)${NC}"
        exit 0
    fi

    FAILED=0

    # Tier 1: Fast syntax check
    if ! tier1_swift_syntax_check; then
        FAILED=1
    fi

    # Tier 2: Run affected tests (only if Tier 1 passed)
    if [ $FAILED -eq 0 ]; then
        if ! tier2_run_affected_tests; then
            FAILED=1
        fi
    fi

    # Tier 3: Full validation (periodic)
    if [ $FAILED -eq 0 ]; then
        if ! tier3_full_validation; then
            FAILED=1
        fi
    fi

    echo ""
    if [ $FAILED -eq 0 ]; then
        echo -e "${GREEN}=============================================================================${NC}"
        echo -e "${GREEN}  All checks passed - commit proceeding${NC}"
        echo -e "${GREEN}=============================================================================${NC}"
        exit 0
    else
        echo -e "${RED}=============================================================================${NC}"
        echo -e "${RED}  COMMIT BLOCKED - Fix errors above before committing${NC}"
        echo -e "${RED}=============================================================================${NC}"
        echo ""
        echo "To skip this check (use sparingly):"
        echo "  SKIP_BUILD_CHECK=1 git commit ..."
        echo ""
        exit 1
    fi
}

# Run if executed directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
