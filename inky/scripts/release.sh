#!/bin/bash
set -e

VERSION=$1
if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/release.sh <version>"
    echo "Example: ./scripts/release.sh 0.1.0"
    exit 1
fi

echo "=== PRE-RELEASE CHECKLIST for v$VERSION ==="

echo ""
echo "1. Running tests..."
cargo test --all-features

echo ""
echo "2. Running clippy..."
cargo clippy --all-features -- -D warnings

echo ""
echo "3. Checking formatting..."
cargo fmt -- --check

echo ""
echo "4. Building docs..."
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features 2>&1 || {
    echo "  Warning: Documentation has warnings"
    cargo doc --no-deps --all-features
}

echo ""
echo "5. Building benchmarks..."
cargo bench --no-run

echo ""
echo "6. Checking MSRV (1.70)..."
if command -v rustup &> /dev/null && rustup show | grep -q "1.70"; then
    cargo +1.70 check --all-features
else
    echo "  Skipping: Rust 1.70 not installed (checked in CI)"
fi

echo ""
echo "7. Security audit..."
cargo audit || {
    echo "  Note: Some warnings found (see above)"
}

echo ""
echo "8. Building examples..."
cargo build --examples

echo ""
echo "9. Dry-run publish..."
cargo publish --dry-run

echo ""
echo "10. Current version in Cargo.toml:"
current_version=$(awk -F'"' '/^version = /{print $2; exit}' Cargo.toml)
echo "  $current_version"
if [ "$current_version" != "$VERSION" ]; then
    echo "  Warning: Cargo.toml version does not match v$VERSION"
fi

echo ""
echo "11. Checking CHANGELOG.md for v$VERSION..."
if command -v rg &> /dev/null; then
    if ! rg -n "## \\[$VERSION\\]" CHANGELOG.md &> /dev/null; then
        echo "  Warning: No CHANGELOG entry for v$VERSION"
    else
        rg -n "## \\[$VERSION\\]" CHANGELOG.md
    fi
else
    if ! grep -q "## \\[$VERSION\\]" CHANGELOG.md; then
        echo "  Warning: No CHANGELOG entry for v$VERSION"
    else
        grep -n "## \\[$VERSION\\]" CHANGELOG.md
    fi
fi

echo ""
echo "12. Checking README test badge count..."
badge_count=""
if command -v rg &> /dev/null; then
    badge_raw=$(rg -o "tests-[0-9]+%20passing" README.md || true)
    if [ -n "$badge_raw" ]; then
        badge_count=$(printf "%s" "$badge_raw" | rg -o "[0-9]+" | head -n 1)
    fi
else
    badge_raw=$(grep -o "tests-[0-9]*%20passing" README.md || true)
    if [ -n "$badge_raw" ]; then
        # Extract only the number between "tests-" and "%20"
        badge_count=$(printf "%s" "$badge_raw" | sed -E 's/tests-([0-9]+)%20passing/\1/' | head -n 1)
    fi
fi

if [ -z "$badge_count" ]; then
    echo "  Warning: Unable to read test count badge from README.md"
else
    if command -v rg &> /dev/null; then
        test_count=$(cargo test --all-features -- --list | rg -c ": test$" || true)
    else
        test_count=$(cargo test --all-features -- --list | grep -c ": test$" || true)
    fi
    if [ -z "$test_count" ] || [ "$test_count" -eq 0 ]; then
        echo "  Warning: Unable to compute test count (got ${test_count:-empty})"
    elif [ "$badge_count" -ne "$test_count" ]; then
        echo "  Warning: README badge shows $badge_count tests, cargo lists $test_count"
    else
        echo "  README test badge matches ($badge_count)"
    fi
fi

echo ""
echo "=== PRE-RELEASE CHECKS COMPLETE ==="
echo ""
echo "To release v$VERSION:"
echo "  1. Update version in Cargo.toml to \"$VERSION\""
echo "  2. Update CHANGELOG.md with release notes"
echo "  3. Commit: git add -A && git commit -m 'Release v$VERSION'"
echo "  4. Tag: git tag v$VERSION"
echo "  5. Push: git push && git push --tags"
echo "  6. Publish: cargo publish"
echo ""
echo "Or use the release workflow by pushing a tag:"
echo "  git tag v$VERSION && git push origin v$VERSION"
