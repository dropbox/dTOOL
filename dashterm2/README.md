<div align="center">

# DashTerm2

| Director | Status |
|:--------:|:------:|
| TOOL | ACTIVE |

### iTerm2, rebuilt for performance and stability.

**Rust core. Swift UI. Zero compromises.**

Based on [iTerm2](https://iterm2.com) by **[George Nachman](https://github.com/gnachman)**

</div>

---

## What Is This?

DashTerm2 is a ground-up rebuild of iTerm2's core in Rust, keeping the beloved macOS UI while eliminating the crashes and memory issues that plague 24/7 workloads.

**We keep everything great about iTerm2:**
- tmux integration
- Shell integration
- Split panes & hotkey windows
- Triggers & session restoration
- Python API & profiles
- All the power features

**We rebuild the engine:**
- VT100/xterm parser → Rust (580 MiB/s throughput)
- Screen buffer → 8-byte cells (not 80+ bytes)
- Scrollback → tiered storage (RAM + disk)
- Renderer → wgpu GPU acceleration (in progress)

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   DashTerm2.app                      │
│  ┌───────────────────────────────────────────────┐  │
│  │              Swift/ObjC UI Layer              │  │
│  │   (PTYSession, PseudoTerminal, Metal views)   │  │
│  └───────────────────────────────────────────────┘  │
│                         │ FFI                        │
│  ┌───────────────────────────────────────────────┐  │
│  │              dterm-core (Rust)                │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────────────┐ │  │
│  │  │ Parser  │ │  Grid   │ │   Scrollback    │ │  │
│  │  │ (SIMD)  │ │(8-byte) │ │ (LZ4+zstd+mmap) │ │  │
│  │  └─────────┘ └─────────┘ └─────────────────┘ │  │
│  │  ┌─────────────────────────────────────────┐ │  │
│  │  │         GPU Renderer (wgpu)             │ │  │
│  │  │  GlyphAtlas │ VertexBuilder │ Shaders   │ │  │
│  │  └─────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

---

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| **dterm-core** | Complete | 1399 tests, 580 MiB/s parser |
| **FFI bridge** | Complete | 889 symbols exported |
| **Parser integration** | Complete | Runs in parallel with validation |
| **Grid/scrollback** | Complete | Reads from Rust |
| **GPU renderer** | In Progress | wgpu backend, Swift integration needed |
| **Bug triage** | Complete | 3,348 upstream issues reviewed |

### Performance (dterm-core)

| Metric | Value |
|--------|-------|
| ASCII throughput | 580 MiB/s |
| SGR throughput | 267 MiB/s |
| Memory (10K lines) | 0.45 MB |
| Cell size | 8 bytes (vs 80+ in iTerm2) |

---

## Building

```bash
git clone https://github.com/dropbox/dTOOL/dashterm2.git
cd dashterm2

# Build the app
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 \
    -configuration Development build \
    CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-"

# Run tests
xcodebuild test -project DashTerm2.xcodeproj -scheme DashTerm2Tests \
    -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```

### Building dterm-core

The Rust core lives in `~/dterm`:

```bash
cd ~/dterm
cargo build --release -p dterm-core --features "ffi,gpu"
cargo test -p dterm-core
```

---

## Roadmap

### Phase 1: Stability - COMPLETE
- Triage all 3,348 upstream iTerm2 issues
- Fix critical crashes and hangs
- Build regression test infrastructure

### Phase 2: Rust Core - COMPLETE
- Build dterm-core terminal emulation library
- VT100/xterm parser with SIMD acceleration
- Memory-efficient grid and scrollback
- FFI bridge for Swift/ObjC integration

### Phase 3: Deep Integration - IN PROGRESS
- ✅ Parser switchover (dterm-core primary)
- ✅ Terminal state migration (grid + scrollback)
- 🔄 GPU renderer (wgpu Swift integration)
- ⏳ Delete legacy ObjC Metal stack

### Phase 4: Swift Migration - PLANNED
- Convert ObjC modules to Swift (safer, cleaner)
- Priority: PTY handling, session management, preferences
- Target: Zero ObjC in 12-18 months

### Phase 5: Platform Expansion - FUTURE
- iOS/iPadOS (SwiftUI + dterm-core)
- Linux (GTK + dterm-core)
- Windows (WinUI + dterm-core)

---

## Documentation

| Document | Description |
|----------|-------------|
| [CLAUDE.md](./CLAUDE.md) | AI worker instructions |
| [docs/ROADMAP.md](./docs/ROADMAP.md) | Detailed roadmap |
| [docs/burn-list/README.md](./docs/burn-list/README.md) | Bug triage status |

---

## Philosophy

**DashTerm2 is a TTY terminal. Nothing more.**

We don't build:
- IDE features
- Chat interfaces
- File browsers
- Code editors

We do build:
- The fastest, most stable terminal
- Hooks for external systems to integrate

---

## Credits

Built on [iTerm2](https://iterm2.com) by **George Nachman**. George built something great. We're rebuilding its foundation in Rust.

- iTerm2: [github.com/gnachman/iTerm2](https://github.com/gnachman/iTerm2)
- dterm-core: Rust terminal emulation engine

---

## License

GPLv3, same as iTerm2.

---

<div align="center">

**iTerm2's UI. Rust's performance. Zero crashes.**

</div>
