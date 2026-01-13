# WezTerm Analysis Report

**Analysis Date:** 2025-12-27
**Repository:** https://github.com/wezterm/wezterm
**Purpose:** Research for dTerm development

---

## 1. Overview

### Basic Information

| Attribute | Value |
|-----------|-------|
| **Language** | Rust |
| **License** | MIT |
| **LOC (Rust)** | ~410,000 lines |
| **Author** | Wez Furlong (@wez) |
| **First Release** | 2018 |
| **Active Development** | Yes (actively maintained) |

### Project Description

WezTerm is a GPU-accelerated cross-platform terminal emulator and multiplexer. It runs on macOS, Windows, and Linux, with native UI implementations for each platform. The project emphasizes:

- GPU-accelerated rendering (OpenGL/WebGPU)
- Built-in terminal multiplexer
- SSH client integration
- Lua-based configuration
- Extensive escape sequence support
- Font rendering with fallback chains

---

## 2. Architecture

### Crate Organization

WezTerm uses a workspace with ~50+ crates organized into clear functional areas:

```
wezterm/
├── Core Terminal Emulation
│   ├── vtparse/              # Low-level VT parser state machine
│   ├── wezterm-escape-parser/# Semantic escape sequence parsing
│   ├── term/                 # Terminal state machine & screen model
│   └── termwiz/              # Terminal library (escape encoding, Surface, widgets)
│
├── Multiplexer Layer
│   ├── mux/                  # Pane/Tab/Window multiplexing
│   ├── codec/                # Wire protocol for mux server
│   ├── wezterm-client/       # Mux client implementation
│   └── wezterm-mux-server*/  # Mux server implementation
│
├── PTY & SSH
│   ├── pty/                  # Cross-platform PTY abstraction
│   ├── wezterm-ssh/          # SSH client (libssh-rs or ssh2)
│   └── filedescriptor/       # Cross-platform file descriptor handling
│
├── Rendering & GUI
│   ├── wezterm-gui/          # Main GUI application
│   ├── wezterm-font/         # Font loading, shaping, rasterization
│   ├── window/               # Cross-platform window abstraction
│   └── wezterm-surface/      # Surface/cell model
│
├── Configuration
│   ├── config/               # Configuration parsing, Lua integration
│   └── lua-api-crates/       # Lua API modules (15+ crates)
│
└── Utilities
    ├── color-types/          # Color type definitions
    ├── wezterm-dynamic/      # Dynamic typing for config
    ├── wezterm-input-types/  # Input event types
    ├── rangeset/             # Range set data structure
    ├── bidi/                 # Bidirectional text support
    └── ... (various helpers)
```

### Key Abstractions

#### 1. Domain Model (`mux/src/domain.rs`)

The `Domain` trait represents different sources of terminal sessions:

```rust
#[async_trait(?Send)]
pub trait Domain: Downcast + Send + Sync {
    async fn spawn(&self, size: TerminalSize, command: Option<CommandBuilder>,
                   command_dir: Option<String>, window: WindowId) -> anyhow::Result<Arc<Tab>>;
    async fn spawn_pane(&self, size: TerminalSize, command: Option<CommandBuilder>,
                        command_dir: Option<String>) -> anyhow::Result<Arc<dyn Pane>>;
    fn domain_id(&self) -> DomainId;
    fn domain_name(&self) -> &str;
    async fn attach(&self, window_id: Option<WindowId>) -> anyhow::Result<()>;
    fn detach(&self) -> anyhow::Result<()>;
    fn state(&self) -> DomainState;
}
```

Domain implementations:
- Local PTY
- SSH remote
- WSL (Windows Subsystem for Linux)
- Serial ports
- Mux server connections

#### 2. Pane Trait (`mux/src/pane.rs`)

The `Pane` trait defines the interface for a terminal view:

```rust
#[async_trait(?Send)]
pub trait Pane: Downcast + Send + Sync {
    fn pane_id(&self) -> PaneId;
    fn get_cursor_position(&self) -> StableCursorPosition;
    fn get_current_seqno(&self) -> SequenceNo;
    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>);
    fn get_changed_since(&self, lines: Range<StableRowIndex>, seqno: SequenceNo)
        -> RangeSet<StableRowIndex>;
    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()>;
    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()>;
    // ... 30+ methods
}
```

#### 3. Tab/Split Management (`mux/src/tab.rs`)

Tabs use a binary tree structure for split management:

```rust
pub type Tree = bintree::Tree<Arc<dyn Pane>, SplitDirectionAndSize>;
```

This enables:
- Horizontal and vertical splits
- Recursive nested splits
- Dynamic resizing
- Zoom functionality

---

## 3. Terminal Emulation

### VT Parser (`vtparse/`)

WezTerm implements a state machine based on the [DEC ANSI Parser](https://vt100.net/emu/dec_ansi_parser), modified for UTF-8 support.

**Key Design Decisions:**

1. **State table-driven parsing** - Uses precomputed transition tables for performance:

```rust
fn lookup(state: State, b: u8) -> (Action, State) {
    let v = unsafe {
        TRANSITIONS
            .get_unchecked(state as usize)
            .get_unchecked(b as usize)
    };
    (Action::from_u16(v >> 8), State::from_u16(v & 0xff))
}
```

2. **VTActor trait** - Allows different implementations to handle parsed sequences:

```rust
pub trait VTActor {
    fn print(&mut self, b: char);
    fn execute_c0_or_c1(&mut self, control: u8);
    fn csi_dispatch(&mut self, params: &[CsiParam], parameters_truncated: bool, byte: u8);
    fn osc_dispatch(&mut self, params: &[&[u8]]);
    fn dcs_hook(&mut self, ...);
    fn apc_dispatch(&mut self, data: Vec<u8>);
}
```

3. **no_std support** - Parser can run without standard library allocation

### Escape Sequence Handling (`wezterm-escape-parser/`)

Semantic parsing layer on top of vtparse:

```rust
pub enum Action {
    Print(char),
    PrintString(String),
    Control(ControlCode),
    DeviceControl(DeviceControlMode),
    OperatingSystemCommand(Box<OperatingSystemCommand>),
    CSI(CSI),
    Esc(Esc),
    Sixel(Box<Sixel>),
    KittyImage(Box<KittyImage>),
    XtGetTcap(Vec<String>),
}
```

**Supported Protocols:**
- ECMA-48 / ANSI escape sequences
- xterm extensions
- Kitty graphics protocol
- Kitty keyboard protocol
- Sixel graphics
- iTerm2 image protocol
- OSC 8 hyperlinks
- OSC 52 clipboard
- OSC 133 shell integration
- DEC private modes

### Terminal State (`term/`)

The `Screen` struct manages terminal state:

```rust
pub struct Screen {
    lines: VecDeque<Line>,              // Scrollback + visible
    stable_row_index_offset: usize,     // For stable line references
    config: Arc<dyn TerminalConfiguration>,
    allow_scrollback: bool,
    keyboard_stack: Vec<KeyboardEncoding>,
    physical_rows: usize,
    physical_cols: usize,
    dpi: u32,
    saved_cursor: Option<SavedCursor>,
}
```

**Features:**
- Primary and alternate screen support
- Line wrapping and rewrapping on resize
- Stable row indices (survive scrollback pruning)
- Sequence numbers for change tracking (differential rendering)
- BiDi text support

---

## 4. Multiplexer

### Architecture

The Mux is the central coordinator for all terminal sessions:

```rust
pub struct Mux {
    tabs: RwLock<HashMap<TabId, Arc<Tab>>>,
    panes: RwLock<HashMap<PaneId, Arc<dyn Pane>>>,
    windows: RwLock<HashMap<WindowId, Window>>,
    default_domain: RwLock<Option<Arc<dyn Domain>>>,
    domains: RwLock<HashMap<DomainId, Arc<dyn Domain>>>,
    domains_by_name: RwLock<HashMap<String, Arc<dyn Domain>>>,
    subscribers: RwLock<HashMap<usize, Box<dyn Fn(MuxNotification) -> bool>>>,
    clients: RwLock<HashMap<ClientId, ClientInfo>>,
    // ...
}
```

### Notification System

Mux uses a pub/sub pattern for state changes:

```rust
pub enum MuxNotification {
    PaneOutput(PaneId),
    PaneAdded(PaneId),
    PaneRemoved(PaneId),
    WindowCreated(WindowId),
    WindowRemoved(WindowId),
    Alert { pane_id: PaneId, alert: Alert },
    AssignClipboard { pane_id: PaneId, selection: ClipboardSelection, clipboard: Option<String> },
    TabAddedToWindow { tab_id: TabId, window_id: WindowId },
    // ...
}
```

### Output Parser Pipeline

Terminal output goes through a pipeline with coalescing:

```rust
fn parse_buffered_data(pane: Weak<dyn Pane>, dead: &Arc<AtomicBool>, mut rx: FileDescriptor) {
    let mut parser = termwiz::escape::parser::Parser::new();
    let mut actions = vec![];
    let mut hold = false;  // For synchronized output mode

    loop {
        match rx.read(&mut buf) {
            Ok(size) => {
                parser.parse(&buf[0..size], |action| {
                    // Handle synchronized output (DECSET 2026)
                    match &action {
                        Action::CSI(CSI::Mode(Mode::SetDecPrivateMode(...))) => {
                            hold = true;
                            // Flush prior actions
                        }
                        Action::CSI(CSI::Mode(Mode::ResetDecPrivateMode(...))) => {
                            hold = false;
                        }
                        _ => {}
                    }
                    action.append_to(&mut actions);
                });
                // Coalesce with delay for unoptimized TUI programs
            }
        }
    }
}
```

---

## 5. Rendering Pipeline

### Dual Renderer Support

WezTerm supports both OpenGL (via glium) and WebGPU (via wgpu):

```rust
pub enum RenderContext {
    Glium(Rc<GliumContext>),
    WebGpu(Rc<WebGpuState>),
}
```

### Shader Architecture (`shader.wgsl`)

The WebGPU shader handles multiple glyph types:

```wgsl
// Vertex input with color blending support
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) alt_color: vec4<f32>,
    @location(4) hsv: vec3<f32>,
    @location(5) has_color: f32,
    @location(6) mix_value: f32,
};

// Fragment types
const IS_GLYPH: f32 = 0.0;        // Monochrome text
const IS_COLOR_EMOJI: f32 = 1.0;  // Color emoji
const IS_BG_IMAGE: f32 = 2.0;     // Background image
const IS_SOLID_COLOR: f32 = 3.0;  // Solid color block
const IS_GRAY_SCALE: f32 = 4.0;   // Non-AA text
```

### Glyph Cache (`glyphcache.rs`)

Sophisticated caching with borrowed keys to avoid allocation:

```rust
pub struct GlyphKey {
    pub font_idx: usize,
    pub glyph_pos: u32,
    pub num_cells: u8,
    pub style: TextStyle,
    pub followed_by_space: bool,
    pub metric: CellMetricKey,
    pub id: LoadedFontId,
}

// Borrowed version to avoid allocation during lookups
pub struct BorrowedGlyphKey<'a> {
    pub font_idx: usize,
    pub glyph_pos: u32,
    // ...
}
```

### Font System (`wezterm-font/`)

Comprehensive font handling:
- Font locator (fontconfig, DirectWrite, CoreText)
- Font shaping (HarfBuzz)
- Font rasterization (FreeType)
- Fallback chains
- Ligature support
- Color emoji support

---

## 6. PTY Integration

### Cross-Platform Abstraction (`pty/`)

```rust
pub trait PtySystem: Downcast {
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair>;
}

pub trait MasterPty: Downcast + Send {
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    fn get_size(&self) -> Result<PtySize, Error>;
    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, Error>;
    fn take_writer(&self) -> Result<Box<dyn Write + Send>, Error>;

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t>;
    fn tty_name(&self) -> Option<PathBuf>;
}

pub trait SlavePty {
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<dyn Child + Send + Sync>, Error>;
}
```

### Platform Implementations

- **Unix** (`unix.rs`): Uses `openpty()` syscall
- **Windows** (`win/`): Uses ConPTY
- **Serial** (`serial.rs`): Serial port support

### File Descriptor Cleanup

Special handling for leaked file descriptors (macOS Big Sur issue):

```rust
/// On Big Sur, Cocoa leaks various file descriptors to child processes,
/// so we need to make a pass through the open descriptors beyond just the
/// stdio descriptors and close them all out.
fn close_random_fds() {
    // Enumerate /dev/fd and close everything > 2
}
```

---

## 7. Configuration System

### Lua-Based Configuration

WezTerm uses Lua (via mlua) for configuration:

```rust
// Configuration loading with dynamic typing
pub fn make_lua_context(config_dir: &Path) -> anyhow::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    // Set up package.path for user modules
    // Register wezterm module
    // Load and execute config file
    Ok(lua)
}
```

### Hot Reload

Configuration is monitored via `notify` crate:

```rust
lazy_static! {
    static ref CONFIG: Configuration = Configuration::new();
    // ...
}

// File watcher triggers reload
fn watch_config_changes() {
    let (tx, rx) = channel();
    let watcher = notify::recommended_watcher(tx)?;
    watcher.watch(config_path, RecursiveMode::NonRecursive)?;

    for event in rx {
        reload_configuration();
    }
}
```

### Config Validation

Runtime config validation using `wezterm-dynamic`:

```rust
fn config_builder_new_index<'lua>(
    lua: &'lua Lua,
    (myself, key, value): (Table, String, Value),
) -> mlua::Result<()> {
    // Validate against Config struct schema
    let config_object = Config::from_dynamic(&dvalue, options)?;
    // ...
}
```

---

## 8. SSH Integration

### Dual Backend Support

WezTerm supports two SSH libraries:

```rust
#[cfg(not(any(feature = "libssh-rs", feature = "ssh2")))]
compile_error!("Either libssh-rs or ssh2 must be enabled!");
```

### SSH Session Management

```rust
// Session modules
mod auth;           // Authentication handling
mod channelwrap;    // Channel abstraction
mod config;         // SSH config parsing (~52K lines!)
mod host;           // Host key verification
mod pty;            // PTY over SSH
mod session;        // Session lifecycle
mod sftp;           // SFTP support
```

### Features

- SSH config file parsing (`~/.ssh/config`)
- Agent forwarding
- Host key verification
- PTY allocation over SSH
- SFTP file transfer
- Jump host support

---

## 9. Performance Optimizations

### 1. Change Tracking with Sequence Numbers

Every change increments a sequence number for differential updates:

```rust
fn get_changed_since(&self, lines: Range<StableRowIndex>, seqno: SequenceNo)
    -> RangeSet<StableRowIndex>;
```

### 2. Synchronized Output (DECSET 2026)

Respects synchronized output mode to batch updates:

```rust
match &action {
    Action::CSI(CSI::Mode(Mode::SetDecPrivateMode(
        DecPrivateMode::Code(DecPrivateModeCode::SynchronizedOutput)
    ))) => {
        hold = true;
        // Flush prior actions
    }
    // ...
}
```

### 3. Output Coalescing

Delays output to coalesce frames from unoptimized TUI programs:

```rust
// If we haven't accumulated too much data,
// pause for a short while to increase the chances
// that we coalesce a full "frame"
if action_size < buf.len() {
    let poll_delay = match deadline {
        // ...
    };
}
```

### 4. Zero-Copy Glyph Cache Lookups

Uses borrowed keys to avoid allocation during cache lookups:

```rust
impl<'a> std::borrow::Borrow<dyn GlyphKeyTrait + 'a> for GlyphKey {
    fn borrow(&self) -> &(dyn GlyphKeyTrait + 'a) { self }
}
```

### 5. Atlas Texture Packing

Uses guillotiere for texture atlas packing:

```rust
fn allocate_texture_atlas(&self, size: usize) -> anyhow::Result<Rc<dyn Texture2d>>
```

### 6. LFU Cache for Glyphs

Uses LFU (Least Frequently Used) caching for glyphs:

```rust
use lfucache::LfuCache;
```

### 7. Action Coalescing

Combines consecutive `Print` actions to reduce heap allocations:

```rust
impl Action {
    pub fn append_to(self, dest: &mut Vec<Self>) {
        if let Action::Print(c) = &self {
            match dest.last_mut() {
                Some(Action::PrintString(s)) => {
                    s.push(*c);
                    return;
                }
                // ...
            }
        }
        dest.push(self);
    }
}
```

---

## 10. Strengths

### 1. Comprehensive Feature Set
- Full terminal emulation with modern protocols
- Built-in multiplexer (tmux-like functionality)
- SSH client integration
- Serial port support
- Extensive image protocol support (Sixel, iTerm2, Kitty)

### 2. Strong Cross-Platform Support
- Native UI on macOS, Windows, Linux
- Same core codebase everywhere
- Platform-specific optimizations

### 3. Lua Configuration
- Powerful, extensible configuration
- Hot reload support
- Event callbacks and hooks
- Plugin system

### 4. Code Organization
- Clear separation of concerns
- Well-documented public APIs
- Reusable crates (termwiz, vtparse published separately)

### 5. Performance Focus
- GPU acceleration
- Change tracking for differential updates
- Sophisticated caching
- Output coalescing

### 6. Modern Protocol Support
- Kitty keyboard protocol
- Kitty graphics protocol
- OSC 133 shell integration
- Synchronized output

---

## 11. Weaknesses/Limitations

### 1. Complexity
- ~410K lines of Rust is substantial
- 50+ crates can be overwhelming
- Deep dependency tree

### 2. No Formal Verification
- No TLA+ specifications
- No Kani proofs
- Relies on testing and fuzzing

### 3. Memory Usage
- Full scrollback is memory-resident
- No compression of scrollback
- Large glyph caches

### 4. Mobile Support Missing
- No iOS/iPadOS support
- No Android support
- Desktop-focused design

### 5. WebGPU Still Maturing
- WebGPU support is newer
- Some edge cases may remain
- OpenGL fallback still needed

### 6. SSH Configuration Complexity
- SSH config parsing is ~52K lines
- Complex edge cases
- Potential for compatibility issues

### 7. Single-Threaded GUI
- Main GUI loop is single-threaded
- Some operations can cause hitches
- Background work uses async but renders on main thread

---

## 12. Lessons for dTerm

### Patterns to Adopt

#### 1. State Machine Parser Design
The vtparse approach of table-driven parsing is efficient and should be adopted:

```rust
// Precomputed transition tables
static TRANSITIONS: [[u16; 256]; NUM_STATES] = ...;

fn lookup(state: State, b: u8) -> (Action, State) {
    // O(1) state transition lookup
}
```

#### 2. Trait-Based Abstractions
Domain, Pane, and PtySystem traits provide excellent extension points:

```rust
pub trait Domain: Downcast + Send + Sync {
    // Abstract over local, SSH, WSL, etc.
}
```

#### 3. Binary Tree for Splits
Using a binary tree for split management is elegant and efficient:

```rust
pub type Tree = bintree::Tree<Arc<dyn Pane>, SplitDirectionAndSize>;
```

#### 4. Sequence Numbers for Change Tracking
Essential for efficient differential rendering:

```rust
pub type SequenceNo = u64;

fn get_changed_since(&self, lines: Range<StableRowIndex>, seqno: SequenceNo)
    -> RangeSet<StableRowIndex>;
```

#### 5. Borrowed Keys for Cache Lookups
Avoids allocation during hot paths:

```rust
pub struct BorrowedGlyphKey<'a> { ... }
impl<'a> std::borrow::Borrow<dyn GlyphKeyTrait + 'a> for GlyphKey { ... }
```

### Patterns to Improve Upon

#### 1. Add Formal Verification
dTerm should add TLA+ specs for:
- Parser state machine
- Mux state transitions
- Screen resize semantics

#### 2. Scrollback Compression
Consider delta compression for scrollback:

```rust
// Instead of VecDeque<Line>
struct CompressedScrollback {
    compressed_blocks: Vec<CompressedBlock>,
    recent: VecDeque<Line>,  // Hot data uncompressed
}
```

#### 3. Mobile-First Design
Design abstractions that work on mobile from day 1:
- Touch input handling
- Power-efficient rendering
- Offline-first sync

#### 4. Agent-Native APIs
Add first-class support for AI agents:

```rust
pub trait AgentTerminal {
    async fn send_command(&self, cmd: &str) -> Result<CommandResult>;
    async fn approve_operation(&self, op: &Operation) -> Result<bool>;
    fn semantic_zones(&self) -> Vec<SemanticZone>;
}
```

### Specific Code Worth Studying

| File | Why |
|------|-----|
| `vtparse/src/lib.rs` | Clean state machine implementation |
| `mux/src/domain.rs` | Domain trait design |
| `mux/src/tab.rs` | Binary tree split management |
| `pty/src/lib.rs` | Cross-platform PTY abstraction |
| `term/src/screen.rs` | Screen model with stable indices |
| `wezterm-gui/src/shader.wgsl` | GPU rendering approach |
| `wezterm-escape-parser/src/csi.rs` | Comprehensive CSI parsing |

### Dependency Choices to Consider

| WezTerm Choice | dTerm Consideration |
|----------------|---------------------|
| mlua (Lua) | Consider embedding Lua or using WASM for plugins |
| glium + wgpu | wgpu-only for simpler codebase |
| parking_lot | Good choice, keep it |
| anyhow + thiserror | Good choice, keep it |
| euclid | Good for geometric types |
| harfbuzz (via custom wrapper) | Consider swash or cosmic-text as alternatives |

---

## Summary

WezTerm is a mature, feature-rich terminal emulator with excellent architecture. For dTerm, the key takeaways are:

1. **Adopt**: State machine parsing, trait abstractions, sequence numbers, binary tree splits
2. **Improve**: Add formal verification, compress scrollback, design for mobile
3. **Skip**: Lua configuration complexity (consider simpler TOML + WASM plugins)
4. **Learn**: Study the crate organization and clean separation of concerns

The codebase demonstrates that a high-quality cross-platform terminal emulator is achievable in Rust, and provides many patterns worth adopting in dTerm.
