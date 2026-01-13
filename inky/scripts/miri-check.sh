#!/bin/bash
# Run Miri for undefined behavior detection
#
# Miri limitations on macOS:
# - Cannot call macOS signal() FFI, skip terminal::signals tests
# - Cannot call clock_gettime in isolation, skip hooks::interval tests
# - serial_test dependency has expected leaks, use -Zmiri-ignore-leaks
#
# Usage: ./scripts/miri-check.sh

set -e

echo "Running Miri undefined behavior checks..."
echo "Note: Skipping signal and interval tests due to Miri FFI limitations"

MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-ignore-leaks" \
    cargo +nightly miri test --lib -- \
    --skip terminal::signals \
    --skip hooks::interval

echo ""
echo "Miri check passed! No undefined behavior detected."
