#!/bin/bash
# =============================================================================
# DashTerm2 Pre-Push Hook
# NASA/NSA Grade - Zero Defect Tolerance
# =============================================================================
# This hook runs before every push. It's more comprehensive than pre-commit
# because pushing shares code with others.
# =============================================================================

set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ERRORS=0
WARNINGS=0

log_header() { echo -e "\n${BLUE}═══════════════════════════════════════════════════════════════${NC}"; echo -e "${BLUE}  $1${NC}"; echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"; }
log_section() { echo -e "\n${BLUE}┌─ $1${NC}"; }
log_success() { echo -e "${GREEN}│ ✓${NC} $1"; }
log_error() { echo -e "${RED}│ ✗${NC} $1"; ((ERRORS++)); }
log_warn() { echo -e "${YELLOW}│ ⚠${NC} $1"; ((WARNINGS++)); }
log_info() { echo -e "${BLUE}│ ℹ${NC} $1"; }
log_end() { echo -e "${BLUE}└─────────────────────────────────────────────────────${NC}"; }

log_header "DashTerm2 Pre-Push Check - NASA/NSA Grade"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_ROOT"

# =============================================================================
# CHECK 1: Build Verification (CRITICAL)
# =============================================================================
log_section "Build Verification"
log_info "Running full build (this may take a while)..."

BUILD_START=$(date +%s)
if xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build \
    CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-" 2>&1 | tail -10; then

    if xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build \
        CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-" 2>&1 | grep -q "BUILD SUCCEEDED"; then
        BUILD_END=$(date +%s)
        BUILD_TIME=$((BUILD_END - BUILD_START))
        log_success "Build succeeded (${BUILD_TIME}s)"
    else
        log_error "Build failed - CANNOT PUSH"
    fi
else
    log_error "Build command failed - CANNOT PUSH"
fi
log_end

# =============================================================================
# CHECK 2: Smoke Test
# =============================================================================
log_section "Smoke Test"
if [ -x "$SCRIPT_DIR/smoke-test.sh" ]; then
    log_info "Running smoke test..."
    if "$SCRIPT_DIR/smoke-test.sh" 2>/dev/null; then
        log_success "Smoke test passed"
    else
        log_error "Smoke test failed - CANNOT PUSH"
    fi
else
    log_warn "Smoke test script not found or not executable"
fi
log_end

# =============================================================================
# CHECK 3: Security Scan (Quick)
# =============================================================================
log_section "Quick Security Scan"

# Check for obvious secrets in entire codebase
log_info "Scanning for secrets..."
SECRET_HITS=$(grep -rniE "(password|secret|api_key|private_key)\s*[:=]\s*[\"'][^\"']+[\"']" sources/ \
    --include="*.swift" --include="*.m" --include="*.h" \
    2>/dev/null | grep -v "// " | grep -v "example" | head -5 || true)

if [ -n "$SECRET_HITS" ]; then
    log_error "Potential secrets found in codebase!"
    echo "$SECRET_HITS"
else
    log_success "No obvious secrets"
fi

# Check for DO NOT SUBMIT in entire codebase
DNS_HITS=$(grep -rni "DO NOT SUBMIT\|DONOTSUBMIT\|#warning.*do not submit" sources/ \
    --include="*.swift" --include="*.m" --include="*.h" --include="*.c" \
    2>/dev/null | head -5 || true)

if [ -n "$DNS_HITS" ]; then
    log_error "DO NOT SUBMIT markers found!"
    echo "$DNS_HITS"
else
    log_success "No DO NOT SUBMIT markers"
fi

log_end

# =============================================================================
# CHECK 4: SwiftLint Full Scan
# =============================================================================
log_section "SwiftLint Full Scan"
if command -v swiftlint &> /dev/null; then
    log_info "Running SwiftLint on entire codebase..."
    SWIFTLINT_OUTPUT=$(swiftlint lint --quiet 2>/dev/null || true)
    SWIFTLINT_ERRORS=$(echo "$SWIFTLINT_OUTPUT" | grep -c "error:" 2>/dev/null || true)
    SWIFTLINT_ERRORS=${SWIFTLINT_ERRORS:-0}
    SWIFTLINT_ERRORS=$((SWIFTLINT_ERRORS + 0))

    if [ "$SWIFTLINT_ERRORS" -gt 0 ]; then
        log_error "SwiftLint: $SWIFTLINT_ERRORS errors found"
        echo "$SWIFTLINT_OUTPUT" | grep "error:" | head -10
    else
        log_success "SwiftLint passed"
    fi
else
    log_warn "SwiftLint not installed"
fi
log_end

# =============================================================================
# FINAL VERDICT
# =============================================================================
echo ""
echo "============================================================================="

if [ "$ERRORS" -gt 0 ]; then
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                     ❌ PUSH BLOCKED ❌                          ║${NC}"
    echo -e "${RED}║                                                               ║${NC}"
    echo -e "${RED}║  $ERRORS error(s) found. Fix before pushing.                     ║${NC}"
    echo -e "${RED}║  Code quality standards not met for NASA/NSA grade.           ║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "To bypass (EXTREMELY NOT RECOMMENDED): git push --no-verify"
    exit 1
elif [ "$WARNINGS" -gt 0 ]; then
    echo -e "${YELLOW}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║                   ⚠️  PUSH WITH WARNINGS ⚠️                     ║${NC}"
    echo -e "${YELLOW}║                                                               ║${NC}"
    echo -e "${YELLOW}║  $WARNINGS warning(s) found. Review recommended before push.    ║${NC}"
    echo -e "${YELLOW}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 0
else
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                    ✅ PRE-PUSH PASSED ✅                        ║${NC}"
    echo -e "${GREEN}║                                                               ║${NC}"
    echo -e "${GREEN}║  All checks passed. Code is NASA/NSA grade.                  ║${NC}"
    echo -e "${GREEN}║  Safe to push.                                               ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 0
fi
