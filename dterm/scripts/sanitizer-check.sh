#!/usr/bin/env bash
#
# sanitizer-check.sh - Run test suite with memory/thread/UB sanitizers
#
# This script runs the dterm-core test suite with various sanitizers enabled
# to detect memory errors, data races, and undefined behavior.
#
# Requires: Rust nightly toolchain with sanitizer support
#
# Usage:
#   ./scripts/sanitizer-check.sh           # Run all sanitizers
#   ./scripts/sanitizer-check.sh asan      # Run only AddressSanitizer
#   ./scripts/sanitizer-check.sh tsan      # Run only ThreadSanitizer
#   ./scripts/sanitizer-check.sh msan      # Run only MemorySanitizer
#   ./scripts/sanitizer-check.sh ubsan     # Run only UndefinedBehaviorSanitizer
#   ./scripts/sanitizer-check.sh miri      # Run only MIRI
#   ./scripts/sanitizer-check.sh all       # Run all (default)
#
# Exit codes:
#   0 - All tests passed
#   1 - Test failures detected
#   2 - Setup error (missing toolchain, etc.)

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0

# Results array
declare -a RESULTS

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_header() {
    echo ""
    echo "================================================================"
    echo -e "${BLUE}$1${NC}"
    echo "================================================================"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check for nightly toolchain
    if ! rustup run nightly rustc --version &>/dev/null; then
        log_error "Nightly toolchain not installed. Install with: rustup toolchain install nightly"
        exit 2
    fi

    # Check for component availability
    if ! rustup run nightly rustc -Z help &>/dev/null; then
        log_error "Nightly rustc doesn't support -Z flags"
        exit 2
    fi

    log_success "Prerequisites satisfied"
}

# Run AddressSanitizer
run_asan() {
    log_header "AddressSanitizer (ASan)"
    log_info "Detects: buffer overflows, use-after-free, memory leaks"

    # ASan requires special target on macOS
    local target=""
    if [[ "$(uname)" == "Darwin" ]]; then
        # macOS needs explicit target
        target="--target aarch64-apple-darwin"
        # Check if target is installed
        if ! rustup run nightly rustc --print target-list | grep -q "aarch64-apple-darwin"; then
            log_warning "aarch64-apple-darwin target may need installation"
        fi
    fi

    # Set environment for ASan
    export RUSTFLAGS="-Z sanitizer=address"
    export ASAN_OPTIONS="detect_leaks=1:detect_stack_use_after_return=1:strict_string_checks=1"

    if RUSTFLAGS="$RUSTFLAGS" cargo +nightly test -p dterm-core --lib $target 2>&1 | tee /tmp/asan_output.txt; then
        log_success "ASan: All tests passed"
        RESULTS+=("ASan: PASS")
        ((TESTS_PASSED++))
    else
        log_error "ASan: Tests failed - check /tmp/asan_output.txt"
        RESULTS+=("ASan: FAIL")
        ((TESTS_FAILED++))
    fi
}

# Run ThreadSanitizer
run_tsan() {
    log_header "ThreadSanitizer (TSan)"
    log_info "Detects: data races, deadlocks"

    export RUSTFLAGS="-Z sanitizer=thread"
    export TSAN_OPTIONS="second_deadlock_stack=1"

    if RUSTFLAGS="$RUSTFLAGS" cargo +nightly test -p dterm-core --lib -- --test-threads=1 2>&1 | tee /tmp/tsan_output.txt; then
        log_success "TSan: All tests passed"
        RESULTS+=("TSan: PASS")
        ((TESTS_PASSED++))
    else
        log_error "TSan: Tests failed - check /tmp/tsan_output.txt"
        RESULTS+=("TSan: FAIL")
        ((TESTS_FAILED++))
    fi
}

# Run MemorySanitizer
run_msan() {
    log_header "MemorySanitizer (MSan)"
    log_info "Detects: uninitialized memory reads"

    # MSan is only supported on Linux x86_64
    if [[ "$(uname)" != "Linux" ]]; then
        log_warning "MSan is only supported on Linux - skipping"
        RESULTS+=("MSan: SKIPPED (not Linux)")
        ((TESTS_SKIPPED++))
        return
    fi

    export RUSTFLAGS="-Z sanitizer=memory"

    if RUSTFLAGS="$RUSTFLAGS" cargo +nightly test -p dterm-core --lib --target x86_64-unknown-linux-gnu 2>&1 | tee /tmp/msan_output.txt; then
        log_success "MSan: All tests passed"
        RESULTS+=("MSan: PASS")
        ((TESTS_PASSED++))
    else
        log_error "MSan: Tests failed - check /tmp/msan_output.txt"
        RESULTS+=("MSan: FAIL")
        ((TESTS_FAILED++))
    fi
}

# Run UndefinedBehaviorSanitizer
run_ubsan() {
    log_header "UndefinedBehaviorSanitizer (UBSan)"
    log_info "Detects: undefined behavior (shifts, overflows, etc.)"

    # Note: Rust's UBSan is experimental and may not catch all UB
    # MIRI is more comprehensive for Rust-specific UB

    # UBSan via overflow checks (built into Rust debug builds)
    export RUSTFLAGS="-C overflow-checks=on"

    if cargo +nightly test -p dterm-core --lib 2>&1 | tee /tmp/ubsan_output.txt; then
        log_success "UBSan (overflow checks): All tests passed"
        RESULTS+=("UBSan: PASS")
        ((TESTS_PASSED++))
    else
        log_error "UBSan: Tests failed - check /tmp/ubsan_output.txt"
        RESULTS+=("UBSan: FAIL")
        ((TESTS_FAILED++))
    fi
}

# Run MIRI for Rust-specific undefined behavior
run_miri() {
    log_header "MIRI (Rust UB Detector)"
    log_info "Detects: Rust-specific undefined behavior, memory model violations"

    # Check if miri is installed
    if ! rustup run nightly miri --version &>/dev/null; then
        log_info "Installing miri component..."
        rustup +nightly component add miri || {
            log_warning "Could not install miri - skipping"
            RESULTS+=("MIRI: SKIPPED (install failed)")
            ((TESTS_SKIPPED++))
            return
        }
    fi

    # MIRI is slow, so we run a subset of tests
    export MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-symbolic-alignment-check"

    # Run key modules that deal with unsafe code
    log_info "Running MIRI on critical modules (this is slow)..."

    local miri_failed=0

    # Grid module (has unsafe code)
    log_info "Testing grid module..."
    if cargo +nightly miri test -p dterm-core grid::page:: 2>&1 | tee -a /tmp/miri_output.txt; then
        log_success "MIRI grid::page: passed"
    else
        log_error "MIRI grid::page: failed"
        miri_failed=1
    fi

    # Row module
    log_info "Testing row module..."
    if cargo +nightly miri test -p dterm-core grid::row:: 2>&1 | tee -a /tmp/miri_output.txt; then
        log_success "MIRI grid::row: passed"
    else
        log_error "MIRI grid::row: failed"
        miri_failed=1
    fi

    # Parser module
    log_info "Testing parser module..."
    if cargo +nightly miri test -p dterm-core parser:: -- --test-threads=1 2>&1 | tee -a /tmp/miri_output.txt; then
        log_success "MIRI parser: passed"
    else
        log_error "MIRI parser: failed"
        miri_failed=1
    fi

    if [[ $miri_failed -eq 0 ]]; then
        log_success "MIRI: All critical modules passed"
        RESULTS+=("MIRI: PASS")
        ((TESTS_PASSED++))
    else
        log_error "MIRI: Some modules failed - check /tmp/miri_output.txt"
        RESULTS+=("MIRI: FAIL")
        ((TESTS_FAILED++))
    fi
}

# Run leak sanitizer separately
run_leak_check() {
    log_header "Leak Sanitizer (LSan)"
    log_info "Detects: memory leaks"

    # LSan is part of ASan on most platforms
    export RUSTFLAGS="-Z sanitizer=address"
    export ASAN_OPTIONS="detect_leaks=1"

    # Run a subset of tests that allocate heavily
    if RUSTFLAGS="$RUSTFLAGS" cargo +nightly test -p dterm-core grid:: 2>&1 | tee /tmp/lsan_output.txt; then
        log_success "LSan: No leaks detected"
        RESULTS+=("LSan: PASS")
        ((TESTS_PASSED++))
    else
        log_error "LSan: Leaks detected - check /tmp/lsan_output.txt"
        RESULTS+=("LSan: FAIL")
        ((TESTS_FAILED++))
    fi
}

# Print summary
print_summary() {
    log_header "SUMMARY"

    echo ""
    echo "Results:"
    for result in "${RESULTS[@]}"; do
        if [[ "$result" == *"PASS"* ]]; then
            echo -e "  ${GREEN}✓${NC} $result"
        elif [[ "$result" == *"FAIL"* ]]; then
            echo -e "  ${RED}✗${NC} $result"
        else
            echo -e "  ${YELLOW}○${NC} $result"
        fi
    done

    echo ""
    echo "Totals:"
    echo -e "  Passed:  ${GREEN}$TESTS_PASSED${NC}"
    echo -e "  Failed:  ${RED}$TESTS_FAILED${NC}"
    echo -e "  Skipped: ${YELLOW}$TESTS_SKIPPED${NC}"
    echo ""

    if [[ $TESTS_FAILED -gt 0 ]]; then
        echo -e "${RED}Some sanitizer checks failed!${NC}"
        echo "Review the output files in /tmp/ for details."
        return 1
    else
        echo -e "${GREEN}All sanitizer checks passed!${NC}"
        return 0
    fi
}

# Main
main() {
    local mode="${1:-all}"

    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║           dterm-core Sanitizer Check Suite                   ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    check_prerequisites

    # Clean previous output
    rm -f /tmp/asan_output.txt /tmp/tsan_output.txt /tmp/msan_output.txt
    rm -f /tmp/ubsan_output.txt /tmp/miri_output.txt /tmp/lsan_output.txt

    case "$mode" in
        asan)
            run_asan
            ;;
        tsan)
            run_tsan
            ;;
        msan)
            run_msan
            ;;
        ubsan)
            run_ubsan
            ;;
        miri)
            run_miri
            ;;
        lsan)
            run_leak_check
            ;;
        all)
            run_asan
            run_tsan
            run_msan
            run_ubsan
            run_miri
            ;;
        quick)
            # Quick mode: just ASan and MIRI
            run_asan
            run_miri
            ;;
        *)
            echo "Usage: $0 [asan|tsan|msan|ubsan|miri|lsan|all|quick]"
            exit 2
            ;;
    esac

    print_summary
}

main "$@"
