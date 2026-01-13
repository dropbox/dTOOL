#!/bin/bash
# Remove unused tokio-test dependencies from Cargo.toml files
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

cd "$(dirname "$0")/.."

# Get list of crates with tokio-test in dev-dependencies
crates=$(find crates -name "Cargo.toml" -exec grep -l 'tokio-test = "0.4"' {} \;)

for toml in $crates; do
    crate_dir=$(dirname "$toml")
    crate_name=$(basename "$crate_dir")

    # Check if tokio_test is actually used (not in comments)
    if grep -r "tokio_test::" "$crate_dir/src" "$crate_dir/tests" 2>/dev/null | grep -qv "^[[:space:]]*//"; then
        echo "  Keeping tokio-test in $crate_name (actually used)"
        continue
    fi

    if grep -r "use tokio_test" "$crate_dir/src" "$crate_dir/tests" 2>/dev/null | grep -qv "^[[:space:]]*//"; then
        echo "  Keeping tokio-test in $crate_name (actually used)"
        continue
    fi

    echo "Removing tokio-test from $crate_name"
    sed -i '' '/^tokio-test = /d' "$toml"
done

echo "Done!"
