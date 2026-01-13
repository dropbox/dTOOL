#!/bin/bash
# vttest-validate.sh
# Automated vttest validation for DashTerm2 with dterm-core parser
#
# This script creates vttest command files and provides a framework for
# automated testing. Note that vttest is inherently visual, so some tests
# still require manual verification.
#
# Usage:
#   ./scripts/vttest-validate.sh [--create-commands] [--run-basic] [--report]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VTTEST_DIR="$PROJECT_ROOT/tests/vttest"
RESULTS_DIR="$VTTEST_DIR/results"

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

# Create directories
setup_dirs() {
    mkdir -p "$VTTEST_DIR"
    mkdir -p "$RESULTS_DIR"
}

# Check prerequisites
check_prereqs() {
    print_header "Checking Prerequisites"

    if ! command -v vttest &> /dev/null; then
        print_error "vttest not found. Install with: brew install vttest"
        exit 1
    fi

    local version
    version=$(vttest -V 2>&1 | head -1 || echo "unknown")
    print_success "vttest found: $version"

    # Check DashTerm2 is built
    if [ ! -d "$HOME/Library/Developer/Xcode/DerivedData/DashTerm2"*/Build/Products/Development/DashTerm2.app ]; then
        print_warning "DashTerm2 may not be built. Run 'xcodebuild' first."
    fi
}

# Create vttest command files for each test category
create_command_files() {
    print_header "Creating vttest Command Files"

    # Menu 1: Cursor Movement Tests
    cat > "$VTTEST_DIR/cmd_cursor.txt" << 'EOF'
1
*
0
0
EOF
    print_success "Created cursor movement commands: cmd_cursor.txt"

    # Menu 2: Screen Features Tests
    cat > "$VTTEST_DIR/cmd_screen.txt" << 'EOF'
2
*
0
0
EOF
    print_success "Created screen features commands: cmd_screen.txt"

    # Menu 3: Character Set Tests
    cat > "$VTTEST_DIR/cmd_charset.txt" << 'EOF'
3
*
0
0
EOF
    print_success "Created character set commands: cmd_charset.txt"

    # Menu 6: Terminal Reports Tests
    cat > "$VTTEST_DIR/cmd_reports.txt" << 'EOF'
6
*
0
0
EOF
    print_success "Created terminal reports commands: cmd_reports.txt"

    # Menu 8: VT102 Features Tests
    cat > "$VTTEST_DIR/cmd_vt102.txt" << 'EOF'
8
*
0
0
EOF
    print_success "Created VT102 features commands: cmd_vt102.txt"

    # Menu 11: Non-VT100 Features Tests
    cat > "$VTTEST_DIR/cmd_nonvt.txt" << 'EOF'
11
*
0
0
EOF
    print_success "Created non-VT100 features commands: cmd_nonvt.txt"

    # Full test suite (sequential run through all menus)
    cat > "$VTTEST_DIR/cmd_full.txt" << 'EOF'
1
*
0
2
*
0
3
*
0
6
*
0
8
*
0
11
*
0
0
EOF
    print_success "Created full test suite commands: cmd_full.txt"

    echo ""
    echo "Command files created in: $VTTEST_DIR"
}

# Run basic automated tests
run_basic_tests() {
    print_header "Running Basic vttest Validation"

    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    local log_file="$RESULTS_DIR/vttest_${timestamp}.log"

    echo "Running vttest with logging to: $log_file"
    echo ""

    # Run vttest with full test suite
    # Note: This is still interactive for visual tests, but logs results
    echo "Starting vttest..."
    echo "Use the command file for automated menu selection:"
    echo "  vttest -l $log_file < $VTTEST_DIR/cmd_full.txt"
    echo ""
    echo "For manual testing, run:"
    echo "  vttest -l $log_file"
    echo ""

    # Run vttest with command file input
    if [ -f "$VTTEST_DIR/cmd_full.txt" ]; then
        # Note: vttest requires a real terminal, so we can't fully automate
        # but we can provide the command file for reference
        print_warning "vttest requires an interactive terminal."
        print_warning "Run manually in DashTerm2 with dterm-core parser enabled."
        echo ""
        echo "Manual testing steps:"
        echo "1. Open DashTerm2"
        echo "2. Ensure dterm-core parser is enabled (Settings > Advanced > dtermCoreParserOutputEnabled)"
        echo "3. Run: vttest -l $log_file"
        echo "4. Press * to run all tests in each menu"
        echo "5. Document results in $RESULTS_DIR"
    fi
}

# Generate conformance report
generate_report() {
    print_header "Generating Conformance Report"

    local report_file="$RESULTS_DIR/vttest_conformance_report.md"
    local timestamp
    timestamp=$(date +%Y-%m-%d)

    cat > "$report_file" << EOF
# DashTerm2 vttest Conformance Report

**Date:** $timestamp
**Parser:** dterm-core (via DTermCoreParserAdapter)
**vttest Version:** $(vttest -V 2>&1 | head -1 || echo "unknown")

## Test Environment

- **macOS Version:** $(sw_vers -productVersion)
- **Xcode Version:** $(xcodebuild -version | head -1)
- **DashTerm2 Build:** Development
- **dterm-core Enabled:** YES
- **Parser Comparison:** YES
- **Parser Output:** YES (using dterm-core tokens)

## Test Results

### Menu 1: Cursor Movement Tests

| Test | Result | Notes |
|------|--------|-------|
| CUU (Cursor Up) | | |
| CUD (Cursor Down) | | |
| CUF (Cursor Forward) | | |
| CUB (Cursor Back) | | |
| CUP (Cursor Position) | | |
| HVP (Horizontal/Vertical Position) | | |
| Cursor absolute moves | | |
| Cursor relative moves | | |
| Cursor wraparound | | |

### Menu 2: Screen Features Tests

| Test | Result | Notes |
|------|--------|-------|
| ED (Erase in Display) | | |
| EL (Erase in Line) | | |
| DCH (Delete Character) | | |
| ICH (Insert Character) | | |
| IL (Insert Line) | | |
| DL (Delete Line) | | |
| DECAWM (Auto Wrap) | | |
| DECSTBM (Scroll Region) | | |
| DECOM (Origin Mode) | | |

### Menu 3: Character Set Tests

| Test | Result | Notes |
|------|--------|-------|
| DEC Special Graphics | | |
| UK Character Set | | |
| G0/G1 Set Selection | | |
| SI/SO (Shift In/Out) | | |

### Menu 6: Terminal Reports Tests

| Test | Result | Notes |
|------|--------|-------|
| DA (Device Attributes) | | |
| DSR (Device Status Report) | | |
| CPR (Cursor Position Report) | | |

### Menu 8: VT102 Features Tests

| Test | Result | Notes |
|------|--------|-------|
| DECSC/DECRC (Save/Restore Cursor) | | |
| Additional scroll regions | | |
| Insert/delete operations | | |

### Menu 11: Non-VT100 Features Tests

| Test | Result | Notes |
|------|--------|-------|
| Cursor styles | | |
| 256-color support | | |
| True color (RGB) | | |
| Bracketed paste mode | | |
| Alternate screen buffer | | |
| Mouse tracking | | |

## Summary

| Category | Tests | Pass | Fail | Skip |
|----------|-------|------|------|------|
| Cursor Movement | | | | |
| Screen Features | | | | |
| Character Sets | | | | |
| Terminal Reports | | | | |
| VT102 Features | | | | |
| Non-VT100 Features | | | | |
| **Total** | | | | |

## Notes

- Fill in this report after running vttest manually in DashTerm2
- Mark PASS for tests that display correctly
- Mark FAIL for tests with incorrect display
- Mark SKIP for tests not applicable to DashTerm2

## Related Documents

- \`docs/DTERM-AI-DIRECTIVE-V3.md\` - Phase 3 directive
- \`docs/dterm-core-validation.log\` - Parser validation log
- \`~/dterm/docs/CONFORMANCE.md\` - dterm-core conformance details
EOF

    print_success "Created conformance report template: $report_file"
    echo ""
    echo "Fill in the report after running vttest manually."
}

# Show instructions for manual testing
show_instructions() {
    print_header "vttest Validation Instructions"

    cat << 'EOF'

DashTerm2 vttest Validation
===========================

vttest is an interactive VT100/VT220 conformance test suite. While some
tests can be automated via command files, full validation requires visual
inspection of test output.

SETUP
-----
1. Build DashTerm2:
   xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-"

2. Ensure dterm-core parser is enabled:
   defaults write com.dashterm.DashTerm2 dtermCoreEnabled -bool YES
   defaults write com.dashterm.DashTerm2 dtermCoreParserComparisonEnabled -bool YES
   defaults write com.dashterm.DashTerm2 dtermCoreParserOutputEnabled -bool YES

3. Launch DashTerm2

RUNNING TESTS
-------------
In DashTerm2 terminal:
   vttest -l /tmp/vttest.log

Test each menu:
- Menu 1: Cursor Movement (press 1, then * to run all)
- Menu 2: Screen Features (press 2, then * to run all)
- Menu 3: Character Sets (press 3, then * to run all)
- Menu 6: Terminal Reports (press 6, then * to run all)
- Menu 8: VT102 Features (press 8, then * to run all)
- Menu 11: Non-VT100 Features (press 11, then * to run all)

For each test:
- Visual inspection: Does the display look correct?
- Compare with expected output described in vttest
- Note any differences or failures

RECORDING RESULTS
-----------------
After testing, update:
- tests/vttest/results/vttest_conformance_report.md
- docs/dterm-core-validation.log (mark vttest as complete)
- docs/DTERM-AI-DIRECTIVE-V3.md (check off vttest in Phase 3.1)

KNOWN ISSUES
------------
- Some VT52 tests may not apply (historical mode)
- Double-sized characters may have rendering differences
- Keyboard tests require specific key combinations

EOF
}

# Main
main() {
    setup_dirs

    case "${1:-}" in
        --create-commands)
            check_prereqs
            create_command_files
            ;;
        --run-basic)
            check_prereqs
            run_basic_tests
            ;;
        --report)
            generate_report
            ;;
        --instructions|--help|-h)
            show_instructions
            ;;
        *)
            check_prereqs
            create_command_files
            generate_report
            show_instructions
            ;;
    esac
}

main "$@"
