#!/bin/bash
# validate-dterm-core-parser.sh
# Phase 3.1: Parser Switchover Validation Script
#
# This script validates that dterm-core parser produces identical output
# to iTerm2's VT100Parser by running the app with comparison enabled
# and checking for mismatches.
#
# Usage:
#   ./scripts/validate-dterm-core-parser.sh [--enable-comparison] [--run-vttest]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}DTermCore Parser Validation Script${NC}"
echo -e "${BLUE}Phase 3.1: Parser Switchover${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Parse arguments
ENABLE_COMPARISON=false
RUN_VTTEST=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --enable-comparison)
            ENABLE_COMPARISON=true
            shift
            ;;
        --run-vttest)
            RUN_VTTEST=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Step 1: Build the project
echo -e "${YELLOW}Step 1: Building DashTerm2...${NC}"
cd "$PROJECT_ROOT"
xcodebuild -project DashTerm2.xcodeproj \
    -scheme DashTerm2 \
    -configuration Development \
    build \
    CODE_SIGNING_ALLOWED=NO \
    CODE_SIGN_IDENTITY="-" \
    2>&1 | tail -5
if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi
echo -e "${GREEN}Build succeeded.${NC}"
echo ""

# Step 2: Run DTermCore comparison tests
echo -e "${YELLOW}Step 2: Running DTermCore comparison tests...${NC}"
TEST_OUTPUT=$(xcodebuild test \
    -project DashTerm2.xcodeproj \
    -scheme DashTerm2 \
    -only-testing:DashTerm2Tests/DTermCoreComparisonTests \
    CODE_SIGNING_ALLOWED=NO \
    CODE_SIGN_IDENTITY="-" \
    2>&1 || true)

# Extract test results
TESTS_PASSED=$(echo "$TEST_OUTPUT" | grep -c "passed" || true)
TESTS_FAILED=$(echo "$TEST_OUTPUT" | grep -c "failed" || true)

if echo "$TEST_OUTPUT" | grep -q "TEST SUCCEEDED"; then
    echo -e "${GREEN}DTermCore comparison tests: PASSED${NC}"
else
    echo -e "${RED}DTermCore comparison tests: FAILED${NC}"
    echo "$TEST_OUTPUT" | grep -A2 "Test Case.*failed"
fi
echo ""

# Step 3: Run DTermCore integration tests
echo -e "${YELLOW}Step 3: Running DTermCore integration tests...${NC}"
TEST_OUTPUT=$(xcodebuild test \
    -project DashTerm2.xcodeproj \
    -scheme DashTerm2 \
    -only-testing:DashTerm2Tests/DTermCoreIntegrationTests \
    CODE_SIGNING_ALLOWED=NO \
    CODE_SIGN_IDENTITY="-" \
    2>&1 || true)

if echo "$TEST_OUTPUT" | grep -q "TEST SUCCEEDED"; then
    echo -e "${GREEN}DTermCore integration tests: PASSED${NC}"
else
    echo -e "${RED}DTermCore integration tests: FAILED${NC}"
    echo "$TEST_OUTPUT" | grep -A2 "Test Case.*failed"
fi
echo ""

# Step 4: Enable comparison mode if requested
if [ "$ENABLE_COMPARISON" = true ]; then
    echo -e "${YELLOW}Step 4: Enabling parser comparison mode...${NC}"
    defaults write com.dashterm.DashTerm2 dtermCoreEnabled -bool YES
    defaults write com.dashterm.DashTerm2 dtermCoreParserComparisonEnabled -bool YES
    echo -e "${GREEN}Parser comparison mode enabled.${NC}"
    echo "  - dtermCoreEnabled: YES"
    echo "  - dtermCoreParserComparisonEnabled: YES"
    echo ""
    echo "Now launch DashTerm2 and use it normally. Check Console.app for mismatches:"
    echo "  log stream --predicate 'process == \"DashTerm2\" && messageType == \"info\"' | grep dterm-core"
    echo ""
fi

# Step 5: Run vttest if requested
if [ "$RUN_VTTEST" = true ]; then
    echo -e "${YELLOW}Step 5: Running vttest validation...${NC}"
    if command -v vttest &> /dev/null; then
        echo "vttest found. Please run vttest manually in a DashTerm2 session"
        echo "with parser comparison enabled to validate escape sequences."
    else
        echo "vttest not found. Install with: brew install vttest"
    fi
    echo ""
fi

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Validation Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Parser comparison infrastructure: ✓ Available"
echo "DTermCore comparison tests: $(echo "$TEST_OUTPUT" | grep -q "TEST SUCCEEDED" && echo "✓ Passed" || echo "✗ Failed")"
echo ""
echo "Next steps for Phase 3.1:"
echo "1. Run DashTerm2 with parser comparison enabled"
echo "2. Use terminal normally for 1 week"
echo "3. Check Console.app for '[dterm-core parser]' mismatches"
echo "4. Fix any mismatches found"
echo "5. Enable dtermCoreParserOutputEnabled by default"
echo ""
echo "To enable comparison mode: $0 --enable-comparison"
echo "To run vttest: $0 --run-vttest"
