#!/bin/bash
# Remove unused test-only dependencies from Cargo.toml files
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

cd "$(dirname "$0")/.."

# Function to check if a dependency is used
check_dep_usage() {
    local crate_dir="$1"
    local dep_name="$2"
    local pattern="$3"

    # Check src and tests directories for usage (excluding comments)
    if grep -r "$pattern" "$crate_dir/src" "$crate_dir/tests" 2>/dev/null | grep -qv "^[[:space:]]*//"; then
        return 0  # Used
    fi
    return 1  # Not used
}

# Process insta dependencies
echo "=== Checking insta dependencies ==="
for toml in $(find crates -name "Cargo.toml" -exec grep -l '^[[:space:]]*insta = ' {} \;); do
    crate_dir=$(dirname "$toml")
    crate_name=$(basename "$crate_dir")

    if check_dep_usage "$crate_dir" "insta" "insta::"; then
        echo "  Keeping insta in $crate_name (used)"
    else
        echo "Removing insta from $crate_name"
        sed -i '' '/^[[:space:]]*insta = /d' "$toml"
    fi
done

# Process mockall dependencies
echo -e "\n=== Checking mockall dependencies ==="
for toml in $(find crates -name "Cargo.toml" -exec grep -l '^[[:space:]]*mockall = ' {} \;); do
    crate_dir=$(dirname "$toml")
    crate_name=$(basename "$crate_dir")

    if check_dep_usage "$crate_dir" "mockall" "mockall::"; then
        echo "  Keeping mockall in $crate_name (used)"
    else
        echo "Removing mockall from $crate_name"
        sed -i '' '/^[[:space:]]*mockall = /d' "$toml"
    fi
done

echo -e "\nDone!"
