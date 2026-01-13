#!/bin/bash
# capture-crashes.sh - Automatically capture and format macOS crash reports
#
# This script monitors for DashTerm2/iTerm2 crash reports and copies them
# to the worker_logs directory in a format the AI worker can understand.
#
# Usage:
#   ./scripts/capture-crashes.sh              # Check for new crashes since last run
#   ./scripts/capture-crashes.sh --watch      # Continuous monitoring mode
#   ./scripts/capture-crashes.sh --since "1 hour ago"  # Check crashes since time
#   ./scripts/capture-crashes.sh --all        # Process all existing crashes
#
# Output:
#   worker_logs/app_crashes/         - Directory with individual crash reports
#   worker_logs/app_crashes.summary  - Summary of all crashes for AI consumption
#   worker_logs/last_crash_check     - Timestamp of last check

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WORKER_LOGS="$PROJECT_DIR/worker_logs"
CRASH_DIR="$WORKER_LOGS/app_crashes"
SUMMARY_FILE="$WORKER_LOGS/app_crashes.summary"
LAST_CHECK_FILE="$WORKER_LOGS/last_crash_check"
DIAGNOSTIC_REPORTS="$HOME/Library/Logs/DiagnosticReports"
NOTIFIED_CRASHES_FILE="$WORKER_LOGS/notified_crashes.txt"

# App names to monitor (both old iTerm2 and new DashTerm2)
APP_PATTERNS=("DashTerm2" "iTerm2" "DashTerm2TestHost" "DashTerm2Tests")

mkdir -p "$CRASH_DIR"

# Parse arguments
MODE="check"
SINCE_TIME=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --watch)
            MODE="watch"
            shift
            ;;
        --since)
            SINCE_TIME="$2"
            shift 2
            ;;
        --all)
            SINCE_TIME="1970-01-01"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Get the timestamp to compare against
get_check_time() {
    if [[ -n "$SINCE_TIME" ]]; then
        date -j -f "%Y-%m-%d %H:%M:%S" "$(date -j -v-1d +"%Y-%m-%d %H:%M:%S" 2>/dev/null || date -d "$SINCE_TIME" +"%Y-%m-%d %H:%M:%S")" +%s 2>/dev/null || date +%s
    elif [[ -f "$LAST_CHECK_FILE" ]]; then
        cat "$LAST_CHECK_FILE"
    else
        # Default: check last 24 hours
        echo $(($(date +%s) - 86400))
    fi
}

# Extract key info from .ips crash report (JSON format, macOS 12+)
parse_ips_crash() {
    local file="$1"
    local output="$2"

    # Use python to parse the IPS JSON format
    python3 << EOF > "$output"
import json
import sys

try:
    with open("$file", "r") as f:
        content = f.read()

    # IPS files have TWO JSON objects:
    # Line 1: metadata header (small JSON with app_name, timestamp, etc.)
    # Lines 2+: main crash report JSON
    lines = content.split('\n')

    # Parse the main crash data (skip the header line)
    data = None
    header = None

    # Try to parse header first
    if lines:
        try:
            header = json.loads(lines[0])
        except:
            pass

    # Parse the main JSON body (lines after header, but may have trailing non-JSON text)
    # Find where the main JSON object ends (closing brace at start of line)
    body_lines = lines[1:]
    json_end = len(body_lines)

    # Look for "~~ Error Logs ~~" or similar markers that indicate end of JSON
    for i, line in enumerate(body_lines):
        if line.startswith('~~') or (line.strip() == '}' and i > 0 and body_lines[i-1].strip() == '}'):
            # Found end marker or double closing brace
            json_end = i + 1 if line.strip() == '}' else i
            break

    # Try parsing just the JSON portion
    json_text = '\n'.join(body_lines[:json_end])
    try:
        data = json.loads(json_text)
    except json.JSONDecodeError:
        # Try with raw.loads which is more lenient about trailing content
        import json.decoder
        decoder = json.JSONDecoder()
        try:
            data, _ = decoder.raw_decode('\n'.join(body_lines))
        except:
            # Fall back to the whole content
            try:
                data, _ = decoder.raw_decode(content)
            except:
                data = None

    if not data:
        raise ValueError("Could not parse crash report JSON")

    # Extract key information
    print("=" * 70)
    print("CRASH REPORT SUMMARY")
    print("=" * 70)

    # Use header info if available
    if header:
        print(f"App: {header.get('app_name', 'Unknown')}")
        print(f"Version: {header.get('app_version', 'Unknown')}")
        print(f"Timestamp: {header.get('timestamp', 'Unknown')}")
        print(f"Bundle: {header.get('bundleID', 'Unknown')}")
    else:
        # Basic info from main data
        if 'procName' in data:
            print(f"Process: {data.get('procName', 'Unknown')}")
        if 'bundleInfo' in data:
            bi = data['bundleInfo']
            print(f"Bundle: {bi.get('CFBundleIdentifier', 'Unknown')}")
            print(f"Version: {bi.get('CFBundleShortVersionString', 'Unknown')}")
        print(f"Date: {data.get('captureTime', data.get('timestamp', 'Unknown'))}")

    # Exception info
    if 'exception' in data:
        exc = data['exception']
        print(f"\nException Type: {exc.get('type', 'Unknown')}")
        if 'signal' in exc:
            print(f"Signal: {exc['signal']}")
        if 'subtype' in exc:
            print(f"Subtype: {exc['subtype']}")
        if 'codes' in exc:
            print(f"Codes: {exc['codes']}")

    # Termination info
    if 'termination' in data:
        term = data['termination']
        print(f"\nTermination Reason: {term.get('reason', 'Unknown')}")
        if 'byProc' in term:
            print(f"By Process: {term['byProc']}")

    # Crashed thread info
    if 'faultingThread' in data:
        print(f"\nCrashed Thread: {data['faultingThread']}")

    # Thread backtraces
    if 'threads' in data:
        crashed_idx = data.get('faultingThread', 0)
        print(f"\n{'=' * 70}")
        print(f"CRASHED THREAD ({crashed_idx}) BACKTRACE")
        print("=" * 70)

        if crashed_idx < len(data['threads']):
            thread = data['threads'][crashed_idx]
            frames = thread.get('frames', [])

            # Get image info for symbolication
            images = {img.get('base', 0): img for img in data.get('usedImages', [])}

            for i, frame in enumerate(frames[:30]):  # Limit to 30 frames
                addr = frame.get('imageOffset', 0)
                img_idx = frame.get('imageIndex', 0)
                symbol = frame.get('symbol', '')

                # Try to get image name
                img_name = "???"
                if 'usedImages' in data and img_idx < len(data['usedImages']):
                    img = data['usedImages'][img_idx]
                    img_name = img.get('name', img.get('path', '???')).split('/')[-1]

                if symbol:
                    print(f"{i:3d}  {img_name:40s}  {symbol}")
                else:
                    print(f"{i:3d}  {img_name:40s}  0x{addr:x}")

    # ASan/diagnostic messages if present
    if 'asilesion' in data or 'diagMessage' in data:
        print(f"\n{'=' * 70}")
        print("DIAGNOSTIC MESSAGE")
        print("=" * 70)
        print(data.get('asilesion', data.get('diagMessage', '')))

    print(f"\n{'=' * 70}")
    print(f"Full report: $file")
    print("=" * 70)

except Exception as e:
    print(f"Error parsing crash report: {e}")
    print(f"File: $file")
    # Fall back to raw content
    with open("$file", "r") as f:
        print(f.read()[:5000])
EOF
}

# Extract key info from .crash report (legacy format)
parse_crash_file() {
    local file="$1"
    local output="$2"

    {
        echo "=" | tr -d '\n' | head -c 70
        echo ""
        echo "CRASH REPORT SUMMARY (Legacy Format)"
        echo "=" | tr -d '\n' | head -c 70
        echo ""

        # Extract key sections
        grep -E "^(Process|Path|Identifier|Version|Date/Time|Exception|Termination|Crashed Thread):" "$file" 2>/dev/null || true

        echo ""
        echo "CRASHED THREAD BACKTRACE"
        echo "=" | tr -d '\n' | head -c 70
        echo ""

        # Extract crashed thread backtrace (between "Crashed:" and next "Thread")
        awk '/^Thread [0-9]+ Crashed/,/^Thread [0-9]+[^C]/' "$file" | head -50

        echo ""
        echo "Full report: $file"
    } > "$output"
}

# Process a single crash file
process_crash() {
    local crash_file="$1"
    local filename=$(basename "$crash_file")
    local output_file="$CRASH_DIR/${filename}.txt"

    echo "Processing: $filename"

    if [[ "$crash_file" == *.ips ]]; then
        parse_ips_crash "$crash_file" "$output_file"
    else
        parse_crash_file "$crash_file" "$output_file"
    fi

    return 0
}

# Update the summary file
update_summary() {
    {
        echo "# DashTerm2 Crash Reports Summary"
        echo "# Generated: $(date)"
        echo "# Location: $CRASH_DIR"
        echo ""
        echo "## Recent Crashes (newest first)"
        echo ""

        # List crashes sorted by modification time (newest first)
        local count=0
        for crash in $(ls -t "$CRASH_DIR"/*.txt 2>/dev/null | head -20); do
            ((count++))
            local basename=$(basename "$crash" .txt)
            local mtime=$(stat -f "%Sm" -t "%Y-%m-%d %H:%M" "$crash" 2>/dev/null || stat -c "%y" "$crash" 2>/dev/null | cut -d. -f1)

            echo "### $count. $basename"
            echo "Date: $mtime"
            echo ""

            # Extract first few lines of summary
            head -20 "$crash" | sed 's/^/    /'
            echo ""
            echo "---"
            echo ""
        done

        if [[ $count -eq 0 ]]; then
            echo "No crash reports found."
        else
            echo ""
            echo "## Instructions for AI Worker"
            echo ""
            echo "When investigating a crash:"
            echo "1. Read the full crash report: cat worker_logs/app_crashes/<filename>.txt"
            echo "2. Look for the crashed thread backtrace to identify the failing code"
            echo "3. Search for the function names in the codebase"
            echo "4. Create a test that reproduces the crash condition"
            echo "5. Fix the underlying bug in production code"
        fi
    } > "$SUMMARY_FILE"
}

# Main check function
check_crashes() {
    local check_time=$(get_check_time)
    local found_new=0

    echo "Checking for crash reports since $(date -r "$check_time" 2>/dev/null || date -d "@$check_time")..."

    if [[ ! -d "$DIAGNOSTIC_REPORTS" ]]; then
        echo "Diagnostic reports directory not found: $DIAGNOSTIC_REPORTS"
        return 0
    fi

    for pattern in "${APP_PATTERNS[@]}"; do
        # Find .ips files (macOS 12+)
        while IFS= read -r -d '' crash_file; do
            local file_time=$(stat -f "%m" "$crash_file" 2>/dev/null || stat -c "%Y" "$crash_file")
            if [[ $file_time -gt $check_time ]]; then
                process_crash "$crash_file" && ((found_new++))
            fi
        done < <(find "$DIAGNOSTIC_REPORTS" -name "${pattern}*.ips" -print0 2>/dev/null)

        # Find .crash files (legacy)
        while IFS= read -r -d '' crash_file; do
            local file_time=$(stat -f "%m" "$crash_file" 2>/dev/null || stat -c "%Y" "$crash_file")
            if [[ $file_time -gt $check_time ]]; then
                process_crash "$crash_file" && ((found_new++))
            fi
        done < <(find "$DIAGNOSTIC_REPORTS" -name "${pattern}*.crash" -print0 2>/dev/null)
    done

    # Update last check time
    date +%s > "$LAST_CHECK_FILE"

    if [[ $found_new -gt 0 ]]; then
        echo ""
        echo "Found $found_new new crash report(s)"
        update_summary
        echo "Summary updated: $SUMMARY_FILE"

        # Also append to worker crashes.log for visibility
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Captured $found_new app crash report(s) - see $SUMMARY_FILE" >> "$WORKER_LOGS/crashes.log"

        # CRITICAL: Create a hint file to alert the worker about new crashes
        # This injects the crash info into the worker's next iteration
        create_crash_hint "$found_new"
    else
        echo "No new crash reports found."
    fi

    return 0
}

# Create a hint file for the worker about new crashes
create_crash_hint() {
    local crash_count="$1"
    local hint_file="$PROJECT_DIR/HINT.txt"

    # Get crashes that haven't been notified yet
    touch "$NOTIFIED_CRASHES_FILE"
    local new_crashes=()
    for crash in $(ls -t "$CRASH_DIR"/*.txt 2>/dev/null | head -10); do
        local crash_name=$(basename "$crash")
        if ! grep -q "^${crash_name}$" "$NOTIFIED_CRASHES_FILE" 2>/dev/null; then
            new_crashes+=("$crash")
        fi
    done

    # If all crashes have been notified already, skip
    if [[ ${#new_crashes[@]} -eq 0 ]]; then
        echo "All crashes have already been notified to worker"
        return
    fi

    local unnotified_count=${#new_crashes[@]}
    echo "Found $unnotified_count crash(es) not yet notified to worker"

    # Get the most recent unnotified crash for context
    local latest_crash="${new_crashes[0]}"
    if [[ -z "$latest_crash" ]]; then
        return
    fi

    # Extract key info from the latest crash
    local crash_function=$(grep -A30 "CRASHED THREAD" "$latest_crash" 2>/dev/null | grep -E "^\s+[0-9]+" | head -5 | awk '{print $NF}' | grep -v "^0x" | head -1 || echo "unknown")
    local crash_signal=$(grep "Signal:" "$latest_crash" 2>/dev/null | head -1 | awk '{print $2}' || echo "unknown")
    local crash_file=$(basename "$latest_crash" .txt)

    # If there's already a hint, append to it
    if [[ -f "$hint_file" ]]; then
        echo "" >> "$hint_file"
        echo "ALSO: $unnotified_count new app crash(es) detected!" >> "$hint_file"
        echo "Latest: $crash_file (Signal: $crash_signal, Function: $crash_function)" >> "$hint_file"
        echo "Run: cat worker_logs/app_crashes.summary" >> "$hint_file"
    else
        # Create the hint
        cat > "$hint_file" << HINT_EOF
🚨 APP CRASH DETECTED - FIX REQUIRED 🚨

$unnotified_count new crash report(s) captured. The app crashed during testing.

**Latest Crash:** $crash_file
**Signal:** $crash_signal
**Crashing Function:** $crash_function

**Your task:**
1. Read the crash summary: cat worker_logs/app_crashes.summary
2. Read the specific crash: cat worker_logs/app_crashes/${crash_file}
3. Look at the CRASHED THREAD BACKTRACE to find the failing function
4. Identify the production code that crashed (NOT the test code)
5. Fix the bug in the production code
6. Run the test to verify the fix

This is a HIGH PRIORITY issue - crashes indicate real bugs that need fixing.
HINT_EOF
    fi

    # Mark these crashes as notified
    for crash in "${new_crashes[@]}"; do
        basename "$crash" >> "$NOTIFIED_CRASHES_FILE"
    done

    echo "Created/updated HINT.txt to alert worker about $unnotified_count crash(es)"
}

# Watch mode - continuous monitoring
watch_crashes() {
    echo "Starting crash report monitor..."
    echo "Press Ctrl+C to stop"
    echo ""

    while true; do
        check_crashes
        sleep 30  # Check every 30 seconds
    done
}

# Run based on mode
case $MODE in
    check)
        check_crashes
        ;;
    watch)
        watch_crashes
        ;;
esac
