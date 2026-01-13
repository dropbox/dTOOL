# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-12-31

### Added

#### Phase 7: GPU Integration & SharedMemory IPC
- `SharedMemoryBuffer` - Zero-copy IPC via memory-mapped files for external process access
- `SharedPerception` - AI agent API to open and read shared terminal buffers
- `discover_shared_buffers()` - Discover all inky buffers on the system
- `DtermBackend` integration with SharedMemory sync on each render
- Memory-mapped file protocol with 64-byte header + cell data
- Path format: `/dev/shm/inky-$PID-buffer` (Linux) or `$TMPDIR/inky-$PID-buffer.shm` (macOS)
- Prelude exports for `SharedPerception` and `discover_shared_buffers`

#### Phase 6: Capability Detection & Graceful Degradation
- `Capabilities` struct for terminal feature detection
- `RenderTier` enum (Tier0Fallback, Tier1Ansi, Tier2Retained, Tier3Gpu)
- `AdaptiveComponent` trait for tier-aware rendering
- Adaptive implementations for Heatmap, Sparkline, Progress, Markdown, DiffView, ChatView, StatusBar
- Automatic tier selection based on terminal capabilities

#### AI Assistant Components (Phase 8)
- `Markdown` - Full markdown rendering with headings, bold, italic, strikethrough, code blocks, lists
- `ChatView` - Chat conversation view with role-aware styling (User/Assistant/System)
- `ChatMessage` - Message struct with role and content
- `DiffView` - Code diff viewer with additions, deletions, context lines
- `DiffLine` - Line struct with add/delete/context variants
- `StatusBar` - Animated status indicator with state colors and spinners
- `StatusState` - Status states: Ready, Working, Thinking, Success, Error, Warning

#### Core Framework
- `BoxNode` - Flexbox container with full CSS Flexbox support via Taffy
- `TextNode` - Styled text with wrapping and truncation
- `StaticNode` - Static content that doesn't re-render
- `Style` struct with complete Taffy mapping
- `LayoutEngine` with Taffy integration

#### Rendering
- `Buffer` - 2D cell buffer with damage tracking
- `Cell` - 10-byte cell type with RGB colors and style flags
- `GpuCell` - 8-byte GPU-compatible cell format
- `GpuBuffer` trait for GPU backend integration
- `CpuGpuBuffer` fallback implementation
- `buffer_to_gpu_cells()` - Bulk conversion to GPU format
- `buffer_to_gpu_cells_dirty()` - Incremental dirty-cell conversion
- Line-level diff algorithm for minimal terminal updates

#### Components
- `Input` - Text input field with cursor support
- `Select` - Selection list with arrow key navigation
- `Progress` - Progress bar with multiple styles (bar, blocks, dots)
- `Spinner` - Animated loading indicator with multiple styles
- `Spacer` - Flexible space filler
- `Scroll` - Scrollable viewport with scrollbar
- `Stack` - Z-axis layering for overlays

#### Visualization Components
- `Heatmap` - 2D color grid with 6 color palettes (Viridis, Plasma, Magma, Inferno, Turbo, Grays)
- `Sparkline` - Inline mini-chart with 4 styles (line, area, bar, dots)
- `Plot` - Line, bar, scatter, and area plots

#### Hooks
- `Signal<T>` - Reactive state with get/set/update/with/with_mut
- `use_signal()` - Create reactive state
- `use_focus()` - Focus management with FocusHandle
- `use_interval()` - Periodic timer updates
- `FocusContext` - Tab/Shift+Tab navigation support
- Event system with `Event`, `FocusEvent`, `EventResult`
- `KeyBinding` helpers for input handling

#### Macros
- `vbox![]` - Vertical layout macro
- `hbox![]` - Horizontal layout macro
- `style!{}` - Declarative style macro
- `text!()` - Text node with modifier support (color, bold, italic, etc.)

#### Application Framework
- `App` - Application runner with event loop
- Panic recovery for terminal state restoration
- Synchronized output for tear-free rendering
- Terminal resize handling

#### GPU Integration (feature: `gpu`)
- `GpuCell` with dterm-compatible packed format
- `GpuPackedColors` - 4-byte color encoding
- `GpuCellFlags` - 2-byte flags
- `copy_buffer_to_gpu()` and `copy_buffer_to_gpu_dirty()` helpers

#### dterm Integration (feature: `dterm`)
- `DtermBackend` with GPU buffer support
- `DtermGpuBuffer` implementation
- GPU submit() hook for direct rendering

### Performance (Phase 9)

39 performance optimizations implemented:
- **Buffer double-buffering**: Eliminated frame-to-frame buffer clones (saves ~100KB/frame)
- **Terminal write batching**: `queue!()` macro for batched terminal output
- **Layout tree caching**: Hash-based invalidation skips unchanged layouts
- **SmallVec optimization**: Stack-allocated vectors for node children and diff cells
- **FxHashMap**: 2-3x faster hashing for integer keys
- **IndexSet**: O(1) focus index lookup
- **parking_lot RwLock**: Faster lock acquisition for input handling
- **ASCII fast path**: O(1) character width for ASCII (no Unicode lookup)
- **VecDeque for sparkline**: O(1) data window operations
- **Flat grid vectors**: Eliminated nested `Vec<Vec<>>` allocations
- **Signal cleanup**: Automatic cleanup of dead subscribers during notification
- **OnceLock focus context**: Lazy initialization with zero runtime cost

Benchmarks run on 200x50 terminal (10,000 cells):

| Benchmark | Time | Throughput |
|-----------|------|------------|
| Buffer creation | 7.06us | 142K/s |
| Cell write (single) | 2ns | 414M cells/s |
| String write (10 chars) | 21ns | 48M chars/s |
| Full row write (200 chars) | 480ns | 2.1M rows/s |
| GPU conversion | 24us | 42K conversions/s |
| Full GPU cycle | 34us | 30K frames/s |

Memory usage:
- CPU Buffer (80x24): 19,200 bytes
- GPU Buffer (80x24): 15,360 bytes

### Examples
- `hello` - Minimal hello world
- `counter` - Reactive counter with signals
- `widgets` - Showcase of all components
- `focus` - Tab navigation demonstration
- `visualization` - Heatmap, sparkline, and plot demo
- `dashboard` - Multi-pane dashboard with live data
- `form` - Form with input validation
- `codex_tui` - Full AI assistant interface with ChatView, DiffView, StatusBar

### Testing
- 947 tests passing (unit + doc tests combined)
- Snapshot tests (visual regression via insta)
- Property-based tests with proptest
- Stress tests for edge cases
- Miri UB detection verified

[Unreleased]: https://github.com/dropbox/dTOOL/inky/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dropbox/dTOOL/inky/releases/tag/v0.1.0
