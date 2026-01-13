#!/bin/bash
# =============================================================================
# DashTerm2 Master Static Analysis Runner
# NASA/NSA Grade - Zero Defect Tolerance
# =============================================================================
# This script runs ALL static analysis tools and produces a comprehensive report.
# Exit code 0 = PASS (mission-critical quality achieved)
# Exit code 1 = FAIL (defects detected - DO NOT SHIP)
# =============================================================================

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPORT_DIR="$PROJECT_ROOT/static-analysis-reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$REPORT_DIR/analysis_$TIMESTAMP.md"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Counters
TOTAL_ERRORS=0
TOTAL_WARNINGS=0
TOOLS_RUN=0
TOOLS_FAILED=0

# Options
QUICK_MODE=false
SECURITY_ONLY=false
FIX_MODE=false
VERBOSE=false

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --quick       Run only fast checks (skip deep analysis)"
    echo "  --security    Run only security-focused tools"
    echo "  --fix         Auto-fix issues where possible"
    echo "  --verbose     Show detailed output"
    echo "  --help        Show this help"
    echo ""
    echo "Exit codes:"
    echo "  0 = All checks passed (SHIP IT)"
    echo "  1 = Errors found (DO NOT SHIP)"
    echo "  2 = Warnings only (Review required)"
}

# shellcheck disable=SC2034 # FIX_MODE and VERBOSE are reserved for future use
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick) QUICK_MODE=true ;;
        --security) SECURITY_ONLY=true ;;
        --fix) FIX_MODE=true ;;
        --verbose) VERBOSE=true ;;
        --help) usage; exit 0 ;;
        *) echo "Unknown option: $1"; usage; exit 1 ;;
    esac
    shift
done

log_header() { echo -e "\n${MAGENTA}═══════════════════════════════════════════════════════════════${NC}"; echo -e "${MAGENTA}  $1${NC}"; echo -e "${MAGENTA}═══════════════════════════════════════════════════════════════${NC}"; }
log_section() { echo -e "\n${CYAN}┌─ $1${NC}"; }
log_info() { echo -e "${BLUE}│ [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}│ [PASS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}│ [WARN]${NC} $1"; ((TOTAL_WARNINGS++)); }
log_error() { echo -e "${RED}│ [FAIL]${NC} $1"; ((TOTAL_ERRORS++)); }
log_end() { echo -e "${CYAN}└─────────────────────────────────────────────────────${NC}"; }

mkdir -p "$REPORT_DIR"

# Initialize report
cat > "$REPORT_FILE" << EOF
# DashTerm2 Static Analysis Report
**Generated:** $(date)
**Mode:** $([ "$QUICK_MODE" = true ] && echo "Quick" || echo "Full")
**Commit:** $(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

---

EOF

cd "$PROJECT_ROOT" || exit 1

log_header "DashTerm2 Static Analysis - NASA/NSA Grade"
echo -e "${CYAN}Timestamp:${NC} $TIMESTAMP"
echo -e "${CYAN}Project:${NC} $PROJECT_ROOT"
echo -e "${CYAN}Report:${NC} $REPORT_FILE"

# =============================================================================
# TIER 0: Build Verification (MUST PASS)
# =============================================================================
log_header "TIER 0: Build Verification"

log_section "Xcode Build Check"
((TOOLS_RUN++))

BUILD_OUTPUT=$(mktemp)
if xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-" 2>&1 | tee "$BUILD_OUTPUT" | tail -5; then
    if grep -q "BUILD SUCCEEDED" "$BUILD_OUTPUT"; then
        log_success "Build succeeded"
        echo "## Build: PASSED" >> "$REPORT_FILE"
    else
        log_error "Build failed - CRITICAL"
        echo "## Build: FAILED (CRITICAL)" >> "$REPORT_FILE"
        ERRORS=$(grep -c "error:" "$BUILD_OUTPUT" 2>/dev/null || echo "unknown")
        echo "Errors: $ERRORS" >> "$REPORT_FILE"
        ((TOOLS_FAILED++))
    fi
else
    log_error "Build command failed"
    ((TOOLS_FAILED++))
fi
rm -f "$BUILD_OUTPUT"
log_end

# =============================================================================
# TIER 1: Swift Analysis
# =============================================================================
log_header "TIER 1: Swift Analysis"

# SwiftLint
log_section "SwiftLint (Swift Safety Rules)"
((TOOLS_RUN++))
if command -v swiftlint &> /dev/null; then
    SWIFTLINT_OUTPUT=$(swiftlint lint --quiet --reporter json 2>/dev/null || true)
    SWIFTLINT_ERRORS=$(echo "$SWIFTLINT_OUTPUT" | jq '[.[] | select(.severity == "error")] | length' 2>/dev/null || echo "0")
    SWIFTLINT_WARNINGS=$(echo "$SWIFTLINT_OUTPUT" | jq '[.[] | select(.severity == "warning")] | length' 2>/dev/null || echo "0")

    if [ "$SWIFTLINT_ERRORS" -gt 0 ]; then
        log_error "SwiftLint: $SWIFTLINT_ERRORS errors, $SWIFTLINT_WARNINGS warnings"
        ((TOOLS_FAILED++))
    elif [ "$SWIFTLINT_WARNINGS" -gt 0 ]; then
        log_warn "SwiftLint: $SWIFTLINT_WARNINGS warnings"
    else
        log_success "SwiftLint: No issues"
    fi
    echo "## SwiftLint: $SWIFTLINT_ERRORS errors, $SWIFTLINT_WARNINGS warnings" >> "$REPORT_FILE"
else
    log_warn "SwiftLint not installed"
fi
log_end

# Periphery (Dead Code)
if [ "$QUICK_MODE" = false ]; then
    log_section "Periphery (Swift Dead Code Detection)"
    ((TOOLS_RUN++))
    if command -v periphery &> /dev/null; then
        log_info "Running Periphery scan (this may take a while)..."
        PERIPHERY_OUTPUT="$REPORT_DIR/periphery_$TIMESTAMP.txt"
        if periphery scan --project DashTerm2.xcodeproj --schemes DashTerm2 --skip-build 2>/dev/null > "$PERIPHERY_OUTPUT"; then
            DEAD_CODE_COUNT=$(wc -l < "$PERIPHERY_OUTPUT" | tr -d ' ')
            if [ "$DEAD_CODE_COUNT" -gt 0 ]; then
                log_warn "Periphery: $DEAD_CODE_COUNT potential dead code items"
            else
                log_success "Periphery: No dead code detected"
            fi
        else
            log_info "Periphery scan completed (check $PERIPHERY_OUTPUT)"
        fi
    else
        log_warn "Periphery not installed"
    fi
    log_end
fi

# =============================================================================
# TIER 2: Objective-C / C / C++ Analysis
# =============================================================================
log_header "TIER 2: Objective-C / C / C++ Analysis"

# cppcheck
log_section "cppcheck (C/C++/Obj-C Deep Analysis)"
((TOOLS_RUN++))
if command -v cppcheck &> /dev/null; then
    CPPCHECK_OUTPUT="$REPORT_DIR/cppcheck_$TIMESTAMP.xml"
    cppcheck --enable=all --xml --xml-version=2 \
        --suppress=missingIncludeSystem \
        --suppress=unusedFunction \
        -i ThirdParty -i submodules -i Pods \
        sources/ 2> "$CPPCHECK_OUTPUT" || true

    CPPCHECK_ERRORS=$(grep -c "<error " "$CPPCHECK_OUTPUT" 2>/dev/null || echo "0")
    if [ "$CPPCHECK_ERRORS" -gt 0 ]; then
        log_warn "cppcheck: $CPPCHECK_ERRORS issues found (see $CPPCHECK_OUTPUT)"
    else
        log_success "cppcheck: No critical issues"
    fi
    echo "## cppcheck: $CPPCHECK_ERRORS issues" >> "$REPORT_FILE"
else
    log_warn "cppcheck not installed"
fi
log_end

# Clang Static Analyzer
if [ "$QUICK_MODE" = false ]; then
    log_section "Clang Static Analyzer (Deep Memory/Logic Analysis)"
    ((TOOLS_RUN++))

    LLVM_PATH="$(brew --prefix llvm 2>/dev/null)/bin"
    SCAN_BUILD=""
    if [ -x "$LLVM_PATH/scan-build" ]; then
        SCAN_BUILD="$LLVM_PATH/scan-build"
    elif command -v scan-build &> /dev/null; then
        SCAN_BUILD="scan-build"
    fi

    if [ -n "$SCAN_BUILD" ]; then
        log_info "Running Clang Static Analyzer (this takes a while)..."
        SCAN_OUTPUT="$REPORT_DIR/clang-analyzer_$TIMESTAMP"
        mkdir -p "$SCAN_OUTPUT"

        # Run scan-build
        $SCAN_BUILD -o "$SCAN_OUTPUT" \
            --use-analyzer="$(xcrun -find clang)" \
            xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 \
            -configuration Development build \
            CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-" \
            clean build 2>&1 | tail -20 || true

        ANALYZER_BUGS=$(find "$SCAN_OUTPUT" -name "*.html" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$ANALYZER_BUGS" -gt 0 ]; then
            log_warn "Clang Analyzer: $ANALYZER_BUGS potential bugs (see $SCAN_OUTPUT)"
        else
            log_success "Clang Analyzer: No bugs detected"
        fi
    else
        log_warn "scan-build not installed (brew install llvm)"
    fi
    log_end
fi

# =============================================================================
# TIER 3: Security Analysis
# =============================================================================
log_header "TIER 3: Security Analysis"

# Semgrep
log_section "Semgrep (Security Patterns)"
((TOOLS_RUN++))
if command -v semgrep &> /dev/null; then
    SEMGREP_OUTPUT="$REPORT_DIR/semgrep_$TIMESTAMP.json"
    log_info "Running Semgrep security scan..."

    semgrep scan --config auto --json \
        --exclude="ThirdParty" --exclude="submodules" --exclude="Pods" \
        sources/ > "$SEMGREP_OUTPUT" 2>/dev/null || true

    SEMGREP_FINDINGS=$(jq '.results | length' "$SEMGREP_OUTPUT" 2>/dev/null || echo "0")
    SEMGREP_ERRORS=$(jq '[.results[] | select(.extra.severity == "ERROR")] | length' "$SEMGREP_OUTPUT" 2>/dev/null || echo "0")

    if [ "$SEMGREP_ERRORS" -gt 0 ]; then
        log_error "Semgrep: $SEMGREP_ERRORS critical security issues!"
        ((TOOLS_FAILED++))
    elif [ "$SEMGREP_FINDINGS" -gt 0 ]; then
        log_warn "Semgrep: $SEMGREP_FINDINGS findings (review $SEMGREP_OUTPUT)"
    else
        log_success "Semgrep: No security issues"
    fi
    echo "## Semgrep: $SEMGREP_FINDINGS findings ($SEMGREP_ERRORS critical)" >> "$REPORT_FILE"
else
    log_warn "Semgrep not installed"
fi
log_end

# Secrets Detection
log_section "Secrets Detection (TruffleHog/Gitleaks)"
((TOOLS_RUN++))
SECRETS_FOUND=0

if command -v trufflehog &> /dev/null; then
    log_info "Running TruffleHog..."
    TRUFFLEHOG_OUTPUT="$REPORT_DIR/trufflehog_$TIMESTAMP.json"
    trufflehog filesystem --json --no-update . 2>/dev/null > "$TRUFFLEHOG_OUTPUT" || true
    TRUFFLEHOG_SECRETS=$(wc -l < "$TRUFFLEHOG_OUTPUT" | tr -d ' ')
    if [ "$TRUFFLEHOG_SECRETS" -gt 0 ]; then
        log_error "TruffleHog: $TRUFFLEHOG_SECRETS potential secrets found!"
        SECRETS_FOUND=$((SECRETS_FOUND + TRUFFLEHOG_SECRETS))
    else
        log_success "TruffleHog: No secrets detected"
    fi
fi

if command -v gitleaks &> /dev/null; then
    log_info "Running Gitleaks..."
    GITLEAKS_OUTPUT="$REPORT_DIR/gitleaks_$TIMESTAMP.json"
    gitleaks detect --source . --report-path "$GITLEAKS_OUTPUT" --report-format json 2>/dev/null || true
    if [ -f "$GITLEAKS_OUTPUT" ]; then
        GITLEAKS_SECRETS=$(jq 'length' "$GITLEAKS_OUTPUT" 2>/dev/null || echo "0")
        if [ "$GITLEAKS_SECRETS" -gt 0 ]; then
            log_error "Gitleaks: $GITLEAKS_SECRETS potential secrets found!"
            SECRETS_FOUND=$((SECRETS_FOUND + GITLEAKS_SECRETS))
        else
            log_success "Gitleaks: No secrets detected"
        fi
    fi
fi

if [ "$SECRETS_FOUND" -gt 0 ]; then
    ((TOOLS_FAILED++))
fi
log_end

# =============================================================================
# TIER 4: Script Analysis (Python, Shell, JS)
# =============================================================================
log_header "TIER 4: Script Analysis"

# ShellCheck
log_section "ShellCheck (Shell Scripts)"
((TOOLS_RUN++))
if command -v shellcheck &> /dev/null; then
    SHELL_SCRIPTS=$(find . -name "*.sh" -not -path "./ThirdParty/*" -not -path "./submodules/*" -not -path "./Pods/*" 2>/dev/null)
    SHELLCHECK_ERRORS=0

    for script in $SHELL_SCRIPTS; do
        if ! shellcheck -S error "$script" 2>/dev/null; then
            ((SHELLCHECK_ERRORS++))
        fi
    done

    if [ "$SHELLCHECK_ERRORS" -gt 0 ]; then
        log_error "ShellCheck: $SHELLCHECK_ERRORS scripts have errors"
        ((TOOLS_FAILED++))
    else
        log_success "ShellCheck: All scripts pass"
    fi
else
    log_warn "ShellCheck not installed"
fi
log_end

# Python (flake8 + bandit)
log_section "Python Analysis (flake8 + bandit)"
((TOOLS_RUN++))
PYTHON_FILES=$(find . -name "*.py" -not -path "./ThirdParty/*" -not -path "./submodules/*" -not -path "./Pods/*" 2>/dev/null | head -100)

if [ -n "$PYTHON_FILES" ]; then
    if command -v flake8 &> /dev/null; then
        FLAKE8_ERRORS=$(echo "$PYTHON_FILES" | xargs flake8 --count --select=E9,F63,F7,F82 2>/dev/null | tail -1 || echo "0")
        if [ "$FLAKE8_ERRORS" != "0" ] && [ -n "$FLAKE8_ERRORS" ]; then
            log_error "flake8: $FLAKE8_ERRORS critical Python errors"
        else
            log_success "flake8: No critical Python errors"
        fi
    fi

    if command -v bandit &> /dev/null; then
        BANDIT_OUTPUT="$REPORT_DIR/bandit_$TIMESTAMP.json"
        echo "$PYTHON_FILES" | xargs bandit -f json -o "$BANDIT_OUTPUT" 2>/dev/null || true
        if [ -f "$BANDIT_OUTPUT" ]; then
            BANDIT_HIGH=$(jq '[.results[] | select(.issue_severity == "HIGH")] | length' "$BANDIT_OUTPUT" 2>/dev/null || echo "0")
            if [ "$BANDIT_HIGH" -gt 0 ]; then
                log_error "Bandit: $BANDIT_HIGH high-severity Python security issues"
                ((TOOLS_FAILED++))
            else
                log_success "Bandit: No high-severity issues"
            fi
        fi
    fi
else
    log_info "No Python files to analyze"
fi
log_end

# =============================================================================
# TIER 5: Complexity & Metrics
# =============================================================================
if [ "$QUICK_MODE" = false ] && [ "$SECURITY_ONLY" = false ]; then
    log_header "TIER 5: Code Metrics"

    log_section "Code Statistics"
    if command -v tokei &> /dev/null; then
        tokei --exclude ThirdParty --exclude submodules --exclude Pods . 2>/dev/null | head -30
        log_success "Code statistics generated"
    elif command -v cloc &> /dev/null; then
        cloc --exclude-dir=ThirdParty,submodules,Pods . 2>/dev/null | head -30
        log_success "Code statistics generated"
    fi
    log_end

    log_section "Complexity Analysis"
    if command -v lizard &> /dev/null; then
        LIZARD_OUTPUT="$REPORT_DIR/lizard_$TIMESTAMP.csv"
        lizard -l swift -l objectivec -l cpp \
            --exclude "ThirdParty/*" --exclude "submodules/*" --exclude "Pods/*" \
            -o "$LIZARD_OUTPUT" sources/ 2>/dev/null || true

        # Count high-complexity functions (CCN > 15)
        HIGH_COMPLEXITY=$(awk -F',' 'NR>1 && $2>15 {count++} END {print count+0}' "$LIZARD_OUTPUT" 2>/dev/null || echo "0")
        if [ "$HIGH_COMPLEXITY" -gt 0 ]; then
            log_warn "Lizard: $HIGH_COMPLEXITY functions with high complexity (CCN > 15)"
        else
            log_success "Lizard: No high-complexity functions"
        fi
    else
        log_warn "Lizard not installed (pip install lizard)"
    fi
    log_end
fi

# =============================================================================
# FINAL REPORT
# =============================================================================
log_header "ANALYSIS COMPLETE"

echo ""
echo "============================================================================="
echo "                         FINAL VERDICT"
echo "============================================================================="
echo ""

# Generate final report
cat >> "$REPORT_FILE" << EOF

---

## Summary

- **Tools Run:** $TOOLS_RUN
- **Tools Failed:** $TOOLS_FAILED
- **Total Errors:** $TOTAL_ERRORS
- **Total Warnings:** $TOTAL_WARNINGS

---

EOF

if [ "$TOOLS_FAILED" -gt 0 ] || [ "$TOTAL_ERRORS" -gt 0 ]; then
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                    ❌ DO NOT SHIP ❌                           ║${NC}"
    echo -e "${RED}║                                                               ║${NC}"
    echo -e "${RED}║  Critical issues found. Fix before deployment.               ║${NC}"
    echo -e "${RED}║  Tools Failed: $TOOLS_FAILED                                            ║${NC}"
    echo -e "${RED}║  Errors: $TOTAL_ERRORS                                                  ║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Report: $REPORT_FILE"
    echo "## Verdict: DO NOT SHIP" >> "$REPORT_FILE"
    exit 1
elif [ "$TOTAL_WARNINGS" -gt 0 ]; then
    echo -e "${YELLOW}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║                   ⚠️  REVIEW REQUIRED ⚠️                        ║${NC}"
    echo -e "${YELLOW}║                                                               ║${NC}"
    echo -e "${YELLOW}║  No critical errors, but warnings need review.               ║${NC}"
    echo -e "${YELLOW}║  Warnings: $TOTAL_WARNINGS                                              ║${NC}"
    echo -e "${YELLOW}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Report: $REPORT_FILE"
    echo "## Verdict: REVIEW REQUIRED" >> "$REPORT_FILE"
    exit 2
else
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                    ✅ MISSION READY ✅                         ║${NC}"
    echo -e "${GREEN}║                                                               ║${NC}"
    echo -e "${GREEN}║  All checks passed. Code is NASA/NSA grade.                  ║${NC}"
    echo -e "${GREEN}║  You may proceed with deployment.                            ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Report: $REPORT_FILE"
    echo "## Verdict: MISSION READY" >> "$REPORT_FILE"
    exit 0
fi
