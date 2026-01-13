# DashTerm: The Terminal for AI Agents

**License:** Apache 2.0
**Philosophy:** Secure + Clean = Fast + Secure
**Platforms:** macOS, Windows, Linux, iOS, iPadOS

---

## Vision

DashTerm is the terminal built for the AI agent era. Same power everywhere. Platform-native experience.

Traditional terminals are keyboard-centric tools for humans typing commands. DashTerm is an **agent control interface** that happens to include a terminal - optimized for natural language, voice, touch, and approval workflows, while maintaining full terminal power when needed.

The core insight from seL4: formally verified systems are also the fastest. Clean design eliminates complexity, and complexity is the enemy of both security AND performance.

---

## Core Principles

### 1. Same Power Everywhere

Every platform gets the same capabilities:
- Full terminal emulation
- Agent orchestration
- Multi-server connections
- Voice input/output
- Approval workflows
- Search, logging, notifications

### 2. Platform Superpowers

Each platform has unique strengths the agent can use:

| Desktop | Mobile |
|---------|--------|
| Full keyboard | Touch gestures |
| Big screen | Camera input |
| Local execution | Location awareness |
| File system | Always with you |
| Docker, builds | Background notifications |
|  | Cellular connectivity |

### 3. Efficiency by Design

Mobile constraints (battery, network) shape the architecture for ALL platforms:
- Push notifications, not polling
- Compressed deltas, not raw streams
- Render only changes
- Offline-first with sync
- Connect on demand

### 4. Correctness by Construction

- TLA+ specifications before code
- Kani verification for unsafe blocks
- Continuous fuzzing
- Type system prevents invalid states

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         PLATFORM UI LAYER                            │
│                    (Native, Platform-Optimized)                      │
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │  macOS   │  │ Windows  │  │  Linux   │  │   iOS    │            │
│  │ SwiftUI  │  │  WinUI   │  │   GTK    │  │ SwiftUI  │            │
│  │ Keyboard │  │ Keyboard │  │ Keyboard │  │  Touch   │            │
│  │  Mouse   │  │  Mouse   │  │  Mouse   │  │  Voice   │            │
│  │  Metal   │  │  DX12    │  │  Vulkan  │  │  Metal   │            │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ C FFI (minimal, safe)
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    DASHTERM-CORE (Rust, Apache 2.0)                  │
│                    TLA+ Specified • Kani Verified • Fuzz Tested      │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Terminal Engine                                                │ │
│  │  • State machine (TLA+ spec, modes always valid)               │ │
│  │  • Parser (Kani verified, handles any input)                   │ │
│  │  • Grid (ring buffer, O(1) scroll, bounded memory)             │ │
│  │  • Scrollback (hot/warm/cold, disk-backed, compressed)         │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Agent Orchestration                                            │ │
│  │  • Tool routing (local vs remote execution)                    │ │
│  │  • Approval workflow (request → approve/reject)                │ │
│  │  • Multi-server management                                      │ │
│  │  • Conversation state                                           │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Platform Abstraction                                           │ │
│  │  • PTY trait (Unix, ConPTY, Remote)                            │ │
│  │  • Capabilities discovery (camera, location, etc.)             │ │
│  │  • Voice I/O trait (platform STT/TTS)                          │ │
│  │  • Notifications trait                                          │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Efficiency Layer                                               │ │
│  │  • Delta compression (only send changes)                       │ │
│  │  • Offline queue (sync when connected)                         │ │
│  │  • Power states (active/background/suspended)                  │ │
│  │  • Render commands (abstract, not pixels)                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       PLATFORM ADAPTERS                              │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   Unix PTY   │  │    ConPTY    │  │     Remote/SSH           │  │
│  │ macOS/Linux  │  │   Windows    │  │  All platforms (iOS)     │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Input Modalities

### Keyboard (Desktop Primary)

Full keyboard when available. All traditional terminal power:
- Ctrl+C, Ctrl+D, Ctrl+Z
- Tab completion
- Arrow keys, readline
- Keyboard shortcuts

### Voice (All Platforms)

```
User speaks → Local STT → Agent intent → Execution → TTS response

Platform APIs:
• macOS/iOS: Speech framework
• Windows: Windows.Media.SpeechRecognition
• Linux: Vosk, Whisper.cpp
```

Use cases:
- "Run the tests"
- "What's the error?"
- "Read me the logs" (while driving)
- "Stop" → Ctrl+C

### Touch (Mobile Primary)

Touch is a superpower, not a limitation:

| Gesture | Action |
|---------|--------|
| **Circle region** | "Explain this" |
| **Swipe left/right** | Switch sessions |
| **Pinch** | Zoom text |
| **Long press** | Context menu |
| **Double tap** | Select word |
| **Draw arrow** | "This caused this" |
| **Drag timeline** | Scrub through history |
| **Drag divider** | Resize split panes |

### Camera (Mobile)

Agent tools that use camera:
- OCR error messages from photos
- "What's wrong with this?" (photo of hardware)
- Scan QR codes for connection info
- Document scanning

### Location (Mobile)

- "When I arrive at office, show prod status"
- Location-aware server selection
- Geofenced notifications

---

## Agent Interaction Model

Traditional terminal:
```
Human → [types exact commands] → Shell → Output
```

Agent terminal:
```
Human → [natural language] → Agent → [generates commands] → Shell → Output
              ↑                              ↓
              └──── [approval if needed] ────┘
```

### Approval Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  Agent wants to: rm -rf node_modules/                       │
│                                                              │
│  [Approve]  [Reject]  [Edit]  [Ask Why]                    │
└─────────────────────────────────────────────────────────────┘
```

### Multi-Server

One interface, multiple machines:

```
┌─────────────────────────────────────────────────────────────┐
│  Sessions                                                    │
├─────────────────────────────────────────────────────────────┤
│  🟢 Mac Mini (home)      Agent fixing tests                 │
│  🟢 EC2 prod             Monitoring                         │
│  🟡 Work laptop          Idle                               │
│  📱 Local (iPhone)       Ready                              │
└─────────────────────────────────────────────────────────────┘
```

Agent routes execution to appropriate server:
- "Run tests" → routes to machine with repo
- "Deploy to prod" → routes to CI/CD server
- "Read this file" → routes to machine with file (or local)

---

## Efficiency Design

### Power States

```rust
enum PowerState {
    Active,     // Full rendering, live connection
    Background, // No rendering, maintain connection
    Suspended,  // Push notifications only
    Offline,    // Cache only, queue operations
}
```

### Network Protocol

Instead of raw SSH byte stream:

```json
{
  "type": "delta",
  "changes": [
    {"line": 42, "range": [10, 50], "text": "...", "compressed": true}
  ],
  "cursor": [42, 15]
}
```

90% less data for typical use.

### Offline-First

No connection? Still works:
- Browse cached scrollback
- Search history
- Queue commands
- Compose messages
- View last known state

Sync when connected.

### Battery Optimization

| Instead of | Do this |
|------------|---------|
| Poll for updates | Push notifications (APNs/FCM) |
| Keep connection open | Connect on demand |
| Full screen redraws | Delta render changed cells |
| Background processing | System background APIs |
| Always-on connection | Disconnect when idle, push notify |

---

## Memory Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  HOT (RAM)         Last 10,000 lines         ~5 MB         │
├─────────────────────────────────────────────────────────────┤
│  WARM (mmap)       Recent scrollback         OS paged      │
├─────────────────────────────────────────────────────────────┤
│  COLD (disk)       Old scrollback            Compressed    │
└─────────────────────────────────────────────────────────────┘

Result: Millions of lines, constant ~50MB RAM
```

---

## Verification Strategy

| Component | Method | Tool | When |
|-----------|--------|------|------|
| State machine | Specification | TLA+ | Design time |
| Parser | Bounded model checking | Kani | Before merge |
| Unsafe code | UB detection | MIRI | Every commit |
| Parser | Fuzzing | cargo-fuzz | 24/7 continuous |
| Invariants | Property testing | proptest | CI |
| Compatibility | Conformance | esctest2 | Release |

### Correctness by Construction

```rust
// Invalid states are unrepresentable
struct GridCoord {
    x: BoundedU16<0, MAX_COLS>,
    y: BoundedU16<0, MAX_ROWS>,
}

// This function CANNOT receive invalid coordinates
fn get_cell(&self, coord: GridCoord) -> &Cell {
    &self.cells[coord.index()]  // Always safe
}
```

---

## Platform Capabilities

```rust
trait PlatformCapabilities {
    // Core (all platforms)
    fn execute_shell(&self, cmd: &str) -> Result<Output>;
    fn read_file(&self, path: &Path) -> Result<Vec<u8>>;
    fn write_file(&self, path: &Path, data: &[u8]) -> Result<()>;
    fn speak(&self, text: &str) -> Result<()>;
    fn listen(&self) -> Result<String>;
    fn notify(&self, msg: &str) -> Result<()>;

    // Desktop
    fn local_docker(&self) -> Option<&dyn Docker>;
    fn local_build(&self) -> Option<&dyn BuildSystem>;

    // Mobile
    fn camera(&self) -> Option<&dyn Camera>;
    fn location(&self) -> Option<&dyn Location>;
    fn haptics(&self) -> Option<&dyn Haptics>;
}
```

Agent discovers capabilities and adapts:
```
"I see you have camera access. Want me to
 analyze that error screenshot?"
```

---

## Crate Structure

```
dashterm/
├── dashterm-core/           # Pure Rust, no platform deps
│   ├── src/
│   │   ├── terminal/        # State machine, parser, grid
│   │   ├── agent/           # Orchestration, routing, approval
│   │   ├── efficiency/      # Delta, compression, offline
│   │   └── render/          # Abstract render commands
│   ├── tla/                 # TLA+ specifications
│   └── fuzz/                # Fuzz targets
│
├── dashterm-pty/            # Platform PTY abstraction
│   ├── unix.rs              # macOS/Linux
│   ├── windows.rs           # ConPTY
│   └── remote.rs            # SSH/network
│
├── dashterm-voice/          # STT/TTS abstraction
│   ├── apple.rs             # Speech framework
│   ├── windows.rs           # Windows Speech
│   └── linux.rs             # Vosk/Whisper
│
├── dashterm-platform/       # Platform-specific UI
│   ├── macos/               # SwiftUI + AppKit
│   ├── windows/             # WinUI
│   ├── linux/               # GTK
│   └── ios/                 # SwiftUI (touch-first)
│
└── dashterm-protocol/       # Wire protocol
    ├── delta.rs             # Delta compression
    ├── sync.rs              # Offline sync
    └── push.rs              # Push notifications
```

---

## Success Metrics

### Performance

| Metric | Target |
|--------|--------|
| Crash rate | **0** per month |
| Memory (1M lines) | <100 MB |
| Memory (30 days) | <500 MB |
| Parser vulnerabilities | **0** |
| Input-to-screen latency | <5 ms |
| Tab switch | <16 ms |
| Search 1M lines | <100 ms |

### Mobile Efficiency

| Metric | Target |
|--------|--------|
| Idle battery drain | <1% per hour |
| Background battery | <0.1% per hour |
| Data per hour (active) | <1 MB |
| Offline functionality | Full read, queued write |
| Time to connect | <500 ms |

### User Experience

| Metric | Target |
|--------|--------|
| Voice recognition accuracy | >95% |
| Touch gesture recognition | >99% |
| Approval workflow taps | 1 tap |
| Settings for good defaults | 0 |

---

## Implementation Phases

### Phase 1: Core Engine
- [ ] TLA+ spec for terminal state machine
- [ ] Rust workspace scaffold
- [ ] Parser with Kani proofs
- [ ] OSS-Fuzz setup
- [ ] Grid with memory architecture
- [ ] Delta compression protocol

### Phase 2: macOS App
- [ ] Swift FFI bindings
- [ ] SwiftUI terminal view
- [ ] Metal renderer
- [ ] Voice I/O integration
- [ ] PTY integration

### Phase 3: Agent Layer
- [ ] Tool routing (local/remote)
- [ ] Approval workflow
- [ ] Multi-server management
- [ ] Conversation state

### Phase 4: Cross-Platform
- [ ] Linux (GTK + Vulkan)
- [ ] Windows (WinUI + ConPTY + DX12)
- [ ] Unified wgpu renderer option

### Phase 5: Mobile
- [ ] iOS app (SwiftUI, touch-first)
- [ ] iPadOS (keyboard + touch)
- [ ] Camera, location integration
- [ ] Background/push notifications
- [ ] Offline sync

### Phase 6: Features
- [ ] Shell integration (OSC 133)
- [ ] Tmux integration
- [ ] Image protocols (Sixel, Kitty)
- [ ] Triggers
- [ ] Indexed search

---

## What We Build vs Use

### Build (Apache 2.0)

| Component | Reason |
|-----------|--------|
| Terminal state machine | Core correctness, must verify |
| Grid/scrollback | Memory architecture is key innovation |
| Agent orchestration | Novel multi-server + approval model |
| Delta protocol | Efficiency is core differentiator |
| Touch gesture system | Terminal-native touch interactions |

### Use (MIT/Apache Compatible)

| Crate | Purpose |
|-------|---------|
| `vte` | Escape sequence parsing foundation |
| `portable-pty` | PTY abstraction reference |
| `wgpu` | Cross-platform GPU (optional) |
| `harfbuzz` | Text shaping |

---

## Research Foundation

This architecture is informed by analysis of:

**Terminal Emulators:**
- Alacritty (VTE parser, GPU rendering, minimal design)
- WezTerm (cross-platform Rust, Lua config, multiplexer)
- Kitty (threading model, SIMD parsing, graphics protocol)
- Warp (block-based output, AI integration)
- foot (daemon mode, Wayland-native)
- xterm (VT sequence reference)
- iTerm2 (shell integration, tmux, features)
- Terminal.app (macOS integration, limitations)
- Windows Terminal (ConPTY, DirectWrite)

**Formal Verification:**
- seL4 (verified microkernel - also fastest)
- CompCert (verified C compiler)
- Amazon TLA+ usage (S3, DynamoDB)
- Kani, MIRI, Prusti for Rust

**Protocols:**
- ECMA-48, VT100-VT520, xterm extensions
- OSC 133 (shell integration)
- Kitty keyboard/graphics protocols
- Tmux control mode

---

## License

Apache 2.0

- Permissive
- Patent protection
- iOS App Store compatible
- No GPL code
- Clean room implementation
