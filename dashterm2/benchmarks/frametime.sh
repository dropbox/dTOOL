#!/bin/bash
# Collect a Metal System Trace to inspect DashTerm2 frame times.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
TRACE_DIR="$RESULTS_DIR/traces"
mkdir -p "$TRACE_DIR"

APP_NAME=${DT_APP_NAME:-DashTerm2}
TEMPLATE=${DT_TRACE_TEMPLATE:-"Metal System Trace"}
DURATION=${DT_TRACE_DURATION:-15}
OPEN_TRACE=0

print_usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  --app-name <name>      Application bundle to trace (default: $APP_NAME)
  --template <template>  xctrace template (default: $TEMPLATE)
  --duration <seconds>   Time limit passed to xctrace (default: $DURATION)
  --open                 Open the resulting trace in Instruments when done
  -h, --help             Show this help

The script looks for a running instance of the app and records a Metal System
Trace focused on frame pacing. Attach DashTerm2 to a heavy workload beforehand
(for example by running yes | head -1000000).
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app-name)
      shift; APP_NAME="$1" ;;
    --template)
      shift; TEMPLATE="$1" ;;
    --duration)
      shift; DURATION="$1" ;;
    --open)
      OPEN_TRACE=1 ;;
    -h|--help)
      print_usage; exit 0 ;;
    *)
      echo "error: unknown option $1" >&2
      print_usage >&2
      exit 1 ;;
  esac
  shift
done

if ! command -v xcrun >/dev/null 2>&1; then
  echo "error: xcrun not found; install Xcode command line tools" >&2
  exit 2
fi

pid=$(pgrep -fx "$APP_NAME" 2>/dev/null | head -1 || true)
if [[ -z "$pid" ]]; then
  pid=$(pgrep -f "$APP_NAME" 2>/dev/null | head -1 || true)
fi
if [[ -z "$pid" ]]; then
  echo "error: could not find a running process for $APP_NAME" >&2
  exit 3
fi

echo "Recording $TEMPLATE for PID $pid ($APP_NAME) for ${DURATION}s..."
TRACE_NAME="frametime_$(date +%Y%m%d_%H%M%S)"
TRACE_PATH="$TRACE_DIR/${TRACE_NAME}.trace"

set +e
xcrun xctrace record \
  --template "$TEMPLATE" \
  --output "$TRACE_PATH" \
  --time-limit "${DURATION}s" \
  --attach "$pid"
status=$?
set -e

if [[ $status -ne 0 ]]; then
  echo "error: xctrace failed with status $status" >&2
  exit $status
fi

echo "Trace saved to $TRACE_PATH"

echo "Open the trace in Instruments and inspect 'Frame Time' under the Graphics
Statistics track to read per-frame durations."

if [[ "$OPEN_TRACE" -eq 1 ]]; then
  open "$TRACE_PATH"
fi
