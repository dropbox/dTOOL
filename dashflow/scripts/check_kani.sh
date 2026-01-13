#!/bin/bash
# Verify that the Kani toolchain is installed and runnable.
#
# Environment:
#   SKIP_KANI=true   Skip checks (exit 0)

set -euo pipefail

if [ "${SKIP_KANI:-false}" = "true" ]; then
    echo "SKIP_KANI=true; skipping Kani checks."
    exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found on PATH."
    exit 1
fi

if ! command -v kani >/dev/null 2>&1; then
    echo "Error: kani not found on PATH."
    echo ""
    echo "Install:"
    echo "  cargo install --locked kani-verifier"
    echo "  kani setup"
    exit 1
fi

if ! cargo kani --version >/dev/null 2>&1; then
    echo "Error: cargo-kani not available."
    echo ""
    echo "Install:"
    echo "  cargo install --locked kani-verifier"
    echo "  kani setup"
    exit 1
fi

echo "=== Kani Toolchain OK ==="
echo -n "kani: "
kani --version | head -1
echo -n "cargo kani: "
cargo kani --version | head -1

