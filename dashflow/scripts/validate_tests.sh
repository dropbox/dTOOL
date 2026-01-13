#!/bin/bash
# Test Validation Suite
# Ensures all tests compile and basic validation passes before commit

set -euo pipefail

echo "=================================================="
echo "Test Validation Suite"
echo "=================================================="
echo ""

# 1. Check all tests compile (including ignored)
echo "🔨 Step 1/4: Checking all tests compile (including ignored)..."
if ! cargo test --workspace --all-targets --no-run 2>&1 | tee /tmp/test_compile.log | grep -q "Finished"; then
    echo ""
    echo "❌ Compilation failed!"
    echo ""
    echo "Some tests have compilation errors. Check output above."
    echo "This includes tests marked #[ignore] - they must still compile."
    echo ""
    exit 1
fi
echo "✅ All tests compile (including ignored tests)"
echo ""

# 2. Run unit tests
echo "🧪 Step 2/4: Running unit tests..."
if ! cargo test --workspace --lib; then
    echo ""
    echo "❌ Unit tests failed!"
    echo ""
    exit 1
fi
echo "✅ Unit tests passed"
echo ""

# 3. Run clippy (production targets, strict)
echo "📎 Step 3/5: Running clippy (prod targets, strict)..."
if ! cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used 2>&1 | tee /tmp/clippy_prod.log; then
    echo ""
    echo "❌ Clippy failed (prod targets)!"
    echo ""
    echo "Note: unwrap()/expect() are forbidden in production targets."
    echo "If truly intentional, add #[allow(clippy::unwrap_used|expect_used)] with a SAFETY justification comment."
    echo ""
    exit 1
fi
echo "✅ Clippy passed (prod targets, zero warnings)"
echo ""

# 4. Run clippy (all targets, advisory warnings)
echo "📎 Step 4/5: Running clippy (all targets, advisory warnings)..."
if ! cargo clippy --workspace --all-targets 2>&1 | tee /tmp/clippy_all.log; then
    echo ""
    echo "❌ Clippy failed (all targets)!"
    echo ""
    exit 1
fi
echo "✅ Clippy passed (all targets)"
echo ""

# 5. Check formatting
echo "📝 Step 5/5: Checking formatting..."
if ! cargo fmt --all -- --check; then
    echo ""
    echo "❌ Formatting check failed!"
    echo ""
    echo "Run: cargo fmt --all"
    echo ""
    exit 1
fi
echo "✅ Formatting check passed"
echo ""

echo "=================================================="
echo "✅ ALL VALIDATION CHECKS PASSED"
echo "=================================================="
echo ""
echo "Safe to commit. All tests compile (including ignored),"
echo "unit tests pass, clippy is clean for prod targets, and formatting is correct."
echo ""
