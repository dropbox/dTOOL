#!/bin/bash
#
# Build script for DashTerm Rust core library
#
# This script builds the Rust crates and generates the static library
# and C header for Swift integration.
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/dashterm-core"
OUTPUT_DIR="$PROJECT_ROOT/DashTerm/Bridge"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building DashTerm Rust core...${NC}"

# Determine build mode
BUILD_MODE="${1:-release}"

if [ "$BUILD_MODE" == "release" ]; then
    CARGO_FLAGS="--release"
    TARGET_DIR="release"
else
    CARGO_FLAGS=""
    TARGET_DIR="debug"
fi

# Build for current architecture
cd "$RUST_DIR"

echo -e "${YELLOW}Building for $(uname -m)...${NC}"

# Build the FFI crate (this also builds dependencies)
cargo build $CARGO_FLAGS -p dashterm-ffi

# Copy the static library
LIB_PATH="$RUST_DIR/target/$TARGET_DIR/libdashterm_ffi.a"
if [ -f "$LIB_PATH" ]; then
    cp "$LIB_PATH" "$OUTPUT_DIR/"
    echo -e "${GREEN}Copied libdashterm_ffi.a to $OUTPUT_DIR${NC}"
else
    echo -e "${RED}Error: Static library not found at $LIB_PATH${NC}"
    exit 1
fi

# Copy the generated header (if cbindgen ran)
HEADER_PATH="$RUST_DIR/dashterm-ffi/include/dashterm.h"
if [ -f "$HEADER_PATH" ]; then
    cp "$HEADER_PATH" "$OUTPUT_DIR/"
    echo -e "${GREEN}Copied dashterm.h to $OUTPUT_DIR${NC}"
fi

# Build universal binary for both arm64 and x86_64 (for distribution)
if [ "$BUILD_MODE" == "release" ] && [ "$2" == "--universal" ]; then
    echo -e "${YELLOW}Building universal binary...${NC}"

    # Build for x86_64
    cargo build --release -p dashterm-ffi --target x86_64-apple-darwin

    # Build for arm64
    cargo build --release -p dashterm-ffi --target aarch64-apple-darwin

    # Create universal binary
    lipo -create \
        "$RUST_DIR/target/x86_64-apple-darwin/release/libdashterm_ffi.a" \
        "$RUST_DIR/target/aarch64-apple-darwin/release/libdashterm_ffi.a" \
        -output "$OUTPUT_DIR/libdashterm_ffi_universal.a"

    echo -e "${GREEN}Created universal binary at $OUTPUT_DIR/libdashterm_ffi_universal.a${NC}"
fi

echo -e "${GREEN}Rust build complete!${NC}"
