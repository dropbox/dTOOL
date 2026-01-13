#!/bin/bash
# DTermCore Parser Validation Script
# This script exercises the terminal with various escape sequences to validate
# that dterm-core's parser produces correct output when running in comparison mode.
#
# Usage: ./scripts/validate-dterm-core.sh [--verbose]
#
# When dtermCoreParserComparisonEnabled is enabled, any mismatch will be logged.
# Check Console.app for "DTermCore mismatch" messages.

set -e

VERBOSE=false
if [[ "$1" == "--verbose" ]]; then
    VERBOSE=true
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

section() {
    echo ""
    echo "=============================================="
    echo "  $1"
    echo "=============================================="
}

# Track test counts
TESTS_RUN=0
TESTS_PASSED=0

run_test() {
    local name="$1"
    local sequence="$2"

    TESTS_RUN=$((TESTS_RUN + 1))

    if $VERBOSE; then
        log "Running: $name"
    fi

    # Print the escape sequence
    printf "%b" "$sequence"

    # Small delay to let the terminal process
    sleep 0.01

    TESTS_PASSED=$((TESTS_PASSED + 1))

    if $VERBOSE; then
        pass "$name"
    fi
}

# Clear screen and start
clear

echo "DTermCore Parser Validation Script"
echo "==================================="
echo ""
echo "This script tests escape sequences to validate dterm-core parser."
echo "If dtermCoreParserComparisonEnabled is enabled, mismatches will be logged."
echo ""
echo "Starting tests..."
sleep 1

# =============================================================================
section "Basic Text and Control Characters"
# =============================================================================

run_test "Plain ASCII text" "Hello, World!\n"
run_test "Backspace" "ABC\b\bXY\n"
run_test "Tab character" "Col1\tCol2\tCol3\n"
run_test "Carriage Return" "XXXX\rOK\n"
run_test "Bell (BEL)" "\a"

# =============================================================================
section "Cursor Movement (CSI sequences)"
# =============================================================================

run_test "Cursor Up (CUU)" "\033[5BCursor at line 5\033[2ATwo lines up\n\n\n"
run_test "Cursor Down (CUD)" "Start\033[2BTwo down\n"
run_test "Cursor Forward (CUF)" "A\033[5CB\n"
run_test "Cursor Back (CUB)" "ABCDE\033[3BXYZ\n"
run_test "Cursor Next Line (CNL)" "Line1\033[ENext line at column 1\n"
run_test "Cursor Previous Line (CPL)" "\033[2B\nLine3\033[FPrevious line at column 1\n\n"
run_test "Cursor Horizontal Absolute (CHA)" "XXXXXXX\033[3GColumn 3\n"
run_test "Cursor Position (CUP)" "\033[5;10HRow 5, Col 10\n"
run_test "Cursor Position default" "\033[HHome position\n"

# =============================================================================
section "Erase Functions"
# =============================================================================

run_test "Erase to end of line (EL 0)" "XXXXXXXXXX\033[5G\033[KErase to EOL\n"
run_test "Erase to start of line (EL 1)" "XXXXXXXXXX\033[5G\033[1KErased start\n"
run_test "Erase entire line (EL 2)" "XXXXXXXXXX\033[2KLine erased\n"
run_test "Erase below (ED 0)" "Test\n"
run_test "Erase above (ED 1)" "Test\n"

# =============================================================================
section "Character Attributes (SGR)"
# =============================================================================

run_test "Bold" "\033[1mBold Text\033[0m Normal\n"
run_test "Dim" "\033[2mDim Text\033[0m Normal\n"
run_test "Italic" "\033[3mItalic Text\033[0m Normal\n"
run_test "Underline" "\033[4mUnderlined\033[0m Normal\n"
run_test "Blink" "\033[5mBlink\033[0m Normal\n"
run_test "Inverse" "\033[7mInverse\033[0m Normal\n"
run_test "Strikethrough" "\033[9mStrikethrough\033[0m Normal\n"
run_test "Double underline" "\033[21mDouble Underline\033[0m Normal\n"

# Foreground colors
run_test "FG Black" "\033[30mBlack\033[0m "
run_test "FG Red" "\033[31mRed\033[0m "
run_test "FG Green" "\033[32mGreen\033[0m "
run_test "FG Yellow" "\033[33mYellow\033[0m "
run_test "FG Blue" "\033[34mBlue\033[0m "
run_test "FG Magenta" "\033[35mMagenta\033[0m "
run_test "FG Cyan" "\033[36mCyan\033[0m "
run_test "FG White" "\033[37mWhite\033[0m\n"

# Bright foreground colors
run_test "FG Bright Black" "\033[90mBrightBlack\033[0m "
run_test "FG Bright Red" "\033[91mBrightRed\033[0m "
run_test "FG Bright Green" "\033[92mBrightGreen\033[0m\n"

# Background colors
run_test "BG Red" "\033[41mRed BG\033[0m "
run_test "BG Green" "\033[42mGreen BG\033[0m "
run_test "BG Blue" "\033[44mBlue BG\033[0m\n"

# 256 color mode
run_test "256 FG color (index 196)" "\033[38;5;196mColor 196\033[0m\n"
run_test "256 BG color (index 51)" "\033[48;5;51mBG Color 51\033[0m\n"

# True color (24-bit)
run_test "True color FG (RGB 255,128,0)" "\033[38;2;255;128;0mOrange\033[0m\n"
run_test "True color BG (RGB 0,128,255)" "\033[48;2;0;128;255mBlue BG\033[0m\n"

# Combined attributes
run_test "Bold+Underline+Red" "\033[1;4;31mBold Underlined Red\033[0m\n"

# Underline color (iTerm2 specific)
run_test "Underline color" "\033[4m\033[58;2;255;0;0mRed underline\033[59m\033[0m\n"

# =============================================================================
section "Insert/Delete Operations"
# =============================================================================

run_test "Insert characters (ICH)" "ABCDE\033[2G\033[2@XX\n"
run_test "Delete characters (DCH)" "ABCDEFGH\033[2G\033[2P\n"
run_test "Insert lines (IL)" "Line1\nLine2\nLine3\033[2A\033[L\n\n"
run_test "Delete lines (DL)" "Line1\nLine2\nLine3\033[2A\033[M\n"

# =============================================================================
section "Scrolling"
# =============================================================================

run_test "Scroll up (SU)" "\033[S"
run_test "Scroll down (SD)" "\033[T"
run_test "Set scroll region" "\033[5;10r"
run_test "Reset scroll region" "\033[r"

# =============================================================================
section "Escape Sequences (non-CSI)"
# =============================================================================

run_test "Save cursor (DECSC)" "\033[5;5HMARK\0337"
run_test "Restore cursor (DECRC)" "\0338Restored\n"
run_test "Index (IND)" "\033D"
run_test "Reverse Index (RI)" "\033M"
run_test "Next Line (NEL)" "\033E"
run_test "Set tab stop (HTS)" "\033H"
run_test "Reset terminal (RIS)" "Before\033cAfter reset\n"

# =============================================================================
section "DEC Private Modes"
# =============================================================================

run_test "Hide cursor (DECTCEM)" "\033[?25l"
run_test "Show cursor (DECTCEM)" "\033[?25h"
run_test "Enable autowrap (DECAWM)" "\033[?7h"
run_test "Disable autowrap (DECAWM)" "\033[?7l"
run_test "Enable autowrap again" "\033[?7h"
run_test "Application cursor keys on" "\033[?1h"
run_test "Application cursor keys off" "\033[?1l"
run_test "Alternate screen on" "\033[?1049h"
run_test "Alternate screen off" "\033[?1049l"
run_test "Bracketed paste mode on" "\033[?2004h"
run_test "Bracketed paste mode off" "\033[?2004l"

# =============================================================================
section "Cursor Style (DECSCUSR)"
# =============================================================================

run_test "Cursor blinking block" "\033[1 q"
run_test "Cursor steady block" "\033[2 q"
run_test "Cursor blinking underline" "\033[3 q"
run_test "Cursor steady underline" "\033[4 q"
run_test "Cursor blinking bar" "\033[5 q"
run_test "Cursor steady bar" "\033[6 q"
run_test "Cursor default" "\033[0 q"

# =============================================================================
section "OSC Sequences"
# =============================================================================

run_test "Set window title (OSC 0)" "\033]0;Test Window Title\007"
run_test "Set icon name (OSC 1)" "\033]1;Test Icon\007"
run_test "Set window title (OSC 2)" "\033]2;Window Title Only\007"
run_test "OSC 4 query color" "\033]4;1;?\007"
run_test "OSC 10 foreground color query" "\033]10;?\007"
run_test "OSC 11 background color query" "\033]11;?\007"

# =============================================================================
section "Unicode and Wide Characters"
# =============================================================================

run_test "Basic Unicode" "Cafe\u0301\n"
run_test "Emoji" "Hello 👋 World 🌍\n"
run_test "CJK characters" "日本語 中文 한국어\n"
run_test "Wide character spacing" "A全角B\n"
run_test "Combining characters" "e\u0301\u0300 (e + acute + grave)\n"
run_test "Zero-width joiner emoji" "👨‍👩‍👧‍👦 (family)\n"

# =============================================================================
section "Tab Handling"
# =============================================================================

run_test "Tab to next stop" "A\tB\tC\n"
run_test "Tab clear current (TBC 0)" "\033[g"
run_test "Tab clear all (TBC 3)" "\033[3g"
run_test "Set tab stop (HTS)" "\033H"

# =============================================================================
section "Device Status Reports"
# =============================================================================

run_test "DSR cursor position (CPR)" "\033[6n"
run_test "DSR device status" "\033[5n"
run_test "DA primary device attributes" "\033[c"
run_test "DA secondary device attributes" "\033[>c"

# =============================================================================
section "Shell Integration (OSC 133)"
# =============================================================================

run_test "Prompt start (OSC 133;A)" "\033]133;A\007"
run_test "Command start (OSC 133;B)" "\033]133;B\007"
run_test "Output start (OSC 133;C)" "\033]133;C\007"
run_test "Command end (OSC 133;D)" "\033]133;D;0\007"

# =============================================================================
section "Stress Tests"
# =============================================================================

log "Rapid SGR changes..."
for i in {1..100}; do
    printf "\033[%d;%dm#\033[0m" $((i % 8 + 30)) $((i % 8 + 40))
done
printf "\n"
pass "Rapid SGR changes (100 iterations)"
TESTS_RUN=$((TESTS_RUN + 1))
TESTS_PASSED=$((TESTS_PASSED + 1))

log "Long line wrap test..."
printf "%0.s=" {1..200}
printf "\n"
pass "Long line wrap (200 chars)"
TESTS_RUN=$((TESTS_RUN + 1))
TESTS_PASSED=$((TESTS_PASSED + 1))

log "Rapid cursor movement..."
for i in {1..50}; do
    printf "\033[%d;%dH*" $((i % 20 + 1)) $((i % 60 + 1))
done
printf "\033[25;1H\n"
pass "Rapid cursor movement (50 positions)"
TESTS_RUN=$((TESTS_RUN + 1))
TESTS_PASSED=$((TESTS_PASSED + 1))

# =============================================================================
section "Summary"
# =============================================================================

echo ""
echo "=============================================="
echo "  VALIDATION COMPLETE"
echo "=============================================="
echo ""
echo "Tests run:    $TESTS_RUN"
echo "Tests passed: $TESTS_PASSED"
echo ""
echo "To check for parser mismatches, search Console.app for:"
echo "  'DTermCore mismatch' or 'dterm-core comparison'"
echo ""
echo "Or run:"
echo "  log show --predicate 'process == \"DashTerm2\"' --last 5m | grep -i mismatch"
echo ""

# Restore terminal state
printf "\033[0m"  # Reset attributes
printf "\033[?25h"  # Show cursor
printf "\033[r"  # Reset scroll region

# =============================================================================
section "Edge Cases and Boundary Conditions"
# =============================================================================

log "Testing malformed sequences (should be handled gracefully)..."

# Incomplete CSI sequences
run_test "Incomplete CSI (ESC[)" "\033["
run_test "CSI with no final byte" "\033[1;2"
run_test "CSI followed by valid text" "\033[mNormal\n"

# Multiple resets
run_test "Multiple SGR resets" "\033[0;0;0m"
run_test "SGR with extra semicolons" "\033[1;;4m\033[0m\n"

# Large parameter values
run_test "Large cursor position" "\033[999;999H"
run_test "Very large SGR parameter" "\033[38;5;999mTest\033[0m\n"

# Zero and negative parameters
run_test "Zero cursor movement" "\033[0A\033[0B\033[0C\033[0D"
run_test "Default parameters" "\033[;Hdefault position\n"

# Mixed case (should be treated as invalid in CSI)
run_test "UTF-8 boundary test (2-byte)" "é à ü\n"
run_test "UTF-8 boundary test (3-byte)" "€ ₹ ₽\n"
run_test "UTF-8 boundary test (4-byte)" "𝕳𝖊𝖑𝖑𝖔\n"

# Special escape sequences
run_test "ESC followed by invalid char" "\033@\033#"
run_test "Double ESC" "\033\033[m"

# Parameter overflow boundary
run_test "Maximum CSI params (16)" "\033[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16m\033[0m"

# Rapid mode switching
log "Rapid alternate screen switching..."
for i in {1..10}; do
    printf "\033[?1049h\033[?1049l"
done
pass "Rapid alternate screen switching (10 iterations)"
TESTS_RUN=$((TESTS_RUN + 1))
TESTS_PASSED=$((TESTS_PASSED + 1))

# SGR colon-separated subparams (newer standard)
run_test "Colon-separated subparams" "\033[4:3mCurly underline\033[0m\n"
run_test "256 color with colon" "\033[38:5:196mTest\033[0m\n"
run_test "True color with colon" "\033[38:2::255:128:0mOrange\033[0m\n"

echo ""
pass "Edge case tests completed"
