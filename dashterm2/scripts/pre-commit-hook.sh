#!/bin/bash
# =============================================================================
# DashTerm2 Pre-Commit Hook
# NASA/NSA Grade - Zero Defect Tolerance
# =============================================================================
# This hook runs before every commit to catch issues early.
# If ANY check fails, the commit is BLOCKED.
# =============================================================================

set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ERRORS=0

log_section() { echo -e "\n${BLUE}═══ $1 ═══${NC}"; }
log_success() { echo -e "${GREEN}✓${NC} $1"; }
log_error() { echo -e "${RED}✗${NC} $1"; ((ERRORS++)); }
log_warn() { echo -e "${YELLOW}⚠${NC} $1"; }
log_info() { echo -e "${BLUE}ℹ${NC} $1"; }

echo ""
echo "============================================================================="
echo "  DashTerm2 Pre-Commit Check - NASA/NSA Grade"
echo "============================================================================="

# Get staged files
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM)
STAGED_SWIFT=$(echo "$STAGED_FILES" | grep '\.swift$' || true)
STAGED_OBJC=$(echo "$STAGED_FILES" | grep -E '\.(m|mm|h)$' || true)
STAGED_C=$(echo "$STAGED_FILES" | grep -E '\.(c|cpp|cc|cxx)$' || true)
STAGED_PYTHON=$(echo "$STAGED_FILES" | grep '\.py$' || true)
STAGED_SHELL=$(echo "$STAGED_FILES" | grep '\.sh$' || true)
STAGED_JS=$(echo "$STAGED_FILES" | grep -E '\.(js|jsx|ts|tsx)$' || true)
STAGED_METAL=$(echo "$STAGED_FILES" | grep '\.metal$' || true)

# =============================================================================
# TIER 0: Critical Checks (Block on any failure)
# =============================================================================
log_section "TIER 0: Critical Checks"

# Check for DO NOT SUBMIT markers
log_info "Checking for DO NOT SUBMIT markers..."
if git diff --cached | grep -iE "(DO NOT SUBMIT|DO NOT MERGE|DONOTSUBMIT|WIP:|FIXME:.*BEFORE COMMIT)" > /dev/null 2>&1; then
    log_error "Found DO NOT SUBMIT or similar marker in staged changes"
else
    log_success "No DO NOT SUBMIT markers"
fi

# Check for debug code markers
log_info "Checking for debug code..."
if git diff --cached | grep -E "(#warning|console\.log|debugger;|NSLog.*TODO|print\(.*DEBUG)" > /dev/null 2>&1; then
    log_warn "Found potential debug code in staged changes - please review"
fi

# Check for secrets (exclude test files and class/method names)
log_info "Checking for potential secrets..."
# Exclude test files from secret detection (they legitimately reference classes like VT100Token, PasswordManager etc.)
# Also exclude CamelCase identifiers (class names) and method names
SECRET_PATTERNS="(password|passwd|secret|api_key|apikey|token|private_key|BEGIN.*PRIVATE KEY)"
# Only check non-test files for secrets (Test files often have class names with Token, Password etc.)
NON_TEST_STAGED=$(echo "$STAGED_FILES" | grep -v "Tests\/" | grep -v "Test\.swift$" | grep -v "Tests\.swift$" || true)
if [ -n "$NON_TEST_STAGED" ]; then
    if git diff --cached -- $NON_TEST_STAGED 2>/dev/null | grep -iE "$SECRET_PATTERNS" | grep -vE "(//|/\*|\*|#)" | grep -vE "[A-Z][a-z]+Token|[A-Z][a-z]+Password|[a-z]+Token[A-Z]|[a-z]+Password[A-Z]" > /dev/null 2>&1; then
        log_error "Potential secret found in staged changes!"
    else
        log_success "No obvious secrets detected"
    fi
else
    log_success "No obvious secrets detected (only test files staged)"
fi

# =============================================================================
# TIER 0.5: FAKE TEST DETECTION (BLOCK COMMITS!)
# =============================================================================
log_section "TIER 0.5: Fake Test Detection"

# Check if BugRegressionTests.swift is staged
if echo "$STAGED_FILES" | grep -q "BugRegressionTests.swift"; then
    log_info "Checking for FAKE test patterns in BugRegressionTests.swift..."

    # Get the diff for this file
    DIFF_CONTENT=$(git diff --cached -- DashTerm2Tests/BugRegressionTests.swift)

    # Count new fake patterns being added (lines starting with +)
    FAKE_CLASS_CHECK=$(echo "$DIFF_CONTENT" | grep -c '^\+.*XCTAssertNotNil(NSClassFromString' || true)
    FAKE_SELECTOR_CHECK=$(echo "$DIFF_CONTENT" | grep -c '^\+.*instancesRespond(to:' || true)
    FAKE_RESPONDS_CHECK=$(echo "$DIFF_CONTENT" | grep -c '^\+.*responds(to:' || true)

    TOTAL_FAKE=$((FAKE_CLASS_CHECK + FAKE_SELECTOR_CHECK + FAKE_RESPONDS_CHECK))

    if [ "$TOTAL_FAKE" -gt 0 ]; then
        log_error "⛔️ FAKE TEST PATTERNS DETECTED! ⛔️"
        log_error "You are adding $TOTAL_FAKE fake test assertions:"
        log_error "  - NSClassFromString checks: $FAKE_CLASS_CHECK"
        log_error "  - instancesRespond checks: $FAKE_SELECTOR_CHECK"
        log_error "  - responds(to:) checks: $FAKE_RESPONDS_CHECK"
        echo ""
        echo -e "${RED}These patterns prove NOTHING about whether bugs are fixed!${NC}"
        echo ""
        echo "❌ WRONG: XCTAssertNotNil(NSClassFromString(\"Foo\"))"
        echo "❌ WRONG: XCTAssertTrue(cls.instancesRespond(to: selector))"
        echo ""
        echo "✅ RIGHT: Actually CALL the method with edge case input:"
        echo "   let obj = Foo()"
        echo "   let result = obj.buggyMethod(nil)  // edge case!"
        echo "   XCTAssertNil(result)"
        echo ""
        echo "Read docs/worker-backlog.md for the correct approach!"
        echo ""
        # Make this a warning for now, not a blocker (set ERRORS++ to block)
        log_warn "Commit allowed but PLEASE FIX THESE TESTS!"
    else
        log_success "No new fake test patterns detected"
    fi
fi

# =============================================================================
# TIER 1: Swift Analysis
# =============================================================================
if [ -n "$STAGED_SWIFT" ]; then
    log_section "TIER 1: Swift Analysis"

    # SwiftLint
    if command -v swiftlint &> /dev/null; then
        log_info "Running SwiftLint on staged Swift files..."
        SWIFTLINT_ERRORS=0

        for file in $STAGED_SWIFT; do
            if [ -f "$file" ]; then
                OUTPUT=$(swiftlint lint --path "$file" --quiet 2>/dev/null || true)
                if echo "$OUTPUT" | grep -q "error:"; then
                    log_error "SwiftLint errors in $file"
                    echo "$OUTPUT" | grep "error:" | head -5
                    ((SWIFTLINT_ERRORS++))
                fi
            fi
        done

        if [ "$SWIFTLINT_ERRORS" -eq 0 ]; then
            log_success "SwiftLint passed"
        fi
    else
        log_warn "SwiftLint not installed - skipping"
    fi
fi

# =============================================================================
# TIER 2: Objective-C / C / C++ Analysis
# =============================================================================
if [ -n "$STAGED_OBJC" ] || [ -n "$STAGED_C" ]; then
    log_section "TIER 2: C/Objective-C Analysis"

    # clang-format check
    if command -v clang-format &> /dev/null; then
        log_info "Checking clang-format..."
        FORMAT_ISSUES=0

        for file in $STAGED_OBJC $STAGED_C; do
            if [ -f "$file" ]; then
                if ! clang-format --dry-run --Werror "$file" 2>/dev/null; then
                    log_warn "Format issues in $file (run: clang-format -i $file)"
                    ((FORMAT_ISSUES++))
                fi
            fi
        done

        if [ "$FORMAT_ISSUES" -eq 0 ]; then
            log_success "clang-format passed"
        fi
    fi

    # Quick cppcheck on staged files
    if command -v cppcheck &> /dev/null; then
        log_info "Running quick cppcheck..."
        CPPCHECK_ERRORS=0

        for file in $STAGED_OBJC $STAGED_C; do
            if [ -f "$file" ]; then
                OUTPUT=$(cppcheck --enable=warning,performance --error-exitcode=1 "$file" 2>&1 || true)
                if echo "$OUTPUT" | grep -qE "^\[.*\]:.*error:"; then
                    log_error "cppcheck error in $file"
                    echo "$OUTPUT" | grep "error:" | head -3
                    ((CPPCHECK_ERRORS++))
                fi
            fi
        done

        if [ "$CPPCHECK_ERRORS" -eq 0 ]; then
            log_success "cppcheck passed"
        fi
    fi
fi

# =============================================================================
# TIER 3: Shell Script Analysis
# =============================================================================
if [ -n "$STAGED_SHELL" ]; then
    log_section "TIER 3: Shell Script Analysis"

    if command -v shellcheck &> /dev/null; then
        log_info "Running ShellCheck..."
        SHELL_ERRORS=0

        for file in $STAGED_SHELL; do
            if [ -f "$file" ]; then
                if ! shellcheck -S error "$file" 2>/dev/null; then
                    log_error "ShellCheck errors in $file"
                    ((SHELL_ERRORS++))
                fi
            fi
        done

        if [ "$SHELL_ERRORS" -eq 0 ]; then
            log_success "ShellCheck passed"
        fi
    else
        log_warn "ShellCheck not installed - skipping"
    fi
fi

# =============================================================================
# TIER 4: Python Analysis
# =============================================================================
if [ -n "$STAGED_PYTHON" ]; then
    log_section "TIER 4: Python Analysis"

    # flake8
    if command -v flake8 &> /dev/null; then
        log_info "Running flake8..."
        FLAKE8_ERRORS=0

        for file in $STAGED_PYTHON; do
            if [ -f "$file" ]; then
                if ! flake8 --select=E9,F63,F7,F82 "$file" 2>/dev/null; then
                    log_error "flake8 errors in $file"
                    ((FLAKE8_ERRORS++))
                fi
            fi
        done

        if [ "$FLAKE8_ERRORS" -eq 0 ]; then
            log_success "flake8 passed"
        fi
    fi

    # bandit security check
    if command -v bandit &> /dev/null; then
        log_info "Running Bandit security check..."
        for file in $STAGED_PYTHON; do
            if [ -f "$file" ]; then
                OUTPUT=$(bandit -q -ll "$file" 2>/dev/null || true)
                if [ -n "$OUTPUT" ]; then
                    log_warn "Bandit findings in $file - review before commit"
                fi
            fi
        done
    fi
fi

# =============================================================================
# TIER 5: JavaScript/TypeScript Analysis
# =============================================================================
if [ -n "$STAGED_JS" ]; then
    log_section "TIER 5: JavaScript Analysis"

    if command -v eslint &> /dev/null; then
        log_info "Running ESLint..."
        ESLINT_ERRORS=0

        for file in $STAGED_JS; do
            if [ -f "$file" ]; then
                if ! eslint --quiet "$file" 2>/dev/null; then
                    log_error "ESLint errors in $file"
                    ((ESLINT_ERRORS++))
                fi
            fi
        done

        if [ "$ESLINT_ERRORS" -eq 0 ]; then
            log_success "ESLint passed"
        fi
    fi
fi

# =============================================================================
# TIER 6: Metal Shader Analysis
# =============================================================================
if [ -n "$STAGED_METAL" ]; then
    log_section "TIER 6: Metal Shader Analysis"

    # Check formatting
    if command -v clang-format &> /dev/null; then
        log_info "Checking Metal shader format..."
        for file in $STAGED_METAL; do
            if [ -f "$file" ]; then
                if ! clang-format --dry-run --Werror "$file" 2>/dev/null; then
                    log_warn "Format issues in $file"
                fi
            fi
        done
        log_success "Metal shaders checked"
    fi
fi

# =============================================================================
# TIER 6.5: Mock Test Detection (BLOCK FAKE TESTS)
# =============================================================================
STAGED_TEST_FILES=$(echo "$STAGED_FILES" | grep -E 'Tests?\.swift$' || true)
if [ -n "$STAGED_TEST_FILES" ]; then
    log_section "TIER 6.5: Mock Test Detection"
    log_info "Checking for forbidden mock test patterns..."

    MOCK_FOUND=0
    for file in $STAGED_TEST_FILES; do
        if [ -f "$file" ]; then
            # Check for inline class definitions (mock pattern)
            if git diff --cached -- "$file" | grep -E '^\+.*class Safe[A-Z]' > /dev/null 2>&1; then
                log_error "MOCK TEST DETECTED in $file: 'class Safe*' pattern is BANNED"
                log_error "  → You must call ACTUAL production code, not create mock classes"
                ((MOCK_FOUND++))
            fi

            # Check for inline func definitions that look like mocks
            if git diff --cached -- "$file" | grep -E '^\+[[:space:]]+func (safe|mock|simulate)[A-Z]' > /dev/null 2>&1; then
                log_error "MOCK TEST DETECTED in $file: 'func safe*/mock*/simulate*' pattern is BANNED"
                log_error "  → You must call ACTUAL production code, not create mock functions"
                ((MOCK_FOUND++))
            fi

            # Check for protocol definitions inside test functions (mock pattern)
            if git diff --cached -- "$file" | grep -E '^\+[[:space:]]+protocol [A-Z].*Delegate' > /dev/null 2>&1; then
                log_error "MOCK TEST DETECTED in $file: inline protocol definitions are BANNED"
                log_error "  → Import and use the ACTUAL protocol from production code"
                ((MOCK_FOUND++))
            fi
        fi
    done

    if [ "$MOCK_FOUND" -gt 0 ]; then
        echo ""
        echo -e "${RED}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${RED}║  🚫 MOCK TESTS ARE FORBIDDEN - READ docs/worker-backlog.md TASK -0.5 🚫     ║${NC}"
        echo -e "${RED}║                                                                              ║${NC}"
        echo -e "${RED}║  Your test creates fake classes/functions instead of testing production code ║${NC}"
        echo -e "${RED}║  This provides ZERO regression protection. DELETE the mock and:              ║${NC}"
        echo -e "${RED}║                                                                              ║${NC}"
        echo -e "${RED}║  1. Import the actual production class (@testable import DashTerm2SharedARC)   ║${NC}"
        echo -e "${RED}║  2. Instantiate the REAL class mentioned in the bug                         ║${NC}"
        echo -e "${RED}║  3. Call the REAL method with edge-case input                               ║${NC}"
        echo -e "${RED}║  4. Assert the result                                                        ║${NC}"
        echo -e "${RED}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
        echo ""
    else
        log_success "No mock test patterns detected"
    fi
fi

# =============================================================================
# TIER 6.5: Performance Anti-Pattern Detection
# =============================================================================
log_section "TIER 6.5: Performance Lint"

# Check for performance anti-patterns in staged files
PERF_ISSUES=0

# Check for comparison mode defaults
if echo "$STAGED_FILES" | grep -q "iTermAdvancedSettingsModel.m"; then
    if git diff --cached -- sources/iTermAdvancedSettingsModel.m | grep -E '^\+.*ComparisonEnabled.*YES' > /dev/null 2>&1; then
        log_error "Performance: ComparisonEnabled defaults to YES - this doubles parsing overhead!"
        ((PERF_ISSUES++))
    fi
    if git diff --cached -- sources/iTermAdvancedSettingsModel.m | grep -E '^\+.*ValidationEnabled.*YES' > /dev/null 2>&1; then
        log_error "Performance: ValidationEnabled defaults to YES - unnecessary overhead in production"
        ((PERF_ISSUES++))
    fi
fi

# Check for "never called" comments being added (potential dead code bugs)
for file in $STAGED_SWIFT $STAGED_OBJC; do
    if [ -f "$file" ]; then
        if git diff --cached -- "$file" | grep -E '^\+.*(never called|not called|currently not called)' > /dev/null 2>&1; then
            log_warn "Dead code indicator added in $file - is this intentional?"
        fi
    fi
done

# Check for blocking operations in startup path
if echo "$STAGED_FILES" | grep -q "iTermApplicationDelegate.m"; then
    # Check for synchronous operations being added to applicationWillFinishLaunching
    if git diff --cached -- sources/iTermApplicationDelegate.m | grep -E '^\+.*dispatch_sync' > /dev/null 2>&1; then
        log_warn "dispatch_sync added to application delegate - verify not in startup path"
    fi
fi

if [ "$PERF_ISSUES" -gt 0 ]; then
    log_error "$PERF_ISSUES performance issue(s) found"
else
    log_success "No performance anti-patterns detected"
fi

# =============================================================================
# TIER 7: Build & Test Validation
# =============================================================================
log_section "TIER 7: Build & Test Validation"

# Resolve symlinks to get real script directory
SCRIPT_SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SCRIPT_SOURCE" ]; do
    SCRIPT_DIR="$( cd -P "$( dirname "$SCRIPT_SOURCE" )" && pwd )"
    SCRIPT_SOURCE="$(readlink "$SCRIPT_SOURCE")"
    [[ $SCRIPT_SOURCE != /* ]] && SCRIPT_SOURCE="$SCRIPT_DIR/$SCRIPT_SOURCE"
done
SCRIPT_DIR="$( cd -P "$( dirname "$SCRIPT_SOURCE" )" && pwd )"
BUILD_CHECK_SCRIPT="$SCRIPT_DIR/pre-commit-build-check.sh"

if [ -x "$BUILD_CHECK_SCRIPT" ]; then
    log_info "Running build & test validation..."
    if ! "$BUILD_CHECK_SCRIPT"; then
        log_error "Build/test validation failed"
    else
        log_success "Build/test validation passed"
    fi
else
    log_warn "Build check script not found or not executable: $BUILD_CHECK_SCRIPT"
fi

# =============================================================================
# FINAL VERDICT
# =============================================================================
echo ""
echo "============================================================================="

if [ "$ERRORS" -gt 0 ]; then
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                    ❌ COMMIT BLOCKED ❌                         ║${NC}"
    echo -e "${RED}║                                                               ║${NC}"
    echo -e "${RED}║  $ERRORS error(s) found. Fix before committing.                  ║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "To bypass (NOT RECOMMENDED): git commit --no-verify"
    exit 1
else
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                    ✅ PRE-COMMIT PASSED ✅                     ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 0
fi
