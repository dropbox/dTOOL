#!/usr/bin/env bash
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found; install Rust toolchains on the agent." >&2
  exit 1
fi

rustup toolchain list | grep -q '^nightly' || {
  echo "nightly toolchain missing; install with: rustup toolchain install nightly" >&2
  exit 1
}
rustup component list --toolchain nightly --installed | grep -q '^miri' || {
  echo "miri component missing; install with: rustup component add miri --toolchain nightly" >&2
  exit 1
}

cargo +nightly miri setup
cargo +nightly miri test -p dterm-core parser:: -- --test-threads=1
cargo clippy -p dterm-core -p dterm-alacritty-bridge -- -D warnings
cargo fmt --all -- --check
