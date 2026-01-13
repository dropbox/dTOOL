#!/bin/bash
# Generate test coverage report using cargo-llvm-cov
# Requirements: cargo install cargo-llvm-cov
#
# Usage:
#   ./scripts/coverage.sh          # Print summary to terminal
#   ./scripts/coverage.sh --html   # Generate HTML report
#   ./scripts/coverage.sh --json   # Generate JSON report

set -e

if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "cargo-llvm-cov not found. Install with: cargo install cargo-llvm-cov"
    exit 1
fi

case "${1:-}" in
    --html)
        echo "Generating HTML coverage report..."
        cargo llvm-cov --html
        echo "Report generated at: target/llvm-cov/html/index.html"
        if command -v open &> /dev/null; then
            open target/llvm-cov/html/index.html
        fi
        ;;
    --json)
        echo "Generating JSON coverage report..."
        cargo llvm-cov --json --output-path coverage.json
        echo "Report generated at: coverage.json"
        ;;
    --codecov)
        echo "Generating Codecov report..."
        cargo llvm-cov --codecov --output-path codecov.json
        echo "Report generated at: codecov.json"
        ;;
    *)
        # Default: print summary
        cargo llvm-cov
        ;;
esac
