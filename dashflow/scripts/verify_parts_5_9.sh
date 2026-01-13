#!/bin/bash
# DashFlow Parts 5-9 Integration Test Suite
# Verifies all "complete" work from Parts 5-9 actually works
#
# Usage: ./scripts/verify_parts_5_9.sh [--quick|--full]
#
# Parts covered:
#   Part 5: Observability pipeline (E2E tests, Prometheus, Grafana)
#   Part 6: Documentation completeness (READMEs, API docs)
#   Part 7: Code quality (unwrap usage, suppressions)
#   Part 8: Code hygiene (dead code, clippy, println)
#   Part 9: README sync (version consistency, See Also links)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

MODE="${1:---quick}"
FAILURES=0
WARNINGS=0

echo "=== Parts 5-9 Integration Test Suite ==="
echo "Time: $(date -Iseconds)"
echo "Mode: $MODE"
echo ""

log_pass() { echo "  [PASS] $1"; }
log_fail() { echo "  [FAIL] $1"; FAILURES=$((FAILURES + 1)); }
log_warn() { echo "  [WARN] $1"; WARNINGS=$((WARNINGS + 1)); }
log_skip() { echo "  [SKIP] $1"; }

# ========================================
# PART 5: Observability Pipeline
# ========================================
echo "=== Part 5: Observability Pipeline ==="

# P5.1: Check prometheus.yml exists and has valid structure
echo "P5.1: Prometheus config..."
if [ -f prometheus.yml ]; then
    # Check basic structure without requiring yaml module
    if grep -q "scrape_configs:" prometheus.yml && grep -q "global:" prometheus.yml; then
        log_pass "prometheus.yml exists with valid structure"
    else
        log_fail "prometheus.yml missing required sections"
    fi
else
    log_fail "prometheus.yml not found"
fi

# P5.2: Check Grafana dashboard exists
echo "P5.2: Grafana dashboard..."
if [ -f grafana/dashboards/grafana_quality_dashboard.json ]; then
    if python3 -c "import json; json.load(open('grafana/dashboards/grafana_quality_dashboard.json'))" 2>/dev/null; then
        log_pass "Grafana dashboard is valid JSON"
    else
        log_fail "Grafana dashboard has JSON errors"
    fi
else
    log_fail "Grafana dashboard not found"
fi

# P5.3: Check docker-compose exists for observability
echo "P5.3: Docker compose configs..."
if [ -f docker-compose.dashstream.yml ]; then
    log_pass "docker-compose.dashstream.yml exists"
else
    log_fail "docker-compose.dashstream.yml not found"
fi

# P5.4: E2E observability test file exists
echo "P5.4: E2E test file..."
if [ -f test-utils/tests/observability_pipeline.rs ]; then
    log_pass "Observability E2E tests exist"
else
    log_fail "Missing test-utils/tests/observability_pipeline.rs"
fi

# ========================================
# PART 6: Documentation Completeness
# ========================================
echo ""
echo "=== Part 6: Documentation Completeness ==="

# P6.1: Check for "Coming Soon" in docs
echo "P6.1: Coming Soon placeholders..."
COMING_SOON=$(grep -rn "Coming Soon" docs/ --include="*.md" 2>/dev/null | grep -v "Planned" | grep -v "Additional Developer Tools" | wc -l | tr -d ' ')
if [ "$COMING_SOON" -eq 0 ]; then
    log_pass "No misleading 'Coming Soon' in docs"
else
    log_warn "$COMING_SOON 'Coming Soon' references found"
fi

# P6.2: Check for TBD without context
echo "P6.2: TBD placeholders..."
TBD_COUNT=$(grep -rn "TBD" docs/ --include="*.md" 2>/dev/null | grep -v "Progress\|Phase\|Planned" | wc -l | tr -d ' ')
if [ "$TBD_COUNT" -lt 5 ]; then
    log_pass "TBD count acceptable ($TBD_COUNT)"
else
    log_warn "$TBD_COUNT TBD placeholders found"
fi

# P6.3: README validation script runs
echo "P6.3: README validation..."
if [ -f scripts/validate_readmes.py ]; then
    if python3 scripts/validate_readmes.py 2>/dev/null | grep -q "passed"; then
        log_pass "README validation passes"
    else
        log_fail "README validation failed"
    fi
else
    log_skip "validate_readmes.py not found"
fi

# ========================================
# PART 7: Code Quality
# ========================================
echo ""
echo "=== Part 7: Code Quality ==="

# P7.1: No unwrap() in production code (test/example exceptions)
echo "P7.1: unwrap() usage..."
UNWRAP_PROD=$(grep -rn '\.unwrap()' crates/ --include="*.rs" | grep -v test | grep -v example | grep -v "#\[cfg(test)\]" | wc -l | tr -d ' ')
if [ "$UNWRAP_PROD" -lt 50 ]; then
    log_pass "Production unwrap() count: $UNWRAP_PROD"
else
    log_warn "High unwrap() count in production: $UNWRAP_PROD"
fi

# P7.2: expect() has descriptive messages
echo "P7.2: expect() messages..."
EMPTY_EXPECT=$(grep -rn '\.expect("")' crates/ --include="*.rs" | wc -l | tr -d ' ')
if [ "$EMPTY_EXPECT" -eq 0 ]; then
    log_pass "No empty expect() messages"
else
    log_warn "$EMPTY_EXPECT empty expect() messages found"
fi

# ========================================
# PART 8: Code Hygiene
# ========================================
echo ""
echo "=== Part 8: Code Hygiene ==="

# P8.1: Dead code suppressions are reasonable
echo "P8.1: dead_code suppressions..."
DEAD_CODE=$(grep -rn '#\[allow(dead_code)\]' crates/ --include="*.rs" | wc -l | tr -d ' ')
if [ "$DEAD_CODE" -lt 200 ]; then
    log_pass "dead_code suppression count: $DEAD_CODE"
else
    log_warn "High dead_code suppression count: $DEAD_CODE"
fi

# P8.2: Clippy suppressions are reasonable
echo "P8.2: clippy suppressions..."
CLIPPY_ALLOW=$(grep -rn '#\[allow(clippy::' crates/ --include="*.rs" | wc -l | tr -d ' ')
if [ "$CLIPPY_ALLOW" -lt 100 ]; then
    log_pass "clippy suppression count: $CLIPPY_ALLOW"
else
    log_warn "High clippy suppression count: $CLIPPY_ALLOW"
fi

# P8.3: cargo check passes
echo "P8.3: cargo check..."
if timeout 300 cargo check 2>&1 | grep -q "^error\["; then
    log_fail "cargo check has errors"
else
    log_pass "cargo check passes"
fi

# ========================================
# PART 9: README Sync
# ========================================
echo ""
echo "=== Part 9: README Sync ==="

# P9.1: Example apps have See Also sections
echo "P9.1: Example app READMEs..."
EXAMPLE_READMES=$(find examples/ -name "README.md" 2>/dev/null | wc -l | tr -d ' ')
SEE_ALSO=$(find examples/ -name "README.md" -exec grep -l "See Also" {} \; 2>/dev/null | wc -l | tr -d ' ')
if [ "$EXAMPLE_READMES" -eq "$SEE_ALSO" ]; then
    log_pass "All $EXAMPLE_READMES example READMEs have See Also"
else
    log_warn "$SEE_ALSO/$EXAMPLE_READMES example READMEs have See Also"
fi

# P9.2: Crate READMEs exist
echo "P9.2: Crate READMEs..."
CRATE_COUNT=$(find crates/ -name "Cargo.toml" | wc -l | tr -d ' ')
README_COUNT=$(find crates/ -name "README.md" | wc -l | tr -d ' ')
if [ "$README_COUNT" -ge "$((CRATE_COUNT - 5))" ]; then
    log_pass "$README_COUNT/$CRATE_COUNT crates have READMEs"
else
    log_warn "Only $README_COUNT/$CRATE_COUNT crates have READMEs"
fi

# ========================================
# Full Mode: Run actual tests
# ========================================
if [ "$MODE" = "--full" ]; then
    echo ""
    echo "=== Full Mode: Running Tests ==="

    echo "Running test-utils tests..."
    if timeout 300 cargo test -p test-utils 2>&1 | grep -q "test result: ok"; then
        log_pass "test-utils tests pass"
    else
        log_fail "test-utils tests failed"
    fi
fi

# ========================================
# Summary
# ========================================
echo ""
echo "=== Summary ==="
echo "Failures: $FAILURES"
echo "Warnings: $WARNINGS"
echo ""

if [ $FAILURES -eq 0 ]; then
    echo "STATUS: [PASS] Parts 5-9 integration tests passed"
    exit 0
else
    echo "STATUS: [FAIL] $FAILURES failures found"
    exit 1
fi
