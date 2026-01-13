#!/bin/bash
# Compare the most recent throughput run against the saved baseline JSON.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
BASELINE_FILE="$RESULTS_DIR/throughput_baseline.json"
LATEST_FILE="$RESULTS_DIR/throughput_latest.json"
THRESHOLD=${THROUGHPUT_REGRESSION_THRESHOLD:-0.05}
AUTO_RUN=0

print_usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  --baseline <file>    Override baseline JSON path (default: throughput_baseline.json)
  --latest <file>      Override latest JSON path (default: throughput_latest.json)
  --threshold <ratio>  Allowable slowdown before failing (default: $THRESHOLD)
  --auto-run           Run throughput.sh before comparing
  -h, --help           Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline)
      shift; BASELINE_FILE="$1" ;;
    --latest)
      shift; LATEST_FILE="$1" ;;
    --threshold)
      shift; THRESHOLD="$1" ;;
    --auto-run)
      AUTO_RUN=1 ;;
    -h|--help)
      print_usage; exit 0 ;;
    *)
      echo "error: unknown option $1" >&2
      print_usage >&2
      exit 1 ;;
  esac
  shift
done

if [[ "$AUTO_RUN" -eq 1 ]]; then
  "$SCRIPT_DIR/throughput.sh"
fi

if [[ ! -f "$BASELINE_FILE" ]]; then
  echo "error: baseline file not found at $BASELINE_FILE" >&2
  exit 2
fi
if [[ ! -f "$LATEST_FILE" ]]; then
  echo "error: latest throughput file not found at $LATEST_FILE" >&2
  exit 3
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required for comparison (brew install jq)" >&2
  exit 4
fi

BASELINE=$(jq '.results[0].mean' "$BASELINE_FILE")
LATEST=$(jq '.results[0].mean' "$LATEST_FILE")

python3 - "$BASELINE" "$LATEST" "$THRESHOLD" <<'PY'
import sys
baseline = float(sys.argv[1])
latest = float(sys.argv[2])
threshold = float(sys.argv[3])
limit = baseline * (1 + threshold)
print(f"Baseline mean: {baseline:.6f}s")
print(f"Latest mean:   {latest:.6f}s")
print(f"Allowed mean:  {limit:.6f}s (threshold {threshold*100:.1f}% slowdown)")
if latest > limit:
    print("REGRESSION: latest throughput is slower than allowed threshold")
    sys.exit(1)
print("✓ Throughput within threshold")
PY
