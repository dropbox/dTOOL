#!/bin/bash
# Build DashTerm2 and run the key test suites; suitable for CI or local smoke checks.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

LOG_DIR="${CI_LOG_DIR:-reports/ci}"
mkdir -p "$LOG_DIR"

XCODEBUILD_BIN="${XCODEBUILD:-xcodebuild}"
if ! command -v "$XCODEBUILD_BIN" >/dev/null 2>&1; then
  echo "error: xcodebuild not found on PATH" >&2
  exit 1
fi

DESTINATION="${CI_DESTINATION:-platform=macOS}"
CONFIGURATION="${CI_CONFIGURATION:-Debug}"

common_flags=(
  -project DashTerm2.xcodeproj
  CODE_SIGN_IDENTITY=
  CODE_SIGNING_REQUIRED=NO
  CODE_SIGNING_ALLOWED=NO
)

run_step() {
  local label="$1"
  shift
  local logfile="$LOG_DIR/${label}.log"
  echo ""
  echo "=== $label ==="
  echo "Log: $logfile"
  "$@" 2>&1 | tee "$logfile"
}

run_step build "$XCODEBUILD_BIN" "${common_flags[@]}" \
  -scheme DashTerm2 -configuration "$CONFIGURATION" build-for-testing

run_step tests-DashTerm2 "$XCODEBUILD_BIN" "${common_flags[@]}" \
  -scheme DashTerm2Tests -configuration "$CONFIGURATION" \
  -destination "$DESTINATION" test

run_step tests-Modern "$XCODEBUILD_BIN" "${common_flags[@]}" \
  -scheme ModernTests -configuration "$CONFIGURATION" \
  -destination "$DESTINATION" test

echo ""
echo "DashTerm2 CI run finished successfully."
