#!/bin/bash
# DashFlow Deprecation Scanner
# Scans the codebase for usage of deprecated items in tests and source code.
#
# Usage: ./scripts/check_deprecations.sh [--strict] [--json] [path]
#
# Options:
#   --strict    Fail if any deprecated usage found (exit 1)
#   --json      Output results as JSON
#   path        Limit scan to specific path (default: crates/)
#
# This script:
# 1. Finds all #[deprecated] declarations in the codebase
# 2. Extracts deprecated function/struct names
# 3. Searches for usages of these deprecated items
# 4. Uses find to scan nested test directories
#
# Exit codes:
#   0 = No deprecated usage found (or --strict not set)
#   1 = Deprecated usage found (with --strict)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

STRICT=false
JSON_OUTPUT=false
SCAN_PATH="crates/"

while [[ $# -gt 0 ]]; do
    case $1 in
        --strict)
            STRICT=true
            shift
            ;;
        --json)
            JSON_OUTPUT=true
            shift
            ;;
        *)
            SCAN_PATH="$1"
            shift
            ;;
    esac
done

if [[ ! -d "$SCAN_PATH" ]]; then
    echo "Error: Path '$SCAN_PATH' does not exist" >&2
    exit 1
fi

DEPRECATED_ITEMS=$(mktemp)
USAGE_REPORT=$(mktemp)
trap 'rm -f "$DEPRECATED_ITEMS" "$USAGE_REPORT"' EXIT

echo "=== DashFlow Deprecation Scanner ===" >&2
echo "Time: $(date -Iseconds)" >&2
echo "Scan Path: $SCAN_PATH" >&2
echo "Mode: $(if $STRICT; then echo "strict"; else echo "normal"; fi)" >&2
echo "" >&2

# Step 1: Find deprecated declarations and extract names
echo "1. Finding deprecated declarations..." >&2

# Use grep to find deprecated attributes, then scan ahead to find the actual item name
# Deprecated attrs can span multiple lines, so we need to find the fn/struct after the )]
grep -rn '#\[deprecated' "$SCAN_PATH" --include="*.rs" 2>/dev/null | while IFS=: read -r file lineno _rest; do
    # Look 15 lines ahead for the item declaration (attributes can span many lines)
    # Skip lines containing note=, since=, or #[ to avoid picking up attr content
    # Extract item name from deprecated declaration
    # Note: grep -m1 may return non-zero if no match (e.g., deprecated fields),
    # so we add || true to prevent script failure with set -e
    item_name=$(sed -n "$lineno,$((lineno+15))p" "$file" 2>/dev/null | \
        grep -v 'note\s*=' | \
        grep -v 'since\s*=' | \
        grep -v '#\[' | \
        grep -m1 -oE '\b(fn|struct|type|const|enum|trait)\s+[a-zA-Z_][a-zA-Z0-9_]*' | \
        awk '{print $2}' || true)

    # Validate we got a real item name, not a keyword or common method name
    # Filter out: Rust keywords AND common method names that cause false positive floods
    # (e.g., deprecated structs have `fn new()` which matches every Foo::new() call)
    if [[ -n "$item_name" && "$item_name" != "fn" && "$item_name" != "struct" && \
          "$item_name" != "type" && "$item_name" != "const" && "$item_name" != "enum" && \
          "$item_name" != "trait" && "$item_name" != "pub" && "$item_name" != "async" && \
          "$item_name" != "new" && "$item_name" != "default" && "$item_name" != "from" && \
          "$item_name" != "into" && "$item_name" != "Self" && "$item_name" != "self" ]]; then
        printf '%s\t%s:%s\n' "$item_name" "$file" "$lineno"
    fi
done | sort -u > "$DEPRECATED_ITEMS"

DEPRECATED_COUNT=$(wc -l < "$DEPRECATED_ITEMS" | tr -d ' ')
echo "   Found $DEPRECATED_COUNT deprecated items" >&2

if [[ $DEPRECATED_COUNT -eq 0 ]]; then
    echo "" >&2
    echo "=== Summary ===" >&2
    echo "No deprecated items declared in codebase." >&2
    if $JSON_OUTPUT; then
        echo '{"deprecated_declarations":0,"deprecated_usages":0,"status":"clean"}'
    fi
    exit 0
fi

# Show found deprecated items
echo "   Items:" >&2
head -10 "$DEPRECATED_ITEMS" | while IFS=$'\t' read -r name loc; do
    echo "     - $name (at $loc)" >&2
done
[[ $DEPRECATED_COUNT -gt 10 ]] && echo "     ... and $((DEPRECATED_COUNT - 10)) more" >&2

# Step 2: Search for usages
echo "" >&2
echo "2. Scanning for deprecated usage (including nested tests)..." >&2

> "$USAGE_REPORT"

# For each deprecated item, search for usages
while IFS=$'\t' read -r item_name declaration_loc; do
    [[ -z "$item_name" ]] && continue

    decl_file=$(echo "$declaration_loc" | cut -d: -f1)

    # Search for word-boundary matches, excluding declaration file and deprecated attributes
    grep -rn --include="*.rs" "\b${item_name}\b" "$SCAN_PATH" 2>/dev/null | \
        grep -v "^${decl_file}:" | \
        grep -v '#\[deprecated' | \
        awk -F: -v name="$item_name" -v decl="$declaration_loc" '{printf "%s\t%s:%s\t%s\n", name, $1, $2, decl}' \
        >> "$USAGE_REPORT" || true
done < "$DEPRECATED_ITEMS"

USAGE_COUNT=$(wc -l < "$USAGE_REPORT" | tr -d ' ')

# Step 3: Report results
echo "" >&2
echo "3. Results:" >&2

if [[ $USAGE_COUNT -eq 0 ]]; then
    echo "   No deprecated item usages found!" >&2
    echo "" >&2
    echo "=== Summary ===" >&2
    echo "STATUS: CLEAN - No deprecated usage in codebase" >&2

    if $JSON_OUTPUT; then
        echo "{\"deprecated_declarations\":$DEPRECATED_COUNT,\"deprecated_usages\":0,\"status\":\"clean\"}"
    fi
    exit 0
fi

echo "   Found $USAGE_COUNT usages of deprecated items:" >&2
echo "" >&2

# Group by deprecated item
current_item=""
while IFS=$'\t' read -r item_name usage_loc declaration_loc; do
    if [[ "$item_name" != "$current_item" ]]; then
        current_item="$item_name"
        echo "   $item_name (deprecated at $declaration_loc):" >&2
    fi
    echo "     - $usage_loc" >&2
done < "$USAGE_REPORT"

echo "" >&2
echo "=== Summary ===" >&2
echo "Deprecated declarations: $DEPRECATED_COUNT" >&2
echo "Deprecated usages found: $USAGE_COUNT" >&2

if $JSON_OUTPUT; then
    json_usages="["
    first=true
    while IFS=$'\t' read -r item_name usage_loc declaration_loc; do
        if $first; then first=false; else json_usages+=","; fi
        json_usages+="{\"item\":\"$item_name\",\"usage\":\"$usage_loc\",\"declaration\":\"$declaration_loc\"}"
    done < "$USAGE_REPORT"
    json_usages+="]"
    echo "{\"deprecated_declarations\":$DEPRECATED_COUNT,\"deprecated_usages\":$USAGE_COUNT,\"usages\":$json_usages,\"status\":\"$(if $STRICT; then echo "fail"; else echo "warn"; fi)\"}"
fi

if $STRICT; then
    echo "" >&2
    echo "STATUS: FAIL - Deprecated items are being used" >&2
    echo "Fix these usages before marking the task complete." >&2
    exit 1
else
    echo "" >&2
    echo "STATUS: WARN - Consider updating deprecated usages" >&2
    exit 0
fi
