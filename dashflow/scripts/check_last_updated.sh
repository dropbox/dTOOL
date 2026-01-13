#!/bin/bash
# scripts/check_last_updated.sh - M-03: Check/update Last Updated dates
#
# Usage:
#   ./scripts/check_last_updated.sh           # Check all tracked docs
#   ./scripts/check_last_updated.sh --fix     # Update stale dates interactively
#   ./scripts/check_last_updated.sh FILE      # Check specific file
#
# Tracked documents:
#   - CLAUDE.md
#   - ROADMAP_CURRENT.md
#   - WORKER_DIRECTIVE.md (optional; only present when the Manager sets an active directive)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"
TODAY=$(date +%Y-%m-%d)

# Tracked documentation files
TRACKED_DOCS=(
    "CLAUDE.md"
    "ROADMAP_CURRENT.md"
    "WORKER_DIRECTIVE.md"
)

check_file() {
    local file="$1"
    local filepath="$REPO_ROOT/$file"

    if [ ! -f "$filepath" ]; then
        if [ "$file" = "WORKER_DIRECTIVE.md" ]; then
            echo "ℹ️  $file: Not present (no active directive)"
            return 0
        fi
        echo "ERROR: $file does not exist"
        return 1
    fi

    # Extract date from "**Last Updated:** YYYY-MM-DD" pattern
    local last_updated=$(grep -m1 "^\*\*Last Updated:\*\*" "$filepath" 2>/dev/null | \
        grep -oE "[0-9]{4}-[0-9]{2}-[0-9]{2}" | head -1)

    if [ -z "$last_updated" ]; then
        echo "❌ $file: No Last Updated date found"
        return 1
    fi

    local expected_date=""
    local expected_reason=""

    if git diff --quiet -- "$file" 2>/dev/null && git diff --cached --quiet -- "$file" 2>/dev/null; then
        expected_date=$(git log -1 --format=%cs -- "$file" 2>/dev/null || true)
        expected_reason="last commit date"
        if [ -z "$expected_date" ]; then
            expected_date="$TODAY"
            expected_reason="fallback (no git history)"
        fi
    else
        expected_date="$TODAY"
        expected_reason="working tree modified"
    fi

    if [ "$last_updated" = "$expected_date" ]; then
        echo "✅ $file: Current ($last_updated; expected $expected_date - $expected_reason)"
        return 0
    fi

    echo "⚠️  $file: Stale ($last_updated; expected $expected_date - $expected_reason)"
    return 1
}

update_file() {
    local file="$1"
    local author="$2"
    local filepath="$REPO_ROOT/$file"

    if [ ! -f "$filepath" ]; then
        echo "ERROR: $file does not exist"
        return 1
    fi

    # Update the Last Updated line with today's date, preserving author
    local current_line=$(grep -m1 "^\*\*Last Updated:\*\*" "$filepath")
    local new_line="**Last Updated:** $TODAY ($author)"

    if [ -n "$current_line" ]; then
        # Use sed to replace the line (macOS compatible)
        sed -i '' "s|^\*\*Last Updated:\*\*.*|$new_line|" "$filepath"
        echo "Updated $file: $new_line"
    else
        echo "ERROR: No Last Updated line found in $file"
        return 1
    fi
}

# Parse arguments
FIX_MODE=false
SPECIFIC_FILE=""
AUTHOR=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --fix)
            FIX_MODE=true
            shift
            ;;
        --author)
            AUTHOR="${2:-}"
            shift 2
            ;;
        *)
            SPECIFIC_FILE="$1"
            shift
            ;;
    esac
done

echo "=== M-03: Last Updated Check ($(date +%Y-%m-%d)) ==="
echo ""

STALE_FILES=()

if [ -n "$SPECIFIC_FILE" ]; then
    # Check specific file
    check_file "$SPECIFIC_FILE" || STALE_FILES+=("$SPECIFIC_FILE")
else
    # Check all tracked docs
    for doc in "${TRACKED_DOCS[@]}"; do
        check_file "$doc" || STALE_FILES+=("$doc")
    done
fi

echo ""

if [ ${#STALE_FILES[@]} -eq 0 ]; then
    echo "All checked files have current Last Updated dates."
    exit 0
fi

if [ "$FIX_MODE" = true ]; then
    echo "=== Fix Mode ==="
    for file in "${STALE_FILES[@]}"; do
        echo ""
        echo "Updating $file..."
        author="$AUTHOR"
        if [ -z "$author" ]; then
            if [ -t 0 ]; then
                read -p "Enter author (e.g., 'Worker #1308'): " author
            else
                echo "ERROR: --fix requires --author in non-interactive mode"
                echo "  Example: ./scripts/check_last_updated.sh --fix --author 'Worker #2422 - Metadata sync'"
                exit 1
            fi
        fi
        if [ -n "$author" ]; then
            update_file "$file" "$author"
        else
            echo "Skipped (no author provided)"
        fi
    done
else
    echo "Found ${#STALE_FILES[@]} stale file(s)."
    echo ""
    echo "To update stale dates, run:"
    echo "  ./scripts/check_last_updated.sh --fix"
    echo ""
    echo "Or manually edit the '**Last Updated:**' line in each file."
    exit 1
fi
