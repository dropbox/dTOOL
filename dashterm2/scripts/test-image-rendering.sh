#!/bin/bash
# Test script for DTermMetalView image rendering (Sixel and Kitty)
# Usage: Run this script in DashTerm2 with GPU renderer enabled:
#   defaults write com.dashterm.dashterm2 dtermCoreRendererEnabled -bool YES
#   ./scripts/test-image-rendering.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
TEST_IMAGE="$REPO_DIR/images/aiterm.png"

echo "=== DTermMetalView Image Rendering Test ==="
echo ""
echo "Prerequisites:"
echo "  - GPU renderer enabled: defaults write com.dashterm.dashterm2 dtermCoreRendererEnabled -bool YES"
echo "  - FPS overlay enabled: defaults write com.dashterm.dashterm2 DtermCoreFPSOverlayEnabled -bool YES"
echo ""

# Check for required tools
if ! command -v img2sixel &> /dev/null; then
    echo "ERROR: img2sixel not found. Install with: brew install libsixel"
    exit 1
fi

if [ ! -f "$TEST_IMAGE" ]; then
    echo "ERROR: Test image not found at $TEST_IMAGE"
    exit 1
fi

echo "Test image: $TEST_IMAGE"
echo ""

# Test 1: Sixel image
echo "=== Test 1: Sixel Image ==="
echo "Sending Sixel image..."
img2sixel -w 200 "$TEST_IMAGE"
echo ""
echo "If the image rendered correctly, you should see the AI terminal icon above."
echo ""
read -p "Did the Sixel image render? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Sixel rendering: PASS"
else
    echo "Sixel rendering: FAIL"
fi
echo ""

# Test 2: Kitty image protocol
echo "=== Test 2: Kitty Image Protocol ==="
echo "Note: Kitty image protocol requires kitty terminal or compatible terminal."
echo "DashTerm2 with dterm-core should support this."
echo ""

# Send a simple Kitty image using the protocol
# Format: APC G <params> ; <payload> ST
# APC = ESC _
# ST = ESC \
# For simplicity, we'll use a 2x2 red square (8 bytes RGBA = 16 hex chars base64)

# Create a simple 2x2 red square (RGBA: 255,0,0,255 for each pixel)
# Base64 of 8 bytes: 0xFF 0x00 0x00 0xFF 0xFF 0x00 0x00 0xFF 0xFF 0x00 0x00 0xFF 0xFF 0x00 0x00 0xFF
# = /wAA//8AAP//AAD//wAA/w==

# Try to send a small test image via Kitty protocol
# a=T - transmit and display
# f=32 - RGBA
# s=2 - width 2
# v=2 - height 2
echo -ne '\e_Ga=T,f=32,s=2,v=2;/wAA//8AAP//AAD//wAA/w==\e\\'
echo ""
echo "If Kitty protocol is working, you should see a small red square above."
echo ""
read -p "Did the Kitty image render? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Kitty rendering: PASS"
else
    echo "Kitty rendering: FAIL (or not yet implemented)"
fi
echo ""

# Test 3: Verify FPS overlay is showing
echo "=== Test 3: FPS Overlay ==="
echo "Check if the FPS overlay is visible in the terminal window."
read -p "Is the FPS overlay showing? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "FPS overlay: PASS"
else
    echo "FPS overlay: FAIL"
fi
echo ""

echo "=== Test Summary ==="
echo "Run DashTerm2 with GPU renderer enabled and check:"
echo "1. Sixel images render via processSixelImages() and Metal pipeline"
echo "2. Kitty images render via processKittyImages() and renderKittyImages()"
echo "3. FPS overlay shows renderer performance"
echo ""
echo "To run DashTerm2:"
echo "  open $REPO_DIR/build/Development/DashTerm2.app"
echo ""
echo "Or from Xcode: Run the DashTerm2 scheme"
