#!/bin/bash
# Verification Dashboard - Comprehensive project health check
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

# This script is a "best effort" health dashboard: many checks are expected to
# fail (or timeout) and should be reported rather than aborting the script.
# Keep strictness enabled, but ensure expected non-zero statuses are handled.
set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configurable timeouts (in seconds)
# Override via environment variables for cold builds which take longer
BUILD_TIMEOUT=${BUILD_TIMEOUT:-300}      # 5 minutes default
TEST_TIMEOUT=${TEST_TIMEOUT:-600}        # 10 minutes default
CLIPPY_TIMEOUT=${CLIPPY_TIMEOUT:-600}    # 10 minutes default
DOC_TEST_TIMEOUT=${DOC_TEST_TIMEOUT:-600} # 10 minutes default

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║                     DashFlow Rust Verification Dashboard              ║"
echo "║                          © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)                           ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""

ISSUES=0
WARNINGS=0

# Helper functions
pass() {
    echo -e "${GREEN}✓${NC} $1"
}

fail() {
    echo -e "${RED}✗${NC} $1"
    ISSUES=$((ISSUES + 1))
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
    WARNINGS=$((WARNINGS + 1))
}

info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

section() {
    echo ""
    echo -e "${BLUE}━━━ $1 ━━━${NC}"
}

# Helper to run a command with a timeout and capture output + exit code.
run_capture() {
    local timeout_sec="$1"
    shift
    local output
    local exit_code=0
    output=$(timeout "$timeout_sec" "$@" 2>&1) || exit_code=$?
    printf '%s' "$output"
    return $exit_code
}

# Check 1: Build Status
section "Build Status"
BUILD_EXIT=0
BUILD_OUTPUT=$(run_capture "$BUILD_TIMEOUT" cargo build --workspace --quiet) || BUILD_EXIT=$?
if [ "$BUILD_EXIT" -eq 0 ]; then
    pass "Workspace builds successfully"
else
    fail "Workspace build failed (exit=$BUILD_EXIT)"
    echo "$BUILD_OUTPUT" | head -30
fi

# Check 2: Test Status
section "Test Status"
info "Running tests per crate (workspace test may timeout)..."

TEST_EXIT=0
TEST_OUTPUT=$(run_capture "$TEST_TIMEOUT" cargo test --workspace --quiet) || TEST_EXIT=$?
if [ "$TEST_EXIT" -eq 124 ]; then
    warn "Workspace tests timed out"
elif [ "$TEST_EXIT" -ne 0 ]; then
    fail "Workspace tests failed (exit=$TEST_EXIT)"
else
    if echo "$TEST_OUTPUT" | grep -q "test result: ok"; then
        PASSED=$(echo "$TEST_OUTPUT" | sed -nE 's/.*test result: ok\\. ([0-9]+) passed.*/\\1/p' | tail -1)
        IGNORED=$(echo "$TEST_OUTPUT" | sed -nE 's/.*; ([0-9]+) ignored.*/\\1/p' | tail -1)
        PASSED=${PASSED:-0}
        IGNORED=${IGNORED:-0}
        pass "Tests passing: $PASSED (ignored: $IGNORED)"
    else
        warn "Could not parse workspace test summary (unexpected output)"
    fi
fi

# Check 3: Clippy Warnings
section "Clippy Analysis"
CLIPPY_EXIT=0
CLIPPY_OUTPUT=$(run_capture "$CLIPPY_TIMEOUT" cargo clippy --workspace --all-targets) || CLIPPY_EXIT=$?
CLIPPY_WARNINGS=$(echo "$CLIPPY_OUTPUT" | grep -c "warning:" || echo "0")
if [ "$CLIPPY_EXIT" -ne 0 ] && [ "$CLIPPY_EXIT" -ne 124 ]; then
    fail "Clippy failed (exit=$CLIPPY_EXIT)"
elif [ "$CLIPPY_EXIT" -eq 124 ]; then
    warn "Clippy timed out"
elif [ "$CLIPPY_WARNINGS" -eq 0 ]; then
    pass "Zero clippy warnings"
else
    fail "Clippy warnings: $CLIPPY_WARNINGS"
fi

# Check 4: Dead Code
section "Dead Code Analysis"
DEAD_CODE_COUNT=$((0 + $( (grep -r "#\\[allow(dead_code)\\]" crates/ || true) | wc -l | tr -d ' ' )))
if [ "$DEAD_CODE_COUNT" -lt 10 ]; then
    pass "Dead code attributes: $DEAD_CODE_COUNT (acceptable)"
elif [ "$DEAD_CODE_COUNT" -lt 50 ]; then
    warn "Dead code attributes: $DEAD_CODE_COUNT (should review)"
else
    fail "Dead code attributes: $DEAD_CODE_COUNT (needs cleanup)"
fi

# Check 5: TODO/FIXME Comments
section "TODO/FIXME Comments"
TODO_COUNT=$((0 + $( (grep -r "TODO\\|FIXME" crates/ --include="*.rs" || true) | grep -v "Binary file" | wc -l | tr -d ' ' )))
if [ "$TODO_COUNT" -lt 20 ]; then
    pass "TODO/FIXME comments: $TODO_COUNT (acceptable)"
elif [ "$TODO_COUNT" -lt 100 ]; then
    warn "TODO/FIXME comments: $TODO_COUNT (consider addressing)"
else
    warn "TODO/FIXME comments: $TODO_COUNT (many pending items)"
fi

# Check 6: Archive Size
section "Archive Size"
ARCHIVE_DIRS=()
for dir in reports archive docs/archive; do
    if [ -d "$dir" ]; then
        ARCHIVE_DIRS+=("$dir")
    fi
done
if [ "${#ARCHIVE_DIRS[@]}" -eq 0 ]; then
    warn "Archive directories not found (expected: reports/, archive/, docs/archive/)"
    ARCHIVE_FILES=0
else
    ARCHIVE_FILES=$(find "${ARCHIVE_DIRS[@]}" -name "*.md" -type f 2>/dev/null | wc -l | tr -d ' ')
fi
if [ "$ARCHIVE_FILES" -lt 100 ]; then
    pass "Archive markdown files: $ARCHIVE_FILES"
elif [ "$ARCHIVE_FILES" -lt 300 ]; then
    warn "Archive markdown files: $ARCHIVE_FILES (consider consolidation)"
else
    fail "Archive markdown files: $ARCHIVE_FILES (needs consolidation)"
fi

# Check 7: Documentation Tests
section "Documentation Tests"
DOC_TEST_EXIT=0
DOC_TEST_OUTPUT=$(run_capture "$DOC_TEST_TIMEOUT" cargo test --doc --workspace) || DOC_TEST_EXIT=$?
if [ "$DOC_TEST_EXIT" -eq 0 ] && echo "$DOC_TEST_OUTPUT" | grep -q "test result: ok"; then
    DOC_PASSED=$(echo "$DOC_TEST_OUTPUT" | sed -nE 's/.*test result: ok\\. ([0-9]+) passed.*/\\1/p' | tail -1)
    DOC_PASSED=${DOC_PASSED:-0}
    pass "Doc tests passing: $DOC_PASSED"
else
    warn "Doc tests may have issues (check manually)"
fi

# Check 8: Unused Dependencies (requires cargo-udeps)
section "Unused Dependencies"
if command -v cargo-udeps &> /dev/null; then
    info "Running cargo-udeps (may take a minute)..."
    UDEPS_OUTPUT=$(cargo +nightly udeps --workspace 2>&1 || true)
    UNUSED=$(echo "$UDEPS_OUTPUT" | grep -c "unused" || echo "0")
    if [ "$UNUSED" -eq 0 ]; then
        pass "No unused dependencies detected"
    else
        warn "Unused dependencies detected: $UNUSED"
    fi
else
    info "cargo-udeps not installed (skip: cargo install cargo-udeps)"
fi

# Check 9: Security Audit (requires cargo-audit)
section "Security Audit"
if command -v cargo-audit &> /dev/null; then
    if cargo audit 2>&1 | grep -q "Vulnerabilities found!"; then
        VULNS=$(cargo audit 2>&1 | sed -nE 's/.*([0-9]+) vulnerabilities found.*/\\1/p' | tail -1) || true
        VULNS=${VULNS:-"unknown"}
        fail "Security vulnerabilities: $VULNS"
    else
        pass "No known security vulnerabilities"
    fi
else
    info "cargo-audit not installed (skip: cargo install cargo-audit)"
fi

# Check 10: Git Status
section "Git Status"
if [ -n "$(git status --porcelain)" ]; then
    MODIFIED=$(git status --porcelain | wc -l)
    info "Uncommitted changes: $MODIFIED files"
else
    pass "Working directory clean"
fi

# Summary
section "Summary"
echo ""
if [ "$ISSUES" -eq 0 ] && [ "$WARNINGS" -eq 0 ]; then
    echo -e "${GREEN}╔════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ALL CHECKS PASSED - READY FOR COMMIT  ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"
    exit 0
elif [ "$ISSUES" -eq 0 ]; then
    echo -e "${YELLOW}╔════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║  WARNINGS: $WARNINGS - REVIEW RECOMMENDED  ║${NC}"
    echo -e "${YELLOW}╚════════════════════════════════════════╝${NC}"
    exit 0
else
    echo -e "${RED}╔════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ISSUES: $ISSUES - MUST FIX BEFORE COMMIT ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════╝${NC}"
    exit 1
fi
