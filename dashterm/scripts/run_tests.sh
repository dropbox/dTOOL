#!/bin/bash
#
# Run all tests for DashTerm
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/dashterm-core"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}Running DashTerm tests...${NC}"

# Run Rust tests
echo -e "${YELLOW}Running Rust tests...${NC}"
cd "$RUST_DIR"
cargo test --all

# Run clippy
echo -e "${YELLOW}Running clippy...${NC}"
cargo clippy --all -- -D warnings

# Check formatting
echo -e "${YELLOW}Checking Rust formatting...${NC}"
cargo fmt --all -- --check

# Run Swift tests (if Xcode project exists)
if [ -f "$PROJECT_ROOT/DashTerm.xcodeproj/project.pbxproj" ]; then
    echo -e "${YELLOW}Running Swift tests...${NC}"
    cd "$PROJECT_ROOT"
    xcodebuild test \
        -project DashTerm.xcodeproj \
        -scheme DashTerm \
        -destination 'platform=macOS' \
        -quiet
fi

echo -e "${GREEN}All tests passed!${NC}"
