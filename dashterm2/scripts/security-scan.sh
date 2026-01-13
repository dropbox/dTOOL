#!/bin/bash
# =============================================================================
# DashTerm2 Security Scan
# NASA/NSA Grade - Zero Vulnerability Tolerance
# =============================================================================
# Comprehensive security scanning for:
# - Secrets/credentials in code
# - Known vulnerabilities
# - Security anti-patterns
# - Injection vulnerabilities
# - Cryptographic issues
# =============================================================================

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPORT_DIR="$PROJECT_ROOT/security-reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m'

CRITICAL_FINDINGS=0
HIGH_FINDINGS=0
MEDIUM_FINDINGS=0
LOW_FINDINGS=0

log_header() { echo -e "\n${MAGENTA}═══════════════════════════════════════════════════════════════${NC}"; echo -e "${MAGENTA}  $1${NC}"; echo -e "${MAGENTA}═══════════════════════════════════════════════════════════════${NC}"; }
log_section() { echo -e "\n${BLUE}┌─ $1${NC}"; }
log_info() { echo -e "${BLUE}│ [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}│ [PASS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}│ [WARN]${NC} $1"; }
log_error() { echo -e "${RED}│ [FAIL]${NC} $1"; }
log_critical() { echo -e "${RED}│ [CRITICAL]${NC} $1"; ((CRITICAL_FINDINGS++)); }
log_end() { echo -e "${BLUE}└─────────────────────────────────────────────────────${NC}"; }

mkdir -p "$REPORT_DIR"
cd "$PROJECT_ROOT"

log_header "DashTerm2 Security Scan - NSA Grade"
echo -e "${BLUE}Timestamp:${NC} $TIMESTAMP"
echo -e "${BLUE}Project:${NC} $PROJECT_ROOT"

# =============================================================================
# 1. SECRETS DETECTION
# =============================================================================
log_header "1. SECRETS DETECTION"

# Manual patterns for common secrets
log_section "Pattern-Based Secret Detection"

# Check for hardcoded secrets patterns
PATTERNS=(
    "password\s*=\s*[\"'][^\"']+[\"']"
    "api_key\s*=\s*[\"'][^\"']+[\"']"
    "apikey\s*=\s*[\"'][^\"']+[\"']"
    "secret\s*=\s*[\"'][^\"']+[\"']"
    "token\s*=\s*[\"'][^\"']+[\"']"
    "private_key"
    "BEGIN RSA PRIVATE KEY"
    "BEGIN OPENSSH PRIVATE KEY"
    "BEGIN EC PRIVATE KEY"
    "AKIA[0-9A-Z]{16}"  # AWS Access Key
    "sk-[a-zA-Z0-9]{48}"  # OpenAI API Key
    "ghp_[a-zA-Z0-9]{36}"  # GitHub Personal Access Token
    "glpat-[a-zA-Z0-9-]{20}"  # GitLab Personal Access Token
)

for pattern in "${PATTERNS[@]}"; do
    MATCHES=$(grep -rniE "$pattern" sources/ --include="*.m" --include="*.swift" --include="*.h" --include="*.c" --include="*.cpp" 2>/dev/null | grep -v "// " | head -5 || true)
    if [ -n "$MATCHES" ]; then
        log_critical "Potential secret found matching: $pattern"
        echo "$MATCHES" | head -3
    fi
done

log_success "Pattern scan complete"
log_end

# TruffleHog
log_section "TruffleHog Deep Scan"
if command -v trufflehog &> /dev/null; then
    TRUFFLEHOG_REPORT="$REPORT_DIR/trufflehog_$TIMESTAMP.json"
    log_info "Running TruffleHog filesystem scan..."
    trufflehog filesystem --json --no-update \
        --exclude-paths=ThirdParty \
        --exclude-paths=submodules \
        --exclude-paths=Pods \
        . 2>/dev/null > "$TRUFFLEHOG_REPORT" || true

    SECRETS_COUNT=$(wc -l < "$TRUFFLEHOG_REPORT" | tr -d ' ')
    if [ "$SECRETS_COUNT" -gt 0 ]; then
        log_critical "TruffleHog found $SECRETS_COUNT potential secrets!"
        log_info "Review: $TRUFFLEHOG_REPORT"
    else
        log_success "TruffleHog: No secrets detected"
    fi
else
    log_warn "TruffleHog not installed (brew install trufflehog)"
fi
log_end

# Gitleaks
log_section "Gitleaks Git History Scan"
if command -v gitleaks &> /dev/null; then
    GITLEAKS_REPORT="$REPORT_DIR/gitleaks_$TIMESTAMP.json"
    log_info "Running Gitleaks on git history..."
    if gitleaks detect --source . --report-path "$GITLEAKS_REPORT" --report-format json 2>/dev/null; then
        log_success "Gitleaks: No secrets in git history"
    else
        GITLEAKS_COUNT=$(jq 'length' "$GITLEAKS_REPORT" 2>/dev/null || echo "0")
        if [ "$GITLEAKS_COUNT" -gt 0 ]; then
            log_critical "Gitleaks found $GITLEAKS_COUNT secrets in git history!"
            log_info "Review: $GITLEAKS_REPORT"
        fi
    fi
else
    log_warn "Gitleaks not installed (brew install gitleaks)"
fi
log_end

# =============================================================================
# 2. VULNERABILITY PATTERNS
# =============================================================================
log_header "2. VULNERABILITY PATTERNS"

# Command Injection
log_section "Command Injection Checks"
log_info "Checking for dangerous command execution..."

# system() calls
SYSTEM_CALLS=$(grep -rn "system(" sources/ --include="*.m" --include="*.c" --include="*.cpp" 2>/dev/null | grep -v "filesystem" | grep -v "// " || true)
if [ -n "$SYSTEM_CALLS" ]; then
    log_critical "system() calls found - potential command injection!"
    echo "$SYSTEM_CALLS"
fi

# popen() calls
POPEN_CALLS=$(grep -rn "popen(" sources/ --include="*.m" --include="*.c" --include="*.cpp" 2>/dev/null | grep -v "// " || true)
if [ -n "$POPEN_CALLS" ]; then
    log_warn "popen() calls found - verify input sanitization"
    echo "$POPEN_CALLS" | head -5
fi

# NSTask with shell
NSTASK_SHELL=$(grep -rn "NSTask\|Process" sources/ --include="*.m" --include="*.swift" 2>/dev/null | grep -E "(/bin/sh|/bin/bash|-c)" | grep -v "// " || true)
if [ -n "$NSTASK_SHELL" ]; then
    log_warn "NSTask/Process with shell found - verify input sanitization"
    echo "$NSTASK_SHELL" | head -5
fi

log_end

# SQL Injection
log_section "SQL Injection Checks"
SQL_CONCAT=$(grep -rn "SELECT\|INSERT\|UPDATE\|DELETE" sources/ --include="*.m" --include="*.swift" 2>/dev/null | grep -E "(\+.*WHERE|\+.*=|stringWithFormat.*WHERE)" | grep -v "// " || true)
if [ -n "$SQL_CONCAT" ]; then
    log_critical "Potential SQL injection - string concatenation in SQL!"
    echo "$SQL_CONCAT" | head -5
fi
log_end

# Path Traversal
log_section "Path Traversal Checks"
PATH_ISSUES=$(grep -rn "\.\./" sources/ --include="*.m" --include="*.swift" 2>/dev/null | grep -v "// " | grep -v "test" || true)
if [ -n "$PATH_ISSUES" ]; then
    log_warn "Potential path traversal patterns found"
    echo "$PATH_ISSUES" | head -5
fi

# Check for unsanitized path operations
UNSANITIZED_PATHS=$(grep -rn "appendingPathComponent\|stringByAppendingPathComponent" sources/ --include="*.m" --include="*.swift" 2>/dev/null | grep -v "// " | head -10 || true)
if [ -n "$UNSANITIZED_PATHS" ]; then
    log_info "Path operations found - ensure input validation"
fi
log_end

# XSS (for any web views)
log_section "XSS Vulnerability Checks"
WEBVIEW_ISSUES=$(grep -rn "loadHTMLString\|stringByEvaluatingJavaScript\|evaluateJavaScript" sources/ --include="*.m" --include="*.swift" 2>/dev/null | grep -v "// " || true)
if [ -n "$WEBVIEW_ISSUES" ]; then
    log_warn "WebView with dynamic content - verify XSS protection"
    echo "$WEBVIEW_ISSUES" | head -5
fi
log_end

# =============================================================================
# 3. CRYPTOGRAPHIC ISSUES
# =============================================================================
log_header "3. CRYPTOGRAPHIC ISSUES"

log_section "Weak Cryptography Detection"

# MD5/SHA1 usage
WEAK_HASH=$(grep -rniE "(MD5|SHA1|sha1|md5)" sources/ --include="*.m" --include="*.swift" --include="*.c" 2>/dev/null | grep -v "// " | grep -v "SHA256\|SHA384\|SHA512" || true)
if [ -n "$WEAK_HASH" ]; then
    log_warn "Weak hash algorithms (MD5/SHA1) found - use SHA256+"
    echo "$WEAK_HASH" | head -5
fi

# DES/3DES/RC4
WEAK_CRYPTO=$(grep -rniE "(DES|3DES|RC4|RC2|Blowfish)" sources/ --include="*.m" --include="*.swift" --include="*.c" 2>/dev/null | grep -v "// " || true)
if [ -n "$WEAK_CRYPTO" ]; then
    log_critical "Weak encryption algorithms found!"
    echo "$WEAK_CRYPTO"
fi

# Hardcoded IVs
HARDCODED_IV=$(grep -rniE "iv\s*=\s*\[|initializationVector\s*=\s*\"" sources/ --include="*.m" --include="*.swift" 2>/dev/null | grep -v "// " || true)
if [ -n "$HARDCODED_IV" ]; then
    log_critical "Hardcoded IV found - use random IVs!"
    echo "$HARDCODED_IV"
fi

# ECB mode
ECB_MODE=$(grep -rniE "ECB|kCCOptionECBMode" sources/ --include="*.m" --include="*.swift" --include="*.c" 2>/dev/null | grep -v "// " || true)
if [ -n "$ECB_MODE" ]; then
    log_critical "ECB mode encryption found - use CBC/GCM!"
    echo "$ECB_MODE"
fi

log_end

# =============================================================================
# 4. MEMORY SAFETY
# =============================================================================
log_header "4. MEMORY SAFETY"

log_section "Buffer Overflow Risks"

# Dangerous C functions
DANGEROUS_FUNCS=$(grep -rniE "\b(strcpy|strcat|sprintf|gets|scanf)\s*\(" sources/ --include="*.m" --include="*.c" --include="*.cpp" 2>/dev/null | grep -v "// " || true)
if [ -n "$DANGEROUS_FUNCS" ]; then
    log_critical "Dangerous C functions found - use safe alternatives!"
    echo "$DANGEROUS_FUNCS"
    ((CRITICAL_FINDINGS++))
fi

# Format string vulnerabilities
FORMAT_STRING=$(grep -rniE "(NSLog|printf|fprintf|sprintf)\s*\(\s*[a-zA-Z_][a-zA-Z0-9_]*\s*\)" sources/ --include="*.m" --include="*.c" 2>/dev/null | grep -v "// " || true)
if [ -n "$FORMAT_STRING" ]; then
    log_warn "Potential format string vulnerabilities"
    echo "$FORMAT_STRING" | head -5
fi

log_end

# =============================================================================
# 5. SEMGREP SECURITY RULES
# =============================================================================
log_header "5. SEMGREP DEEP ANALYSIS"

log_section "Semgrep Security Scan"
if command -v semgrep &> /dev/null; then
    SEMGREP_REPORT="$REPORT_DIR/semgrep_security_$TIMESTAMP.json"
    log_info "Running Semgrep with security rules..."

    semgrep scan \
        --config "p/security-audit" \
        --config "p/secrets" \
        --config "p/owasp-top-ten" \
        --json \
        --exclude="ThirdParty" \
        --exclude="submodules" \
        --exclude="Pods" \
        sources/ > "$SEMGREP_REPORT" 2>/dev/null || true

    if [ -f "$SEMGREP_REPORT" ]; then
        SEMGREP_CRITICAL=$(jq '[.results[] | select(.extra.severity == "ERROR")] | length' "$SEMGREP_REPORT" 2>/dev/null || echo "0")
        SEMGREP_HIGH=$(jq '[.results[] | select(.extra.severity == "WARNING")] | length' "$SEMGREP_REPORT" 2>/dev/null || echo "0")
        SEMGREP_TOTAL=$(jq '.results | length' "$SEMGREP_REPORT" 2>/dev/null || echo "0")

        if [ "$SEMGREP_CRITICAL" -gt 0 ]; then
            log_critical "Semgrep: $SEMGREP_CRITICAL critical findings!"
        fi
        if [ "$SEMGREP_HIGH" -gt 0 ]; then
            log_warn "Semgrep: $SEMGREP_HIGH high-severity findings"
            ((HIGH_FINDINGS += SEMGREP_HIGH))
        fi
        if [ "$SEMGREP_TOTAL" -eq 0 ]; then
            log_success "Semgrep: No security issues found"
        fi
        log_info "Full report: $SEMGREP_REPORT"
    fi
else
    log_warn "Semgrep not installed (brew install semgrep)"
fi
log_end

# =============================================================================
# 6. DEPENDENCY VULNERABILITIES
# =============================================================================
log_header "6. DEPENDENCY VULNERABILITIES"

log_section "Third-Party Code Review"
log_info "Checking ThirdParty directory..."

# List third-party components
if [ -d "ThirdParty" ]; then
    THIRD_PARTY_COUNT=$(find ThirdParty -maxdepth 1 -type d | wc -l | tr -d ' ')
    log_info "Found $((THIRD_PARTY_COUNT - 1)) third-party components"

    # Check for known vulnerable libraries
    if [ -f "ThirdParty/openssl" ] || grep -r "openssl" ThirdParty/ &>/dev/null; then
        log_warn "OpenSSL found - ensure it's up to date"
    fi
fi

# Python dependencies
if [ -f "requirements.txt" ]; then
    log_section "Python Dependency Check"
    if command -v safety &> /dev/null; then
        SAFETY_REPORT="$REPORT_DIR/safety_$TIMESTAMP.json"
        safety check -r requirements.txt --json > "$SAFETY_REPORT" 2>/dev/null || true
        VULN_COUNT=$(jq 'length' "$SAFETY_REPORT" 2>/dev/null || echo "0")
        if [ "$VULN_COUNT" -gt 0 ]; then
            log_critical "Safety: $VULN_COUNT vulnerable Python dependencies!"
        else
            log_success "Safety: No vulnerable Python dependencies"
        fi
    fi
fi

log_end

# =============================================================================
# FINAL SECURITY REPORT
# =============================================================================
log_header "SECURITY SCAN COMPLETE"

echo ""
echo "============================================================================="
echo "                      SECURITY SUMMARY"
echo "============================================================================="
echo ""

TOTAL_FINDINGS=$((CRITICAL_FINDINGS + HIGH_FINDINGS + MEDIUM_FINDINGS + LOW_FINDINGS))

echo -e "  Critical:  ${RED}$CRITICAL_FINDINGS${NC}"
echo -e "  High:      ${YELLOW}$HIGH_FINDINGS${NC}"
echo -e "  Medium:    ${BLUE}$MEDIUM_FINDINGS${NC}"
echo -e "  Low:       $LOW_FINDINGS"
echo ""
echo "  Reports saved to: $REPORT_DIR"
echo ""

if [ "$CRITICAL_FINDINGS" -gt 0 ]; then
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║              🚨 CRITICAL SECURITY ISSUES 🚨                   ║${NC}"
    echo -e "${RED}║                                                               ║${NC}"
    echo -e "${RED}║  DO NOT DEPLOY until critical issues are resolved.           ║${NC}"
    echo -e "${RED}║  Review reports in: $REPORT_DIR${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 1
elif [ "$HIGH_FINDINGS" -gt 0 ]; then
    echo -e "${YELLOW}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║              ⚠️  HIGH SEVERITY ISSUES ⚠️                        ║${NC}"
    echo -e "${YELLOW}║                                                               ║${NC}"
    echo -e "${YELLOW}║  Review and remediate before deployment.                     ║${NC}"
    echo -e "${YELLOW}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 2
else
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║              ✅ SECURITY SCAN PASSED ✅                        ║${NC}"
    echo -e "${GREEN}║                                                               ║${NC}"
    echo -e "${GREEN}║  No critical or high-severity issues detected.               ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 0
fi
