# Zellij Terminal Multiplexer Analysis

**Analyzed for:** dTerm session management research
**Date:** 2025-12-28
**Zellij Version:** 0.41 (as of repo snapshot)

---

## Executive Summary

Zellij is a Rust-based terminal multiplexer that provides valuable patterns for dTerm's session management. Key takeaways:

1. **Client-Server Architecture**: Clean separation allows sessions to survive client disconnects
2. **Session Resurrection**: Sessions can be serialized to KDL layout files and restored
3. **WASM Plugin System**: Sandboxed plugins with explicit permissions
4. **Thread-per-Component**: Each major subsystem runs in its own thread with message passing
5. **VTE Parser**: Uses the `vte` crate for terminal emulation

---

## 1. Architecture Overview

### 1.1 Crate Structure

```
zellij/
├── zellij/                  # CLI entry point
│   └── src/main.rs         # Main binary
├── zellij-client/          # Terminal client handling
│   └── src/lib.rs          # Input handling, rendering
├── zellij-server/          # Core server logic
│   ├── src/lib.rs          # Server startup
│   ├── src/pty.rs          # PTY management
│   ├── src/panes/          # Pane and grid logic
│   │   └── grid.rs         # Terminal grid state
│   ├── src/plugins/        # WASM plugin system
│   │   └── wasm_bridge.rs  # WASM runtime
│   └── src/thread_bus.rs   # Inter-thread communication
├── zellij-utils/           # Shared utilities
│   ├── src/ipc.rs          # Client-server IPC
│   ├── src/input/          # Input handling
│   │   ├── config.rs       # Configuration
│   │   └── layout.rs       # Layout definitions
│   ├── src/kdl/            # KDL parsing
│   └── src/sessions.rs     # Session management
└── zellij-tile/            # Plugin SDK
    └── src/lib.rs          # ZellijPlugin trait
```

### 1.2 Component Communication

**Key File:** `/zellij-server/src/thread_bus.rs`

```rust
pub struct ThreadSenders {
    pub to_screen: Option<SenderWithContext<ScreenInstruction>>,
    pub to_pty: Option<SenderWithContext<PtyInstruction>>,
    pub to_plugin: Option<SenderWithContext<PluginInstruction>>,
    pub to_server: Option<SenderWithContext<ServerInstruction>>,
    pub to_pty_writer: Option<SenderWithContext<PtyWriteInstruction>>,
    pub to_background_jobs: Option<SenderWithContext<BackgroundJob>>,
}
```

Each thread has its own instruction enum and receiver. The `Bus<T>` struct wraps receivers with OS input access:

```rust
pub(crate) struct Bus<T> {
    receivers: Vec<channels::Receiver<(T, ErrorContext)>>,
    pub senders: ThreadSenders,
    pub os_input: Option<Box<dyn ServerOsApi>>,
}
```

**dTerm Pattern:** This message-passing architecture is excellent for dTerm's multi-platform needs. Each platform could have its own input/output thread while sharing core logic.

---

## 2. Session Persistence

### 2.1 Session Discovery

**Key File:** `/zellij-utils/src/sessions.rs`

Sessions are identified by Unix sockets in `ZELLIJ_SOCK_DIR`:

```rust
pub fn get_sessions() -> Result<Vec<(String, Duration)>, io::ErrorKind> {
    match fs::read_dir(&*ZELLIJ_SOCK_DIR) {
        Ok(files) => {
            let mut sessions = Vec::new();
            files.for_each(|file| {
                if file.file_type().unwrap().is_socket() && assert_socket(&file_name) {
                    sessions.push((file_name, duration));
                }
            });
            Ok(sessions)
        },
        // ...
    }
}
```

Socket validation checks if the server responds:

```rust
fn assert_socket(name: &str) -> bool {
    match LocalSocketStream::connect(path) {
        Ok(stream) => {
            let mut sender: IpcSenderWithContext<ClientToServerMsg> =
                IpcSenderWithContext::new(stream);
            sender.send_client_msg(ClientToServerMsg::ConnStatus);
            let mut receiver = sender.get_receiver();
            match receiver.recv_server_msg() {
                Some((ServerToClientMsg::Connected, _)) => true,
                _ => false,
            }
        },
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            drop(fs::remove_file(path)); // Cleanup stale socket
            false
        },
        Err(_) => false,
    }
}
```

### 2.2 Session Resurrection

**Key File:** `/zellij-utils/src/sessions.rs`

Sessions can be "resurrected" from cached layout files:

```rust
pub fn get_resurrectable_sessions() -> Vec<(String, Duration)> {
    match fs::read_dir(&*ZELLIJ_SESSION_INFO_CACHE_DIR) {
        Ok(files_in_session_info_folder) => {
            files_that_are_folders
                .filter_map(|folder_name| {
                    let layout_file_name = session_layout_cache_file_name(&folder_name);
                    if std::path::Path::new(&layout_file_name).exists() {
                        Some((session_name, elapsed_duration))
                    } else {
                        None
                    }
                })
                .collect()
        },
        // ...
    }
}

pub fn resurrection_layout(session_name: &str) -> Result<Option<Layout>, String> {
    let layout_file_name = session_layout_cache_file_name(&session_name);
    let raw_layout = std::fs::read_to_string(&layout_file_name)?;
    Layout::from_kdl(&raw_layout, Some(layout_file_name), None, None)
}
```

### 2.3 Session Metadata

**Key File:** `/zellij-server/src/session_layout_metadata.rs`

```rust
pub struct SessionLayoutMetadata {
    default_layout: Box<Layout>,
    global_cwd: Option<PathBuf>,
    pub default_shell: Option<PathBuf>,
    pub default_editor: Option<PathBuf>,
    tabs: Vec<TabLayoutMetadata>,
}
```

Metadata tracks:
- Terminal commands running in each pane
- Working directories
- Focused panes per client
- Whether the session is "dirty" (modified from base layout)

### 2.4 Session Serialization

**Key File:** `/zellij-utils/src/session_serialization.rs`

Sessions serialize to KDL format:

```rust
pub fn serialize_session_layout(
    global_layout_manifest: GlobalLayoutManifest,
) -> Result<(String, BTreeMap<String, String>), &'static str> {
    let mut document = KdlDocument::new();
    let mut layout_node = KdlNode::new("layout");
    // ... serialize tabs, panes, plugins
    Ok((document.to_string(), pane_contents))
}
```

**dTerm Pattern:** For dTerm, consider:
1. Serialize session state to a structured format (KDL, TOML, or JSON)
2. Store scrollback/pane contents separately from layout
3. Track "dirty" state to know when resurrection is needed

---

## 3. Layout System

### 3.1 Layout Data Structures

**Key File:** `/zellij-utils/src/input/layout.rs`

```rust
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

pub enum SplitSize {
    Percent(usize),  // 1 to 100
    Fixed(usize),    // Absolute rows/columns
}

pub enum Run {
    Plugin(RunPluginOrAlias),
    Command(RunCommand),
    EditFile(PathBuf, Option<usize>, Option<PathBuf>),
    Cwd(PathBuf),
}
```

### 3.2 Pane Geometry

```rust
pub struct PaneGeom {
    pub x: usize,
    pub y: usize,
    pub rows: Dimension,
    pub cols: Dimension,
}

pub struct Dimension {
    pub constraint: Constraint,
    pub inner: usize,
}

pub enum Constraint {
    Fixed(usize),
    Percent(f64),
}
```

### 3.3 Tab and Pane Manifests

```rust
pub struct TabLayoutManifest {
    pub tiled_panes: Vec<PaneLayoutManifest>,
    pub floating_panes: Vec<PaneLayoutManifest>,
    pub is_focused: bool,
    pub hide_floating_panes: bool,
}

pub struct PaneLayoutManifest {
    pub geom: PaneGeom,
    pub run: Option<Run>,
    pub cwd: Option<PathBuf>,
    pub is_borderless: bool,
    pub title: Option<String>,
    pub is_focused: bool,
    pub pane_contents: Option<String>,
}
```

**dTerm Pattern:** The constraint-based sizing (fixed vs percent) is essential for responsive layouts. Consider:
- Making invalid geometries unrepresentable via types
- Supporting stacked panes (multiple panes in same space, one visible)

---

## 4. Plugin System

### 4.1 Plugin Architecture

**Key File:** `/zellij-server/src/plugins/wasm_bridge.rs`

Plugins run in WASM sandboxes using `wasmi`:

```rust
pub struct WasmBridge {
    connected_clients: Arc<Mutex<Vec<ClientId>>>,
    plugin_map: Arc<Mutex<PluginMap>>,
    plugin_executor: Arc<PinnedExecutor>,
    plugin_ids_waiting_for_permission_request: HashSet<PluginId>,
    cached_events_for_pending_plugins: HashMap<PluginId, Vec<EventOrPipeMessage>>,
    // ...
}
```

### 4.2 Plugin Trait

**Key File:** `/zellij-tile/src/lib.rs`

```rust
pub trait ZellijPlugin: Default {
    fn load(&mut self, configuration: BTreeMap<String, String>) {}
    fn update(&mut self, event: Event) -> bool { false }  // return true to render
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool { false }
    fn render(&mut self, rows: usize, cols: usize) {}
}
```

### 4.3 Plugin Registration

```rust
#[macro_export]
macro_rules! register_plugin {
    ($t:ty) => {
        thread_local! {
            static STATE: std::cell::RefCell<$t> = std::cell::RefCell::new(Default::default());
        }

        fn main() {
            std::panic::set_hook(Box::new(|info| {
                report_panic(info);
            }));
        }

        #[no_mangle]
        fn load() {
            STATE.with(|state| {
                let protobuf_bytes: Vec<u8> = object_from_stdin().unwrap();
                let config = ProtobufPluginConfiguration::decode(protobuf_bytes.as_slice()).unwrap();
                state.borrow_mut().load(plugin_configuration);
            });
        }
        // ... update, render, etc.
    };
}
```

### 4.4 Worker Threads

For background tasks, plugins can spawn workers:

```rust
pub trait ZellijWorker<'de>: Default + Serialize + Deserialize<'de> {
    fn on_message(&mut self, message: String, payload: String) {}
}
```

### 4.5 Permission System

```rust
fn check_event_permission(
    plugin_env: &PluginEnv,
    event: &Event,
) -> (PermissionStatus, Option<PermissionType>) {
    if plugin_env.plugin.is_builtin() {
        return (PermissionStatus::Granted, None);
    }
    let permission = match event {
        Event::ModeUpdate(..) | Event::TabUpdate(..) => PermissionType::ReadApplicationState,
        Event::WebServerStatus(..) => PermissionType::StartWebServer,
        Event::PaneRenderReport(..) => PermissionType::ReadPaneContents,
        // ...
    };
    // Check against granted permissions
}
```

**dTerm Pattern:** For AI agent integration:
1. WASM provides excellent sandboxing
2. Explicit permission model is critical for agent safety
3. Event subscription prevents unnecessary wake-ups
4. Background workers prevent UI blocking

---

## 5. IPC (Client-Server Communication)

### 5.1 Message Types

**Key File:** `/zellij-utils/src/ipc.rs`

```rust
pub enum ClientToServerMsg {
    DetachSession { client_ids: Vec<ClientId> },
    TerminalResize { new_size: Size },
    FirstClientConnected { cli_assets: CliAssets, is_web_client: bool },
    AttachClient { cli_assets: CliAssets, tab_position_to_focus: Option<usize>, ... },
    Action { action: Action, terminal_id: Option<u32>, client_id: Option<ClientId>, ... },
    Key { key: KeyWithModifier, raw_bytes: Vec<u8>, is_kitty_keyboard_protocol: bool },
    ClientExited,
    KillSession,
    ConnStatus,
    // ...
}

pub enum ServerToClientMsg {
    Render { content: String },
    UnblockInputThread,
    Exit { exit_reason: ExitReason },
    Connected,
    Log { lines: Vec<String> },
    SwitchSession { connect_to_session: ConnectToSession },
    QueryTerminalSize,
    // ...
}
```

### 5.2 Wire Protocol

Uses Protobuf with length-prefixed framing:

```rust
fn read_protobuf_message<T: Message + Default>(reader: &mut impl Read) -> Result<T> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;

    T::decode(&buf[..]).map_err(Into::into)
}

fn write_protobuf_message<T: Message>(writer: &mut impl Write, msg: &T) -> Result<()> {
    let encoded = msg.encode_to_vec();
    let len = encoded.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&encoded)?;
    Ok(())
}
```

### 5.3 Socket Communication

```rust
pub struct IpcSenderWithContext<T: Serialize> {
    sender: io::BufWriter<LocalSocketStream>,
    _phantom: PhantomData<T>,
}

pub struct IpcReceiverWithContext<T> {
    receiver: io::BufReader<LocalSocketStream>,
    _phantom: PhantomData<T>,
}
```

**dTerm Pattern:** For cross-platform IPC:
1. Unix sockets work well on macOS/Linux
2. Consider named pipes for Windows
3. Length-prefixed messages are essential for streaming
4. Protobuf provides good versioning

---

## 6. Terminal Emulation

### 6.1 Grid Structure

**Key File:** `/zellij-server/src/panes/grid.rs`

```rust
pub struct Grid {
    pub(crate) lines_above: VecDeque<Row>,  // Scrollback buffer
    pub(crate) viewport: Vec<Row>,           // Visible area
    pub(crate) lines_below: Vec<Row>,        // Lines scrolled past
    horizontal_tabstops: BTreeSet<usize>,
    cursor: Cursor,
    cursor_is_hidden: bool,
    scroll_region: (usize, usize),

    // Terminal modes
    pub cursor_key_mode: bool,
    pub bracketed_paste_mode: bool,
    pub insert_mode: bool,
    pub disable_linewrap: bool,

    // Mouse handling
    pub mouse_mode: MouseMode,
    pub mouse_tracking: MouseTracking,

    // Rendering
    pub(crate) output_buffer: OutputBuffer,
    pub should_render: bool,

    // Features
    sixel_grid: SixelGrid,  // Image support
    pub supports_kitty_keyboard_protocol: bool,
}
```

### 6.2 VTE Parser Integration

Zellij uses the `vte` crate for ANSI parsing:

```rust
use vte::{Params, Perform};

// Grid implements vte::Perform
impl Perform for Grid {
    fn print(&mut self, c: char) { /* handle printable character */ }
    fn execute(&mut self, byte: u8) { /* handle C0/C1 control codes */ }
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ...) { /* CSI sequences */ }
    fn osc_dispatch(&mut self, params: &[&[u8]], ...) { /* OSC sequences */ }
    fn esc_dispatch(&mut self, intermediates: &[u8], ...) { /* ESC sequences */ }
}
```

### 6.3 Mouse Handling

```rust
pub enum MouseMode {
    NoEncoding,
    Utf8,
    Sgr,  // Extended mouse mode
}

pub enum MouseTracking {
    Off,
    Normal,           // Button events only
    ButtonEventTracking,
    AnyEventTracking, // Motion events too
}
```

**dTerm Pattern:** For terminal emulation:
1. Use `vte` crate - it's battle-tested
2. Separate scrollback from viewport
3. Support modern protocols (Kitty keyboard, Sixel)
4. Track terminal modes carefully for compatibility

---

## 7. Rust Patterns and Idioms

### 7.1 Error Handling

Uses `anyhow` for error propagation with context:

```rust
use zellij_utils::errors::prelude::*;

fn load_plugin(&mut self, ...) -> Result<(PluginId, ClientId)> {
    let err_context = move || format!("failed to load plugin");

    let client_id = client_id
        .with_context(|| "Plugins must have a client id")?;

    let plugin = PluginConfig::from_run_plugin(run)
        .with_context(|| format!("failed to resolve plugin"))
        .with_context(err_context)?;

    Ok((plugin_id, client_id))
}
```

### 7.2 Thread-Safe State

Arc<Mutex<T>> for shared mutable state:

```rust
pub struct WasmBridge {
    connected_clients: Arc<Mutex<Vec<ClientId>>>,
    plugin_map: Arc<Mutex<PluginMap>>,
    plugin_executor: Arc<PinnedExecutor>,
    // ...
}
```

### 7.3 Message Passing

Channels for inter-thread communication:

```rust
pub fn send_to_screen(&self, instruction: ScreenInstruction) -> Result<()> {
    self.to_screen
        .as_ref()
        .context("failed to get screen sender")?
        .send(instruction)
        .to_anyhow()
        .context("failed to send message to screen")
}
```

### 7.4 Builder Pattern with Defaults

```rust
impl LoadingContext {
    pub fn new(
        wasm_bridge: &WasmBridge,
        cwd: Option<PathBuf>,
        plugin_config: PluginConfig,
        plugin_id: PluginId,
        client_id: ClientId,
        tab_index: Option<usize>,
        size: Size,
    ) -> Self {
        LoadingContext {
            client_id,
            plugin_id,
            // ... set fields from parameters and wasm_bridge
            ..Default::default()
        }
    }
}
```

### 7.5 Type-Safe Enums for Instructions

```rust
pub enum PtyInstruction {
    SpawnTerminal(Option<TerminalAction>, Option<String>, NewPanePlacement, ...),
    OpenInPlaceEditor(PathBuf, Option<usize>, ClientTabIndexOrPaneId, ...),
    UpdateActivePane(Option<PaneId>, ClientId),
    ClosePane(PaneId, Option<NotificationEnd>),
    Exit,
}

impl From<&PtyInstruction> for PtyContext {
    fn from(pty_instruction: &PtyInstruction) -> Self {
        match *pty_instruction {
            PtyInstruction::SpawnTerminal(..) => PtyContext::SpawnTerminal,
            PtyInstruction::Exit => PtyContext::Exit,
            // ...
        }
    }
}
```

---

## 8. KDL Configuration

### 8.1 Config Structure

**Key File:** `/zellij-utils/src/input/config.rs`

```rust
pub struct Config {
    pub keybinds: Keybinds,
    pub options: Options,
    pub themes: Themes,
    pub plugins: PluginAliases,
    pub ui: UiConfig,
    pub env: EnvironmentVariables,
    pub background_plugins: HashSet<RunPluginOrAlias>,
    pub web_client: WebClientConfig,
}
```

### 8.2 KDL Parsing

**Key File:** `/zellij-utils/src/kdl/mod.rs`

Uses the `kdl` crate with extensive macro helpers:

```rust
#[macro_export]
macro_rules! kdl_children_nodes_or_error {
    ( $kdl_node:expr, $error:expr ) => {
        $kdl_node
            .children()
            .ok_or(ConfigError::new_kdl_error(
                $error.into(),
                $kdl_node.span().offset(),
                $kdl_node.span().len(),
            ))?
            .nodes()
    };
}
```

### 8.3 Error Reporting

KDL errors include source location for helpful messages:

```rust
pub struct KdlError {
    pub error_message: String,
    pub src: Option<NamedSource>,
    pub offset: Option<usize>,
    pub len: Option<usize>,
    pub help_message: Option<String>,
}

impl Diagnostic for KdlError {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        if let (Some(offset), Some(len)) = (self.offset, self.len) {
            let label = LabeledSpan::new(Some(self.error_message.clone()), offset, len);
            Some(Box::new(std::iter::once(label)))
        } else {
            None
        }
    }
}
```

**dTerm Pattern:** KDL is excellent for human-editable configs:
1. More readable than TOML for complex structures
2. Good error messages with source spans
3. Supports comments and multi-line values

---

## 9. Recommendations for dTerm

### 9.1 Session Management

1. **Unix Socket Discovery**: Follow Zellij's pattern of discovering sessions via socket files
2. **Session Resurrection**: Serialize session state to a structured file (KDL or similar)
3. **Dirty Detection**: Track when sessions differ from base layout to know when to save

### 9.2 Architecture

1. **Thread-per-Component**: Separate threads for PTY, rendering, plugins, background tasks
2. **Message Passing**: Use channels with typed instruction enums
3. **Error Context**: Use anyhow with context() for debugging

### 9.3 Plugin System

1. **WASM Sandboxing**: Essential for agent safety
2. **Permission Model**: Explicit permissions for file access, network, etc.
3. **Event Subscription**: Let plugins declare interest to reduce overhead
4. **Background Workers**: Prevent UI blocking during heavy operations

### 9.4 Terminal Emulation

1. **Use vte Crate**: Well-tested ANSI parser
2. **Viewport/Scrollback Split**: Separate visible area from history
3. **Modern Protocols**: Support Kitty keyboard, Sixel images
4. **Mouse Modes**: Full mouse support with SGR encoding

### 9.5 Cross-Platform Considerations

1. **IPC Abstraction**: Unix sockets on macOS/Linux, named pipes on Windows
2. **PTY Abstraction**: openpty on Unix, ConPTY on Windows
3. **Path Handling**: Use PathBuf consistently
4. **Configuration**: Use a human-readable format with good error messages

---

## 10. Key Source Files Reference

| Purpose | File Path |
|---------|-----------|
| Main entry | `/zellij/src/main.rs` |
| Server lib | `/zellij-server/src/lib.rs` |
| Client lib | `/zellij-client/src/lib.rs` |
| IPC | `/zellij-utils/src/ipc.rs` |
| Sessions | `/zellij-utils/src/sessions.rs` |
| Session serialization | `/zellij-utils/src/session_serialization.rs` |
| Layout types | `/zellij-utils/src/input/layout.rs` |
| Config | `/zellij-utils/src/input/config.rs` |
| KDL parsing | `/zellij-utils/src/kdl/mod.rs` |
| Thread bus | `/zellij-server/src/thread_bus.rs` |
| PTY handling | `/zellij-server/src/pty.rs` |
| Terminal grid | `/zellij-server/src/panes/grid.rs` |
| WASM bridge | `/zellij-server/src/plugins/wasm_bridge.rs` |
| Plugin trait | `/zellij-tile/src/lib.rs` |
| Session metadata | `/zellij-server/src/session_layout_metadata.rs` |

---

## Appendix: Dependencies

Key Rust crates used by Zellij:

- `vte` - VT100 terminal parser
- `kdl` - KDL document format
- `interprocess` - Cross-platform IPC
- `wasmi` - WASM interpreter
- `prost` - Protocol Buffers
- `crossbeam` - Concurrent programming primitives
- `async-std` - Async runtime
- `anyhow` / `thiserror` - Error handling
- `miette` - Diagnostic error reporting
- `serde` - Serialization
- `notify` - File system watching
