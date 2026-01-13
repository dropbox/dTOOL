#!/bin/bash
# check_dead_code_justifications.sh - Enforce #[allow(dead_code)] attribute limit
#
# Purpose: Prevent proliferation of dead code attributes in the codebase.
# After systematic cleanup (N=301-318), we maintain justified attributes with comments.
# Updated N=2445: limit increased to 132 attributes; added abbreviated keyword recognition (Deserialize:,
# Architectural:, Test:, API Parity:, Debug:, M-XXX milestone refs).
#
# Usage: ./scripts/check_dead_code_justifications.sh
# Exit codes: 0 = success, 1 = limit exceeded or missing justifications
#
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

# Configuration
readonly MAX_ATTRIBUTES=132
readonly CRATES_DIR="crates"
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly NC='\033[0m' # No Color

echo "=== Dead Code Attribute Enforcement ==="
echo "Maximum allowed: $MAX_ATTRIBUTES attributes"
echo ""

# Count #[allow(dead_code)] attributes (excluding commented-out instances)
count=$(grep -r "#\[allow(dead_code)\]" "$CRATES_DIR/" --include="*.rs" \
    | grep -v "^[^:]*:[[:space:]]*///" \
    | grep -v "^[^:]*:[[:space:]]*//" \
    | wc -l \
    | tr -d ' ')

echo "Current count: $count attributes"

# Check if limit exceeded
if [ "$count" -gt "$MAX_ATTRIBUTES" ]; then
    echo -e "${RED}❌ FAIL: Dead code attribute limit exceeded!${NC}"
    echo ""
    echo "Found $count attributes, maximum allowed is $MAX_ATTRIBUTES"
    echo ""
    echo "Action required:"
    echo "  1. Remove unnecessary #[allow(dead_code)] attributes"
    echo "  2. Or justify the increase and update MAX_ATTRIBUTES in this script"
    echo ""
    echo "Files with dead_code attributes:"
    grep -r "#\[allow(dead_code)\]" "$CRATES_DIR/" --include="*.rs" \
        | grep -v "^[^:]*:[[:space:]]*///" \
        | grep -v "^[^:]*:[[:space:]]*//" \
        | cut -d: -f1 \
        | sort -u
    exit 1
fi

echo -e "${GREEN}✓ PASS: Within limit ($count / $MAX_ATTRIBUTES)${NC}"

# Check for attributes without justifications
echo ""
echo "Checking justification comments..."

# Find attributes that lack proper justification
# We expect either:
# 1. Comment above attribute explaining why it's needed
# 2. Inline comment on same line
attributes_without_justification=()

while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)

    # Check if there's a comment within 10 lines before the attribute
    # (JUSTIFICATION comment, or Category comment like "Serde deserialization", etc.)
    start_line=$((line_num - 10))
    if [ $start_line -lt 1 ]; then
        start_line=1
    fi

    # Extract context around the attribute
    context=$(sed -n "${start_line},${line_num}p" "$file")

    # Check if context contains justification keywords
    # Includes both full forms and abbreviated forms used in codebase
    if ! echo "$context" | grep -qiE "JUSTIFICATION|Serde deserialization|Deserialize:|Test infrastructure|Test:|Architectural field|Architectural:|Example demonstration|Compile-time validation|Feature-gated|Public API|API Parity:|Lifetime management|Reserved for future|Test-only field|Test-only|Debug:|M-[0-9]"; then
        attributes_without_justification+=("$file:$line_num")
    fi
done < <(grep -rn "#\[allow(dead_code)\]" "$CRATES_DIR/" --include="*.rs" \
    | grep -v "^[^:]*:[[:space:]]*///" \
    | grep -v "^[^:]*:[[:space:]]*//" )

if [ ${#attributes_without_justification[@]} -gt 0 ]; then
    echo -e "${YELLOW}⚠ WARNING: ${#attributes_without_justification[@]} attribute(s) missing justification:${NC}"
    for attr in "${attributes_without_justification[@]}"; do
        echo "  - $attr"
    done
    echo ""
    echo "All #[allow(dead_code)] attributes should have a comment explaining why they exist."
    echo "See N=305-318 commits for justification patterns."
    # Don't fail - just warn (justifications are best practice but not required)
fi

echo ""
echo -e "${GREEN}=== Enforcement check complete ===${NC}"
echo ""
echo "History:"
echo "  - N=301-304: Deleted 111 placeholder implementations"
echo "  - N=305-318: Justified 55 remaining attributes across 12 categories"
echo "  - N=319: Deleted 1 unused validation function (55 → 54 attributes)"
echo "  - N=2445: Updated to 132 attrs, added abbreviated keyword recognition"
echo "  - Current: $count attributes (limit $MAX_ATTRIBUTES), enforced by this script"
echo ""
echo "To add new #[allow(dead_code)] attributes:"
echo "  1. Add comprehensive justification comment (see existing examples)"
echo "  2. Update MAX_ATTRIBUTES if genuinely needed"
echo "  3. Document in commit message why the increase is necessary"

exit 0
