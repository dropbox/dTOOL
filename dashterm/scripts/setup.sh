#!/bin/bash
#
# Setup script for DashTerm development environment
#
# This script installs dependencies and prepares the build environment.
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}Setting up DashTerm development environment...${NC}"

# Check for Rust
if ! command -v rustc &> /dev/null; then
    echo -e "${YELLOW}Rust not found. Installing via rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo -e "${GREEN}Rust version: $(rustc --version)${NC}"

# Install required Rust targets for universal binary
echo -e "${YELLOW}Installing Rust targets...${NC}"
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Install cargo tools
echo -e "${YELLOW}Installing cargo tools...${NC}"
cargo install cbindgen 2>/dev/null || true

# Check for Xcode
if ! command -v xcodebuild &> /dev/null; then
    echo -e "${RED}Xcode not found. Please install Xcode from the App Store.${NC}"
    exit 1
fi

echo -e "${GREEN}Xcode version: $(xcodebuild -version | head -1)${NC}"

# Build Rust library
echo -e "${YELLOW}Building Rust library...${NC}"
"$SCRIPT_DIR/build_rust.sh" debug

# Create Xcode project if it doesn't exist
if [ ! -f "$PROJECT_ROOT/DashTerm.xcodeproj/project.pbxproj" ]; then
    echo -e "${YELLOW}Note: Xcode project file not found.${NC}"
    echo -e "${YELLOW}Please create a new Xcode project with the following settings:${NC}"
    echo "  - Product Name: DashTerm"
    echo "  - Team: Your team"
    echo "  - Organization Identifier: com.dashterm"
    echo "  - Interface: SwiftUI"
    echo "  - Language: Swift"
    echo "  - Minimum Deployment: macOS 14.0"
    echo ""
    echo "Then configure:"
    echo "  1. Add DashTerm-Bridging-Header.h to Build Settings > Swift Compiler > Objective-C Bridging Header"
    echo "  2. Add \$(PROJECT_DIR)/DashTerm/Bridge to Header Search Paths"
    echo "  3. Add \$(PROJECT_DIR)/DashTerm/Bridge to Library Search Paths"
    echo "  4. Add libdashterm_ffi.a to Link Binary With Libraries"
    echo "  5. Add libresolv.tbd to Link Binary With Libraries (for PTY)"
fi

echo -e "${GREEN}Setup complete!${NC}"
