#!/bin/bash
# =============================================================================
# DashTerm2 Static Analysis Tool Installer
# NASA/NSA Grade - Zero Defect Tolerance
# =============================================================================
# This script installs ALL static analysis tools required for mission-critical
# code quality. Every tool here catches bugs that others miss.
# =============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo "============================================================================="
echo "  DashTerm2 Static Analysis Installer - NASA/NSA Grade"
echo "  Zero Defect Tolerance - Mission Critical Quality"
echo "============================================================================="
echo ""

# Track installation status
FAILED_INSTALLS=()

install_brew_package() {
    local package="$1"
    local description="$2"

    if command -v "$package" &> /dev/null || brew list "$package" &> /dev/null 2>&1; then
        log_success "$package already installed - $description"
    else
        log_info "Installing $package - $description"
        if brew install "$package" 2>/dev/null; then
            log_success "$package installed"
        else
            log_warn "Failed to install $package - may need manual installation"
            FAILED_INSTALLS+=("$package")
        fi
    fi
}

install_pip_package() {
    local package="$1"
    local description="$2"

    if pip3 show "$package" &> /dev/null 2>&1; then
        log_success "$package already installed - $description"
    else
        log_info "Installing $package - $description"
        if pip3 install "$package" 2>/dev/null; then
            log_success "$package installed"
        else
            log_warn "Failed to install $package"
            FAILED_INSTALLS+=("$package")
        fi
    fi
}

install_npm_package() {
    local package="$1"
    local description="$2"

    if npm list -g "$package" &> /dev/null 2>&1; then
        log_success "$package already installed - $description"
    else
        log_info "Installing $package - $description"
        if npm install -g "$package" 2>/dev/null; then
            log_success "$package installed"
        else
            log_warn "Failed to install $package"
            FAILED_INSTALLS+=("$package")
        fi
    fi
}

echo "============================================================================="
echo "  TIER 1: Deep Static Analyzers (Find bugs without running code)"
echo "============================================================================="

# Clang Static Analyzer (via LLVM)
log_info "Checking LLVM/Clang Static Analyzer..."
if command -v scan-build &> /dev/null; then
    log_success "scan-build (Clang Static Analyzer) available"
else
    install_brew_package "llvm" "Clang Static Analyzer - memory leaks, null derefs, logic errors"
    # Add LLVM to path hint
    echo ""
    log_info "Add to your shell profile: export PATH=\"\$(brew --prefix llvm)/bin:\$PATH\""
fi

# Facebook Infer - Catches what others miss
install_brew_package "infer" "Facebook Infer - null safety, memory leaks, race conditions"

# cppcheck - C/C++ deep analysis
install_brew_package "cppcheck" "cppcheck - buffer overflows, memory leaks, undefined behavior"

# OCLint - Objective-C specific
install_brew_package "oclint" "OCLint - Objective-C code smells and bugs"

# Periphery - Swift dead code detection
install_brew_package "periphery" "Periphery - Swift dead code, unused declarations"

echo ""
echo "============================================================================="
echo "  TIER 2: Language-Specific Linters (Enforced coding standards)"
echo "============================================================================="

install_brew_package "swiftlint" "SwiftLint - Swift safety and style"
install_brew_package "swiftformat" "SwiftFormat - Swift code formatting"
install_brew_package "clang-format" "clang-format - C/C++/Obj-C formatting"
install_brew_package "shellcheck" "ShellCheck - Shell script analysis"

install_pip_package "flake8" "flake8 - Python linting"
install_pip_package "mypy" "mypy - Python type checking"
install_pip_package "pylint" "pylint - Python deep analysis"

install_npm_package "eslint" "ESLint - JavaScript linting"
install_npm_package "markdownlint-cli" "markdownlint - Markdown linting"
install_npm_package "jsonlint" "jsonlint - JSON validation"
install_npm_package "htmlhint" "HTMLHint - HTML validation"
install_npm_package "stylelint" "stylelint - CSS linting"

echo ""
echo "============================================================================="
echo "  TIER 3: Security Analysis (Find vulnerabilities)"
echo "============================================================================="

install_brew_package "semgrep" "Semgrep - Security patterns, OWASP rules"
install_brew_package "trufflehog" "TruffleHog - Secrets detection"
install_brew_package "gitleaks" "Gitleaks - Git secrets scanning"

install_pip_package "bandit" "Bandit - Python security analysis"
install_pip_package "safety" "Safety - Python dependency vulnerabilities"

echo ""
echo "============================================================================="
echo "  TIER 4: Documentation & Metrics"
echo "============================================================================="

install_brew_package "doxygen" "Doxygen - Documentation generation"
install_brew_package "cloc" "cloc - Lines of code counter"
install_brew_package "tokei" "tokei - Fast code statistics"

install_pip_package "lizard" "Lizard - Cyclomatic complexity analyzer"
install_pip_package "radon" "Radon - Python code metrics"

echo ""
echo "============================================================================="
echo "  TIER 5: Dependency & Build Analysis"
echo "============================================================================="

install_brew_package "graphviz" "Graphviz - Dependency visualization"
install_brew_package "jq" "jq - JSON processing"
install_brew_package "yq" "yq - YAML processing"
install_brew_package "xmllint" "xmllint - XML/Plist validation" || true  # Usually pre-installed

echo ""
echo "============================================================================="
echo "  POST-INSTALL: Verify Critical Tools"
echo "============================================================================="

echo ""
log_info "Verifying critical tool availability..."

CRITICAL_TOOLS=(
    "swiftlint:SwiftLint"
    "clang-format:clang-format"
    "cppcheck:cppcheck"
    "semgrep:Semgrep"
    "shellcheck:ShellCheck"
    "flake8:flake8"
)

MISSING_CRITICAL=()
for tool_pair in "${CRITICAL_TOOLS[@]}"; do
    tool="${tool_pair%%:*}"
    name="${tool_pair##*:}"
    if command -v "$tool" &> /dev/null; then
        log_success "$name is available"
    else
        log_error "$name is NOT available - CRITICAL"
        MISSING_CRITICAL+=("$tool")
    fi
done

echo ""
echo "============================================================================="
echo "  INSTALLATION SUMMARY"
echo "============================================================================="

if [ ${#MISSING_CRITICAL[@]} -eq 0 ]; then
    log_success "All critical tools installed successfully!"
else
    log_error "Missing critical tools: ${MISSING_CRITICAL[*]}"
    echo "Please install manually before running static analysis."
fi

if [ ${#FAILED_INSTALLS[@]} -gt 0 ]; then
    log_warn "Some optional tools failed to install: ${FAILED_INSTALLS[*]}"
    echo "These can be installed manually if needed."
fi

echo ""
echo "============================================================================="
echo "  NEXT STEPS"
echo "============================================================================="
echo ""
echo "  1. Run full static analysis:  ./scripts/static-analysis.sh"
echo "  2. Install git hooks:         ./scripts/install-hooks.sh"
echo "  3. Run security scan:         ./scripts/security-scan.sh"
echo ""
echo "  For LLVM tools, add to your shell profile:"
echo "    export PATH=\"\$(brew --prefix llvm)/bin:\$PATH\""
echo ""
echo "============================================================================="
