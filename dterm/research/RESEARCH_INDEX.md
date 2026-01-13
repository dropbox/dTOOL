# dterm Terminal Emulator Research Index

**Last Updated:** 2024-12-28
**Purpose:** Competitive analysis and pattern mining for dterm development

---

## Analysis Reports

| Terminal | Language | LOC | License | Focus | Report |
|----------|----------|-----|---------|-------|--------|
| **Alacritty** | Rust | ~33K | Apache 2.0 | Speed, simplicity | [alacritty_ANALYSIS.md](./alacritty_ANALYSIS.md) |
| **Kitty** | C/Python/Go | ~248K | GPL v3 | Graphics, SIMD | [kitty_ANALYSIS.md](./kitty_ANALYSIS.md) |
| **WezTerm** | Rust | ~410K | MIT | Cross-platform, Lua | [wezterm_ANALYSIS.md](./wezterm_ANALYSIS.md) |
| **Ghostty** | Zig/Swift | ~250K | MIT | Native UX, pages | [ghostty_ANALYSIS.md](./ghostty_ANALYSIS.md) |
| **Rio** | Rust | ~178K | MIT | wgpu, modern | [rio_ANALYSIS.md](./rio_ANALYSIS.md) |
| **Contour** | C++20 | ~80K | Apache 2.0 | VT520 compliance | [contour_ANALYSIS.md](./contour_ANALYSIS.md) |
| **Windows Terminal** | C++17/20 | ~300K | MIT | ConPTY, DirectWrite | [windows-terminal_ANALYSIS.md](./windows-terminal_ANALYSIS.md) |
| **foot** | C | ~15K | MIT | Latency, Wayland | [foot_ANALYSIS.md](./foot_ANALYSIS.md) |
| **Zellij** | Rust | ~80K | MIT | Sessions, plugins | [zellij_ANALYSIS.md](./zellij_ANALYSIS.md) |
| **SwiftTerm** | Swift | ~17K | MIT | iOS/macOS, pluggable | [swiftterm_ANALYSIS.md](./swiftterm_ANALYSIS.md) |
| **Terminal.app** | Obj-C | ~800K | Proprietary | macOS native | (reverse-engineered, no public source) |
| **iTerm2** | Obj-C/Swift | ~500K | GPL v2 | Features, shell integration | [ANALYSIS.md](./ANALYSIS.md) (dashterm2 fork analysis) |
| **Warp** | Rust (closed) | N/A | Proprietary | AI-native UX | [warp_ANALYSIS.md](./warp_ANALYSIS.md) |

---

## Key Patterns by Category

### Memory Architecture

| Terminal | Approach | Key Innovation |
|----------|----------|----------------|
| Ghostty | Offset-based pages | Serializable, mmap-able |
| Alacritty | Ring buffer | O(1) scroll, damage tracking |
| foot | Circular + lazy alloc | Power-of-2 for fast modulo |
| Windows Terminal | RLE attributes | Compression built-in |
| Zellij | Per-pane grid | Session serialization to KDL |

**Best for dterm:** Ghostty's offset-based pages + foot's lazy allocation

### Parser Implementation

| Terminal | Approach | SIMD | Throughput |
|----------|----------|------|------------|
| Alacritty | `vte` crate | No | ~400 MB/s |
| Kitty | Custom C | Yes | ~500 MB/s |
| Ghostty | Custom Zig | Yes (C++ helpers) | ~600 MB/s |
| foot | Custom C | No | ~400 MB/s |
| iTerm2 | Custom Obj-C | No | ~60 MB/s |

**Best for dterm:** Table-driven state machine + memchr for escape scanning

### Rendering

| Terminal | API | Key Technique |
|----------|-----|---------------|
| Alacritty | OpenGL | Glyph atlas, instanced |
| Ghostty | Metal/OpenGL | Direct Metal, triple buffer |
| foot | Pixman (CPU) | Cell-level damage, batched |
| WezTerm/Rio | wgpu | Cross-platform GPU |
| iTerm2 | Metal + CoreText | 2-level glyph cache |

**Best for dterm:** wgpu for cross-platform, dirty bitmap from foot

### Session Management

| Terminal | Persistence | Restoration |
|----------|-------------|-------------|
| Zellij | Socket files + KDL | Full resurrection |
| iTerm2 | Server mode | Process + scrollback |
| tmux | Socket + state | Full session |
| Ghostty | None | None |
| Alacritty | None | None |

**Best for dterm:** Zellij's KDL serialization + iTerm2's server mode

### Shell Integration

| Terminal | Protocol | Features |
|----------|----------|----------|
| iTerm2 | OSC 133 (inventor) | Marks, semantic history, triggers |
| Kitty | OSC 133 + custom | Graphics, keyboard protocol |
| Warp | Shell hooks | Block-based output |
| Terminal.app | OSC 133 | Bookmarks, marks |

**Best for dterm:** iTerm2's comprehensive OSC 133 + Warp's block model

### AI/Agent Integration

| Terminal | Approach | Features |
|----------|----------|----------|
| Warp | AI-native | Blocks, suggestions, explain |
| iTerm2 | None | (scriptable via Python) |
| Others | None | N/A |

**Best for dterm:** Warp's block model + approval workflows (novel)

---

## Patterns to Adopt

### From Ghostty
- Offset-based page references (enables serialization)
- Memory pooling with preheating
- Pin system for stable references across eviction
- Style deduplication (12x memory savings)
- Comptime state machine generation

### From foot
- Cell-level dirty tracking (finest granularity)
- Timer-based render batching (coalesce rapid updates)
- Frame callback sync (no wasted frames)
- Daemon mode (instant window spawn)
- Power-of-2 scrollback sizing

### From Zellij
- Session resurrection from KDL layouts
- Thread-per-component with message passing
- WASM plugin system with permissions
- Typed instruction enums for IPC

### From iTerm2
- OSC 133 shell integration (marks, semantic history)
- Triggers system (regex → action)
- Tmux control mode integration
- Profile inheritance with per-session overrides
- Smart selection rules

### From Warp
- Block-based output model (command+output as unit)
- AI command generation (# prefix)
- Error explanation on right-click
- Workflows (parameterized command templates)
- Modern input editor (not line buffer)

### From Terminal.app
- Dirty bitmap tracking
- Secure keyboard entry
- Bracketed paste validation
- Process tracking for tab titles
- Bookmarks system

### From Alacritty
- Ring buffer for hot scrollback
- Damage tracking for partial redraws
- Library/application separation
- FairMutex for thread synchronization

### From Windows Terminal
- ConPTY reference implementation
- RLE attribute compression
- JSON settings with profiles

---

## Patterns to Avoid

| Pattern | Source | Why Avoid |
|---------|--------|-----------|
| GPL license | Kitty, iTerm2 | Limits embedding |
| Python runtime | Kitty | Distribution complexity |
| 16-byte cell struct | iTerm2 | Cache inefficient |
| God objects (20K+ LOC files) | iTerm2, WezTerm | Unmaintainable |
| Per-character allocation | iTerm2 | Allocation overhead |
| Main thread everything | Terminal.app | Latency |
| No formal verification | All | dterm differentiator |
| CPU-only rendering | Terminal.app | Performance ceiling |
| Unbounded scrollback RAM | iTerm2 | OOM for huge sessions |

---

## Performance Baselines

### Throughput (M3 Max)

| Terminal | `cat 100MB` | Throughput |
|----------|-------------|------------|
| Alacritty | ~0.3s | ~300 MB/s |
| Kitty | ~0.5s | ~200 MB/s |
| foot | ~0.4s | ~250 MB/s |
| WezTerm | ~0.8s | ~125 MB/s |
| iTerm2 | ~2-4s | ~40 MB/s |

### Memory (Empty Window)

| Terminal | RSS |
|----------|-----|
| Alacritty | ~50 MB |
| foot | ~30 MB |
| Kitty | ~80 MB |
| WezTerm | ~100 MB |
| iTerm2 | ~200 MB |

### dterm Targets

| Metric | Target | Method |
|--------|--------|--------|
| Throughput | 400+ MB/s | SIMD + batching |
| Memory (empty) | <50 MB | Lazy allocation |
| Memory (1M lines) | <100 MB | Tiered storage |
| Search 1M lines | <10ms | Trigram index |
| Crash recovery | <1s | Checkpoints |

---

## dterm Differentiation Strategy

### What No Terminal Has

1. **Tiered memory** - Hot/warm/cold with disk backing
2. **Indexed search** - O(1) for huge sessions
3. **Formal verification** - TLA+ specs, Kani proofs
4. **Agent-native** - Approval workflows, not just AI assist
5. **Session persistence** - Crash recovery with scrollback
6. **Cross-platform mobile** - iOS/iPadOS as primary

### dterm = Best of All Worlds

```
Ghostty's memory model
    + foot's latency tricks
    + Zellij's session management
    + iTerm2's shell integration
    + Warp's block model
    + Novel: tiered storage, verification, agent workflows
```

---

## Research Sources

### Analysis Files
All analysis documents are in `research/`:
- `alacritty_ANALYSIS.md` - Rust GPU terminal
- `kitty_ANALYSIS.md` - C terminal with protocols
- `wezterm_ANALYSIS.md` - Rust cross-platform
- `ghostty_ANALYSIS.md` - Zig native terminal
- `rio_ANALYSIS.md` - Rust wgpu terminal
- `contour_ANALYSIS.md` - C++ VT520 terminal
- `windows-terminal_ANALYSIS.md` - C++ ConPTY terminal
- `foot_ANALYSIS.md` - C Wayland terminal
- `zellij_ANALYSIS.md` - Rust multiplexer
- `warp_ANALYSIS.md` - Warp UX analysis
- `ANALYSIS.md` - iTerm2/dashterm2 fork analysis
- `DTERM_DESIGN.md` - Design synthesis from all analyses

### External References
- `~/dashterm2/` - iTerm2 fork (separate repo)

### dashterm2 Benchmarks
- `~/dashterm2/reports/main/COMPETITIVE_ANALYSIS_001.md`
- `~/dashterm2/reports/main/COMPETITIVE_GAP_ANALYSIS_114.md`
- `~/dashterm2/docs/PERFORMANCE_BASELINE.md`
- `~/dashterm2/benchmarks/results/`

---

## Next Steps

1. **Prototype offset-based pages** - Validate serialization
2. **Implement dirty bitmap** - Cell-level tracking
3. **Build table-driven parser** - With memchr for escapes
4. **Add block model** - Command+output units
5. **Design approval workflow** - Agent-native UX
6. **Write TLA+ spec** - Parser state machine
