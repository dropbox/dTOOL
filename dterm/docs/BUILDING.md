# Building dterm-core

Platform-specific build instructions for dterm-core and dterm-alacritty-bridge.

---

## Prerequisites (All Platforms)

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Git

---

## macOS

### Requirements
- Xcode Command Line Tools: `xcode-select --install`
- No additional dependencies needed

### Build
```bash
cargo build -p dterm-core --release
cargo build -p dterm-alacritty-bridge --release
```

### Test
```bash
cargo test -p dterm-core
cargo test -p dterm-alacritty-bridge
```

### With Speech Features
```bash
cargo build -p dterm-core --release --features macos-speech
```

---

## Windows

### Requirements
- Windows 10 version 1809+ (build 17763) for ConPTY support
- Visual Studio Build Tools 2019+ with:
  - "Desktop development with C++" workload
  - Windows 10/11 SDK
- Rust with `x86_64-pc-windows-msvc` target (default on Windows)

### Install Build Tools
1. Download [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
2. Run installer and select "Desktop development with C++"
3. Ensure Windows SDK is checked

### Build
```powershell
cargo build -p dterm-core --release
cargo build -p dterm-alacritty-bridge --release
```

### Test
```powershell
# Unit tests
cargo test -p dterm-core
cargo test -p dterm-alacritty-bridge

# ConPTY integration tests (requires terminal allocation)
cargo test -p dterm-alacritty-bridge --test integration_test -- --ignored
```

### With Speech Features
```powershell
cargo build -p dterm-core --release --features windows-speech
```

### Troubleshooting

**"zstd-sys" build fails:**
- Ensure Windows SDK is installed
- Try: `set ZSTD_SYS_USE_PKG_CONFIG=0`

**ConPTY tests fail:**
- Verify Windows version: `winver` should show 1809 or later
- Run as Administrator if permission issues occur

---

## Linux

### Requirements (Debian/Ubuntu)
```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libasound2-dev  # For linux-speech feature
```

### Requirements (Fedora/RHEL)
```bash
sudo dnf install -y \
    gcc \
    pkg-config \
    openssl-devel \
    alsa-lib-devel  # For linux-speech feature
```

### Requirements (Arch Linux)
```bash
sudo pacman -S \
    base-devel \
    pkg-config \
    openssl \
    alsa-lib  # For linux-speech feature
```

### Build
```bash
cargo build -p dterm-core --release
cargo build -p dterm-alacritty-bridge --release
```

### Test
```bash
cargo test -p dterm-core
cargo test -p dterm-alacritty-bridge

# PTY integration tests
cargo test -p dterm-alacritty-bridge --test integration_test
```

### With Speech Features
```bash
# Requires espeak-ng for TTS, Vosk for STT
sudo apt-get install -y espeak-ng libespeak-ng-dev

cargo build -p dterm-core --release --features linux-speech
```

### Troubleshooting

**"zstd-sys" build fails:**
```bash
sudo apt-get install -y libzstd-dev
export ZSTD_SYS_USE_PKG_CONFIG=1
```

**PTY tests timeout:**
- Ensure `/dev/ptmx` exists and is readable
- Check if running in a container without PTY support

---

## Cross-Compilation

Cross-compilation from macOS to Windows/Linux is **not recommended** due to native C library dependencies (`zstd-sys`).

For CI/CD, use native runners:
- GitHub Actions: `windows-latest`, `ubuntu-latest`
- Docker containers for reproducible Linux builds

---

## Feature Flags

| Feature | Description | Platforms |
|---------|-------------|-----------|
| `ffi` | C FFI exports for Swift/ObjC integration | All |
| `gpu` | GPU rendering pipeline (wgpu) | All |
| `macos-speech` | Native TTS/STT via AVFoundation | macOS, iOS |
| `ios-speech` | Native TTS/STT via AVFoundation | iOS |
| `windows-speech` | Native TTS/STT via WinRT | Windows |
| `linux-speech` | Native TTS/STT via espeak-ng/Vosk | Linux |

### Build with All Features
```bash
# macOS
cargo build -p dterm-core --release --features ffi,gpu,macos-speech

# Windows
cargo build -p dterm-core --release --features ffi,gpu,windows-speech

# Linux
cargo build -p dterm-core --release --features ffi,gpu,linux-speech
```

---

## Verification

### Run All Checks
```bash
# Tests
cargo test --workspace

# Clippy
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --all -- --check

# MIRI (nightly required, parser tests only)
cargo +nightly miri test -p dterm-core parser::
```

---

## CI Status

The project uses GitHub Actions for continuous integration:
- **macOS**: Full test suite + clippy
- **Windows**: Full test suite + ConPTY integration tests
- **Linux**: Full test suite + PTY integration tests + MIRI verification

See `.github/workflows/ci.yml` for details.
