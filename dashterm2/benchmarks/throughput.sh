#!/bin/bash
# Measure shell-driven throughput workloads with hyperfine and store JSON results.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
mkdir -p "$RESULTS_DIR"

CMD='echo "Testing in DashTerm2" && yes | head -500000 > /dev/null'
RUNS=${HYPERFINE_RUNS:-10}
WARMUPS=${HYPERFINE_WARMUPS:-3}
UPDATE_BASELINE=0
BASELINE_FILE="$RESULTS_DIR/throughput_baseline.json"
OUTPUT_FILE="$RESULTS_DIR/throughput_latest.json"

print_usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  -c, --command <cmd>       Override benchmark command (default: $CMD)
  --runs <n>                Number of hyperfine samples (default: $RUNS)
  --warmups <n>             Number of warmup runs (default: $WARMUPS)
  --output <file>           File name (inside results/) for JSON output
  --update-baseline         Copy the latest results to throughput_baseline.json
  -h, --help                Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -c|--command)
      shift
      [[ $# -gt 0 ]] || { echo "error: missing command" >&2; exit 1; }
      CMD="$1"
      ;;
    --runs)
      shift
      RUNS="$1"
      ;;
    --warmups)
      shift
      WARMUPS="$1"
      ;;
    --output)
      shift
      [[ $# -gt 0 ]] || { echo "error: missing output name" >&2; exit 1; }
      OUTPUT_FILE="$RESULTS_DIR/$1"
      ;;
    --update-baseline)
      UPDATE_BASELINE=1
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      echo "error: unknown option $1" >&2
      print_usage >&2
      exit 1
      ;;
  esac
  shift
done

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "error: hyperfine not found; install via 'brew install hyperfine'" >&2
  exit 2
fi

TMP_FILE="${OUTPUT_FILE}.tmp"
rm -f "$TMP_FILE"

hyperfine --warmup "$WARMUPS" --runs "$RUNS" --export-json "$TMP_FILE" "$CMD"

mv "$TMP_FILE" "$OUTPUT_FILE"

echo "Saved hyperfine results to $OUTPUT_FILE"

if command -v jq >/dev/null 2>&1; then
  jq '.results[] | {command, mean, stddev, min, max}' "$OUTPUT_FILE"
fi

if [[ "$UPDATE_BASELINE" -eq 1 ]]; then
  cp "$OUTPUT_FILE" "$BASELINE_FILE"
  echo "Updated throughput baseline -> $BASELINE_FILE"
fi
