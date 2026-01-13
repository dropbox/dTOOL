#!/bin/bash
# Remove MIT/Apache license declarations from all Cargo.toml files
# This software is proprietary per LICENSE file
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

echo "Removing MIT/Apache license declarations..."

# Find all Cargo.toml files with MIT or Apache licenses
FILES=$(grep -rl "license.*=.*[\"']MIT\|license.*=.*[\"']Apache" Cargo.toml */Cargo.toml examples/apps/*/Cargo.toml crates/*/Cargo.toml test-utils/Cargo.toml benchmarks/*/Cargo.toml 2>/dev/null || true)

if [ -z "$FILES" ]; then
    echo "No license fields found."
    exit 0
fi

# Remove license lines from each file
for file in $FILES; do
    echo "Processing: $file"
    # Remove lines containing license = "MIT..." or license = 'MIT...'
    sed -i '' '/^license[[:space:]]*=[[:space:]]*["'\'']MIT\|^license[[:space:]]*=[[:space:]]*["'\'']Apache/d' "$file"
done

echo "Complete. Verifying..."
REMAINING=$(grep -r "license.*MIT\|license.*Apache" Cargo.toml */Cargo.toml examples/apps/*/Cargo.toml crates/*/Cargo.toml test-utils/Cargo.toml benchmarks/*/Cargo.toml 2>/dev/null || true)

if [ -z "$REMAINING" ]; then
    echo "✅ All MIT/Apache licenses removed"
else
    echo "⚠️  Some licenses may remain:"
    echo "$REMAINING"
fi
