#!/usr/bin/env bash
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found; install Rust toolchains on the agent." >&2
  exit 1
fi

rustup default stable

cargo build -p dterm-core --release
cargo build -p dterm-alacritty-bridge --release
cargo test -p dterm-core
cargo test -p dterm-alacritty-bridge
cargo test -p dterm-alacritty-bridge --test integration_test
