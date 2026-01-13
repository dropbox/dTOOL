//! Built-in UI components for terminal applications.
//!
//! This module provides ready-to-use UI components that handle common terminal
//! interface patterns. Each component manages its own rendering, input handling,
//! and state, allowing you to compose complex interfaces from simple building blocks.
//!
//! # Component Categories
//!
//! ## Interactive Components
//!
//! - [`Input`] - Single-line text input with cursor, selection, and editing
//! - [`Select`] - Scrollable selection list with customizable options
//!
//! ## Progress & Status
//!
//! - [`Progress`] - Horizontal progress bar with multiple styles
//! - [`Spinner`] - Animated spinner with multiple character sets
//! - [`StatusBar`] - Status indicator with state-based coloring and spinner
//!
//! ## Layout Components
//!
//! - [`Spacer`] - Flexible space that expands to fill available room
//! - [`Scroll`] - Scrollable container with optional scrollbars
//! - [`Stack`] - Z-axis layering for overlapping elements (modals, tooltips)
//!
//! ## Data Visualization
//!
//! - [`Heatmap`] - 2D grid with color-coded intensity values
//! - [`Sparkline`] - Compact inline charts for time series
//! - [`Plot`] - Full-featured charts (line, bar, scatter, area)
//!
//! ## Text Rendering
//!
//! - [`Markdown`] - Renders markdown text with styled headings, lists, code blocks
//!
//! ## Conversation UI
//!
//! - [`ChatView`] - Conversation history with role-aware styling and grouping
//!
//! ## Code Diff UI
//!
//! - [`DiffView`] - Code diff display with adds, deletes, and context lines
//!
//! ## AI Streaming
//!
//! - [`StreamingText`] - Efficient token-by-token text rendering for LLM output
//! - [`StreamingMarkdown`] - Markdown rendering for streamed LLM output
//! - [`VirtualList`] - Virtualized list for long conversations (only renders visible items)
//! - [`ThinkingBlock`] - Collapsible block for displaying Claude's reasoning process
//!
//! ## Modals and Popups
//!
//! - [`Modal`] - Generic modal wrapper with backdrop and centering
//! - [`SelectPopup`] - List selection with keyboard navigation and filtering
//! - [`ConfirmDialog`] - Yes/No confirmation dialog
//! - [`ErrorPopup`] - Error message display
//! - [`InfoPopup`] - Information message display
//!
//! ## Mouse Interaction
//!
//! - [`Clickable`] - Wrapper for click-responsive elements
//! - [`Draggable`] - Wrapper for draggable elements
//! - [`DropZone`] - Wrapper for drop target zones
//!
//! # Example: Building a Form
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! let name_input = Input::new()
//!     .placeholder("Enter your name...")
//!     .width(30);
//!
//! let status = Select::new(vec![
//!     SelectOption::new("active", "Active"),
//!     SelectOption::new("inactive", "Inactive"),
//! ]);
//!
//! let progress = Progress::new()
//!     .value(0.75)
//!     .style(ProgressStyle::Block);
//! ```
//!
//! # Example: Data Dashboard
//!
//! ```ignore
//! use inky::prelude::*;
//! use inky::components::{Sparkline, Heatmap, Plot, Series};
//!
//! // CPU usage sparkline
//! let cpu_spark = Sparkline::new(&cpu_history)
//!     .style(SparklineStyle::Braille)
//!     .color(Color::Green);
//!
//! // Memory heatmap
//! let mem_heat = Heatmap::new(&memory_grid)
//!     .palette(HeatmapPalette::Thermal);
//!
//! // Network throughput plot
//! let net_plot = Plot::new()
//!     .series(Series::new("rx", &rx_data).color(Color::Blue))
//!     .series(Series::new("tx", &tx_data).color(Color::Red))
//!     .plot_type(PlotType::Area);
//! ```
//!
//! # Component Philosophy
//!
//! Inky components follow these principles:
//!
//! 1. **Immutable builders** - Configure via method chains that return new instances
//! 2. **Sensible defaults** - Components work out of the box
//! 3. **Style separation** - Visual appearance is configurable without subclassing
//! 4. **Focus-aware** - Interactive components integrate with the focus system
//! 5. **Zero allocations** - Hot paths avoid heap allocations where possible
//!
//! [`Input`]: crate::components::Input
//! [`Select`]: crate::components::Select
//! [`Progress`]: crate::components::Progress
//! [`Spinner`]: crate::components::Spinner
//! [`StatusBar`]: crate::components::StatusBar
//! [`Spacer`]: crate::components::Spacer
//! [`Scroll`]: crate::components::Scroll
//! [`Stack`]: crate::components::Stack
//! [`Heatmap`]: crate::components::Heatmap
//! [`Sparkline`]: crate::components::Sparkline
//! [`Plot`]: crate::components::Plot
//! [`Markdown`]: crate::components::Markdown
//! [`ChatView`]: crate::components::ChatView
//! [`DiffView`]: crate::components::DiffView
//! [`StreamingText`]: crate::components::StreamingText
//! [`StreamingMarkdown`]: crate::components::StreamingMarkdown
//! [`VirtualList`]: crate::components::VirtualList
//! [`Clickable`]: crate::components::Clickable
//! [`Draggable`]: crate::components::Draggable
//! [`DropZone`]: crate::components::DropZone
//! [`ThinkingBlock`]: crate::components::ThinkingBlock
//! [`Modal`]: crate::components::Modal
//! [`SelectPopup`]: crate::components::SelectPopup
//! [`ConfirmDialog`]: crate::components::ConfirmDialog
//! [`ErrorPopup`]: crate::components::ErrorPopup
//! [`InfoPopup`]: crate::components::InfoPopup

pub mod adaptive;
mod background_task;
mod chat_view;
mod diff_view;
mod input;
mod modal;
mod progress;
mod scroll;
mod select;
mod spacer;
mod spinner;
mod stack;
mod status_bar;
mod thinking_block;
mod todo_panel;
mod tool_execution;

// Visualization components
mod heatmap;
mod plot;
mod sparkline;

// Text rendering components
mod markdown;
pub mod syntax;

// Text transformation
pub mod transform;

// Streaming text
mod streaming;
mod streaming_markdown;

// Virtual list
mod virtual_list;

// Image rendering
mod image;

// Mouse interaction
mod clickable;
mod draggable;
mod drop_zone;

// Upgrade prompt
mod upgrade_prompt;

pub use background_task::{
    BackgroundTask, BackgroundTaskBadge, BackgroundTaskList, BackgroundTaskStatus,
};
pub use chat_view::{ChatMessage, ChatView, MessageRole};
pub use diff_view::{DiffLine, DiffLineKind, DiffView};
pub use input::{Input, UndoEntry};
pub use modal::{ConfirmDialog, ErrorPopup, InfoPopup, Modal, SelectPopup, SelectPopupItem};
pub use progress::{Progress, ProgressStyle};
pub use scroll::{
    clear_scroll_registry, get_scroll_offset, get_scroll_offsets, get_scroll_state,
    scroll_to_bottom, scroll_to_top, set_scroll_offset, set_scroll_offsets, unregister_scroll,
    Scroll, ScrollDirection, ScrollState, ScrollbarVisibility,
};
pub use select::{Select, SelectOption};
pub use spacer::Spacer;
pub use spinner::{Spinner, SpinnerStyle};
pub use stack::Stack;
pub use status_bar::{DataDirection, StatusBar, StatusState, TokenStatusBar};
pub use thinking_block::ThinkingBlock;
pub use todo_panel::{TodoBadge, TodoItem, TodoPanel, TodoStatus};
pub use tool_execution::{ToolExecution, ToolStatus};

// Visualization exports
pub use heatmap::{Heatmap, HeatmapPalette, HeatmapStyle};
pub use plot::{Plot, PlotType, Series};
pub use sparkline::{Sparkline, SparklineStyle};

// Text rendering exports
pub use markdown::{CodeTheme, Markdown};
pub use syntax::{HighlightedLine, HighlightedSpan, SyntaxHighlighter, SyntaxTheme};

// Transform exports
pub use transform::{Align, TextTransform, Transform};

// Streaming text exports
pub use streaming::{StreamingText, StreamingTextHandle};
pub use streaming_markdown::{StreamingMarkdown, StreamingMarkdownHandle};

// Virtual list exports
pub use virtual_list::{ItemHeight, VirtualList, VirtualScrollbarVisibility};

// Image exports
pub use image::{Image, ImageProtocol, ScaleMode};

// Mouse interaction exports
pub use clickable::{ClickEvent, ClickHandler, ClickModifiers, Clickable, ClickableRegistry};
pub use draggable::Draggable;
pub use drop_zone::DropZone;

// Adaptive rendering exports
pub use adaptive::{
    AdaptiveComponent, AsciiRenderer, DegradationNotice, Tier0Fallback, TierFeatures,
};

// Upgrade prompt exports
pub use upgrade_prompt::{UpgradePrompt, UpgradePromptPresets};
