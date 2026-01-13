#!/bin/bash
# Verification script for documentation claims
# Ensures all quantitative claims in README match measured reality
# Usage: ./scripts/verify_documentation_claims.sh

set -euo pipefail

echo "=== Documentation Claim Verification ==="
echo "Date: $(date)"
echo ""

ERRORS=0

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Config
# - DASHFLOW_DOC_CLAIMS_MODE:
#     - standard (default): verifies README claims for the primary package (default: dashflow)
#     - workspace: verifies workspace-wide build/check/clippy (much slower)
# - DASHFLOW_DOC_CLAIMS_PACKAGE: package to verify in standard mode (default: dashflow)
MODE="${DASHFLOW_DOC_CLAIMS_MODE:-standard}"
PACKAGE="${DASHFLOW_DOC_CLAIMS_PACKAGE:-dashflow}"

# Helpers to extract claims from README.md.
extract_first_match() {
    local pattern="$1"
    local file="$2"
    # Uses perl for portable regex capture.
    perl -ne "if (/$pattern/) { print \"\$1\\n\"; exit 0 }" "$file" 2>/dev/null || true
}

extract_readme_test_claim_number() {
    # Prefer the tests badge claim: tests-<N>%2B%20passing
    local badge_num
    badge_num="$(extract_first_match 'tests-([0-9]+)%2B' README.md)"
    if [ -n "$badge_num" ]; then
        echo "$badge_num"
        return 0
    fi

    # Fallback to markdown text: "<N>+ tests"
    local text_num
    text_num="$(extract_first_match '([0-9][0-9,]*)\+ tests' README.md | tr -d ',')"
    if [ -n "$text_num" ]; then
        echo "$text_num"
        return 0
    fi

    echo "0"
}

extract_readme_crate_claim_number() {
    # README tagline: "108 crates. Pure Rust..."
    local crate_num
    crate_num="$(extract_first_match '([0-9]+) crates\.' README.md)"
    if [ -n "$crate_num" ]; then
        echo "$crate_num"
        return 0
    fi
    echo "0"
}

# Function to compare values
check_match() {
    local name="$1"
    local actual="$2"
    local claimed="$3"
    local tolerance="${4:-0}" # Optional tolerance for approximate matches

    echo -n "Checking $name... "

    if [ "$actual" == "$claimed" ]; then
        echo -e "${GREEN}✅ PASS${NC} (actual: $actual, claimed: $claimed)"
        return 0
    elif [ -n "$tolerance" ] && [ "$tolerance" -gt 0 ]; then
        local diff=$((actual - claimed))
        local abs_diff=${diff#-}
        if [ "$abs_diff" -le "$tolerance" ]; then
            echo -e "${GREEN}✅ PASS${NC} (actual: $actual, claimed: $claimed, within tolerance: ±$tolerance)"
            return 0
        fi
    fi

    echo -e "${RED}❌ FAIL${NC} (actual: $actual, claimed: $claimed)"
    ERRORS=$((ERRORS + 1))
    return 1
}

check_lower_bound() {
    local name="$1"
    local actual="$2"
    local claimed_min="$3"

    echo -n "Checking $name... "

    if [ "$actual" -ge "$claimed_min" ]; then
        echo -e "${GREEN}✅ PASS${NC} (actual: $actual, claimed: >= $claimed_min)"
        return 0
    fi

    echo -e "${RED}❌ FAIL${NC} (actual: $actual, claimed: >= $claimed_min)"
    ERRORS=$((ERRORS + 1))
    return 1
}

# 1. Test Count
echo "=== 1. Test Count ==="
README_TEST_MIN="$(extract_readme_test_claim_number)"
echo "Measuring: #[test] + #[tokio::test] in crates/ (fast, matches README counting convention)"
TEST_ATTR_COUNT="$( (rg -F -g'*.rs' '#[test]' crates || true) | wc -l | tr -d '[:space:]' )"
TOKIO_TEST_ATTR_COUNT="$( (rg -F -g'*.rs' '#[tokio::test' crates || true) | wc -l | tr -d '[:space:]' )"
TEST_COUNT="$((TEST_ATTR_COUNT + TOKIO_TEST_ATTR_COUNT))"
echo "Found: ${TEST_ATTR_COUNT} #[test] + ${TOKIO_TEST_ATTR_COUNT} #[tokio::test] = ${TEST_COUNT}"
check_lower_bound "Test count (crates/)" "$TEST_COUNT" "$README_TEST_MIN"
echo ""

# 2. Version Consistency
echo "=== 2. Version Consistency ==="
CARGO_VER=$(grep "^version = " Cargo.toml | head -1 | grep -o "[0-9]\+\.[0-9]\+\.[0-9]\+" || echo "unknown")
README_VER=$(grep -o "version-[0-9]\+\.[0-9]\+\.[0-9]\+" README.md | head -1 | grep -o "[0-9]\+\.[0-9]\+\.[0-9]\+" || echo "unknown")
LATEST_TAG=$(git tag -l "v[0-9]*.[0-9]*.[0-9]*" | sed 's/^v//' | sort -V | tail -1 || echo "no-tags")
echo "Cargo.toml version: $CARGO_VER"
echo "README badge version: $README_VER"
echo "Latest git tag: $LATEST_TAG"
check_match "Cargo vs README version" "$CARGO_VER" "$README_VER"
echo ""

# 3. Crate Count
echo "=== 3. Crate Count ==="
ACTUAL_CRATES="$(ls crates/*/Cargo.toml 2>/dev/null | wc -l | tr -d ' ')"
README_CRATES="$(extract_readme_crate_claim_number)"
echo "Crates found in crates/: $ACTUAL_CRATES"
echo "README claims: $README_CRATES crates"
check_match "Crate count" "$ACTUAL_CRATES" "$README_CRATES"
echo ""

if [ "$MODE" = "standard" ]; then
    # 4. Compiler Warnings (package)
    echo "=== 4. Compiler Warnings Check (${PACKAGE}) ==="
    echo "Running: cargo check -p ${PACKAGE} --quiet 2>&1 | grep -i warning | wc -l"
    WARNING_COUNT="$(cargo check -p "${PACKAGE}" --quiet 2>&1 | (grep -i "warning:" || true) | wc -l | tr -d ' ')"
    echo "Compiler warnings: $WARNING_COUNT"
    check_match "Zero warnings (${PACKAGE})" "$WARNING_COUNT" "0"
    echo ""

    # 5. Clippy (production unwrap/expect)
    echo "=== 5. Clippy Production Unwrap/Expect Check (${PACKAGE}) ==="
    echo "Running: cargo clippy -p ${PACKAGE} --lib --bins --quiet -- -D clippy::unwrap_used -D clippy::expect_used"
    if cargo clippy -p "${PACKAGE}" --lib --bins --quiet -- -D clippy::unwrap_used -D clippy::expect_used >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC} No production unwrap()/expect()"
    else
        echo -e "${RED}❌ FAIL${NC} unwrap()/expect() used in prod targets"
        ERRORS=$((ERRORS + 1))
    fi
    echo ""
elif [ "$MODE" = "workspace" ]; then
    # 4. Compiler Warnings (workspace)
    echo "=== 4. Compiler Warnings Check (workspace) ==="
    echo "Running: cargo check --workspace --quiet 2>&1 | grep -i warning | wc -l"
    WARNING_COUNT="$(cargo check --workspace --quiet 2>&1 | (grep -i "warning:" || true) | wc -l | tr -d ' ')"
    echo "Compiler warnings: $WARNING_COUNT"
    check_match "Zero warnings (workspace)" "$WARNING_COUNT" "0"
    echo ""

    # 5. Clippy (production unwrap/expect)
    echo "=== 5. Clippy Production Unwrap/Expect Check (workspace) ==="
    echo "Running: cargo clippy --workspace --lib --bins --quiet -- -D clippy::unwrap_used -D clippy::expect_used"
    if cargo clippy --workspace --lib --bins --quiet -- -D clippy::unwrap_used -D clippy::expect_used >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC} No production unwrap()/expect()"
    else
        echo -e "${RED}❌ FAIL${NC} unwrap()/expect() used in prod targets"
        ERRORS=$((ERRORS + 1))
    fi
    echo ""
else
    echo -e "${YELLOW}⚠️  WARNING${NC} DASHFLOW_DOC_CLAIMS_MODE=\"$MODE\" is not supported (use \"standard\" or \"workspace\"); skipping compiler/clippy checks"
    echo ""
fi

# 6. Release Notes Exist
echo "=== 6. Release Notes Existence ==="
if [ -f "docs/RELEASE_NOTES_v${CARGO_VER}.md" ]; then
    echo -e "${GREEN}✅ PASS${NC} docs/RELEASE_NOTES_v${CARGO_VER}.md exists"
else
    echo -e "${YELLOW}⚠️  WARNING${NC} docs/RELEASE_NOTES_v${CARGO_VER}.md not found"
    # Not counted as error since this may be in development
fi
echo ""

if [ "$MODE" = "standard" ]; then
    # 7. Build Success (package)
    echo "=== 7. Build Verification (${PACKAGE}) ==="
    echo "Running: cargo build -p ${PACKAGE} --quiet"
    if cargo build -p "${PACKAGE}" --quiet >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC} Package builds successfully"
    else
        echo -e "${RED}❌ FAIL${NC} Build errors found"
        ERRORS=$((ERRORS + 1))
    fi
    echo ""
elif [ "$MODE" = "workspace" ]; then
    # 7. Build Success (workspace)
    echo "=== 7. Build Verification (workspace) ==="
    echo "Running: cargo build --workspace --quiet"
    if cargo build --workspace --quiet >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC} Workspace builds successfully"
    else
        echo -e "${RED}❌ FAIL${NC} Build errors found"
        ERRORS=$((ERRORS + 1))
    fi
    echo ""
fi

# Summary
echo "=== Summary ==="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}All checks passed! ✅${NC}"
    echo "Documentation claims are consistent with measured reality."
    exit 0
else
    echo -e "${RED}$ERRORS check(s) failed ❌${NC}"
    echo "Please update documentation to match actual measurements."
    exit 1
fi

# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
