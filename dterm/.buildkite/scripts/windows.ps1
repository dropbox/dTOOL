Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  Write-Error "rustup not found; install Rust toolchains on the agent."
}

rustup default stable

cargo build -p dterm-core --release
cargo build -p dterm-alacritty-bridge --release
cargo test -p dterm-core
cargo test -p dterm-alacritty-bridge
cargo test -p dterm-alacritty-bridge --test integration_test -- --ignored
