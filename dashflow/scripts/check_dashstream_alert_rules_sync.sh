#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(git -C "${script_dir}/.." rev-parse --show-toplevel)"

canonical="${repo_root}/monitoring/alert_rules.yml"
k8s_copy="${repo_root}/deploy/kubernetes/base/configs/alert_rules.yml"

if [[ ! -f "${canonical}" ]]; then
  echo "Missing canonical alert rules: ${canonical}" >&2
  exit 1
fi

if [[ ! -f "${k8s_copy}" ]]; then
  echo "Missing Kubernetes alert rules copy: ${k8s_copy}" >&2
  exit 1
fi

if ! diff -u "${canonical}" "${k8s_copy}" >/dev/null; then
  echo "DashStream alert rule drift detected:" >&2
  echo "  canonical: ${canonical}" >&2
  echo "  k8s copy:  ${k8s_copy}" >&2
  echo >&2
  diff -u "${canonical}" "${k8s_copy}" >&2 || true
  echo >&2
  echo "Fix by syncing the Kubernetes copy to canonical (or make a single source of truth)." >&2
  exit 1
fi

echo "OK: DashStream alert rules are in sync."

if [[ "${1:-}" == "--promtool" ]]; then
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required for --promtool" >&2
    exit 1
  fi
  docker run --rm --entrypoint promtool -v "${canonical}:/rules.yml:ro" prom/prometheus:v2.49.1 check rules /rules.yml
fi
