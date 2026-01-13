# dTerm

| Director | Status |
|:--------:|:------:|
| TOOL | ACTIVE |

**The Terminal for AI Agents**

*Beautiful. Stable. Fast. Everywhere.*

---

## Vision

dTerm is a terminal built for the AI agent era. Same power everywhere. Platform-native experience.

Traditional terminals are keyboard-centric tools for humans typing commands. dTerm is an **agent control interface** that happens to include a terminal - optimized for natural language, voice, touch, and approval workflows, while maintaining full terminal power when needed.

**Core insight:** Formally verified systems are also the fastest. Clean design eliminates complexity, and complexity is the enemy of both security AND performance.

---

## Platforms

| Platform | Status | Input |
|----------|--------|-------|
| macOS | Planned | Keyboard, Mouse, Voice |
| Windows | Planned | Keyboard, Mouse, Voice |
| Linux | Planned | Keyboard, Mouse, Voice |
| iOS | Planned | Touch, Voice, Camera |
| iPadOS | Planned | Touch, Keyboard, Voice |

---

## Integration Strategy

dterm-core is a **pluggable terminal emulation engine** designed to integrate with existing open source terminal UIs:

| Platform | UI Target | Integration |
|----------|-----------|-------------|
| **macOS** | [iTerm2](https://github.com/gnachman/iTerm2) (dashterm2 fork) | Replace terminal core |
| **Windows** | [Alacritty](https://github.com/alacritty/alacritty) | Replace `alacritty_terminal` crate |
| **Linux** | [Alacritty](https://github.com/alacritty/alacritty) | Replace `alacritty_terminal` crate |
| **iOS/iPadOS** | [SwiftTerm](https://github.com/migueldeicaza/SwiftTerm) | Replace `Terminal.swift` via C FFI |

### Why This Approach?

1. **Leverage mature UIs** - These projects have years of polish on rendering, input handling, and platform integration
2. **Focus on the core** - dterm-core brings formal verification, SIMD parsing, and advanced features
3. **Faster time to market** - Ship on all platforms without building 4 separate UI layers
4. **Community benefit** - Improvements flow back to open source ecosystem

### Architecture Per Platform

```
┌─────────────────────────────────────────────────────────────────┐
│  macOS: iTerm2 UI          │  iOS: SwiftTerm UI                 │
│  (Obj-C/Swift, AppKit)     │  (Swift, UIKit/SwiftUI)            │
├────────────────────────────┴────────────────────────────────────┤
│                         C FFI Bridge                             │
├─────────────────────────────────────────────────────────────────┤
│                       dterm-core (Rust)                          │
│  SIMD Parser • Offset Pages • TLA+ Verified • OSC 133 Blocks    │
├────────────────────────────┬────────────────────────────────────┤
│  Windows: Alacritty UI     │  Linux: Alacritty UI               │
│  (Rust, ConPTY)            │  (Rust, X11/Wayland)               │
└────────────────────────────┴────────────────────────────────────┘
```

See `research/` for detailed analysis of each integration target:
- `research/alacritty_ANALYSIS.md` - Windows/Linux strategy
- `research/swiftterm_ANALYSIS.md` - iOS/iPadOS strategy
- `research/ANALYSIS.md` - macOS/iTerm2 strategy

---

## Features

### Same Power Everywhere

- Full VT100/ANSI terminal emulation
- Agent orchestration with approval workflows
- Multi-server connections (SSH, local, remote)
- Voice input/output on all platforms
- Automatic compressed logging
- Indexed search across scrollback

### Platform Superpowers

| Desktop | Mobile |
|---------|--------|
| Full keyboard | Touch gestures |
| Big screen | Camera input |
| Local execution | Location awareness |
| Docker, builds | Always with you |
| File system | Background notifications |

### Efficiency by Design

- Push notifications, not polling
- Compressed deltas, not raw streams
- Render only what changes
- Offline-first with sync
- <1% battery drain per hour idle

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    PLATFORM UI (Native)                          │
│  macOS/SwiftUI • Windows/WinUI • Linux/GTK • iOS/SwiftUI        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ C FFI
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DTERM-CORE (Rust)                             │
│                    TLA+ Specified • Kani Verified                │
│                                                                  │
│  • Terminal engine (parser, grid, scrollback)                   │
│  • Agent orchestration (routing, approval, multi-server)        │
│  • Efficiency layer (delta compression, offline sync)           │
│  • Platform abstraction (PTY, voice, notifications)             │
└─────────────────────────────────────────────────────────────────┘
```

---

## Verification

| Component | Method | Tool |
|-----------|--------|------|
| State machine | Specification | TLA+ |
| Parser | Bounded model checking | Kani |
| Unsafe code | UB detection | MIRI |
| Parser | Fuzzing | cargo-fuzz (24/7) |

**Target: Zero crashes. Zero vulnerabilities.**

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Crash rate | **0** per month |
| Memory (1M lines) | <100 MB |
| Parser vulnerabilities | **0** |
| Input-to-screen latency | <5 ms |
| Mobile idle battery | <1% per hour |

---

## Building

```bash
git clone https://github.com/dropbox/dTOOL/dterm.git
cd dterm
cargo build --release
```

---

## Project Structure

```
dterm/
├── dterm-core/        # Rust core (terminal, agent, efficiency)
├── dterm-pty/         # Platform PTY abstraction
├── dterm-voice/       # STT/TTS abstraction
├── dterm-protocol/    # Wire protocol (delta, sync, push)
├── dterm-platform/    # Platform-specific UI
│   ├── macos/
│   ├── windows/
│   ├── linux/
│   └── ios/
└── docs/
    └── architecture/
```

---

## License

Apache License 2.0

---

## Author

**Andrew Yates**

---

## Status

**Active Development** - Building the foundation.
