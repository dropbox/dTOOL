#!/bin/bash
# Capture RSS/VSZ for DashTerm2 (or another bundle) to establish memory baselines.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
mkdir -p "$RESULTS_DIR"

APP_NAME=${DT_APP_NAME:-DashTerm2}
WAIT_SECS=${DT_WAIT_SECS:-5}
SAMPLES=${DT_MEM_SAMPLES:-1}
SLEEP_BETWEEN=${DT_MEM_SLEEP:-1}
OUTPUT_FILE="$RESULTS_DIR/memory_latest.json"
BASELINE_FILE="$RESULTS_DIR/memory_baseline.json"
AUTO_QUIT=0
SKIP_LAUNCH=0
UPDATE_BASELINE=0

print_usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  --app-name <name>     Application bundle name (default: $APP_NAME)
  --wait <seconds>      Seconds to wait after launch (default: $WAIT_SECS)
  --samples <n>         Number of samples (default: $SAMPLES)
  --sleep <seconds>     Delay between samples (default: $SLEEP_BETWEEN)
  --output <file>       File name (inside results/) for JSON output
  --skip-launch         Assume app is already running (do not call open)
  --quit-after          Quit the app after sampling (uses osascript)
  --update-baseline     Copy latest JSON into memory_baseline.json
  -h, --help            Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app-name)
      shift; APP_NAME="$1" ;;
    --wait)
      shift; WAIT_SECS="$1" ;;
    --samples)
      shift; SAMPLES="$1" ;;
    --sleep)
      shift; SLEEP_BETWEEN="$1" ;;
    --output)
      shift; OUTPUT_FILE="$RESULTS_DIR/$1" ;;
    --skip-launch)
      SKIP_LAUNCH=1 ;;
    --quit-after)
      AUTO_QUIT=1 ;;
    --update-baseline)
      UPDATE_BASELINE=1 ;;
    -h|--help)
      print_usage; exit 0 ;;
    *)
      echo "error: unknown option $1" >&2
      print_usage >&2
      exit 1 ;;
  esac
  shift
done

launch_app() {
  if [[ "$SKIP_LAUNCH" -eq 1 ]]; then
    return
  fi
  if pgrep -fx "$APP_NAME" >/dev/null 2>&1; then
    return
  fi
  echo "Launching $APP_NAME..."
  open -a "$APP_NAME"
  sleep "$WAIT_SECS"
}

find_pid() {
  pgrep -fx "$APP_NAME" 2>/dev/null | head -1 || pgrep -f "$APP_NAME" 2>/dev/null | head -1 || true
}

collect_sample() {
  local pid="$1"
  local stats
  stats=$(ps -p "$pid" -o rss=,vsz= 2>/dev/null | tr -s ' ')
  [[ -n "$stats" ]] || return 1
  local rss_kb=$(echo "$stats" | awk '{print $1}')
  local vsz_kb=$(echo "$stats" | awk '{print $2}')
  echo "$rss_kb" "$vsz_kb"
}

launch_app

samples_json='[]'
for ((i=1; i<=SAMPLES; i++)); do
  pid=$(find_pid)
  if [[ -z "$pid" ]]; then
    echo "error: could not find PID for $APP_NAME" >&2
    exit 2
  fi
  if metrics=$(collect_sample "$pid"); then
    rss=$(echo "$metrics" | awk '{print $1}')
    vsz=$(echo "$metrics" | awk '{print $2}')
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    samples_json=$(SAMPLES_JSON="$samples_json" PID_VALUE="$pid" RSS_VALUE="$rss" VSZ_VALUE="$vsz" TIMESTAMP="$timestamp" python3 - <<'PY'
import json
import os

samples = json.loads(os.environ['SAMPLES_JSON'])
samples.append({
    "timestamp": os.environ['TIMESTAMP'],
    "pid": int(os.environ['PID_VALUE']),
    "rss_kb": int(os.environ['RSS_VALUE'] or 0),
    "vsz_kb": int(os.environ['VSZ_VALUE'] or 0)
})
print(json.dumps(samples))
PY
)
    echo "Sample $i: RSS=${rss}KB VSZ=${vsz}KB"
  else
    echo "warning: failed to read memory for PID $pid" >&2
  fi
  if (( i < SAMPLES )); then
    sleep "$SLEEP_BETWEEN"
  fi
 done

cat <<JSON > "$OUTPUT_FILE"
{
  "app": "$APP_NAME",
  "samples": $samples_json
}
JSON

echo "Saved memory samples to $OUTPUT_FILE"

if [[ "$AUTO_QUIT" -eq 1 ]]; then
  osascript -e "tell application \"$APP_NAME\" to quit" >/dev/null 2>&1 || true
fi

if [[ "$UPDATE_BASELINE" -eq 1 ]]; then
  cp "$OUTPUT_FILE" "$BASELINE_FILE"
  echo "Updated memory baseline -> $BASELINE_FILE"
fi
