#!/bin/bash
# vttest.sh - VT compatibility testing script for dterm
#
# This script helps run vttest (VT100/VT220/VT320/etc. conformance tests)
# against dterm to validate terminal emulation correctness.
#
# Usage:
#   ./scripts/vttest.sh [options]
#
# Options:
#   --install       Install vttest if not present (macOS/Linux)
#   --run           Run vttest interactively
#   --check         Check if vttest is available
#   --help          Show this help message
#
# Requirements:
#   - vttest binary (installable via package manager or from source)
#   - A terminal emulator to test (this script provides setup instructions)
#
# Reference:
#   - vttest: https://invisible-island.net/vttest/
#   - ECMA-48: https://www.ecma-international.org/publications-and-standards/standards/ecma-48/
#   - VT100 User Guide: https://vt100.net/docs/vt100-ug/

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
}

print_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_vttest() {
    if command -v vttest &> /dev/null; then
        local version
        version=$(vttest -V 2>&1 | head -1 || echo "unknown")
        print_success "vttest found: $version"
        return 0
    else
        print_warning "vttest not found in PATH"
        return 1
    fi
}

install_vttest() {
    print_header "Installing vttest"

    local os_type
    os_type=$(uname -s)

    case "$os_type" in
        Darwin)
            if command -v brew &> /dev/null; then
                echo "Installing via Homebrew..."
                brew install vttest
            else
                print_error "Homebrew not found. Please install Homebrew first:"
                echo "  /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
                return 1
            fi
            ;;
        Linux)
            if command -v apt-get &> /dev/null; then
                echo "Installing via apt..."
                sudo apt-get update && sudo apt-get install -y vttest
            elif command -v dnf &> /dev/null; then
                echo "Installing via dnf..."
                sudo dnf install -y vttest
            elif command -v pacman &> /dev/null; then
                echo "Installing via pacman..."
                sudo pacman -S --noconfirm vttest
            else
                print_warning "No supported package manager found."
                echo "Please install vttest from source:"
                echo "  wget https://invisible-island.net/archives/vttest/vttest.tar.gz"
                echo "  tar xzf vttest.tar.gz && cd vttest-* && ./configure && make && sudo make install"
                return 1
            fi
            ;;
        *)
            print_error "Unsupported OS: $os_type"
            echo "Please install vttest manually from:"
            echo "  https://invisible-island.net/vttest/"
            return 1
            ;;
    esac

    # Verify installation
    if check_vttest; then
        print_success "vttest installed successfully"
    else
        print_error "vttest installation failed"
        return 1
    fi
}

run_vttest() {
    print_header "Running vttest"

    if ! check_vttest; then
        print_error "vttest not found. Run with --install first."
        return 1
    fi

    echo ""
    echo "vttest will now run interactively."
    echo ""
    echo "Test Categories:"
    echo "  1. Test of cursor movements"
    echo "  2. Test of screen features"
    echo "  3. Test of character sets"
    echo "  4. Test of double-sized characters"
    echo "  5. Test of keyboard"
    echo "  6. Test of terminal reports"
    echo "  7. Test of VT52 mode"
    echo "  8. Test of VT102 features"
    echo "  9. Test of known bugs"
    echo " 10. Test of reset and self-test"
    echo " 11. Test non-VT100/VT220/VT420 features"
    echo ""
    echo "Recommended test order for dterm verification:"
    echo "  1 -> 2 -> 3 -> 6 -> 8 -> 11"
    echo ""
    echo "Press Enter to start vttest, or Ctrl+C to cancel..."
    read -r

    vttest
}

show_conformance_guide() {
    print_header "dterm VT Conformance Testing Guide"

    cat << 'EOF'

OVERVIEW
--------
vttest is the standard tool for testing VT100/VT220/VT320/VT420 conformance.
Running it against dterm helps identify missing or incorrect escape sequence
implementations.

TESTING METHODOLOGY
-------------------

1. CURSOR MOVEMENT TESTS (Menu 1)
   - Tests: CUU, CUD, CUF, CUB, CUP, HVP
   - Expected: All cursor movement patterns should work correctly
   - Watch for: Off-by-one errors, boundary handling

2. SCREEN TESTS (Menu 2)
   - Tests: ED, EL, DCH, ICH, IL, DL
   - Expected: Erase and insert/delete operations should work
   - Watch for: Scroll region interaction, attribute preservation

3. CHARACTER SET TESTS (Menu 3)
   - Tests: SCS (G0-G3), DEC Special Graphics
   - Expected: Line drawing characters, national character sets
   - Watch for: Correct character mapping

4. DOUBLE-SIZED CHARACTERS (Menu 4)
   - Tests: DECDHL, DECDWL, DECSWL
   - Expected: Double-height/width line attributes
   - Note: Many modern terminals skip this

5. KEYBOARD TESTS (Menu 5)
   - Tests: Application/Normal cursor keys, keypad modes
   - Expected: Correct key sequences generated
   - Watch for: Mode switching behavior

6. TERMINAL REPORTS (Menu 6)
   - Tests: DA, DSR, DECID
   - Expected: Correct device attribute responses
   - Watch for: Response format, timing

7. VT102 FEATURES (Menu 8)
   - Tests: DECSC, DECRC, DECSTBM, and more
   - Expected: Save/restore cursor, scroll regions
   - Watch for: State management

8. NON-VT FEATURES (Menu 11)
   - Tests: ISO 6429 (ECMA-48), XTerm extensions
   - Expected: 256-color, true color, titles
   - Watch for: Modern terminal extensions

RECORDING RESULTS
-----------------
For each test category, note:
- Tests passed (displayed correctly)
- Tests failed (incorrect display)
- Tests skipped (not implemented)

Create a conformance matrix in docs/CONFORMANCE.md

KNOWN LIMITATIONS
-----------------
Some vttest tests assume specific terminal behavior:
- VT52 mode (historical, often skipped)
- Printer support (not relevant for modern use)
- DEC private modes (implementation varies)

TARGET CONFORMANCE LEVELS
-------------------------
dterm aims for:
- VT220: Full conformance
- VT420: Substantial conformance
- XTerm: Common extension support
- ECMA-48: Full conformance

EOF
}

show_help() {
    cat << EOF
vttest.sh - VT compatibility testing for dterm

Usage: $0 [options]

Options:
    --install       Install vttest if not present
    --run           Run vttest interactively
    --check         Check if vttest is available
    --guide         Show conformance testing guide
    --help          Show this help message

Examples:
    $0 --check              Check if vttest is installed
    $0 --install            Install vttest
    $0 --run                Run vttest interactively
    $0 --guide              Show testing methodology

For more information:
    https://invisible-island.net/vttest/
    https://vt100.net/

EOF
}

# Main
case "${1:-}" in
    --install)
        install_vttest
        ;;
    --run)
        run_vttest
        ;;
    --check)
        if check_vttest; then
            exit 0
        else
            echo ""
            echo "To install vttest, run: $0 --install"
            exit 1
        fi
        ;;
    --guide)
        show_conformance_guide
        ;;
    --help|-h|"")
        show_help
        ;;
    *)
        print_error "Unknown option: $1"
        show_help
        exit 1
        ;;
esac
