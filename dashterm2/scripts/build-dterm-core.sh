#!/bin/bash
# build-dterm-core.sh
# Builds dterm-core from ~/dterm and copies to DashTerm2/DTermCore
#
# This script should be run:
# 1. Before building DashTerm2 in Xcode (as a build phase)
# 2. After making changes to dterm-core in ~/dterm
#
# IMPORTANT: dterm-core source lives in ~/dterm, NOT in DashTerm2.
# Do NOT create copies of dterm-core inside DashTerm2.

set -e

DTERM_REPO="${HOME}/dterm"
DASHTERM2_REPO="${HOME}/dashterm2"
DTERM_CORE_CRATE="${DTERM_REPO}/crates/dterm-core"
OUTPUT_DIR="${DASHTERM2_REPO}/DTermCore"
LIB_FILE="${OUTPUT_DIR}/lib/libdterm_core.a"
HEADER_FILE="${OUTPUT_DIR}/include/dterm.h"

# Skip build if library and header already exist
# This allows building DashTerm2 even when dterm-core has compilation errors
if [ -f "${LIB_FILE}" ] && [ -f "${HEADER_FILE}" ]; then
    SYMBOL_COUNT=$(nm "${LIB_FILE}" 2>/dev/null | grep -c "dterm_" || echo "0")
    if [ "${SYMBOL_COUNT}" -gt 100 ]; then
        echo "=== dterm-core: Using existing library (${SYMBOL_COUNT} FFI symbols) ==="
        echo "Library: ${LIB_FILE}"
        echo "Header:  ${HEADER_FILE}"
        echo "To force rebuild, delete ${LIB_FILE} and run again"
        exit 0
    fi
fi

echo "=== Building dterm-core from ${DTERM_CORE_CRATE} ==="

# Verify dterm repo exists
if [ ! -d "${DTERM_CORE_CRATE}" ]; then
    echo "ERROR: dterm-core not found at ${DTERM_CORE_CRATE}"
    echo "Please clone the dterm repo to ~/dterm"
    exit 1
fi

# Build dterm-core with FFI and GPU support
echo "Building dterm-core with --features ffi,gpu..."
cd "${DTERM_REPO}"
cargo build --release -p dterm-core --features ffi,gpu

# Verify the library was built
BUILT_LIB="${DTERM_REPO}/target/release/libdterm_core.a"
if [ ! -f "${BUILT_LIB}" ]; then
    echo "ERROR: Library not found at ${BUILT_LIB}"
    exit 1
fi

# Copy library to DashTerm2
echo "Copying libdterm_core.a to ${OUTPUT_DIR}/lib/"
mkdir -p "${OUTPUT_DIR}/lib"
cp "${BUILT_LIB}" "${OUTPUT_DIR}/lib/"

# Copy header to DashTerm2 only if changed (to avoid PCH invalidation)
echo "Checking dterm.h..."
mkdir -p "${OUTPUT_DIR}/include"
SOURCE_HEADER="${DTERM_CORE_CRATE}/include/dterm.h"
DEST_HEADER="${OUTPUT_DIR}/include/dterm.h"
if [ ! -f "${DEST_HEADER}" ] || ! cmp -s "${SOURCE_HEADER}" "${DEST_HEADER}"; then
    echo "Copying dterm.h to ${OUTPUT_DIR}/include/ (content changed)"
    cp "${SOURCE_HEADER}" "${DEST_HEADER}"
else
    echo "dterm.h unchanged, skipping copy to preserve PCH"
fi

# Verify FFI symbols are exported
SYMBOL_COUNT=$(nm "${OUTPUT_DIR}/lib/libdterm_core.a" 2>/dev/null | grep -c "dterm_" || echo "0")
echo "FFI symbols found: ${SYMBOL_COUNT}"
if [ "${SYMBOL_COUNT}" -lt 4 ]; then
    echo "WARNING: Expected at least 4 FFI symbols, found ${SYMBOL_COUNT}"
    echo "The library may not have been built with --features ffi"
fi

echo "=== dterm-core build complete ==="
echo "Library: ${OUTPUT_DIR}/lib/libdterm_core.a"
echo "Header:  ${OUTPUT_DIR}/include/dterm.h"
