#!/bin/bash
# Check documentation freshness, accuracy, and docstring coverage
# Run this in CI or before commits to catch stale docs

set -euo pipefail

echo "=== DashFlow Documentation Freshness Check ==="
echo ""

ERROR_LOG=$(mktemp)

# 1. Check for broken internal links
echo "1. Checking internal markdown links..."
for doc in docs/*.md *.md; do
    if [ -f "$doc" ]; then
        # Extract markdown links and check if targets exist
        grep -oE '\[.*\]\([^)]+\.md\)' "$doc" 2>/dev/null | while read -r link; do
            target=$(echo "$link" | sed 's/.*(\(.*\))/\1/' | sed 's/#.*//')
            dir=$(dirname "$doc")
            if [ -n "$target" ] && [ ! -f "$dir/$target" ] && [ ! -f "$target" ]; then
                echo "  ERROR: $doc has broken link to $target"
                echo "broken_link" >> "$ERROR_LOG"
            fi
        done
    fi
done
echo "  Done."
echo ""

# 2. Check for references to non-existent files
echo "2. Checking referenced file paths..."
for doc in docs/*.md CLAUDE.md WORKER_DIRECTIVE.md; do
    if [ -f "$doc" ]; then
        # Skip files marked as proposals (they may reference non-existent future crates)
        if grep -q "^> \*\*Status:\*\* PROPOSAL" "$doc" 2>/dev/null; then
            continue
        fi
        # Look for crates/ paths and verify they exist
        # Filter out template patterns like crates/dashflow-{name}
        grep -oE 'crates/[a-zA-Z0-9_-]+' "$doc" 2>/dev/null | sort -u | while read -r path; do
            # Skip if followed by template syntax (check original context)
            if grep -q "${path}\[{}\[]" "$doc" 2>/dev/null || grep -q "${path}{" "$doc" 2>/dev/null; then
                continue
            fi
            # Skip known documentation placeholder examples
            case "$path" in
                crates/X|crates/dashflow-my-crate|crates/dashflow-myservice|crates/dashflow-newcrate)
                    continue
                    ;;
            esac
            if [ ! -d "$path" ]; then
                echo "  WARNING: $doc references $path which doesn't exist"
            fi
        done
    fi
done
echo "  Done."
echo ""

# 3. Check README files for common issues
echo "3. Checking crate READMEs for common issues..."
MALFORMED=$(grep -l "dashflow-dashflow" crates/*/README.md 2>/dev/null | wc -l | tr -d ' ' || echo "0")
BROKEN_LINKS=$(grep -l 'PYTHON_TO_RUST_GUIDE.md' crates/*/README.md 2>/dev/null | wc -l | tr -d ' ' || echo "0")
if [ "$MALFORMED" -gt 0 ]; then
    echo "  ERROR: $MALFORMED READMEs have malformed crate names (dashflow-dashflow)"
    for i in $(seq 1 $MALFORMED); do echo "malformed_name" >> "$ERROR_LOG"; done
fi
if [ "$BROKEN_LINKS" -gt 0 ]; then
    echo "  ERROR: $BROKEN_LINKS READMEs have broken migration guide links"
    for i in $(seq 1 $BROKEN_LINKS); do echo "broken_readme_link" >> "$ERROR_LOG"; done
fi
echo "  Done."
echo ""

# 4. Check rustdoc compilation
echo "4. Checking rustdoc compilation (core crate only)..."
RUSTDOC_LOG=$(mktemp)
set +e
RUSTDOCFLAGS="-W missing_docs" cargo doc --package dashflow --no-deps >"$RUSTDOC_LOG" 2>&1
RUSTDOC_EXIT_CODE=$?
set -e
if [ "$RUSTDOC_EXIT_CODE" -ne 0 ] || grep -q "error\\[" "$RUSTDOC_LOG"; then
    echo "  ERROR: rustdoc has compilation errors"
    echo "rustdoc_error" >> "$ERROR_LOG"
else
    echo "  rustdoc compiles successfully"
fi
echo ""

# 5. Check docstring coverage (public items with missing docs)
echo "5. Checking docstring coverage..."
# Count public items missing documentation
# This uses the Rust compiler's built-in missing_docs warning
MISSING_DOCS=$(grep -c "missing documentation" "$RUSTDOC_LOG" || true)
THRESHOLD=100

if [ "$MISSING_DOCS" -gt "$THRESHOLD" ]; then
    echo "  WARNING: $MISSING_DOCS public items missing documentation (threshold: $THRESHOLD)"
    echo "  This is informational - docstring coverage should improve over time."
    echo ""
    echo "  To see all missing docs, run:"
    echo "    RUSTDOCFLAGS=\"-W missing_docs\" cargo doc --package dashflow --no-deps 2>&1 | grep \"missing documentation\""
else
    echo "  Docstring coverage good: $MISSING_DOCS items missing docs (threshold: $THRESHOLD)"
fi
rm -f "$RUSTDOC_LOG"
echo ""

# 6. Check that key docs exist
echo "6. Checking required documentation files..."
REQUIRED_DOCS=(
    "README.md"
    "CLAUDE.md"
    "CHANGELOG.md"
    "docs/QUICK_START_PRODUCTION.md"
    "docs/CLI_REFERENCE.md"
    "docs/ERROR_TYPES.md"
    "docs/TESTING.md"
    "docs/CONFIGURATION.md"
)
for doc in "${REQUIRED_DOCS[@]}"; do
    if [ ! -f "$doc" ]; then
        echo "  ERROR: Required doc missing: $doc"
        echo "missing_required_doc" >> "$ERROR_LOG"
    fi
done
echo "  Done."
echo ""

# Count errors from log file
ERRORS=$(wc -l < "$ERROR_LOG" 2>/dev/null | tr -d ' ' || echo "0")
rm -f "$ERROR_LOG"

# Summary
echo "=== Summary ==="
if [ "$ERRORS" -gt 0 ]; then
    echo "Found $ERRORS documentation issues."
    echo ""
    echo "To fix broken links:"
    echo "  1. Update the link to point to an existing file"
    echo "  2. Or remove the link if the target is no longer relevant"
    echo ""
    exit 1
else
    echo "All documentation checks passed."
    exit 0
fi
