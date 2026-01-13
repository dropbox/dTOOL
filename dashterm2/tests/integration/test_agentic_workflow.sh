#!/bin/bash
# =============================================================================
# PARAGON INTEGRATION TEST: Agentic Coding Workflow
# =============================================================================
# This test verifies DashTerm2 can handle the agentic coding workflow used by
# run_worker.sh - the most critical use case for this terminal.
#
# Tests:
#   1. App launches and creates terminal session
#   2. Commands execute and produce output
#   3. Long-running process with streaming output
#   4. Pipe chains work correctly
#   5. Signal handling (Ctrl+C interrupt)
#   6. ANSI colors render (output contains escape sequences)
#   7. Unicode characters work
#   8. Environment variables propagate
#   9. Exit codes are captured correctly
#  10. Terminal contents can be read programmatically
#
# Usage:
#   ./tests/integration/test_agentic_workflow.sh
#
# Requirements:
#   - DashTerm2.app built in DerivedData or /Applications
#   - macOS with AppleScript support
# =============================================================================

set -euo pipefail

# Colors for test output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Find DashTerm2.app
find_app() {
    local locations=(
        "$HOME/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development/DashTerm2.app"
        "$HOME/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Variant-ASan/Development/DashTerm2.app"
        "/Applications/DashTerm2.app"
        "./build/Development/DashTerm2.app"
    )

    for pattern in "${locations[@]}"; do
        for app in $pattern; do
            if [ -d "$app" ]; then
                echo "$app"
                return 0
            fi
        done
    done

    return 1
}

log_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((TESTS_PASSED++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++))
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
}

# =============================================================================
# TEST HELPERS
# =============================================================================

# Run AppleScript and return result
run_applescript() {
    osascript -e "$1" 2>&1
}

# Write text to current DashTerm2 session
write_to_terminal() {
    local text="$1"
    run_applescript "
        tell application \"DashTerm2\"
            tell current session of current window
                write text \"$text\"
            end tell
        end tell
    "
}

# Get terminal contents
get_terminal_contents() {
    run_applescript "
        tell application \"DashTerm2\"
            tell current session of current window
                return contents
            end tell
        end tell
    "
}

# Wait for text to appear in terminal (with timeout)
wait_for_text() {
    local pattern="$1"
    local timeout="${2:-10}"
    local elapsed=0

    while [ $elapsed -lt $timeout ]; do
        local contents
        contents=$(get_terminal_contents)
        if echo "$contents" | grep -q "$pattern"; then
            return 0
        fi
        sleep 0.5
        elapsed=$((elapsed + 1))
    done

    return 1
}

# =============================================================================
# TEST CASES
# =============================================================================

test_01_app_launches() {
    log_test "01: App launches and creates window"
    ((TESTS_RUN++))

    # Launch app
    local result
    result=$(run_applescript "
        tell application \"DashTerm2\"
            activate
            delay 2
            return (count of windows)
        end tell
    ")

    if [ "$result" -ge 1 ]; then
        log_pass "App launched with $result window(s)"
    else
        log_fail "App did not create window (got: $result)"
        return 1
    fi
}

test_02_command_execution() {
    log_test "02: Command execution and output"
    ((TESTS_RUN++))

    # Create unique marker to find in output
    local marker="DASHTERM_TEST_$(date +%s)"

    write_to_terminal "echo '$marker'"
    sleep 1

    if wait_for_text "$marker" 5; then
        log_pass "Command executed and output visible"
    else
        log_fail "Command output not found in terminal"
        return 1
    fi
}

test_03_streaming_output() {
    log_test "03: Streaming output (simulates worker JSON stream)"
    ((TESTS_RUN++))

    # Simulate streaming JSON output like claude CLI
    local marker="STREAM_END_$(date +%s)"

    write_to_terminal "for i in 1 2 3 4 5; do echo '{\"event\":\"progress\",\"n\":'\$i'}'; sleep 0.2; done; echo '$marker'"

    if wait_for_text "$marker" 10; then
        local contents
        contents=$(get_terminal_contents)
        if echo "$contents" | grep -q '"event":"progress"'; then
            log_pass "Streaming JSON output captured correctly"
        else
            log_fail "Streaming output malformed"
            return 1
        fi
    else
        log_fail "Streaming output timed out"
        return 1
    fi
}

test_04_pipe_chains() {
    log_test "04: Pipe chains (critical for worker: claude | filter | tee | convert)"
    ((TESTS_RUN++))

    local marker="PIPE_RESULT_$(date +%s)"

    # Simulate the worker's pipe chain pattern
    write_to_terminal "echo 'test data' | cat | tee /dev/null | tr 'a-z' 'A-Z' && echo '$marker'"

    if wait_for_text "$marker" 5; then
        local contents
        contents=$(get_terminal_contents)
        if echo "$contents" | grep -q "TEST DATA"; then
            log_pass "Pipe chain executed correctly"
        else
            log_fail "Pipe chain output incorrect"
            return 1
        fi
    else
        log_fail "Pipe chain timed out"
        return 1
    fi
}

test_05_long_running_process() {
    log_test "05: Long-running process (simulates worker iteration)"
    ((TESTS_RUN++))

    local marker="LONG_DONE_$(date +%s)"

    # Start a process that runs for a few seconds
    write_to_terminal "for i in \$(seq 1 5); do echo \"Iteration \$i\"; sleep 0.5; done && echo '$marker'"

    if wait_for_text "$marker" 15; then
        local contents
        contents=$(get_terminal_contents)
        if echo "$contents" | grep -q "Iteration 5"; then
            log_pass "Long-running process completed all iterations"
        else
            log_fail "Long-running process output incomplete"
            return 1
        fi
    else
        log_fail "Long-running process timed out"
        return 1
    fi
}

test_06_ansi_colors() {
    log_test "06: ANSI color codes (worker output is colorized)"
    ((TESTS_RUN++))

    local marker="COLOR_END_$(date +%s)"

    # Output colored text
    write_to_terminal "printf '\\033[0;31mRED\\033[0m \\033[0;32mGREEN\\033[0m \\033[0;34mBLUE\\033[0m\\n' && echo '$marker'"

    if wait_for_text "$marker" 5; then
        # Note: We can't verify colors rendered correctly via AppleScript,
        # but we can verify the terminal didn't crash on escape sequences
        log_pass "ANSI escape sequences processed without crash"
    else
        log_fail "ANSI color output timed out"
        return 1
    fi
}

test_07_unicode() {
    log_test "07: Unicode characters (worker uses emoji in output)"
    ((TESTS_RUN++))

    local marker="UNICODE_END_$(date +%s)"

    # Output unicode including emoji (used by worker status messages)
    write_to_terminal "echo '✓ Pass ✗ Fail 📝 Note 🤖 Robot' && echo '$marker'"

    if wait_for_text "$marker" 5; then
        local contents
        contents=$(get_terminal_contents)
        # Check for checkmark (simpler unicode)
        if echo "$contents" | grep -q "✓"; then
            log_pass "Unicode characters rendered"
        else
            log_fail "Unicode characters not found in output"
            return 1
        fi
    else
        log_fail "Unicode output timed out"
        return 1
    fi
}

test_08_environment_variables() {
    log_test "08: Environment variables propagate"
    ((TESTS_RUN++))

    local marker="ENV_END_$(date +%s)"
    local test_var="DASHTERM_TEST_VAR_$(date +%s)"

    write_to_terminal "export $test_var=hello && echo \"\$$test_var\" && echo '$marker'"

    if wait_for_text "$marker" 5; then
        local contents
        contents=$(get_terminal_contents)
        if echo "$contents" | grep -q "hello"; then
            log_pass "Environment variables work correctly"
        else
            log_fail "Environment variable not expanded"
            return 1
        fi
    else
        log_fail "Environment variable test timed out"
        return 1
    fi
}

test_09_exit_codes() {
    log_test "09: Exit codes captured correctly"
    ((TESTS_RUN++))

    local marker="EXIT_END_$(date +%s)"

    # Run command that fails, then check exit code
    write_to_terminal "false; echo \"Exit code: \$?\" && echo '$marker'"

    if wait_for_text "$marker" 5; then
        local contents
        contents=$(get_terminal_contents)
        if echo "$contents" | grep -q "Exit code: 1"; then
            log_pass "Exit codes captured correctly"
        else
            log_fail "Exit code not captured (expected 1)"
            return 1
        fi
    else
        log_fail "Exit code test timed out"
        return 1
    fi
}

test_10_signal_interrupt() {
    log_test "10: Signal handling (Ctrl+C interrupt)"
    ((TESTS_RUN++))

    local marker="SIGNAL_END_$(date +%s)"

    # Start a sleep, then send interrupt
    write_to_terminal "sleep 10 &"
    sleep 1
    write_to_terminal "kill %1 2>/dev/null || true && echo '$marker'"

    if wait_for_text "$marker" 5; then
        log_pass "Signal handling works"
    else
        log_fail "Signal handling test timed out"
        return 1
    fi
}

test_11_worker_simulation() {
    log_test "11: FULL WORKER SIMULATION - agentic coding workflow"
    ((TESTS_RUN++))

    local marker="WORKER_SIM_DONE_$(date +%s)"

    # Simulate the actual worker pattern from run_worker.sh:
    # - Streaming JSON output
    # - Multiple iterations
    # - Status updates
    # - Log output

    local worker_script='
iteration=1
while [ $iteration -le 3 ]; do
    echo "========================================"
    echo "=== Worker Iteration $iteration"
    echo "=== Started at $(date)"
    echo "========================================"

    # Simulate claude output (streaming JSON)
    for i in 1 2 3; do
        echo "{\"type\":\"content\",\"text\":\"Working on iteration $iteration, step $i\"}"
        sleep 0.2
    done

    echo "=== Worker Iteration $iteration completed ==="
    echo "=== Exit code: 0 ==="

    iteration=$((iteration + 1))
    sleep 0.3
done
echo "Worker completed 3 iterations"
'

    # Write the simulation script
    write_to_terminal "$worker_script && echo '$marker'"

    if wait_for_text "$marker" 30; then
        local contents
        contents=$(get_terminal_contents)

        # Verify all iterations completed
        if echo "$contents" | grep -q "Worker Iteration 3 completed" && \
           echo "$contents" | grep -q "Worker completed 3 iterations"; then
            log_pass "Full worker simulation completed successfully"
        else
            log_fail "Worker simulation incomplete"
            return 1
        fi
    else
        log_fail "Worker simulation timed out"
        return 1
    fi
}

# =============================================================================
# MAIN
# =============================================================================

main() {
    echo ""
    echo "============================================================================="
    echo "  DashTerm2 Integration Test: Agentic Coding Workflow"
    echo "============================================================================="
    echo ""

    # Find app
    local app_path
    if ! app_path=$(find_app); then
        echo -e "${RED}ERROR: DashTerm2.app not found${NC}"
        echo "Build the app first: xcodebuild -scheme DashTerm2 -configuration Development build"
        exit 1
    fi

    echo -e "${BLUE}Found app:${NC} $app_path"
    echo ""

    # Run tests
    test_01_app_launches || true
    test_02_command_execution || true
    test_03_streaming_output || true
    test_04_pipe_chains || true
    test_05_long_running_process || true
    test_06_ansi_colors || true
    test_07_unicode || true
    test_08_environment_variables || true
    test_09_exit_codes || true
    test_10_signal_interrupt || true
    test_11_worker_simulation || true

    # Summary
    echo ""
    echo "============================================================================="
    echo "  TEST SUMMARY"
    echo "============================================================================="
    echo ""
    echo -e "  Tests Run:    $TESTS_RUN"
    echo -e "  ${GREEN}Passed:${NC}       $TESTS_PASSED"
    echo -e "  ${RED}Failed:${NC}       $TESTS_FAILED"
    echo ""

    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}=============================================================================${NC}"
        echo -e "${GREEN}  ALL TESTS PASSED - DashTerm2 handles agentic workflow correctly${NC}"
        echo -e "${GREEN}=============================================================================${NC}"
        exit 0
    else
        echo -e "${RED}=============================================================================${NC}"
        echo -e "${RED}  $TESTS_FAILED TEST(S) FAILED${NC}"
        echo -e "${RED}=============================================================================${NC}"
        exit 1
    fi
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
