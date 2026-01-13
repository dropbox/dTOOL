# inky: The Terminal UI Framework for the AI Era

## Executive Summary

**inky** is a Rust-native terminal UI framework that treats the terminal as a first-class application platform—not a legacy text interface, but a GPU-accelerated canvas capable of 120 FPS real-time visualization.

**Core thesis:** The terminal is the new browser for AI. Claude Code, Cursor, Copilot CLI, and the next generation of AI tools live in the terminal. They deserve a UI framework as sophisticated as React, as fast as a game engine, and as memory-efficient as embedded systems.

```
┌─────────────────────────────────────────────────────────────────┐
│                    THE TERMINAL RENAISSANCE                      │
├─────────────────────────────────────────────────────────────────┤
│  1970s: Teletype → glass terminal                               │
│  1980s: ANSI colors, cursor control                             │
│  1990s: ncurses, text UIs                                       │
│  2000s: Unicode, 256 colors                                     │
│  2010s: True color, Ink/React model                             │
│  2020s: GPU acceleration, AI-native interfaces      ← WE ARE HERE│
└─────────────────────────────────────────────────────────────────┘
```

---

## Requirements Checklist

Every feature here is **mandatory**. This is the union of:
- Google's Ink PRs (production requirements they discovered)
- AI agent requirements (robot army use case)
- Stability requirements (24/7 unattended operation)

### Google Ink PR Features (Gemini CLI needed these)

| Requirement | Ink PR | inky Solution | Status |
|-------------|--------|---------------|--------|
| **Overflow scrolling** | #788 | `Scroll` component with virtual scrollbar, `overflow: scroll` style | Phase 3 |
| **Early alternate buffer** | #815 | `Terminal::enter_alt_screen_immediate()`, Kitty protocol support | Phase 2 |
| **Frame timing hook** | #795 | `on_render` callback with frame duration, built into App | Core |
| **Terminal resize** | #787 | `on_resize` event, automatic relayout, ResizeObserver pattern | Phase 2 |
| **Text wrapping fixes** | #786 | Correct Unicode word boundaries via `unicode-segmentation` | Phase 3 |
| **Text measurement** | #789 | Proper `unicode-width`, handle edge cases ("constructor") | Core |

### AI Agent Requirements (Robot Army)

| Requirement | Why | inky Solution | Status |
|-------------|-----|---------------|--------|
| **Zero-latency keyboard** | Typing must feel instant | Dedicated input thread, immediate echo, dterm GPU direct | Core |
| **Streaming token render <1ms** | LLM output must feel instant | Tier 3 GPU direct, append-only fast path | Phase 7 |
| **AI can read screen as text** | Agents need to see what's displayed | `Perception::as_text()`, `as_tokens()`, `as_marked_text()` | Core |
| **AI can read screen as image** | Vision models need screenshots | `Perception::as_image()`, `as_image_base64()` | Core |
| **AI can read screen from memory** | Zero-copy for in-process AI | `Perception::cells()`, `mmap_buffer()` (dterm) | Phase 7 |
| **AI can perceive changes over time** | Detect motion, new output | `Perception::semantic_diff()`, `activity_regions()` | Core |
| **AI can write screen** | Agents generate UI | AI-friendly builder API, `ink!{}` macro | Phase 5 |
| **Direct cell buffer access** | Zero-copy for visualization | `terminal.cells()` returns `&mut [Cell]` | Phase 7 |
| **Works on dashterm2** | Primary development terminal | Tier 3 GPU backend via dterm IPC | Phase 7 |
| **Works on iPad** | Mobile AI interface | Touch + voice input, dterm-ios | Phase 8 |
| **Visualize weights/attention** | See what AI is thinking | `Heatmap`, `Plot`, `Sparkline` components | Phase 7 |
| **Voice control** | Hands-free operation | STT command parser, TTS feedback | Phase 8 |
| **AI-generatable API** | Agents write inky code | Consistent builder pattern, macro DSL | Core |

### Stability Requirements (24/7 Operation)

| Requirement | Why | inky Solution | Status |
|-------------|-----|---------------|--------|
| **No crashes** | Unattended operation | Rust safety, no `unwrap()` in library code | Core |
| **No memory leaks** | Long-running processes | Arena allocators, buffer reuse, no Rc cycles | Core |
| **No deadlocks** | Concurrent agents | Lock-free channels, no nested locks | Core |
| **Graceful degradation** | Unknown terminals | Capability detection, tier fallback | Phase 6 |
| **Panic recovery** | Don't kill the agent | `catch_unwind` at boundaries, restore terminal | Core |
| **Terminal restore** | Clean exit on any signal | RAII terminal state, signal handlers | Phase 2 |

### Protocol Support

| Protocol | Purpose | inky Solution | Status |
|----------|---------|---------------|--------|
| **Kitty keyboard** | Unambiguous key events | Detect and enable via `\x1b[>u` | Phase 6 |
| **Kitty graphics** | Inline images | `Image` component with protocol detection | Phase 6 |
| **Sixel** | Legacy image support | Fallback when Kitty unavailable | Phase 6 |
| **Synchronized output** | No tearing | `\x1b[?2026h` when supported | Phase 2 |
| **OSC 133** | Shell integration | Semantic prompt/command markers | Phase 6 |
| **OSC 52** | Clipboard access | `Clipboard::copy()`, `Clipboard::paste()` | Phase 3 |

### Performance Requirements

| Metric | Requirement | How Verified |
|--------|-------------|--------------|
| **Keypress → Display** | **<1ms** | Dedicated input thread, immediate echo |
| **Paste → Display** | **<5ms** for 1000 chars | Direct fd write, no buffering |
| Frame time (no change) | <0.1ms | `on_render` hook measurement |
| Frame time (1 cell) | <0.1ms | Benchmark: `bench_single_cell` |
| Frame time (streaming token) | <1ms | Benchmark: `bench_streaming` |
| Frame time (full 200x50) | <4ms Tier 2, <1ms Tier 3 | Benchmark: `bench_full_redraw` |
| Memory (empty app) | <1MB | `heaptrack` profiling |
| Memory (10K nodes) | <2MB | Benchmark: `bench_memory_10k` |
| Startup time | <5ms | Benchmark: `bench_startup` |
| Input latency (event processing) | <3ms Tier 2, <1ms Tier 3 | End-to-end measurement |

### Input Latency Breakdown

Traditional TUI frameworks:
```
Keypress → Poll (0-16ms) → Queue → Event Loop → State → Render → Display

Total: 20-50ms (UNACCEPTABLE)
```

inky with immediate echo:
```
Keypress → Immediate Echo → Display    (0-1ms)
    └───→ Queue → Process (async)      (background, doesn't block display)
```

---

## Design Philosophy

### 1. The Terminal is the New Browser for AI

| Dimension | Web Browser | Terminal (with inky) |
|-----------|-------------|----------------------|
| Rendering | HTML/CSS/JS → GPU | Cells/Flexbox → GPU |
| Typical FPS | 60 | 120 (ProMotion) |
| Memory baseline | 500MB+ | <50MB |
| Input model | Mouse-first | Keyboard-first |
| Latency | Network-bound | Zero (local) |
| AI integration | DOM scraping | Direct cell access |
| Streaming | Chunked HTTP | Native byte streams |

**Why terminals win for AI:**
- AI agents produce text naturally—terminals consume text natively
- Streaming tokens render incrementally without framework overhead
- Structured grid (rows × cols) beats chaotic DOM trees
- SSH means remote AI runs feel local
- No browser security sandbox blocking agent automation

### 2. Three Rendering Tiers

inky provides three rendering modes, each optimized for different use cases:

```
┌─────────────────────────────────────────────────────────────────┐
│ TIER 1: ANSI Compatible                                         │
│ ─────────────────────                                           │
│ Output: Escape sequences to stdout                              │
│ Works with: Any terminal (iTerm, Terminal.app, Windows Terminal)│
│ Latency: 8-16ms                                                 │
│ Use case: Portable CLI tools, simple UIs                        │
│                                                                 │
│ Text::new("Hello").bold().render(&mut stdout);                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ TIER 2: Retained Mode                                           │
│ ────────────────────                                            │
│ Output: Node tree → diff → minimal ANSI updates                 │
│ Works with: Any terminal                                        │
│ Latency: 4-8ms                                                  │
│ Use case: Interactive apps, dashboards, Claude Code TUI         │
│                                                                 │
│ App::new().render(|ctx| vbox![...]).run();                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ TIER 3: GPU Direct                                              │
│ ─────────────────                                               │
│ Output: Zero-copy writes to GPU cell buffer                     │
│ Works with: dterm, dashterm2 (GPU-accelerated terminals)        │
│ Latency: <1ms                                                   │
│ Use case: Real-time visualization, 120 FPS updates              │
│                                                                 │
│ Heatmap::gpu(&weights).render(&mut terminal.gpu_buffer());      │
└─────────────────────────────────────────────────────────────────┘
```

### 3. Union of the Best Ideas

inky synthesizes the best concepts from every major TUI framework:

| Framework | Language | Key Innovation | inky Adoption |
|-----------|----------|----------------|---------------|
| **Ink** | JavaScript | React component model, Yoga flexbox | Builder API + `ink!` macro DSL |
| **Bubbletea** | Go | Elm architecture (Model-Update-View) | Optional `App<Model, Msg>` pattern |
| **Textual** | Python | CSS-like styling, web deployment | Style sheets with cascading |
| **Lip Gloss** | Go | Chainable immutable styles | `Style::new().bold().fg(Red)` |
| **Brick** | Haskell | Declarative layout combinators | `hbox![]`, `vbox![]` macros |
| **Ratatui** | Rust | Immediate mode widgets, Buffer | `.render(area, buf)` escape hatch |
| **notcurses** | C | Direct rendering, multimedia | Tier 3 GPU mode, image protocols |
| **Cursive** | Rust | Callback-based events, views | `on_click`, `on_key`, `on_focus` |
| **Terminal.Gui** | C# | Rich widget library | Complete component set |
| **Taffy** | Rust | Pure-Rust flexbox/grid | Layout engine (replaces Yoga) |

### 4. Performance is a Feature

Every architectural decision optimizes for speed and memory:

| Principle | Implementation |
|-----------|----------------|
| Zero-copy when possible | Borrow data, don't clone |
| Incremental updates | Diff algorithm finds minimal changes |
| GPU-native data | 8-byte cells match GPU buffer layout |
| Allocation reuse | Buffer pools, arena allocators |
| SIMD where applicable | Unicode width, string operations |
| Lock-free channels | Cross-thread updates without mutex |

---

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        USER APPLICATION                          │
│                                                                  │
│   App::new()                                                     │
│       .state(MyState::default())                                 │
│       .render(|ctx| { ... })                                     │
│       .on_key(Key::Enter, |ctx| { ... })                         │
│       .run();                                                    │
│                                                                  │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                         INKY CORE                                │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Signals   │  │  Component  │  │      Node Tree          │  │
│  │  Reactivity │  │    Model    │  │  (Box, Text, Custom)    │  │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘  │
│         │                │                     │                 │
│         └────────────────┼─────────────────────┘                 │
│                          ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    LAYOUT ENGINE (Taffy)                    ││
│  │         Flexbox • Grid • Absolute • Percent • Auto          ││
│  └─────────────────────────────────────────────────────────────┘│
│                          │                                       │
│                          ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    RENDER PIPELINE                          ││
│  │                                                             ││
│  │   Node Tree ──► Layout ──► Cell Buffer ──► Diff ──► Output ││
│  │                                                             ││
│  └─────────────────────────────────────────────────────────────┘│
│                          │                                       │
└──────────────────────────┼──────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
    ┌────────────┐  ┌────────────┐  ┌────────────┐
    │ ANSI/VT100 │  │  Crossterm │  │ GPU Direct │
    │  (Tier 1)  │  │  (Tier 2)  │  │  (Tier 3)  │
    └────────────┘  └────────────┘  └────────────┘
           │               │               │
           ▼               ▼               ▼
    ┌────────────┐  ┌────────────┐  ┌────────────┐
    │ Any Term   │  │ Any Term   │  │   dterm    │
    │            │  │            │  │ dashterm2  │
    └────────────┘  └────────────┘  └────────────┘
```

### Terminal Safety (24/7 Stability Requirements)

```rust
/// RAII terminal state - guarantees cleanup on any exit
pub struct TerminalGuard {
    original_mode: TerminalMode,
    alternate_buffer: bool,
    raw_mode: bool,
}

impl TerminalGuard {
    /// Enter managed terminal mode
    pub fn new() -> Result<Self, TerminalError> {
        // Save original terminal state
        let original = TerminalMode::capture()?;

        // Install signal handlers for cleanup
        Self::install_signal_handlers();

        // Install panic hook for cleanup
        Self::install_panic_hook();

        Ok(Self {
            original_mode: original,
            alternate_buffer: false,
            raw_mode: false,
        })
    }

    /// Enter alternate buffer immediately (Google Ink PR #815)
    /// Call before first render if you need early buffer switch
    pub fn enter_alt_screen_immediate(&mut self) -> Result<(), TerminalError> {
        if !self.alternate_buffer {
            execute!(stdout(), EnterAlternateScreen)?;
            self.alternate_buffer = true;
        }
        Ok(())
    }

    /// Enable Kitty keyboard protocol if supported
    pub fn enable_kitty_keyboard(&mut self) -> Result<bool, TerminalError> {
        // Query terminal, enable if supported
        // Returns false if not supported
    }

    /// Enable synchronized output if supported (no tearing)
    pub fn enable_sync_output(&mut self) -> Result<bool, TerminalError> {
        // \x1b[?2026h
    }

    fn install_signal_handlers() {
        // SIGINT, SIGTERM, SIGHUP -> clean exit
        // SIGWINCH -> resize event
        // SIGTSTP -> suspend (restore terminal first)
        // SIGCONT -> resume (re-enter raw mode)
    }

    fn install_panic_hook() {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Restore terminal BEFORE printing panic
            let _ = Self::emergency_restore();
            original_hook(info);
        }));
    }

    fn emergency_restore() -> Result<(), TerminalError> {
        // Best-effort terminal restoration
        let _ = execute!(stdout(), LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(stdout(), Show); // cursor
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Always restore terminal state
        if self.alternate_buffer {
            let _ = execute!(stdout(), LeaveAlternateScreen);
        }
        if self.raw_mode {
            let _ = crossterm::terminal::disable_raw_mode();
        }
        // Restore original mode
        let _ = self.original_mode.restore();
    }
}

/// Run a closure with terminal safety guarantees
/// Even if the closure panics, terminal is restored
pub fn with_terminal<F, R>(f: F) -> Result<R, TerminalError>
where
    F: FnOnce(&mut TerminalGuard) -> R + std::panic::UnwindSafe,
{
    let mut guard = TerminalGuard::new()?;

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut guard))) {
        Ok(result) => Ok(result),
        Err(panic) => {
            // Guard will restore terminal on drop
            drop(guard);
            std::panic::resume_unwind(panic);
        }
    }
}
```

### Zero-Latency Input (Keyboard Must Be Instant)

Input latency is unacceptable. Characters must appear the instant keys are pressed.

**The Problem with Traditional TUI Input:**
```
Keypress → Event Queue → Event Loop (16ms tick) → Update State → Re-render → Display

Total latency: 20-50ms (UNACCEPTABLE)
```

**inky's Solution: Immediate Echo + Async Processing**
```
Keypress ─┬─→ Immediate Echo (0ms) ─→ Terminal ─→ Display
          │
          └─→ Event Queue ─→ State Update ─→ Re-render (background)
```

```rust
/// Zero-latency input system
pub struct InputSystem {
    /// High-priority input thread (dedicated, never blocks)
    input_thread: JoinHandle<()>,

    /// Lock-free channel for input events
    event_tx: crossbeam::channel::Sender<InputEvent>,
    event_rx: crossbeam::channel::Receiver<InputEvent>,

    /// Direct terminal write handle for immediate echo
    echo_writer: RawTerminalWriter,

    /// Input mode determines echo behavior
    mode: InputMode,
}

#[derive(Clone, Copy)]
pub enum InputMode {
    /// Echo immediately, process async (default for text input)
    ImmediateEcho,

    /// Buffer input, echo on commit (for passwords)
    Buffered,

    /// No echo, raw events only (for hotkeys, games)
    Raw,

    /// Custom echo function
    Custom,
}

impl InputSystem {
    pub fn new() -> Self {
        // Spawn dedicated input thread at highest priority
        let input_thread = std::thread::Builder::new()
            .name("inky-input".into())
            .spawn(move || {
                // Set thread to real-time priority if possible
                #[cfg(unix)]
                Self::set_realtime_priority();

                // Tight polling loop - no sleeps
                loop {
                    if let Ok(event) = crossterm::event::poll(Duration::ZERO) {
                        if event {
                            if let Ok(e) = crossterm::event::read() {
                                self.handle_input(e);
                            }
                        }
                    }
                    // Yield to prevent 100% CPU, but stay responsive
                    std::hint::spin_loop();
                }
            })
            .unwrap();

        Self { /* ... */ }
    }

    fn handle_input(&self, event: crossterm::event::Event) {
        match event {
            Event::Key(key) => {
                // IMMEDIATE ECHO - before any processing
                if self.mode == InputMode::ImmediateEcho {
                    if let KeyCode::Char(c) = key.code {
                        // Write directly to terminal - bypasses everything
                        self.echo_writer.write_char(c);
                    }
                }

                // Then queue for async processing
                let _ = self.event_tx.try_send(InputEvent::Key(key));
            }
            Event::Paste(text) => {
                // Immediate echo for paste too
                if self.mode == InputMode::ImmediateEcho {
                    self.echo_writer.write_str(&text);
                }
                let _ = self.event_tx.try_send(InputEvent::Paste(text));
            }
            _ => {
                let _ = self.event_tx.try_send(event.into());
            }
        }
    }

    #[cfg(unix)]
    fn set_realtime_priority() {
        // Try to set SCHED_FIFO for lowest latency
        // Falls back gracefully if not permitted
        unsafe {
            let param = libc::sched_param { sched_priority: 1 };
            libc::sched_setscheduler(0, libc::SCHED_FIFO, &param);
        }
    }
}

/// Raw terminal writer that bypasses all buffering
pub struct RawTerminalWriter {
    fd: RawFd,
}

impl RawTerminalWriter {
    /// Write single char with zero buffering
    #[inline(always)]
    pub fn write_char(&self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        unsafe {
            libc::write(self.fd, s.as_ptr() as *const _, s.len());
        }
    }

    /// Write string with zero buffering
    #[inline(always)]
    pub fn write_str(&self, s: &str) {
        unsafe {
            libc::write(self.fd, s.as_ptr() as *const _, s.len());
        }
    }
}

/// Input component with immediate echo
pub struct Input {
    value: String,
    cursor: usize,
    mode: InputMode,

    /// Pre-rendered cursor position for instant updates
    cursor_screen_pos: (u16, u16),
}

impl Input {
    pub fn new() -> Self { ... }

    /// Enable immediate echo mode (default)
    pub fn immediate_echo(self) -> Self {
        Self { mode: InputMode::ImmediateEcho, ..self }
    }

    /// Disable echo (for passwords)
    pub fn password(self) -> Self {
        Self { mode: InputMode::Buffered, ..self }
    }

    /// Handle key with immediate visual feedback
    pub fn handle_key(&mut self, key: KeyEvent, terminal: &mut Terminal) {
        match key.code {
            KeyCode::Char(c) => {
                // Character already echoed by InputSystem
                // Just update internal state
                self.value.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                    // Immediate visual: move back, clear char
                    terminal.write_raw(b"\x08 \x08");
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    terminal.write_raw(b"\x08"); // Move cursor back
                }
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
                    terminal.write_raw(b"\x1b[C"); // Move cursor forward
                }
            }
            _ => {}
        }
    }
}
```

**Latency Comparison:**

| Operation | Traditional | inky (Tier 1/2) | inky + dterm (Tier 3) |
|-----------|-------------|-----------------|----------------------|
| Keypress → Display | 20-50ms | **<1ms** | **<0.1ms** |
| Paste 1000 chars | 200-500ms | **<5ms** | **<1ms** |
| Cursor movement | 16-32ms | **<1ms** | **<0.1ms** |
| Backspace | 16-32ms | **<1ms** | **<0.1ms** |

**dterm Acceleration (Tier 3):**

When running in dterm/dashterm2, input handling is even faster:

```rust
impl InputSystem {
    fn new_with_dterm(dterm: &DtermConnection) -> Self {
        // dterm provides:
        // 1. In-process input events (no IPC latency)
        // 2. Direct GPU buffer writes (no terminal escape codes)
        // 3. Pre-rendered glyph atlas (character already rasterized)

        Self {
            // dterm gives us a memory-mapped event ring buffer
            event_source: dterm.input_ring_buffer(),

            // Direct write to GPU cell buffer
            echo_writer: dterm.gpu_cell_writer(),

            // GPU renders at next vsync (< 8ms at 120Hz, usually <1ms)
            ..Default::default()
        }
    }
}

/// dterm-accelerated immediate echo
impl DtermEchoWriter {
    #[inline(always)]
    pub fn write_char(&mut self, c: char, style: &TextStyle) {
        // Write directly to GPU cell buffer - no escape codes, no parsing
        let cell = Cell::from_char(c, style);
        self.buffer[self.cursor_offset()] = cell;
        self.cursor.0 += 1;

        // Mark single cell dirty - GPU picks it up at next vsync
        self.damage.mark_cell(self.cursor.0 - 1, self.cursor.1);

        // Total time: ~100 nanoseconds
    }
}
```

**Graceful Fallback:**

```rust
impl InputSystem {
    pub fn new(terminal: &Terminal) -> Self {
        match terminal.backend() {
            Backend::Dterm(dterm) => {
                // Tier 3: Direct GPU buffer, <0.1ms input latency
                Self::new_with_dterm(dterm)
            }
            Backend::Crossterm => {
                // Tier 2: Immediate echo via raw fd write, <1ms
                Self::new_with_immediate_echo()
            }
            Backend::Ansi => {
                // Tier 1: Standard buffered I/O, ~5ms
                // Still better than traditional frameworks
                Self::new_buffered()
            }
        }
    }
}
```

**Key Techniques:**

1. **Dedicated input thread** - Never blocked by rendering
2. **Real-time thread priority** - OS schedules input handling first
3. **Direct write to fd** - Bypasses all Rust buffering
4. **Immediate echo** - Write to terminal before event processing
5. **Lock-free channels** - No mutex contention with render thread
6. **Cursor math** - Track cursor position, update with raw escapes

```rust
// Usage in App
App::new()
    .input_mode(InputMode::ImmediateEcho)  // Zero-latency typing
    .render(|ctx| {
        vbox![
            Text::new("Type something:"),
            Input::new()
                .immediate_echo()
                .value(&ctx.state.input)
                .on_change(|s| Msg::InputChanged(s)),
        ]
    })
    .run();
```

### AI Perception System (Robot Army)

AI agents need to perceive the terminal like humans do - but better. Multiple modalities:

```rust
/// AI perception of terminal state
pub struct Perception {
    terminal: Arc<Terminal>,
    history: FrameHistory,
}

impl Perception {
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // TEXT MODALITY (tokens for LLM)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Read screen as plain text (for LLM context)
    pub fn as_text(&self) -> String {
        self.terminal.buffer().to_text()
    }

    /// Read screen as text with style markers (XML-like)
    /// <bold>Error:</bold> <red>File not found</red>
    pub fn as_marked_text(&self) -> String {
        self.terminal.buffer().to_marked_text()
    }

    /// Read screen as ANSI (preserves exact formatting)
    pub fn as_ansi(&self) -> String {
        self.terminal.buffer().to_ansi()
    }

    /// Read specific region
    pub fn read_region(&self, rect: Rect) -> String {
        self.terminal.buffer().read_region(rect)
    }

    /// Tokenize screen for LLM (splits on whitespace, preserves structure)
    pub fn as_tokens(&self) -> Vec<Token> {
        let mut tokens = Vec::new();
        for (y, line) in self.as_text().lines().enumerate() {
            for (x, word) in line.split_whitespace_with_indices() {
                tokens.push(Token {
                    text: word.to_string(),
                    position: (x as u16, y as u16),
                    style: self.style_at(x as u16, y as u16),
                });
            }
            tokens.push(Token::newline(y as u16));
        }
        tokens
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // IMAGE MODALITY (for vision models)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Render screen as PNG image (for vision LLMs)
    pub fn as_image(&self) -> Vec<u8> {
        self.render_to_png(self.terminal.buffer())
    }

    /// Render screen as base64 PNG (for API calls)
    pub fn as_image_base64(&self) -> String {
        base64::encode(self.as_image())
    }

    /// Render at specific resolution (for bandwidth control)
    pub fn as_image_scaled(&self, max_width: u32, max_height: u32) -> Vec<u8> {
        // Render and scale
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // DIRECT MEMORY ACCESS (dterm only - zero copy)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Direct cell buffer access (Tier 3 only)
    /// AI running in-process can read/write cells directly
    pub fn cells(&self) -> Option<&[Cell]> {
        self.terminal.as_dterm()?.cells()
    }

    /// Memory-mapped buffer for external AI process
    pub fn mmap_buffer(&self) -> Option<&MmapBuffer> {
        self.terminal.as_dterm()?.shared_buffer()
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // TEMPORAL PERCEPTION (motion, changes over time)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Get diff since last frame
    pub fn frame_diff(&self) -> FrameDiff {
        self.history.diff_last()
    }

    /// Get diff over time window
    pub fn diff_since(&self, duration: Duration) -> FrameDiff {
        self.history.diff_since(duration)
    }

    /// Get recent frame history as sequence (for motion understanding)
    pub fn recent_frames(&self, count: usize) -> Vec<FrameSnapshot> {
        self.history.recent(count)
    }

    /// Detect regions of change (where is activity happening?)
    pub fn activity_regions(&self, since: Duration) -> Vec<Rect> {
        self.history.activity_regions(since)
    }

    /// Get semantic changes (text added, removed, modified)
    pub fn semantic_diff(&self) -> SemanticDiff {
        SemanticDiff {
            added_lines: self.history.added_lines(),
            removed_lines: self.history.removed_lines(),
            modified_regions: self.history.modified_regions(),
            cursor_moved: self.history.cursor_delta(),
            scroll_delta: self.history.scroll_delta(),
        }
    }
}

/// Frame history for temporal perception
pub struct FrameHistory {
    frames: VecDeque<FrameSnapshot>,
    max_frames: usize,
    max_age: Duration,
}

#[derive(Clone)]
pub struct FrameSnapshot {
    pub timestamp: Instant,
    pub cells: Vec<Cell>,  // or compressed
    pub cursor: (u16, u16),
    pub dimensions: (u16, u16),
}

/// Diff between frames
pub struct FrameDiff {
    pub changed_cells: Vec<(u16, u16, Cell, Cell)>,  // (x, y, old, new)
    pub changed_regions: Vec<Rect>,
    pub text_added: String,
    pub text_removed: String,
}

/// Semantic understanding of changes
pub struct SemanticDiff {
    pub added_lines: Vec<(u16, String)>,      // (line_num, content)
    pub removed_lines: Vec<(u16, String)>,
    pub modified_regions: Vec<(Rect, String, String)>,  // (region, old, new)
    pub cursor_moved: Option<((u16, u16), (u16, u16))>, // (from, to)
    pub scroll_delta: i32,
}

// Usage: AI agent reading terminal
impl Agent {
    fn observe(&self, perception: &Perception) {
        // Text for LLM context
        let screen_text = perception.as_text();

        // What changed since last observation?
        let diff = perception.semantic_diff();
        if !diff.added_lines.is_empty() {
            // New output appeared
            for (line, content) in &diff.added_lines {
                self.process_new_output(line, content);
            }
        }

        // Where is activity? (for attention)
        let active_regions = perception.activity_regions(Duration::from_secs(1));
        for region in active_regions {
            self.focus_attention(region);
        }

        // For vision model analysis
        let screenshot = perception.as_image_base64();
        self.vision_model.analyze(screenshot);
    }
}
```

### Streaming Token Fast Path (AI Agent Requirement)

```rust
/// Optimized path for streaming LLM output
/// Appends text without full re-render
pub struct StreamingText {
    buffer: String,
    cursor: (u16, u16),
    style: TextStyle,
}

impl StreamingText {
    pub fn new() -> Self { ... }

    /// Append tokens without full re-render
    /// Returns in <1ms for typical token sizes
    pub fn append(&mut self, text: &str, terminal: &mut impl Terminal) -> io::Result<()> {
        // Fast path: just write to terminal at cursor position
        // No layout, no diff, no buffer sync

        for ch in text.chars() {
            if ch == '\n' {
                self.cursor.0 = 0;
                self.cursor.1 += 1;
                terminal.write(b"\r\n")?;
            } else {
                terminal.write_styled(ch, &self.style)?;
                self.cursor.0 += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
            }
        }

        terminal.flush()?;
        Ok(())
    }

    /// Sync with buffer (call periodically or on scroll)
    pub fn sync_to_buffer(&self, buffer: &mut Buffer) {
        // Write accumulated text to buffer for diff/scroll
    }
}

/// StreamingText component for use in render tree
pub struct Stream {
    content: Arc<RwLock<String>>,
    style: TextStyle,
}

impl Stream {
    /// Create streaming text that updates via channel
    pub fn new(rx: mpsc::Receiver<String>) -> Self { ... }

    /// Append text (from LLM token stream)
    pub fn append(&self, text: &str) {
        self.content.write().unwrap().push_str(text);
        // Triggers minimal re-render of just this component
    }
}

// Usage in Claude Code style app
let (tx, rx) = mpsc::channel();
let stream = Stream::new(rx);

// In LLM response handler
for token in llm_stream {
    tx.send(token).unwrap();  // <1ms render update
}
```

### Module Structure

```
inky/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Public API, feature flags
│   │
│   ├── # ══════════════════════════════════════════════════════
│   ├── # CORE TYPES
│   ├── # ══════════════════════════════════════════════════════
│   ├── node.rs                # Node enum (Root, Box, Text, Static, Custom)
│   ├── style.rs               # Style → Taffy mapping, colors, borders
│   ├── layout.rs              # Taffy integration, computed layouts
│   │
│   ├── # ══════════════════════════════════════════════════════
│   ├── # RENDERING
│   ├── # ══════════════════════════════════════════════════════
│   ├── render/
│   │   ├── mod.rs             # Render trait, pipeline coordinator
│   │   ├── buffer.rs          # Cell buffer (2D grid)
│   │   ├── cell.rs            # Cell type (8 bytes, GPU-compatible)
│   │   ├── diff.rs            # Line-level diff algorithm
│   │   ├── ansi.rs            # ANSI escape code generation (Tier 1)
│   │   ├── painter.rs         # High-level drawing API
│   │   └── gpu.rs             # GPU buffer interface (Tier 3)
│   │
│   ├── # ══════════════════════════════════════════════════════
│   ├── # TERMINAL BACKEND
│   ├── # ══════════════════════════════════════════════════════
│   ├── terminal/
│   │   ├── mod.rs             # Terminal trait, detection
│   │   ├── crossterm.rs       # Crossterm backend (default)
│   │   ├── events.rs          # Input event types
│   │   ├── capabilities.rs    # Feature detection (colors, unicode, etc)
│   │   └── dterm.rs           # dterm GPU backend (optional)
│   │
│   ├── # ══════════════════════════════════════════════════════
│   ├── # APPLICATION FRAMEWORK
│   ├── # ══════════════════════════════════════════════════════
│   ├── app.rs                 # Application runner, event loop
│   ├── context.rs             # Render context (state, terminal info)
│   ├── hooks/
│   │   ├── mod.rs             # Hook system
│   │   ├── signal.rs          # Signal<T> reactive state
│   │   ├── input.rs           # use_input() keyboard handling
│   │   ├── focus.rs           # use_focus() focus management
│   │   ├── interval.rs        # use_interval() timers
│   │   └── effect.rs          # use_effect() side effects
│   │
│   ├── # ══════════════════════════════════════════════════════
│   ├── # COMPONENTS
│   ├── # ══════════════════════════════════════════════════════
│   ├── components/
│   │   ├── mod.rs             # Component trait
│   │   │
│   │   ├── # Layout primitives
│   │   ├── r#box.rs           # Flexbox container
│   │   ├── text.rs            # Styled text with wrapping
│   │   ├── spacer.rs          # Flexible space filler
│   │   ├── stack.rs           # Z-axis layering
│   │   ├── scroll.rs          # Scrollable viewport
│   │   │
│   │   ├── # Interactive
│   │   ├── input.rs           # Text input field
│   │   ├── select.rs          # Selection list
│   │   ├── checkbox.rs        # Toggle checkbox
│   │   ├── button.rs          # Clickable button
│   │   │
│   │   ├── # Display
│   │   ├── spinner.rs         # Animated spinner
│   │   ├── progress.rs        # Progress bar
│   │   ├── table.rs           # Data table
│   │   ├── tree.rs            # Collapsible tree
│   │   │
│   │   ├── # AI-native
│   │   ├── markdown.rs        # Markdown renderer
│   │   ├── code.rs            # Syntax highlighted code
│   │   ├── stream.rs          # Token streaming display
│   │   ├── diff_view.rs       # Side-by-side diff
│   │   │
│   │   └── # Visualization (Tier 3)
│   │       ├── canvas.rs      # Arbitrary drawing
│   │       ├── heatmap.rs     # 2D color grid
│   │       ├── sparkline.rs   # Inline chart
│   │       └── plot.rs        # Line/scatter plots
│   │
│   ├── # ══════════════════════════════════════════════════════
│   ├── # MACROS
│   ├── # ══════════════════════════════════════════════════════
│   └── macros.rs              # vbox![], hbox![], ink!{} macros
│
├── inky-macros/               # Proc-macro crate for ink!{}
│   ├── Cargo.toml
│   └── src/lib.rs
│
├── examples/
│   ├── hello.rs               # Minimal hello world
│   ├── counter.rs             # Interactive counter
│   ├── todo.rs                # Todo list app
│   ├── dashboard.rs           # Multi-pane dashboard
│   ├── markdown.rs            # Markdown viewer
│   ├── neural_net.rs          # NN weight visualization (Tier 3)
│   └── claude_tui.rs          # Claude Code recreation
│
├── benches/
│   ├── layout.rs              # Taffy layout benchmarks
│   ├── render.rs              # Rendering benchmarks
│   ├── diff.rs                # Diff algorithm benchmarks
│   └── throughput.rs          # End-to-end throughput
│
└── tests/
    ├── snapshots/             # Insta snapshot tests
    ├── layout_tests.rs
    ├── render_tests.rs
    └── component_tests.rs
```

---

## Core Types

### Cell (8 bytes, GPU-compatible)

```rust
/// A single terminal cell, optimized for GPU buffer layout.
///
/// Memory layout (8 bytes total):
/// - char_data: 2 bytes (BMP char or overflow index)
/// - fg: 3 bytes (RGB foreground)
/// - bg: 3 bytes (RGB background)
/// - flags: 2 bytes (bold, italic, underline, etc.)
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Cell {
    pub char_data: u16,      // Character or overflow table index
    pub fg: PackedColor,     // 3 bytes
    pub bg: PackedColor,     // 3 bytes
    pub flags: CellFlags,    // 2 bytes
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PackedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

bitflags! {
    #[repr(transparent)]
    pub struct CellFlags: u16 {
        const BOLD          = 0b0000_0000_0001;
        const ITALIC        = 0b0000_0000_0010;
        const UNDERLINE     = 0b0000_0000_0100;
        const STRIKETHROUGH = 0b0000_0000_1000;
        const DIM           = 0b0000_0001_0000;
        const INVERSE       = 0b0000_0010_0000;
        const HIDDEN        = 0b0000_0100_0000;
        const BLINK         = 0b0000_1000_0000;
        const WIDE_CHAR     = 0b0001_0000_0000;  // This is left half of wide char
        const WIDE_SPACER   = 0b0010_0000_0000;  // This is right half (spacer)
        const OVERFLOW      = 0b0100_0000_0000;  // char_data is overflow index
        const DIRTY         = 0b1000_0000_0000;  // Needs redraw
    }
}
```

### Node Tree

```rust
/// The UI node tree, similar to a virtual DOM.
pub enum Node {
    /// Root container for the entire UI
    Root(RootNode),

    /// Flexbox container
    Box(BoxNode),

    /// Text content with styling
    Text(TextNode),

    /// Static content that never re-renders (logs, etc.)
    Static(StaticNode),

    /// Custom component
    Custom(Box<dyn Component>),
}

/// Flexbox container with full CSS Flexbox support
pub struct BoxNode {
    pub id: NodeId,
    pub style: Style,
    pub children: Vec<Node>,
}

/// Text with inline styling
pub struct TextNode {
    pub id: NodeId,
    pub content: TextContent,
    pub style: TextStyle,
}

/// Text content can be static or dynamic
pub enum TextContent {
    Static(String),
    Dynamic(Box<dyn Fn() -> String + Send + Sync>),
}
```

### Style (Taffy-compatible)

```rust
/// Complete style specification for a node.
/// Maps directly to Taffy's Style struct.
#[derive(Clone, Default)]
pub struct Style {
    // Display
    pub display: Display,           // Flex, Block, None

    // Flex container
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub gap: Gap,

    // Flex item
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub align_self: AlignSelf,

    // Size
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,

    // Spacing
    pub padding: Edges,
    pub margin: Edges,

    // Visual
    pub border: BorderStyle,
    pub background: Option<Color>,
    pub overflow: Overflow,
}

/// Convert to Taffy style for layout computation
impl Style {
    pub fn to_taffy(&self) -> taffy::Style {
        taffy::Style {
            display: self.display.into(),
            flex_direction: self.flex_direction.into(),
            // ... complete mapping
        }
    }
}
```

---

## Rendering Pipeline

### Overview

```
┌─────────┐    ┌────────┐    ┌────────┐    ┌──────┐    ┌────────┐
│  Node   │───►│ Layout │───►│ Paint  │───►│ Diff │───►│ Output │
│  Tree   │    │(Taffy) │    │(Buffer)│    │      │    │        │
└─────────┘    └────────┘    └────────┘    └──────┘    └────────┘
     │              │             │            │            │
     │              │             │            │            │
   O(n)          O(n)          O(n)        O(dirty)    O(delta)
  nodes         nodes         cells        cells       bytes
```

### 1. Layout Phase (Taffy)

```rust
pub struct LayoutEngine {
    taffy: TaffyTree,
    node_to_taffy: HashMap<NodeId, TaffyNodeId>,
}

impl LayoutEngine {
    /// Build Taffy tree from inky node tree
    pub fn build(&mut self, root: &Node) -> Result<(), LayoutError> {
        self.build_recursive(root, None)
    }

    /// Compute layout for given viewport size
    pub fn compute(&mut self, width: u16, height: u16) {
        let available = Size {
            width: AvailableSpace::Definite(width as f32),
            height: AvailableSpace::Definite(height as f32),
        };
        self.taffy.compute_layout(self.root, available).unwrap();
    }

    /// Get computed layout for a node
    pub fn get(&self, node_id: NodeId) -> Option<Layout> {
        let taffy_id = self.node_to_taffy.get(&node_id)?;
        let layout = self.taffy.layout(*taffy_id).ok()?;
        Some(Layout {
            x: layout.location.x as u16,
            y: layout.location.y as u16,
            width: layout.size.width as u16,
            height: layout.size.height as u16,
        })
    }
}
```

### 2. Paint Phase (Buffer)

```rust
pub struct Buffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
    damage: DamageTracker,
}

impl Buffer {
    /// Paint a node and its children to the buffer
    pub fn paint(&mut self, node: &Node, layout: &Layout, engine: &LayoutEngine) {
        match node {
            Node::Box(box_node) => {
                self.paint_box(box_node, layout);
                for child in &box_node.children {
                    let child_layout = engine.get(child.id()).unwrap();
                    self.paint(child, &child_layout, engine);
                }
            }
            Node::Text(text_node) => {
                self.paint_text(text_node, layout);
            }
            // ...
        }
    }

    /// Set a cell, marking it dirty
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if self.cells[idx] != cell {
            self.cells[idx] = cell;
            self.damage.mark(x, y);
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // AI AGENT ACCESS (Robot Army Requirements)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Direct cell access for AI agents (read)
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Direct cell access for AI agents (write) - marks all as dirty
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        self.damage.mark_all();
        &mut self.cells
    }

    /// Read a rectangular region as text (for AI to read screen)
    pub fn read_region(&self, rect: Rect) -> String {
        let mut result = String::new();
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                result.push(self.get(x, y).char());
            }
            result.push('\n');
        }
        result
    }

    /// Convert entire buffer to plain text (for AI screen reading)
    pub fn to_text(&self) -> String {
        self.read_region(Rect::new(0, 0, self.width, self.height))
    }

    /// Convert to text with ANSI styling preserved (for AI analysis)
    pub fn to_ansi(&self) -> String {
        let mut result = String::new();
        let mut last_style = CellFlags::empty();
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = self.get(x, y);
                if cell.flags != last_style {
                    result.push_str(&cell.ansi_escape());
                    last_style = cell.flags;
                }
                result.push(cell.char());
            }
            result.push_str("\x1b[0m\n");
            last_style = CellFlags::empty();
        }
        result
    }

    /// Find text in buffer (for AI to locate content)
    pub fn find(&self, needle: &str) -> Vec<(u16, u16)> {
        let text = self.to_text();
        let mut positions = Vec::new();
        for (line_idx, line) in text.lines().enumerate() {
            for (col_idx, _) in line.match_indices(needle) {
                positions.push((col_idx as u16, line_idx as u16));
            }
        }
        positions
    }
}
```

### 3. Diff Phase

```rust
pub struct Differ {
    // Previous frame's buffer (for comparison)
    prev: Buffer,
}

impl Differ {
    /// Compute minimal set of changes between frames
    pub fn diff(&self, current: &Buffer) -> Vec<Change> {
        let mut changes = Vec::new();

        for row in current.damage.dirty_rows() {
            let (start, end) = current.damage.dirty_cols(row);
            changes.push(Change::Row {
                y: row,
                x_start: start,
                x_end: end,
                cells: current.row_slice(row, start, end).to_vec(),
            });
        }

        changes
    }
}

pub enum Change {
    /// Update a contiguous range of cells in a row
    Row {
        y: u16,
        x_start: u16,
        x_end: u16,
        cells: Vec<Cell>,
    },
    /// Clear entire screen (on resize, etc.)
    Clear,
    /// Move cursor without drawing
    Cursor { x: u16, y: u16 },
}
```

### 4. Output Phase

```rust
pub trait Renderer {
    fn render(&mut self, changes: &[Change]) -> io::Result<()>;
}

/// ANSI renderer for any terminal (Tier 1/2)
pub struct AnsiRenderer<W: Write> {
    out: W,
    cursor: (u16, u16),
}

impl<W: Write> Renderer for AnsiRenderer<W> {
    fn render(&mut self, changes: &[Change]) -> io::Result<()> {
        for change in changes {
            match change {
                Change::Row { y, x_start, cells, .. } => {
                    // Move cursor if needed
                    if self.cursor != (*x_start, *y) {
                        write!(self.out, "\x1b[{};{}H", y + 1, x_start + 1)?;
                    }
                    // Write cells with minimal escape codes
                    self.write_cells(cells)?;
                    self.cursor.0 += cells.len() as u16;
                }
                Change::Clear => {
                    write!(self.out, "\x1b[2J\x1b[H")?;
                    self.cursor = (0, 0);
                }
                // ...
            }
        }
        self.out.flush()
    }
}

/// GPU renderer for dterm (Tier 3)
pub struct GpuRenderer {
    buffer: GpuBuffer<Cell>,
}

impl Renderer for GpuRenderer {
    fn render(&mut self, changes: &[Change]) -> io::Result<()> {
        for change in changes {
            match change {
                Change::Row { y, x_start, cells, .. } => {
                    let offset = (*y as usize) * self.width + (*x_start as usize);
                    self.buffer.write(offset, cells);
                }
                // ...
            }
        }
        Ok(()) // GPU syncs on vsync
    }
}
```

---

## Component System

### Component Trait

```rust
/// A reusable UI component.
pub trait Component: Send + Sync {
    /// Render this component to a node tree
    fn render(&self, ctx: &RenderContext) -> Node;

    /// Unique ID for diffing (optional, auto-generated if not provided)
    fn id(&self) -> Option<NodeId> { None }

    /// Handle events (optional)
    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }
}

/// Result of handling an event
pub enum EventResult {
    Ignored,
    Consumed,
    Redraw,
    Exit,
}
```

### Built-in Components

#### Box (Flexbox Container)

```rust
/// Flexbox container with builder API
pub struct Box {
    style: Style,
    children: Vec<Node>,
}

impl Box {
    pub fn new() -> Self { ... }

    // Direction
    pub fn row(self) -> Self { self.flex_direction(FlexDirection::Row) }
    pub fn column(self) -> Self { self.flex_direction(FlexDirection::Column) }

    // Flex container
    pub fn flex_direction(self, v: FlexDirection) -> Self { ... }
    pub fn flex_wrap(self, v: FlexWrap) -> Self { ... }
    pub fn justify_content(self, v: JustifyContent) -> Self { ... }
    pub fn align_items(self, v: AlignItems) -> Self { ... }
    pub fn gap(self, v: impl Into<Gap>) -> Self { ... }

    // Flex item
    pub fn flex_grow(self, v: f32) -> Self { ... }
    pub fn flex_shrink(self, v: f32) -> Self { ... }
    pub fn flex_basis(self, v: impl Into<Dimension>) -> Self { ... }

    // Size
    pub fn width(self, v: impl Into<Dimension>) -> Self { ... }
    pub fn height(self, v: impl Into<Dimension>) -> Self { ... }

    // Spacing
    pub fn padding(self, v: impl Into<Edges>) -> Self { ... }
    pub fn margin(self, v: impl Into<Edges>) -> Self { ... }

    // Visual
    pub fn border(self, v: BorderStyle) -> Self { ... }
    pub fn background(self, v: impl Into<Color>) -> Self { ... }

    // Children
    pub fn child(self, child: impl Into<Node>) -> Self { ... }
    pub fn children(self, children: impl IntoIterator<Item = impl Into<Node>>) -> Self { ... }
}

// Usage
Box::new()
    .column()
    .padding(1)
    .gap(1)
    .border(BorderStyle::Rounded)
    .child(Text::new("Hello"))
    .child(Text::new("World"))
```

#### Text

```rust
/// Styled text with wrapping/truncation
pub struct Text {
    content: TextContent,
    style: TextStyle,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self { ... }
    pub fn dynamic(f: impl Fn() -> String + Send + Sync + 'static) -> Self { ... }

    // Style
    pub fn bold(self) -> Self { ... }
    pub fn italic(self) -> Self { ... }
    pub fn underline(self) -> Self { ... }
    pub fn strikethrough(self) -> Self { ... }
    pub fn dim(self) -> Self { ... }

    // Colors
    pub fn color(self, v: impl Into<Color>) -> Self { ... }
    pub fn fg(self, v: impl Into<Color>) -> Self { self.color(v) }
    pub fn bg(self, v: impl Into<Color>) -> Self { ... }

    // Wrapping
    pub fn wrap(self, v: TextWrap) -> Self { ... }
    pub fn truncate(self) -> Self { self.wrap(TextWrap::Truncate) }
    pub fn no_wrap(self) -> Self { self.wrap(TextWrap::NoWrap) }
}

// Usage
Text::new("Error: File not found")
    .bold()
    .color(Color::Red)
```

#### Scroll Component (Google Ink PR #788)

```rust
/// Scrollable viewport with virtual scrollbar
/// Implements overflow: scroll from CSS
pub struct Scroll {
    /// Content to scroll
    children: Vec<Node>,
    /// Scroll position
    offset_x: u16,
    offset_y: u16,
    /// Visible size (set by layout)
    viewport: Option<(u16, u16)>,
    /// Virtual scrollbar visibility
    scrollbar: ScrollbarVisibility,
    /// Scroll direction
    direction: ScrollDirection,
}

#[derive(Default)]
pub enum ScrollbarVisibility {
    #[default]
    Auto,       // Show when content overflows
    Always,     // Always show
    Never,      // Never show (still scrollable)
}

#[derive(Default)]
pub enum ScrollDirection {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

impl Scroll {
    pub fn new() -> Self { ... }

    /// Set initial scroll position
    pub fn offset(self, x: u16, y: u16) -> Self { ... }

    /// Scroll direction
    pub fn direction(self, dir: ScrollDirection) -> Self { ... }

    /// Scrollbar visibility
    pub fn scrollbar(self, vis: ScrollbarVisibility) -> Self { ... }

    /// Add child content
    pub fn child(self, child: impl Into<Node>) -> Self { ... }

    // Programmatic scroll control (for AI/keyboard navigation)

    /// Scroll to absolute position
    pub fn scroll_to(&mut self, x: u16, y: u16) { ... }

    /// Scroll by relative amount
    pub fn scroll_by(&mut self, dx: i16, dy: i16) { ... }

    /// Scroll to make a position visible
    pub fn scroll_into_view(&mut self, x: u16, y: u16) { ... }

    /// Get current scroll position
    pub fn scroll_position(&self) -> (u16, u16) { ... }

    /// Get content size
    pub fn content_size(&self) -> (u16, u16) { ... }

    /// Get viewport size
    pub fn viewport_size(&self) -> Option<(u16, u16)> { ... }
}

// Style support for overflow
impl Style {
    pub fn overflow(self, overflow: Overflow) -> Self { ... }
}

pub enum Overflow {
    Visible,    // Content can extend beyond bounds (default)
    Hidden,     // Content clipped at bounds
    Scroll,     // Scrollable when content exceeds bounds
    Auto,       // Scroll only if needed
}

// Usage
Scroll::new()
    .direction(ScrollDirection::Vertical)
    .scrollbar(ScrollbarVisibility::Auto)
    .child(
        vbox(log_lines.iter().map(|line| Text::new(line)))
    )
```

#### Visualization Components (Tier 3)

```rust
/// 2D heatmap for weight/activation visualization
pub struct Heatmap<'a> {
    data: HeatmapData<'a>,
    width: u16,
    height: u16,
    colormap: Colormap,
    range: (f32, f32),
}

pub enum HeatmapData<'a> {
    Slice(&'a [f32]),
    GpuBuffer(&'a wgpu::Buffer),
}

impl<'a> Heatmap<'a> {
    pub fn new(data: &'a [f32]) -> Self { ... }
    pub fn gpu(buffer: &'a wgpu::Buffer) -> Self { ... }

    pub fn shape(self, width: u16, height: u16) -> Self { ... }
    pub fn colormap(self, cm: Colormap) -> Self { ... }
    pub fn range(self, min: f32, max: f32) -> Self { ... }

    /// Render directly to GPU buffer (Tier 3)
    pub fn render_gpu(&self, buffer: &mut GpuBuffer<Cell>) { ... }
}

/// Available colormaps
pub enum Colormap {
    Viridis,    // Perceptually uniform, colorblind-safe
    Plasma,
    Magma,
    Inferno,
    Turbo,      // Rainbow (not colorblind-safe)
    RdBu,       // Diverging red-blue
    Greys,      // Greyscale
    Custom(Box<dyn Fn(f32) -> Color>),
}

/// Inline sparkline chart
pub struct Sparkline<'a> {
    data: &'a [f32],
    height: u16,
    color: Color,
}

/// Line/scatter plot
pub struct Plot {
    series: Vec<Series>,
    x_range: Option<(f32, f32)>,
    y_range: Option<(f32, f32)>,
    height: u16,
}
```

---

## Hooks System

### Signal (Reactive State)

```rust
/// Reactive state container, inspired by SolidJS signals
pub struct Signal<T> {
    value: Arc<RwLock<T>>,
    subscribers: Arc<RwLock<Vec<Weak<dyn Fn() + Send + Sync>>>>,
}

impl<T: Clone> Signal<T> {
    pub fn new(value: T) -> Self { ... }

    /// Get current value
    pub fn get(&self) -> T {
        self.value.read().unwrap().clone()
    }

    /// Set new value, notifying subscribers
    pub fn set(&self, value: T) {
        *self.value.write().unwrap() = value;
        self.notify();
    }

    /// Update value with a function
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        f(&mut *self.value.write().unwrap());
        self.notify();
    }
}

// Usage
let count = Signal::new(0);
count.update(|n| *n += 1);
println!("Count: {}", count.get());
```

### Input Handling

```rust
/// Register a keyboard input handler
pub fn use_input<F>(handler: F)
where
    F: Fn(KeyEvent) + Send + Sync + 'static
{
    // Registered with the app's event loop
}

/// Key event from terminal
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

pub enum KeyCode {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
    F(u8),
    // ...
}

bitflags! {
    pub struct Modifiers: u8 {
        const SHIFT = 0b001;
        const CTRL  = 0b010;
        const ALT   = 0b100;
    }
}

// Usage
use_input(|key| {
    match key.code {
        KeyCode::Char('q') if key.modifiers.contains(Modifiers::CTRL) => {
            std::process::exit(0);
        }
        KeyCode::Up => { /* ... */ }
        _ => {}
    }
});
```

### Focus Management

```rust
/// Focus context for managing focus between components
pub struct FocusContext {
    focused: Option<NodeId>,
    focusable: Vec<NodeId>,
}

impl FocusContext {
    pub fn focus(&mut self, id: NodeId) { ... }
    pub fn focus_next(&mut self) { ... }
    pub fn focus_prev(&mut self) { ... }
    pub fn is_focused(&self, id: NodeId) -> bool { ... }
}

/// Hook for focus state
pub fn use_focus() -> FocusHandle {
    // Returns handle to check/set focus
}

pub struct FocusHandle {
    id: NodeId,
    ctx: Arc<RwLock<FocusContext>>,
}

impl FocusHandle {
    pub fn is_focused(&self) -> bool { ... }
    pub fn focus(&self) { ... }
}
```

---

## Application Framework

### App Builder

```rust
/// Application builder with fluent API
pub struct App<S> {
    state: S,
    render_fn: Box<dyn Fn(&RenderContext<S>) -> Node>,
    event_handlers: Vec<EventHandler<S>>,
    fps: u32,
    backend: Backend,
}

impl<S: Default> App<S> {
    pub fn new() -> Self { ... }
}

impl<S> App<S> {
    pub fn state(self, state: S) -> Self { ... }

    pub fn render<F>(self, f: F) -> Self
    where
        F: Fn(&RenderContext<S>) -> Node + 'static
    { ... }

    pub fn on_key<F>(self, key: KeyCode, handler: F) -> Self
    where
        F: Fn(&mut S) + 'static
    { ... }

    pub fn on_event<F>(self, handler: F) -> Self
    where
        F: Fn(&mut S, &Event) -> EventResult + 'static
    { ... }

    /// Frame timing hook (Google Ink PR #795)
    /// Called after every render with frame statistics
    pub fn on_render<F>(self, hook: F) -> Self
    where
        F: Fn(FrameStats) + 'static
    { ... }

    /// Terminal resize handler (Google Ink PR #787)
    pub fn on_resize<F>(self, handler: F) -> Self
    where
        F: Fn(&mut S, u16, u16) + 'static  // (state, width, height)
    { ... }

    pub fn fps(self, fps: u32) -> Self { ... }

    pub fn backend(self, backend: Backend) -> Self { ... }

    pub fn run(self) -> Result<(), AppError> { ... }
}

/// Frame statistics for performance monitoring
#[derive(Debug, Clone, Copy)]
pub struct FrameStats {
    /// Total frame time (layout + render + output)
    pub frame_time: Duration,
    /// Time spent in Taffy layout
    pub layout_time: Duration,
    /// Time spent painting to buffer
    pub paint_time: Duration,
    /// Time spent in diff algorithm
    pub diff_time: Duration,
    /// Time spent writing to terminal
    pub output_time: Duration,
    /// Number of cells changed this frame
    pub cells_changed: usize,
    /// Number of bytes written to terminal
    pub bytes_written: usize,
    /// Frame number (monotonically increasing)
    pub frame_number: u64,
}

pub enum Backend {
    Auto,           // Auto-detect best backend
    Crossterm,      // Standard terminal via crossterm
    Ansi,           // Raw ANSI output (Tier 1)
    Dterm(DtermConfig),  // GPU-accelerated (Tier 3)
}
```

### Elm Architecture (Optional)

```rust
/// Elm-style Model-Update-View pattern
pub trait ElmApp {
    type Model;
    type Msg;

    fn init() -> Self::Model;
    fn update(model: &mut Self::Model, msg: Self::Msg);
    fn view(model: &Self::Model) -> Node;
}

// Usage
struct Counter;

impl ElmApp for Counter {
    type Model = i32;
    type Msg = CounterMsg;

    fn init() -> i32 { 0 }

    fn update(model: &mut i32, msg: CounterMsg) {
        match msg {
            CounterMsg::Increment => *model += 1,
            CounterMsg::Decrement => *model -= 1,
        }
    }

    fn view(model: &i32) -> Node {
        vbox![
            Text::new(format!("Count: {}", model)),
            hbox![
                Button::new("-").on_click(|| CounterMsg::Decrement),
                Button::new("+").on_click(|| CounterMsg::Increment),
            ]
        ]
    }
}

fn main() {
    App::elm::<Counter>().run().unwrap();
}
```

---

## Macros

### Layout Macros

```rust
/// Vertical box (column)
macro_rules! vbox {
    [$($child:expr),* $(,)?] => {
        Box::new()
            .column()
            $(.child($child))*
    };
}

/// Horizontal box (row)
macro_rules! hbox {
    [$($child:expr),* $(,)?] => {
        Box::new()
            .row()
            $(.child($child))*
    };
}

// Usage
vbox![
    Text::new("Header").bold(),
    hbox![
        Text::new("Left"),
        Spacer::new(),
        Text::new("Right"),
    ],
    Text::new("Footer").dim(),
]
```

### JSX-like Macro (Future)

```rust
/// React/JSX-like syntax via proc macro
ink! {
    <Box column padding={1} border={BorderStyle::Rounded}>
        <Text bold>{"Hello, "}</Text>
        <Text color={Color::Cyan}>{name}</Text>

        {if show_details {
            <Box margin_top={1}>
                <Text dim>{"Details here..."}</Text>
            </Box>
        }}

        {items.iter().map(|item| {
            <Text key={item.id}>{&item.name}</Text>
        })}
    </Box>
}
```

---

## GPU Integration (Tier 3)

### dterm Integration

```rust
/// GPU-accelerated terminal interface
pub struct DtermTerminal {
    // Shared memory with dterm process
    cells: MmapMut,
    width: u16,
    height: u16,

    // GPU buffer handle (when in-process)
    gpu_buffer: Option<wgpu::Buffer>,
}

impl DtermTerminal {
    /// Connect to running dterm instance
    pub fn connect() -> Result<Self, DtermError> { ... }

    /// Get direct access to cell buffer
    pub fn cells(&mut self) -> &mut [Cell] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.cells.as_mut_ptr() as *mut Cell,
                (self.width * self.height) as usize,
            )
        }
    }

    /// Get GPU buffer for zero-copy rendering
    pub fn gpu_buffer(&self) -> Option<&wgpu::Buffer> {
        self.gpu_buffer.as_ref()
    }
}

/// Direct GPU rendering for visualizations
pub trait GpuRenderable {
    fn render_gpu(&self, buffer: &mut GpuBuffer<Cell>, rect: Rect);
}

impl<'a> GpuRenderable for Heatmap<'a> {
    fn render_gpu(&self, buffer: &mut GpuBuffer<Cell>, rect: Rect) {
        match &self.data {
            HeatmapData::Slice(data) => {
                // Upload to GPU, run compute shader
                buffer.upload_and_compute(data, self.colormap_shader());
            }
            HeatmapData::GpuBuffer(gpu_data) => {
                // Already on GPU, just run compute shader
                buffer.compute(gpu_data, self.colormap_shader());
            }
        }
    }
}
```

### Metal Shader (Heatmap)

```metal
// inky_heatmap.metal

#include <metal_stdlib>
using namespace metal;

struct Cell {
    uint16_t char_data;
    uint8_t fg_r, fg_g, fg_b;
    uint8_t bg_r, bg_g, bg_b;
    uint16_t flags;
};

struct HeatmapParams {
    uint width;
    uint height;
    float min_val;
    float max_val;
    uint colormap;  // 0=viridis, 1=plasma, etc.
};

// Viridis colormap lookup (256 entries baked in)
constant float3 viridis[256] = { /* ... */ };

kernel void heatmap_to_cells(
    device const float* weights [[buffer(0)]],
    device Cell* cells [[buffer(1)]],
    constant HeatmapParams& params [[buffer(2)]],
    uint2 gid [[thread_position_in_grid]]
) {
    if (gid.x >= params.width || gid.y >= params.height) return;

    uint idx = gid.y * params.width + gid.x;
    float w = weights[idx];

    // Normalize
    float t = saturate((w - params.min_val) / (params.max_val - params.min_val));

    // Colormap lookup
    uint ci = uint(t * 255.0);
    float3 color = viridis[ci];

    // Write to cell
    cells[idx].char_data = 0x2588;  // █
    cells[idx].bg_r = uint8_t(color.r * 255);
    cells[idx].bg_g = uint8_t(color.g * 255);
    cells[idx].bg_b = uint8_t(color.b * 255);
    cells[idx].flags |= 0x0800;  // DIRTY flag
}
```

---

## Performance Targets

| Metric | JS Ink | Ratatui | inky (Tier 2) | inky (Tier 3) |
|--------|--------|---------|---------------|---------------|
| **Startup time** | ~100ms | <5ms | <5ms | <5ms |
| **Memory (empty)** | ~30MB | ~1MB | <1MB | <1MB |
| **Memory (10K nodes)** | ~50MB | ~5MB | <2MB | <2MB |
| **Frame (no change)** | ~1ms | ~0.5ms | ~0.1ms | ~0ms |
| **Frame (1 cell)** | ~0.5ms | ~0.3ms | ~0.1ms | <0.05ms |
| **Frame (full 80x24)** | ~5ms | ~2ms | ~1ms | <0.5ms |
| **Frame (full 200x50)** | ~15ms | ~8ms | ~4ms | <1ms |
| **Max FPS** | 60 | 60 | 120 | 120 |
| **Input latency** | ~10ms | ~5ms | <3ms | <1ms |

### Benchmark Scenarios

```rust
// bench/throughput.rs

#[bench]
fn bench_static_render(b: &mut Bencher) {
    // Render static content, measure frame time
    // Target: <0.1ms
}

#[bench]
fn bench_single_cell_update(b: &mut Bencher) {
    // Change one cell, measure full pipeline
    // Target: <0.1ms
}

#[bench]
fn bench_streaming_text(b: &mut Bencher) {
    // Append characters one at a time (LLM simulation)
    // Target: >10K chars/sec
}

#[bench]
fn bench_full_redraw(b: &mut Bencher) {
    // Complete redraw of 200x50 terminal
    // Target: <4ms (Tier 2), <1ms (Tier 3)
}

#[bench]
fn bench_layout_1000_nodes(b: &mut Bencher) {
    // Taffy layout computation
    // Target: <2ms
}

#[bench]
fn bench_heatmap_64x64(b: &mut Bencher) {
    // Heatmap visualization update
    // Target: <1ms (Tier 3)
}
```

---

## Implementation Phases

### Phase 1: Core Foundation ✅
**Goal:** Make existing code compile and add layout

- [x] Node types (Root, Box, Text, Static)
- [x] Style struct with Taffy mapping
- [x] Create stub modules (layout, render, diff, app, etc.)
- [x] Implement LayoutEngine with Taffy
- [x] Basic Buffer and Cell types
- [x] Unit tests for core types

**Deliverable:** `cargo test` passes, basic layout works

### Phase 2: Rendering Pipeline ✅
**Goal:** Render to terminal

- [x] Implement Buffer with damage tracking
- [x] ANSI renderer (Tier 1)
- [x] Diff algorithm
- [x] Crossterm backend
- [x] Basic App runner

**Deliverable:** Hello world renders to terminal

### Phase 3: Component Library ✅
**Goal:** Useful components

- [x] Box with full flexbox
- [x] Text with wrapping/truncation
- [x] Spacer, Stack
- [x] Input field
- [x] Select list
- [x] Progress bar, Spinner

**Deliverable:** Interactive todo app example

### Phase 4: Hooks and Reactivity ✅
**Goal:** React-like state management

- [x] Signal<T> implementation
- [x] use_input() hook
- [x] use_focus() hook
- [x] use_interval() hook
- [x] Event propagation

**Deliverable:** Counter example with reactive state

### Phase 5: Macros and Ergonomics ✅
**Goal:** Developer experience

- [x] vbox![], hbox![] macros
- [x] ink!{} proc macro (JSX-like)
- [x] Style builder improvements
- [x] Error messages

**Deliverable:** Claude TUI recreation

### Phase 6: Capability Detection & Degradation (Complete)
**Goal:** Work everywhere, shine on dterm

- [x] Capabilities struct and detection
- [x] Tier selection logic
- [x] AdaptiveComponent trait
- [x] Graceful degradation for Heatmap, Sparkline, Progress
- [x] Upgrade prompt component
- [x] Tier 0 fallback (dumb terminals, CI logs)

**Deliverable:** App works on Terminal.app, suggests dashterm2

### Phase 7: GPU Integration (Tier 3) ✅
**Goal:** Real-time visualization

- [x] dterm IPC protocol (SharedMemory via mmap)
- [x] GpuBuffer abstraction
- [x] GPU Sparkline, Plot components
- [x] Zero-copy data binding (SharedMemoryBuffer + SharedPerception)
- [x] Prelude exports for SharedPerception, discover_shared_buffers
- [⏸] GPU Heatmap with Metal shader (deferred - CPU works)

**Deliverable:** Neural network visualization at 120 FPS

### Phase 8: Mobile Support (iOS/iPadOS) (Deferred)
**Goal:** Touch and voice input

- [ ] Touch gesture system
- [ ] Gesture recognizers (tap, swipe, pinch, pan)
- [ ] Voice input (STT) integration
- [ ] Voice command parser
- [ ] Voice output (TTS) for accessibility
- [ ] Unified Action system
- [ ] Virtual keyboard fallback

**Deliverable:** inky app runs on iPad with voice control

### Phase 9: Polish and Release ✅
**Goal:** Production ready

- [x] Documentation
- [x] Benchmarks
- [x] Examples gallery
- [x] Accessibility audit
- [x] crates.io release (ready)

---

## Success Criteria

1. **Performance:** Faster than JS Ink in every metric
2. **Memory:** <2MB for typical apps (vs Ink's 30MB+)
3. **Features:** Superset of Ink's component model
4. **Visualization:** 120 FPS heatmaps (Tier 3)
5. **Ergonomics:** As pleasant as React to write
6. **Compatibility:** Works with any terminal (Tier 0-3)
7. **Graceful degradation:** Same app runs everywhere, adapts to capabilities
8. **Mobile:** Voice + touch input on iOS/iPadOS
9. **Accessibility:** Full TTS/STT support, screen reader compatible
10. **Proof:** Claude Code TUI runs on inky across all platforms

---

## Platform Support & Graceful Degradation

### Capability Detection

inky automatically detects terminal capabilities and selects the best rendering tier:

```rust
let caps = Capabilities::detect();
match caps.tier {
    RenderTier::Tier3Gpu => /* dashterm2/dterm: 120 FPS, <1ms */,
    RenderTier::Tier2Retained => /* iTerm2/Kitty: 60 FPS, true color */,
    RenderTier::Tier1Ansi => /* Terminal.app: 30 FPS, 256 colors */,
    RenderTier::Tier0Fallback => /* dumb terminal: text only */,
}
```

### Degradation Examples

| Component | Tier 3 (GPU) | Tier 2 (Retained) | Tier 1 (ANSI) | Tier 0 (Fallback) |
|-----------|--------------|-------------------|---------------|-------------------|
| **Heatmap** | GPU shader, smooth gradients | Unicode blocks + true color | ASCII density chars | Summary stats only |
| **Sparkline** | GPU line rendering | Braille characters | ASCII graph | Min/max/trend text |
| **Progress** | Smooth animation | Unicode bar ▓░ | ASCII [====] | Percentage text |
| **Images** | Native display | Sixel/Kitty protocol | ASCII art | [Image: description] |

### Upgrade Suggestions

When features are degraded, inky can suggest terminal upgrades:

```
┌────────────────────────────────────────────────────────────────┐
│ ⚡ This visualization runs faster in dashterm2                 │
│                                                                │
│ Current: Terminal.app (30 FPS, ~16ms latency)                  │
│ Recommended: dashterm2 (120 FPS, <1ms latency)                 │
│                                                                │
│ [Download dashterm2]  [Dismiss]  [Don't show again]            │
└────────────────────────────────────────────────────────────────┘
```

---

## Mobile Support (iOS/iPadOS)

### Input Hierarchy

Mobile terminals powered by dterm support multiple input modes:

```
Priority 1: VOICE (STT)     - "scroll down", "select line 42", "type hello"
Priority 2: GESTURE         - swipe, pinch, pan for navigation
Priority 3: TOUCH           - tap to select, long press for context menu
Priority 4: VIRTUAL KB      - fallback on-screen keyboard
```

### Voice Commands (STT)

```rust
pub enum VoiceCommand {
    // Navigation
    ScrollUp(Option<u32>),      // "scroll up 10 lines"
    ScrollDown(Option<u32>),
    GoToTop,                    // "go to top"
    GoToBottom,

    // Selection
    Select(String),             // "select error"
    SelectLine(u32),            // "select line 42"
    Copy, Paste,

    // Input
    Type(String),               // "type hello world"
    Enter, Cancel, Delete,

    // App control
    Quit, Help, Undo, Redo,
}
```

### Voice Output (TTS)

For accessibility and hands-free operation:

```rust
pub enum AnnounceMode {
    All,              // Full screen reader (accessibility)
    FocusAndEvents,   // Announce focus changes and errors
    OnDemand,         // Only when requested
    None,             // Silent
}
```

### Touch Gestures

| Gesture | Action |
|---------|--------|
| Tap | Select / Activate |
| Double tap | Expand / Collapse |
| Long press | Context menu |
| Swipe up/down | Scroll |
| Pinch | Zoom |
| Two-finger pan | Scroll (precise) |
| Two-finger tap | Secondary action |
| Three-finger swipe | Jump to top/bottom |

### Unified Input System

All input sources (keyboard, mouse, touch, voice, gamepad) normalize to unified `Action` types:

```rust
pub enum Action {
    // Navigation
    ScrollUp(u32), ScrollDown(u32),
    ScrollToTop, ScrollToBottom,
    FocusNext, FocusPrev,

    // Selection
    Select, Activate, Cancel,
    SelectText(String), SelectLine(u32),

    // Editing
    InsertText(String), Delete, Submit,
    Undo, Redo, Copy, Paste,

    // App
    ShowHelp, Quit,
    Custom(String),
}
```

---

## References

### Frameworks Researched
- [Ink](https://github.com/vadimdemedes/ink) - React for CLIs
- [Ratatui](https://github.com/ratatui/ratatui) - Rust TUI
- [Textual](https://github.com/Textualize/textual) - Python TUI with CSS
- [Bubbletea](https://github.com/charmbracelet/bubbletea) - Go Elm architecture
- [Lip Gloss](https://github.com/charmbracelet/lipgloss) - Go styling
- [Cursive](https://github.com/gyscos/cursive) - Rust callback TUI
- [Brick](https://github.com/jtdaugherty/brick) - Haskell declarative TUI
- [notcurses](https://github.com/dankamongmen/notcurses) - C high-performance
- [Terminal.Gui](https://github.com/gui-cs/Terminal.Gui) - .NET TUI

### Dependencies
- [Taffy](https://github.com/DioxusLabs/taffy) - Flexbox/Grid layout
- [Crossterm](https://github.com/crossterm-rs/crossterm) - Terminal I/O
- [wgpu](https://github.com/gfx-rs/wgpu) - GPU abstraction

### Internal
- `~/dterm/` - GPU-accelerated terminal emulator
- `~/dashterm2/` - iTerm2 fork with dterm-core
