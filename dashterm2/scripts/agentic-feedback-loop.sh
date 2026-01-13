#!/bin/bash
# agentic-feedback-loop.sh
# Continuous feedback system for AI coding agents
#
# This script manages background watchers that provide instant feedback
# on code changes. AI agents can monitor output files to react to errors
# without manually invoking build commands.

set -e

FEEDBACK_DIR="${FEEDBACK_DIR:-/tmp/dashterm-feedback}"
RUST_PROJECT_DIR="${RUST_PROJECT_DIR:-$(pwd)/rust-core}"
XCODE_PROJECT="${XCODE_PROJECT:-DashTerm2.xcodeproj}"

# Colors for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

mkdir -p "$FEEDBACK_DIR"

# PID tracking
PIDS_FILE="$FEEDBACK_DIR/watcher_pids"

log() {
    echo -e "${BLUE}[feedback-loop]${NC} $1"
}

error() {
    echo -e "${RED}[feedback-loop ERROR]${NC} $1" >&2
}

success() {
    echo -e "${GREEN}[feedback-loop]${NC} $1"
}

# === Feedback Files (AI agents read these) ===
#
# $FEEDBACK_DIR/
#   ├── status.json          # Overall status summary
#   ├── rust_errors.txt      # Rust compilation errors
#   ├── rust_warnings.txt    # Rust warnings (clippy)
#   ├── rust_test_fails.txt  # Failed test names
#   ├── swift_errors.txt     # Swift/ObjC build errors
#   ├── last_update          # Timestamp of last change
#   └── watcher_pids         # PIDs for cleanup

update_status() {
    local component="$1"
    local status="$2"  # "ok", "error", "warning"
    local message="$3"
    local timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    # Update last_update timestamp
    echo "$timestamp" > "$FEEDBACK_DIR/last_update"

    # Update status.json
    local status_file="$FEEDBACK_DIR/status.json"

    # Read existing or create new
    if [[ -f "$status_file" ]]; then
        local existing=$(cat "$status_file")
    else
        local existing='{}'
    fi

    # Update component status using jq if available, else simple approach
    if command -v jq &> /dev/null; then
        echo "$existing" | jq --arg comp "$component" \
                              --arg stat "$status" \
                              --arg msg "$message" \
                              --arg ts "$timestamp" \
                              '.[$comp] = {"status": $stat, "message": $msg, "updated": $ts}' \
                              > "$status_file"
    else
        # Fallback: simple key=value format
        echo "$component=$status:$message:$timestamp" >> "$status_file.tmp"
        mv "$status_file.tmp" "$status_file"
    fi
}

# === Rust Watchers ===

start_bacon() {
    if ! command -v bacon &> /dev/null; then
        error "bacon not installed. Run: cargo install bacon"
        return 1
    fi

    if [[ ! -d "$RUST_PROJECT_DIR" ]]; then
        log "No Rust project at $RUST_PROJECT_DIR, skipping bacon"
        return 0
    fi

    log "Starting bacon watcher for Rust..."

    cd "$RUST_PROJECT_DIR"

    # bacon with JSON export (if supported) or text parsing
    bacon --export-locations 2>&1 | while IFS= read -r line; do
        echo "$line" >> "$FEEDBACK_DIR/bacon_raw.log"

        # Parse bacon output for errors
        if echo "$line" | grep -q "error\["; then
            echo "$line" >> "$FEEDBACK_DIR/rust_errors.txt"
            update_status "rust_build" "error" "Compilation error detected"
        elif echo "$line" | grep -q "warning:"; then
            echo "$line" >> "$FEEDBACK_DIR/rust_warnings.txt"
            update_status "rust_build" "warning" "Warnings present"
        elif echo "$line" | grep -q "Finished"; then
            # Clear errors on successful build
            > "$FEEDBACK_DIR/rust_errors.txt"
            update_status "rust_build" "ok" "Build successful"
        fi
    done &

    echo $! >> "$PIDS_FILE"
    success "bacon started (PID: $!)"
}

start_cargo_watch_tests() {
    if ! command -v cargo-watch &> /dev/null; then
        log "cargo-watch not installed, using watchexec fallback"
        start_watchexec_tests
        return
    fi

    if [[ ! -d "$RUST_PROJECT_DIR" ]]; then
        return 0
    fi

    log "Starting cargo-watch for tests..."

    cd "$RUST_PROJECT_DIR"

    cargo watch -x "test --no-fail-fast 2>&1" | while IFS= read -r line; do
        echo "$line" >> "$FEEDBACK_DIR/test_raw.log"

        # Parse test output
        if echo "$line" | grep -q "FAILED"; then
            echo "$line" >> "$FEEDBACK_DIR/rust_test_fails.txt"
            update_status "rust_tests" "error" "Test failures"
        elif echo "$line" | grep -q "test result: ok"; then
            > "$FEEDBACK_DIR/rust_test_fails.txt"
            update_status "rust_tests" "ok" "All tests passing"
        fi
    done &

    echo $! >> "$PIDS_FILE"
    success "cargo-watch tests started (PID: $!)"
}

start_watchexec_tests() {
    if ! command -v watchexec &> /dev/null; then
        error "Neither cargo-watch nor watchexec installed"
        return 1
    fi

    cd "$RUST_PROJECT_DIR"

    watchexec -e rs -r "cargo test --no-fail-fast 2>&1 | tee $FEEDBACK_DIR/test_output.txt" &
    echo $! >> "$PIDS_FILE"
}

# === Swift/Obj-C Watchers ===

start_swift_watcher() {
    if [[ ! -f "$XCODE_PROJECT/project.pbxproj" ]]; then
        log "No Xcode project found, skipping Swift watcher"
        return 0
    fi

    log "Starting Swift/ObjC file watcher..."

    # Watch for .swift, .m, .mm, .h changes
    if command -v fswatch &> /dev/null; then
        fswatch -0 sources/*.swift sources/*.m sources/*.mm sources/*.h 2>/dev/null | while IFS= read -r -d '' file; do
            log "Changed: $file - triggering build check"

            # Quick syntax check (faster than full build)
            if [[ "$file" == *.swift ]]; then
                swiftc -parse "$file" 2>> "$FEEDBACK_DIR/swift_errors.txt" && \
                    update_status "swift_syntax" "ok" "Syntax OK: $file" || \
                    update_status "swift_syntax" "error" "Syntax error: $file"
            fi

            # Run swiftlint on changed file
            if command -v swiftlint &> /dev/null; then
                swiftlint lint --path "$file" --quiet 2>&1 | \
                    grep "error:" >> "$FEEDBACK_DIR/swift_errors.txt"
            fi
        done &

        echo $! >> "$PIDS_FILE"
        success "Swift file watcher started (PID: $!)"
    else
        log "fswatch not installed, Swift watching disabled"
    fi
}

start_incremental_build_watcher() {
    log "Starting incremental build watcher..."

    # Every 30 seconds, do a quick incremental build check
    while true; do
        sleep 30

        # Check if any source files changed recently (last 30 sec)
        local changed=$(find sources -name "*.m" -o -name "*.swift" -mmin -0.5 2>/dev/null | head -1)

        if [[ -n "$changed" ]]; then
            log "Recent changes detected, running incremental build..."

            # Quick build (no clean)
            xcodebuild -project "$XCODE_PROJECT" \
                       -scheme DashTerm2 \
                       -configuration Development \
                       build \
                       CODE_SIGNING_ALLOWED=NO \
                       CODE_SIGN_IDENTITY="-" \
                       2>&1 | tee "$FEEDBACK_DIR/xcode_build.log" | \
            while IFS= read -r line; do
                if echo "$line" | grep -q "error:"; then
                    echo "$line" >> "$FEEDBACK_DIR/swift_errors.txt"
                    update_status "xcode_build" "error" "Build error"
                fi
            done

            if [[ ${PIPESTATUS[0]} -eq 0 ]]; then
                > "$FEEDBACK_DIR/swift_errors.txt"
                update_status "xcode_build" "ok" "Build successful"
            fi
        fi
    done &

    echo $! >> "$PIDS_FILE"
    success "Incremental build watcher started (PID: $!)"
}

# === Clippy Deep Analysis ===

start_clippy_watcher() {
    if [[ ! -d "$RUST_PROJECT_DIR" ]]; then
        return 0
    fi

    log "Starting clippy deep analysis (runs every 60s)..."

    cd "$RUST_PROJECT_DIR"

    while true; do
        sleep 60

        cargo clippy --all-targets --all-features -- \
            -W clippy::pedantic \
            -W clippy::nursery \
            -A clippy::module_name_repetitions \
            2>&1 | tee "$FEEDBACK_DIR/clippy_full.txt" | \
        grep -E "^(error|warning)" >> "$FEEDBACK_DIR/rust_warnings.txt"

        local warn_count=$(grep -c "warning:" "$FEEDBACK_DIR/clippy_full.txt" 2>/dev/null || echo 0)
        local err_count=$(grep -c "error:" "$FEEDBACK_DIR/clippy_full.txt" 2>/dev/null || echo 0)

        if [[ "$err_count" -gt 0 ]]; then
            update_status "clippy" "error" "$err_count errors, $warn_count warnings"
        elif [[ "$warn_count" -gt 0 ]]; then
            update_status "clippy" "warning" "$warn_count warnings"
        else
            update_status "clippy" "ok" "No issues"
        fi
    done &

    echo $! >> "$PIDS_FILE"
    success "Clippy watcher started (PID: $!)"
}

# === Agent Query Interface ===

# This function is meant to be called by AI agents to check status
query_status() {
    local component="${1:-all}"

    if [[ "$component" == "all" ]]; then
        if [[ -f "$FEEDBACK_DIR/status.json" ]]; then
            cat "$FEEDBACK_DIR/status.json"
        else
            echo '{"status": "no_data"}'
        fi
    else
        if command -v jq &> /dev/null && [[ -f "$FEEDBACK_DIR/status.json" ]]; then
            jq --arg comp "$component" '.[$comp]' "$FEEDBACK_DIR/status.json"
        fi
    fi
}

# Get current errors (for AI to fix)
get_errors() {
    echo "=== Rust Errors ==="
    cat "$FEEDBACK_DIR/rust_errors.txt" 2>/dev/null || echo "(none)"

    echo ""
    echo "=== Swift/ObjC Errors ==="
    cat "$FEEDBACK_DIR/swift_errors.txt" 2>/dev/null || echo "(none)"

    echo ""
    echo "=== Test Failures ==="
    cat "$FEEDBACK_DIR/rust_test_fails.txt" 2>/dev/null || echo "(none)"
}

# Wait for clean state (useful for agent to block until errors fixed)
wait_for_clean() {
    local timeout="${1:-300}"  # 5 minute default
    local start=$(date +%s)

    while true; do
        local now=$(date +%s)
        local elapsed=$((now - start))

        if [[ $elapsed -gt $timeout ]]; then
            error "Timeout waiting for clean state"
            return 1
        fi

        # Check if all components are OK
        local rust_ok=$(query_status "rust_build" | grep -c '"ok"' || echo 0)
        local swift_ok=$(query_status "xcode_build" | grep -c '"ok"' || echo 0)
        local tests_ok=$(query_status "rust_tests" | grep -c '"ok"' || echo 0)

        if [[ "$rust_ok" -gt 0 ]] && [[ "$tests_ok" -gt 0 ]]; then
            success "All checks passing!"
            return 0
        fi

        log "Waiting for clean state... (${elapsed}s elapsed)"
        sleep 5
    done
}

# === Control Functions ===

start_all() {
    log "Starting all watchers..."

    # Clear previous state
    > "$FEEDBACK_DIR/rust_errors.txt"
    > "$FEEDBACK_DIR/rust_warnings.txt"
    > "$FEEDBACK_DIR/rust_test_fails.txt"
    > "$FEEDBACK_DIR/swift_errors.txt"
    > "$PIDS_FILE"

    # Initialize status
    update_status "system" "starting" "Initializing watchers"

    start_bacon
    start_cargo_watch_tests
    start_swift_watcher
    start_incremental_build_watcher
    start_clippy_watcher

    update_status "system" "ok" "All watchers running"

    success "All watchers started. Feedback dir: $FEEDBACK_DIR"
    echo ""
    echo "Agent integration:"
    echo "  - Read errors:    cat $FEEDBACK_DIR/rust_errors.txt"
    echo "  - Check status:   $0 status"
    echo "  - Get all errors: $0 errors"
    echo "  - Wait for clean: $0 wait"
}

stop_all() {
    log "Stopping all watchers..."

    if [[ -f "$PIDS_FILE" ]]; then
        while IFS= read -r pid; do
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid" 2>/dev/null || true
                log "Stopped PID $pid"
            fi
        done < "$PIDS_FILE"
        rm "$PIDS_FILE"
    fi

    # Also kill any orphaned watchers
    pkill -f "bacon" 2>/dev/null || true
    pkill -f "cargo-watch" 2>/dev/null || true
    pkill -f "fswatch.*sources" 2>/dev/null || true

    update_status "system" "stopped" "Watchers stopped"
    success "All watchers stopped"
}

show_status() {
    echo "=== Feedback Loop Status ==="
    echo ""

    if [[ -f "$FEEDBACK_DIR/status.json" ]]; then
        if command -v jq &> /dev/null; then
            jq '.' "$FEEDBACK_DIR/status.json"
        else
            cat "$FEEDBACK_DIR/status.json"
        fi
    else
        echo "No status data yet"
    fi

    echo ""
    echo "=== Active Watchers ==="
    if [[ -f "$PIDS_FILE" ]]; then
        while IFS= read -r pid; do
            if kill -0 "$pid" 2>/dev/null; then
                echo "  Running: PID $pid"
            else
                echo "  Dead: PID $pid"
            fi
        done < "$PIDS_FILE"
    else
        echo "  No watchers running"
    fi

    echo ""
    echo "=== Last Update ==="
    cat "$FEEDBACK_DIR/last_update" 2>/dev/null || echo "Never"
}

# === Main ===

case "${1:-}" in
    start)
        start_all
        ;;
    stop)
        stop_all
        ;;
    restart)
        stop_all
        sleep 1
        start_all
        ;;
    status)
        show_status
        ;;
    errors)
        get_errors
        ;;
    wait)
        wait_for_clean "${2:-300}"
        ;;
    query)
        query_status "${2:-all}"
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|errors|wait|query [component]}"
        echo ""
        echo "Commands:"
        echo "  start   - Start all background watchers"
        echo "  stop    - Stop all watchers"
        echo "  restart - Restart all watchers"
        echo "  status  - Show current status"
        echo "  errors  - Show all current errors"
        echo "  wait    - Block until all checks pass (timeout: 300s)"
        echo "  query   - Query status JSON (optionally for specific component)"
        echo ""
        echo "For AI agent integration, see: $FEEDBACK_DIR/"
        exit 1
        ;;
esac
