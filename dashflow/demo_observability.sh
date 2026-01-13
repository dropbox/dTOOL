#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
demo_observability.sh

Starts the local DashFlow observability stack (Kafka + websocket-server + Jaeger, etc),
launches the observability UI dev server, and runs the traced_agent example to generate live data.

Usage:
  ./demo_observability.sh
  ./demo_observability.sh --down

Options:
  --down   Stop docker-compose.dashstream.yml on exit
EOF
}

STOP_STACK_ON_EXIT=0
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ "${1:-}" == "--down" ]]; then
  STOP_STACK_ON_EXIT=1
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

docker_compose() {
  if command -v docker-compose >/dev/null 2>&1; then
    docker-compose "$@"
  else
    docker compose "$@"
  fi
}

wait_for_url() {
  local url="$1"
  local name="$2"
  local retries="${3:-60}"
  local sleep_s="${4:-1}"

  for _ in $(seq 1 "$retries"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      echo "✓ $name ready: $url"
      return 0
    fi
    sleep "$sleep_s"
  done

  echo "✗ Timed out waiting for $name at $url" >&2
  return 1
}

ui_pid=""
cleanup() {
  if [[ -n "${ui_pid}" ]]; then
    kill "${ui_pid}" >/dev/null 2>&1 || true
  fi
  if [[ "$STOP_STACK_ON_EXIT" -eq 1 ]]; then
    docker_compose -f docker-compose.dashstream.yml down
  fi
}
trap cleanup EXIT

echo "==> Starting DashFlow DashStream stack (docker-compose.dashstream.yml)"
docker_compose -f docker-compose.dashstream.yml up -d

wait_for_url "http://localhost:3002/health" "websocket-server"
wait_for_url "http://localhost:16686" "jaeger"

echo "==> Starting observability UI (observability-ui)"
pushd observability-ui >/dev/null
if [[ ! -d node_modules ]]; then
  npm install
fi
npm run dev >/dev/null 2>&1 &
ui_pid="$!"
popd >/dev/null

wait_for_url "http://localhost:5173" "observability-ui"

echo "==> Running traced_agent example (emits DashStream + traces)"
cargo run -p dashflow --example traced_agent --features observability,dashstream

cat <<'EOF'

Open:
- UI:     http://localhost:5173
- Jaeger: http://localhost:16686

Tip:
- Re-run the generator any time:
  cargo run -p dashflow --example traced_agent --features observability,dashstream
EOF

wait "$ui_pid"

