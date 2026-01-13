# DashTerm2 Vision

**The terminal built for AI agents. Beautiful. Stable. Fast.**

---

## Why DashTerm2 Exists

DashTerm2 is based on **iTerm2** - the terminal everybody loves. George Nachman built something great. We're not throwing that away.

But iTerm2 has accumulated cruft:
- AI chat sidebar (lightweight feature, not core)
- Built-in browser (why?)
- Features that distract from being a great terminal

And it has limitations:
- Memory grows unbounded
- macOS only
- No automatic logging
- Still crashes on edge cases

**DashTerm2 = iTerm2's great foundation + focus + improvements**

We keep what makes iTerm2 great:
- tmux integration
- Shell integration
- Split panes
- Hotkey windows
- Triggers
- Session restoration
- Python API
- Profiles
- Search
- All the power features

We remove the cruft and add what's missing for 24/7 agent workloads.

---

## Design Philosophy

### "Make It Better"

We don't build complex APIs. We don't replace text with JSON. We don't bundle IDE features.

**We make the terminal itself so good that everything using it automatically gets better.**

The test: Claude Code works unchanged. It's just faster, more stable, and more reliable.

### NASA/NSA Grade

Terminals running 24/7 with AI agents need:
- Zero crashes
- Constant memory usage
- No performance degradation over time
- Complete audit trail

If it's not good enough for a space mission, it's not good enough.

### Beautiful by Default

A terminal can be both powerful and beautiful. Good design isn't decoration - it's clarity.

---

## What We're Building

### Phase 1: Make The Basics Perfect

The features everyone uses, working flawlessly:

| Feature | Current Problem | Target |
|---------|-----------------|--------|
| **Copy & Paste** | Large copies block UI, Unicode breaks | Instant, any size, perfect Unicode |
| **Search** | Lags on large scrollback | <100ms on 1M lines |
| **Many Tabs** | Memory grows linearly | 100 tabs < 500MB |
| **Logging** | Off by default, buried settings | One checkbox, compressed, automatic |

### Phase 2: 24/7 Agent Support

For terminals that never stop:

| Requirement | Solution |
|-------------|----------|
| **Memory stability** | Disk-backed scrollback. 10K lines in RAM, rest on disk (compressed). |
| **Performance over time** | Incremental search index. Lazy rendering. No degradation. |
| **Automatic logging** | Global setting. Compressed. 30-day retention. |
| **Zero crashes** | Audit all force unwraps, bounds checks. Defensive everywhere. |
| **Tab management** | Hot/warm/cold states. Inactive tabs use minimal RAM. |

**Metrics:**
- Memory after 30 days: <500MB
- Crash rate: 0
- Tab switch: <16ms
- Search 1M lines: <100ms

### Phase 3: Mobile

Real terminals on iOS and iPadOS. Not SSH clients - actual terminals.

| Platform | Approach |
|----------|----------|
| **iOS/iPadOS** | Native SwiftUI app. Same core as macOS. |
| **visionOS** | Spatial terminal. Lower priority but same core enables it. |

**Why mobile matters:**
- Check agent status from phone
- Review logs on the go
- Emergency intervention when away from desk
- Tablets as secondary displays

**Technical approach:**
- Rust core for terminal emulation (cross-platform)
- Native UI per platform (SwiftUI for Apple)
- Shared scrollback via iCloud/sync

### Phase 4: Cross-Platform

Eventually: Linux and Windows. Same quality everywhere.

| Platform | Frontend | Core |
|----------|----------|------|
| macOS | Current app (Swift/ObjC) | Rust core via FFI |
| iOS | SwiftUI | Rust core via FFI |
| Linux | GTK or native Wayland | Rust core via FFI |
| Windows | WinUI | Rust core via FFI |

The Rust core is the investment. Frontends are platform-native thin layers.

### Phase 5: Beautiful

A terminal that's a pleasure to use:

| Element | Vision |
|---------|--------|
| **Typography** | Best-in-class font rendering. Ligatures. Variable fonts. |
| **Colors** | Perceptually uniform. Accessible. Dark/light that actually look good. |
| **Animation** | Smooth 120fps. Cursor blink that doesn't feel cheap. |
| **Layout** | Clean chrome. Focus on content. No visual clutter. |
| **Sound** | Subtle, optional audio feedback. Bell that doesn't make you hate life. |

**Inspiration:**
- Linear (clean, fast, focused)
- Apple SF Mono (typography)
- Material Design 3 (color system)
- Nothing Phone (playful but professional)

---

## Architecture

### Core Principle: Separate Engine from UI

```
┌─────────────────────────────────────────────────────────────┐
│  Platform UI (Swift/SwiftUI/GTK/WinUI)                      │
│  - Native look and feel                                     │
│  - Platform-specific features                               │
│  - Thin layer                                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ FFI (C ABI)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  dashterm-core (Rust)                                       │
│  - VT100/xterm emulation                                    │
│  - Screen buffer (ring buffer, disk-backed)                 │
│  - Escape sequence parsing (vte crate)                      │
│  - Memory management                                        │
│  - Search indexing                                          │
│  - Logging/compression                                      │
└─────────────────────────────────────────────────────────────┘
```

### Memory Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   HOT (in RAM)                              │
│                   Last 10,000 lines                         │
│                   ~5MB                                      │
├─────────────────────────────────────────────────────────────┤
│                   WARM (memory-mapped)                      │
│                   Recent scrollback                         │
│                   Paged by OS as needed                     │
├─────────────────────────────────────────────────────────────┤
│                   COLD (compressed on disk)                 │
│                   Old scrollback                            │
│                   Loaded on demand                          │
└─────────────────────────────────────────────────────────────┘

Total: Millions of lines, constant ~50MB RAM
```

### Logging Architecture

```
Terminal Output ──→ Ring Buffer ──→ Compressor ──→ Disk
                         │              │
                    (main thread)  (background)
                         │              │
                         ▼              ▼
                      Screen       ~/.dashterm/logs/
                                   ├── 2025-12-27/
                                   │   └── session-*.log.gz
                                   └── (auto-cleanup after 30 days)
```

---

## User Experience

### Settings That Make Sense

**Before (iTerm2):**
```
Preferences > Profiles > [Select Profile] > Session >
  Automatically log session input to files in: [____]
  ☐ Log plain text  ☐ Log HTML  ☐ Raw  ☐ Asciicast
  When logging, record: ☐ Input  ☐ Output
  ... (5 more options)
```

**After (DashTerm2):**
```
Settings > General

☑ Save all terminal output
  Location: ~/.dashterm/logs/        [Change]

That's it.
```

### Agent Mode

One toggle for 24/7 use:

```
Settings > Agent Mode

☑ Enable Agent Mode
  - Memory-efficient scrollback (disk-backed)
  - Automatic logging with compression
  - Crash reporting
  - Optimized for long-running sessions
```

### Beautiful Defaults

Out of the box:
- Dark mode that doesn't strain eyes
- Font that's readable at any size
- Colors that are distinguishable (accessibility)
- No configuration required to look good

---

## Roadmap

### Now: Foundation
- [x] Fix 367 upstream bugs
- [x] 3,578 regression tests
- [ ] Global logging with compression
- [ ] Memory-efficient scrollback

### Next: Stability
- [ ] Audit all crashes (force unwraps, bounds)
- [ ] Disk-backed scrollback
- [ ] Tab thermal management
- [ ] 24/7 agent mode

### Then: Mobile
- [ ] Rust core with C FFI
- [ ] iOS app (SwiftUI + Rust core)
- [ ] iPadOS app (same, keyboard optimized)
- [ ] iCloud sync for scrollback/logs

### Later: Beauty
- [ ] New default theme
- [ ] Typography improvements
- [ ] Animation polish
- [ ] Sound design

### Eventually: Everywhere
- [ ] Linux (GTK frontend)
- [ ] Windows (WinUI frontend)
- [ ] visionOS (spatial terminal)

---

## Success Criteria

### Performance
| Metric | Target |
|--------|--------|
| Memory (1M lines) | <100MB |
| Memory (30 days running) | <500MB |
| Tab switch | <16ms |
| Search 1M lines | <100ms |
| Render latency | <1ms |

### Stability
| Metric | Target |
|--------|--------|
| Crash rate | 0 per month |
| Data loss | Never |
| Hang rate | 0 |

### Usability
| Metric | Target |
|--------|--------|
| Time to enable logging | <5 seconds |
| Settings needed for good defaults | 0 |
| Mobile app rating | 4.5+ stars |

---

## Non-Goals

Things we're NOT building:

- **IDE features** - Use VS Code, Cursor, etc.
- **File browser** - Use Finder, ranger, etc.
- **Code search** - Use ripgrep, ag, etc.
- **Text editor** - Use vim, emacs, etc.
- **Chat UI** - External systems, not built-in
- **Browser** - External systems, not built-in

DashTerm2 is a **TTY terminal**. Nothing more.

---

## Hooks for External Systems

DashTerm2 is a terminal, but external systems need to integrate with it.

**What we provide:**

| Hook | Purpose | Example |
|------|---------|---------|
| **Session events** | Notify when session starts/ends | Trigger automation on session close |
| **Output stream** | Subscribe to terminal output | Feed to external logging/monitoring |
| **Command events** | Notify on command start/finish | Track what agents are doing |
| **State queries** | Get terminal state on demand | External tools read buffer |

**What we DON'T do:**

- Build the monitoring dashboard (external system)
- Build the AI chat (external system)
- Build the agent orchestrator (external system)
- Build anything that isn't a TTY terminal

**Implementation:**
- Unix socket or named pipe for IPC
- Simple JSON event format
- External systems connect and subscribe
- Terminal stays focused on being a terminal

```
DashTerm2                          External Systems
┌──────────────────┐                   ┌──────────────────┐
│              │ ──── events ────→ │ Monitoring       │
│   Terminal   │                   ├──────────────────┤
│   (TTY)      │ ──── events ────→ │ Agent Manager    │
│              │                   ├──────────────────┤
│              │ ←─── queries ──── │ Automation       │
└──────────────────┘                   └──────────────────┘
```

The terminal does terminal things. Hooks let other systems do their things.

---

## The Test

**Before:** Claude Code works in Terminal.app.

**After:** Claude Code works in DashTerm2 - faster, more stable, with automatic logging, on any device.

No changes to Claude Code required.

---

*"The terminal is the primary interface between AI and computers. Time to build it right."*
