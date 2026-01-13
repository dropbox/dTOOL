#!/bin/bash
set -euo pipefail
# Utility script to manually update README stats
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
# Usage: ./scripts/update-readme-stats.sh

echo "📝 Updating README.md statistics..."

# Get current date
current_date=$(date +%Y-%m-%d)

# Get latest commit number
latest_commit=$(git log --oneline | head -1 | grep -o "# [0-9]*:" | grep -o "[0-9]*")
if [ -z "$latest_commit" ]; then
    latest_commit=$(git rev-list --count HEAD)
fi

# Count tests
echo "Counting tests (this may take 10-20 seconds)..."
test_count=$(cargo test --lib --all 2>&1 | grep -o "running [0-9]* tests" | awk '{sum+=$2} END {print sum}')

# Count crates
crate_count=$(find crates -name "Cargo.toml" | wc -l | tr -d ' ')

# Format test count with commas
formatted_test_count=$(printf "%'d" $test_count)

echo ""
echo "Current stats:"
echo "  Date: $current_date"
echo "  Commit: #$latest_commit"
echo "  Tests: $formatted_test_count"
echo "  Crates: $crate_count"
echo ""

# Update README
sed -i '' "s/\*\*Last Updated:\*\* .*/\*\*Last Updated:\*\* $current_date (Commit #$latest_commit)/" README.md
sed -i '' "s/[0-9,]* tests passing/$formatted_test_count tests passing/g" README.md
sed -i '' "s/[0-9]* crates/$crate_count crates/g" README.md

echo "✓ README.md updated successfully"
echo ""
echo "Review changes with: git diff README.md"
