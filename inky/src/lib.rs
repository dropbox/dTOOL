// Allow cfg(feature = "dterm") even when dterm feature is not defined in Cargo.toml
// The dterm feature is only available when building from source with a local dterm-core checkout
#![allow(unexpected_cfgs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

//! # inky
//!
//! A Rust-native terminal UI library inspired by [Ink](https://github.com/vadimdemedes/ink),
//! providing React-like components and flexbox layout powered by [Taffy](https://github.com/DioxusLabs/taffy).
//!
//! ## Why inky?
//!
//! inky is designed for building modern terminal applications that are:
//! - **Fast**: GPU-accelerated rendering with <1ms frame times
//! - **Ergonomic**: React-like component model with flexbox layout
//! - **Memory-efficient**: <2MB for typical applications (vs 30MB+ for JS Ink)
//! - **AI-ready**: Direct buffer access for AI agents to read/write terminal state
//!
//! ## Features
//!
//! - **Flexbox Layout**: CSS-like layout with `flex-direction`, `justify-content`, `align-items`, etc.
//! - **Retained Mode**: Virtual DOM with diff-based incremental updates
//! - **Components**: Built-in `Box`, `Text`, `Input`, `Select`, `Progress`, `Spinner`, `Scroll`, and more
//! - **Visualization**: `Heatmap`, `Sparkline`, and `Plot` components for data visualization
//! - **Hooks**: `use_signal`, `use_input`, `use_focus`, `use_interval` for state and interaction
//! - **Macros**: `vbox![]`, `hbox![]`, `text!()`, `style!{}` for declarative UIs
//! - **GPU Support**: Optional GPU-accelerated rendering via dterm integration
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use inky::prelude::*;
//!
//! fn main() -> Result<()> {
//!     App::new()
//!         .render(|_ctx| {
//!             BoxNode::new()
//!                 .padding(1)
//!                 .child(TextNode::new("Hello, World!").color(Color::Blue).bold())
//!                 .into()
//!         })
//!         .run()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Using Macros
//!
//! For more concise UI definitions, use the layout macros:
//!
//! ```rust
//! use inky::prelude::*;
//!
//! let ui = vbox![
//!     text!("Header").bold(),
//!     hbox![
//!         text!("Left"),
//!         Spacer::new(),
//!         text!("Right").color(Color::Cyan),
//!     ],
//!     text!("Footer").dim(),
//! ];
//! ```
//!
//! ## Reactive State with Signals
//!
//! Use signals for reactive state management:
//!
//! ```rust,ignore
//! use inky::prelude::*;
//!
//! let count = use_signal(0);
//!
//! // Read the value
//! let current = count.get();
//!
//! // Update the value (triggers re-render)
//! count.set(current + 1);
//!
//! // Or use update for read-modify-write
//! count.update(|n| *n += 1);
//! ```
//!
//! ## Components
//!
//! ### Layout Components
//!
//! - [`BoxNode`]: Flexbox container with full CSS Flexbox support
//! - [`TextNode`]: Styled text with wrapping and truncation
//! - [`Spacer`]: Flexible space filler
//! - [`Stack`]: Z-axis layering for overlays
//! - [`Scroll`]: Scrollable viewport with scrollbar
//!
//! ### Interactive Components
//!
//! - [`Input`]: Text input field
//! - [`Select`]: Selection list
//! - [`Progress`]: Progress bar with multiple styles
//! - [`Spinner`]: Animated loading indicator
//!
//! ### Visualization Components
//!
//! - [`Heatmap`]: 2D color grid for weights/activations
//! - [`Sparkline`]: Inline mini-chart
//! - [`Plot`]: Line, bar, scatter, and area plots
//!
//! ## Architecture
//!
//! ```text
//! Component Tree → Virtual DOM → Taffy Layout → Diff → Terminal
//!      ↓              ↓              ↓           ↓        ↓
//!   render()      Node tree    Layout tree   LineDiff  ANSI output
//! ```
//!
//! ## Rendering Tiers
//!
//! inky supports three rendering modes:
//!
//! | Tier | Backend | Latency | Use Case |
//! |------|---------|---------|----------|
//! | Tier 1 | ANSI | ~8-16ms | Any terminal, simple UIs |
//! | Tier 2 | Crossterm | ~4-8ms | Interactive apps, dashboards |
//! | Tier 3 | GPU (dterm) | <1ms | Real-time visualization, 120 FPS |
//!
//! ## Feature Flags
//!
//! - `default`: Standard terminal rendering via crossterm
//! - `gpu`: Enable wgpu-based GPU buffer support
//! - `dterm`: Full dterm GPU backend integration
//! - `tracing`: Enable tracing instrumentation
//!
//! ## Comparison with Other Frameworks
//!
//! | Feature | inky | Ink (JS) | Ratatui | Textual |
//! |---------|------|----------|---------|---------|
//! | Language | Rust | JavaScript | Rust | Python |
//! | Layout | Flexbox | Flexbox | Manual | CSS-like |
//! | Memory (empty) | <1MB | ~30MB | ~1MB | ~10MB |
//! | GPU Support | Yes | No | No | No |
//! | Streaming | <1ms | ~10ms | ~5ms | ~5ms |
//!
//! [`BoxNode`]: node::BoxNode
//! [`TextNode`]: node::TextNode
//! [`Spacer`]: components::Spacer
//! [`Stack`]: components::Stack
//! [`Scroll`]: components::Scroll
//! [`Input`]: components::Input
//! [`Select`]: components::Select
//! [`Progress`]: components::Progress
//! [`Spinner`]: components::Spinner
//! [`Heatmap`]: components::Heatmap
//! [`Sparkline`]: components::Sparkline
//! [`Plot`]: components::Plot

// Core modules
/// ANSI escape sequence parsing.
/// Stability: Stable.
pub mod ansi;
/// Application runtime and component trait.
/// Stability: Stable.
pub mod app;
/// Diffing and change tracking.
/// Stability: Stable.
pub mod diff;
/// Flexbox layout integration.
/// Stability: Stable.
pub mod layout;
/// Core node types.
/// Stability: Stable.
pub mod node;
/// Rendering pipeline and buffer types.
/// Stability: Stable. GPU-specific APIs in `render::gpu` are unstable.
pub mod render;
/// Styling primitives.
/// Stability: Stable.
pub mod style;

// Macros module (must be before modules that use them)
/// Macros for declarative UI composition.
/// Stability: Stable.
pub mod macros;

// Component modules
/// Built-in components.
/// Stability: Stable for core components; AI assistant and image components are unstable.
pub mod components;

// Hook modules
/// Hooks for signals, input, focus, and intervals.
/// Stability: Stable.
pub mod hooks;

// Terminal abstraction
/// Terminal backends and events.
/// Stability: Stable for Crossterm backend. Dterm integration is unstable.
pub mod terminal;

// AI perception
/// AI perception utilities.
/// Stability: Unstable.
pub mod perception;

// Clipboard
/// Clipboard integration (OSC 52).
/// Stability: Unstable.
pub mod clipboard;

// StyleSheet
/// CSS-like stylesheet support.
/// Stability: Stable.
pub mod stylesheet;

// Animation
/// Animation system.
/// Stability: Unstable.
pub mod animation;

// Accessibility
/// Accessibility helpers.
/// Stability: Unstable.
pub mod accessibility;

// Elm Architecture
/// Elm-style model/update/view architecture.
/// Stability: Unstable.
pub mod elm;

// Hit Testing
/// Hit testing for mouse coordinate to node mapping.
/// Stability: Stable.
pub mod hit_test;

// Compatibility layers
/// Compatibility layers for gradual migration from other TUI frameworks.
/// Stability: Unstable.
pub mod compat;

// Re-export commonly used types
/// Prelude module for convenient imports.
///
/// Import everything needed for typical inky applications:
///
/// ```rust,ignore
/// use inky::prelude::*;
/// ```
pub mod prelude {
    // Core types
    pub use crate::app::{App, AppEvent, AppEventResult, AppHandle, Component, Context};

    // External state support
    pub use crate::app::{ExternalContext, RenderOnce, StreamingRenderer};

    // Async support (requires `async` feature)
    #[cfg(feature = "async")]
    pub use crate::app::{AsyncApp, AsyncAppHandle};
    pub use crate::layout::Layout;
    pub use crate::node::{
        BoxNode, CustomNode, Node, NodeChildren, NodeId, RootNode, StaticNode, TextContent,
        TextNode, Widget, WidgetContext,
    };
    pub use crate::style::{
        auto, length, percent, AlignContent, AlignItems, AlignSelf, BorderStyle, Color, Dimension,
        Display, Edges, FlexDirection, FlexWrap, JustifyContent, Line, Overflow, Style, StyledSpan,
        StyledSpanOwned, TextStyle, TextWrap,
    };

    // Components
    pub use crate::components::{
        get_scroll_offset, get_scroll_offsets, get_scroll_state, scroll_to_bottom, scroll_to_top,
        set_scroll_offset, set_scroll_offsets, BackgroundTask, BackgroundTaskBadge,
        BackgroundTaskList, BackgroundTaskStatus, ClickEvent, ClickModifiers, Clickable,
        ClickableRegistry, ConfirmDialog, DataDirection, Draggable, DropZone, ErrorPopup,
        InfoPopup, Input, Modal, Progress, ProgressStyle, Scroll, ScrollDirection, ScrollState,
        ScrollbarVisibility, Select, SelectOption, SelectPopup, SelectPopupItem, Spacer, Spinner,
        SpinnerStyle, Stack, StatusBar, StatusState, StreamingMarkdown, StreamingMarkdownHandle,
        StreamingText, StreamingTextHandle, ThinkingBlock, TodoBadge, TodoItem, TodoPanel,
        TodoStatus, TokenStatusBar, ToolExecution, ToolStatus, VirtualList,
        VirtualScrollbarVisibility,
    };

    // Visualization components
    pub use crate::components::{
        Heatmap, HeatmapPalette, HeatmapStyle, Plot, PlotType, Series, Sparkline, SparklineStyle,
    };

    // Adaptive rendering and tier support
    pub use crate::components::{
        AdaptiveComponent, Tier0Fallback, TierFeatures, UpgradePrompt, UpgradePromptPresets,
    };

    // GPU rendering (always available, GPU features are runtime-detected)
    pub use crate::render::gpu::{CpuGpuBuffer, GpuBuffer, GpuCell, GpuCellFlags, GpuPackedColors};
    // Shared memory IPC for AI agents
    pub use crate::render::ipc::SharedMemoryBuffer;

    // Hooks
    pub use crate::hooks::{
        blur_all, focus_group, focus_next, focus_next_in_group, focus_next_trapped, focus_prev,
        focus_prev_in_group, focus_prev_trapped, focused_group, focused_id, has_focus_trap,
        pop_focus_trap, push_focus_trap, set_focus, use_app, use_focus, use_focus_in_group,
        use_focus_with_id, use_input, use_interval, use_mouse, use_signal, DragEvent, DropEvent,
        Event, EventResult, FocusEvent, FocusHandle, FocusTrap, FocusTrapId, IntervalHandle,
        KeyCode, KeyEvent, KeyModifiers, MouseHandler, Signal,
    };

    // Mouse event types
    pub use crate::terminal::{MouseButton, MouseEvent, MouseEventKind};

    // Terminal
    pub use crate::terminal::signals::{
        clear_shutdown_request, install_signal_handlers, request_shutdown, shutdown_requested,
    };
    #[cfg(feature = "dterm")]
    pub use crate::terminal::DtermBackend;
    pub use crate::terminal::{
        emergency_restore, install_panic_hook, Backend, Capabilities, CrosstermBackend, RenderTier,
        Terminal, TerminalEvent,
    };

    // AI Perception
    pub use crate::perception::{
        discover_shared_buffers, Perception, Region, SemanticDiff, SharedPerception, Token,
    };

    // Clipboard
    pub use crate::clipboard::{Clipboard, ClipboardSelection};

    // StyleSheet
    pub use crate::stylesheet::StyleSheet;

    // Animation
    pub use crate::animation::{Animation, AnimationState, Easing, Parallel, Sequence};

    // Accessibility
    pub use crate::accessibility::{
        AccessibleNode, AccessibleState, Announcement, AnnouncementQueue, LiveRegion, Role,
    };

    // Elm Architecture
    pub use crate::elm::{Cmd, ElmApp, ElmModel, Sub};

    // Hit Testing
    pub use crate::hit_test::{HitTestResult, HitTester, MouseTarget};

    // Compatibility layers (for gradual migration from ratatui)
    pub use crate::compat::ratatui::{
        BackendCell, BackendError, ClearType, InkyBackend, Position, Size, TerminalBackend,
        WindowSize,
    };

    // ANSI parsing
    pub use crate::ansi::{parse_ansi, strip_ansi};

    // Macros (re-exported at crate root via #[macro_export])
    pub use crate::{hbox, ink, style, text, vbox};

    // Errors
    pub use anyhow::Result;
}

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
