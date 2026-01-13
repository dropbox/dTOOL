# DashTerm

| Director | Status |
|:--------:|:------:|
| TOOL | ACTIVE |

A native macOS terminal application with AI agent computation graph visualization.
Created by Andrew Yates.

![macOS](https://img.shields.io/badge/macOS-14%2B-blue)
![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)
![Swift](https://img.shields.io/badge/Swift-5.9%2B-red)

## Features

- **Native Terminal Emulator**: Full-featured terminal with VTE-based ANSI escape sequence support
- **Computation Graph Visualization**: Interactive, real-time visualization of AI agent execution graphs (LangGraph-style)
- **Metal-Accelerated Rendering**: 60fps graph visualization with smooth animations
- **Core Text Terminal**: High-performance text rendering using Core Text

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    DashTerm (macOS App)                 │
├─────────────────────┬───────────────────────────────────┤
│   Terminal View     │       Graph Panel                 │
│   (AppKit/CoreText) │       (Metal/SwiftUI)             │
├─────────────────────┴───────────────────────────────────┤
│                   Swift FFI Bridge                       │
├─────────────────────────────────────────────────────────┤
│                   dashterm-core (Rust)                   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌───────────┐ │
│  │ dashterm-       │ │ dashterm-       │ │ dashterm- │ │
│  │ terminal        │ │ graph           │ │ ffi       │ │
│  │ (VTE parser)    │ │ (computation)   │ │ (C API)   │ │
│  └─────────────────┘ └─────────────────┘ └───────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Requirements

- macOS 14.0+ (Sonoma)
- Xcode 15+
- Rust 1.70+

## Quick Start

```bash
# Clone the repository
git clone https://github.com/dropbox/dTOOL/dashterm.git
cd dashterm

# Set up development environment
make setup

# Build the Rust library
make rust-build

# Open in Xcode and build the app
# (See Xcode Setup below)
```

## Xcode Setup

After running `make setup`, create a new Xcode project:

1. **Create Project**: File → New → Project → macOS → App
   - Product Name: `DashTerm`
   - Interface: SwiftUI
   - Language: Swift
   - Minimum Deployment: macOS 14.0

2. **Configure Build Settings**:
   - Swift Compiler → Objective-C Bridging Header: `$(PROJECT_DIR)/DashTerm/Bridge/DashTerm-Bridging-Header.h`
   - Header Search Paths: `$(PROJECT_DIR)/DashTerm/Bridge`
   - Library Search Paths: `$(PROJECT_DIR)/DashTerm/Bridge`

3. **Link Libraries** (Build Phases → Link Binary With Libraries):
   - Add `libdashterm_ffi.a`
   - Add `libresolv.tbd`

4. **Add Source Files**: Drag all files from `DashTerm/` folder into the project

## Build Commands

```bash
make setup          # Initial setup (install Rust deps, build library)
make build          # Build everything (debug)
make build-release  # Build for release
make test           # Run all tests
make lint           # Run linters
make fmt            # Format code
make run            # Build and run the app
```

## Project Structure

```
dashterm/
├── DashTerm/                    # Swift/SwiftUI macOS app
│   ├── DashTermApp.swift        # App entry point
│   ├── Views/                   # UI views
│   │   ├── ContentView.swift    # Main layout
│   │   ├── TerminalView.swift   # Terminal emulator
│   │   ├── GraphPanelView.swift # Graph visualization
│   │   └── SettingsView.swift   # Preferences
│   ├── Models/                  # Data models
│   ├── Bridge/                  # Swift-Rust FFI
│   └── Metal/                   # GPU shaders
├── dashterm-core/               # Rust workspace
│   ├── dashterm-terminal/       # Terminal emulation
│   ├── dashterm-graph/          # Computation graph engine
│   └── dashterm-ffi/            # C FFI bindings
├── scripts/                     # Build scripts
└── Makefile
```

## Roadmap

### Phase 1: Terminal Foundation (Current)
- [x] Project structure
- [x] Rust terminal emulator core
- [x] Swift FFI bridge
- [x] Basic terminal rendering
- [ ] PTY integration
- [ ] Full ANSI escape sequence support
- [ ] Scrollback buffer

### Phase 2: Graph Visualization
- [x] Graph data model
- [x] Metal shader infrastructure
- [ ] Node rendering
- [ ] Edge rendering with animation
- [ ] Interactive pan/zoom
- [ ] Real-time status updates

### Phase 3: Integration
- [ ] Agent output parsing
- [ ] Live graph updates from terminal
- [ ] Split view (terminal + graph)

### Phase 4: Polish
- [ ] Themes/color schemes
- [ ] Keyboard shortcuts
- [ ] Performance optimization
- [ ] Distribution packaging

## License

Apache-2.0
