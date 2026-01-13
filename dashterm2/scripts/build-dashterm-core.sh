#!/bin/bash
# Build dashterm-core Rust library for DashTerm2
#
# This script builds the Rust library as a static library for linking into
# the DashTerm2 Xcode project.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/dashterm-core"

echo "Building dashterm-core..."
cd "$RUST_DIR"

# Build for release
cargo build --release

# Generate header if cbindgen is available
if command -v cbindgen &> /dev/null; then
    echo "Generating C header..."
    mkdir -p include
    cbindgen -c cbindgen.toml -o include/dashterm_core.h 2>/dev/null || true
fi

# Create universal library if both architectures are needed
# For now, just use the native architecture
ARCH=$(uname -m)
echo "Built for architecture: $ARCH"
echo "Library: $RUST_DIR/target/release/libdashterm_core.a"
ls -la "$RUST_DIR/target/release/libdashterm_core.a"

echo "Done!"
