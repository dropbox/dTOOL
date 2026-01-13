//! # dterm-core
//!
//! High-performance, formally verified terminal emulation core.
//!
//! ## Verification
//!
//! Every component in this crate is:
//! - **Specified** in TLA+ (see `tla/`)
//! - **Proven** safe with Kani
//! - **Fuzzed** continuously
//! - **Tested** with property-based tests
//!
//! ## Components
//!
//! - [`parser`] - VT100/ANSI escape sequence parser (~400 MB/s)
//! - [`grid`] - Terminal grid with offset-based pages (8 bytes/cell)
//! - [`scrollback`] - Tiered storage (hot/warm/cold)
//! - [`search`] - Trigram-indexed search (O(1))
//! - [`checkpoint`] - Crash recovery
//!
//! ## FFI
//!
//! Enable the `ffi` feature for C bindings:
//!
//! ```toml
//! dterm-core = { version = "0.1", features = ["ffi"] }
//! ```
//!
//! ## Code Guidelines
//!
//! ### Error Handling: `expect` vs `unwrap`
//!
//! - **Use `expect("reason")`** for invariants that "should never fail":
//!   - Mutex poisoning (internal corruption - unrecoverable anyway)
//!   - Type conversions known valid at compile time
//!   - Iterator operations on known-non-empty collections
//!
//! - **Use `?` operator** for errors that can propagate:
//!   - I/O operations, parsing external input
//!   - Anything that might fail in normal operation
//!
//! - **Use `unwrap_or` / `unwrap_or_else` / `unwrap_or_default`** for fallbacks:
//!   - When a sensible default exists
//!   - Prefer over `.unwrap()` in parsing code
//!
//! - **Avoid bare `unwrap()`** in non-test code:
//!   - In tests: `unwrap()` is fine (test failures are expected)
//!   - In production: prefer `expect()` with reason, or proper error handling
//!
//! Example:
//! ```ignore
//! // Good: explains invariant
//! lock.lock().expect("mutex poisoned - terminal state corrupt");
//!
//! // Good: propagates error
//! let data = file.read_to_string()?;
//!
//! // Good: provides default
//! let value = map.get(&key).copied().unwrap_or(0);
//!
//! // Avoid in production code (no context on failure)
//! let x = opt.unwrap();
//! ```

// =============================================================================
// LINT CONFIGURATION
// =============================================================================
//
// This crate uses strict linting with explicit exceptions documented below.
// Allows are grouped by category. Module-specific allows should be placed
// in the module file, not here. Only crate-wide policy decisions belong here.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(unexpected_cfgs)] // Required for `#[cfg(kani)]` formal verification

// -----------------------------------------------------------------------------
// CRATE-WIDE STYLE POLICY: These are intentional style choices for consistency
// -----------------------------------------------------------------------------
#![allow(clippy::must_use_candidate)]        // Not all functions need #[must_use]
#![allow(clippy::module_name_repetitions)]   // e.g., parser::Parser is fine
#![allow(clippy::similar_names)]             // e.g., fg/bg, row/col are clear
#![allow(clippy::too_many_lines)]            // Complex state machines are acceptable
#![allow(clippy::struct_excessive_bools)]    // Terminal state has many flags
#![allow(clippy::wildcard_imports)]          // `use prelude::*` is idiomatic
#![allow(clippy::match_same_arms)]           // Explicit arms aid readability
#![allow(clippy::match_bool)]                // Sometimes clearer than if/else
#![allow(clippy::collapsible_if)]            // Nested ifs can be clearer
#![allow(clippy::items_after_statements)]    // Helper fns near usage is fine
#![allow(clippy::redundant_closure_for_method_calls)] // Style preference

// -----------------------------------------------------------------------------
// DOCUMENTATION: Will be addressed incrementally, does not affect correctness
// -----------------------------------------------------------------------------
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]              // e.g., VT100 without backticks
#![allow(clippy::missing_fields_in_debug)]

// -----------------------------------------------------------------------------
// API COMPATIBILITY: Required for public API design
// -----------------------------------------------------------------------------
#![allow(clippy::should_implement_trait)]    // Custom from/into patterns
#![allow(clippy::inherent_to_string_shadow_display)]
#![allow(clippy::inherent_to_string)]

// -----------------------------------------------------------------------------
// PERFORMANCE: Benchmarked decisions (see docs/BENCHMARKS.md)
// -----------------------------------------------------------------------------
#![allow(clippy::needless_pass_by_value)]    // Ownership semantics for API clarity
#![allow(clippy::explicit_iter_loop)]        // Can be clearer than .iter()
#![allow(clippy::map_unwrap_or)]             // Sometimes clearer than and_then

// -----------------------------------------------------------------------------
// LEGACY PATTERNS: Gradual migration in progress
// Move these to module-level #![allow(...)] as modules are updated.
// -----------------------------------------------------------------------------
#![allow(clippy::manual_let_else)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::iter_without_into_iter)]
#![allow(clippy::ptr_as_ptr)]
#![allow(clippy::large_stack_arrays)]        // Intentional for parser buffers
#![allow(clippy::unused_self)]               // Method signatures for future compat

pub mod agent;
pub mod bidi;
pub mod checkpoint;
pub mod config;
pub mod coalesce;
pub mod domain;
pub mod drcs;
pub mod grapheme;
pub mod grid;
pub mod integration;
pub mod iterm_image;
pub mod kitty_graphics;
pub mod media;
pub mod parser;
pub mod plugins;
pub mod render;
pub mod rle;
pub mod scrollback;
pub mod search;
pub mod selection;
pub mod session;
pub mod sixel;
pub mod sync;
pub mod terminal;
pub mod tmux;
pub mod triggers;
pub mod ui;
pub mod vi_mode;
pub mod vt_level;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "gpu")]
pub mod gpu;

// Kani proofs module (only compiled when running kani)
#[cfg(kani)]
mod verification;

// Property tests module (only compiled when testing)
#[cfg(test)]
mod tests;

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::config::{ConfigBuilder, ConfigChange, ConfigObserver, TerminalConfig};
    pub use crate::agent::{
        Agent, AgentId, AgentState, Capability, Command, CommandId, CommandQueue, CommandType,
        Execution, ExecutionId, ExecutionState, Orchestrator, OrchestratorConfig,
        OrchestratorError, OrchestratorResult, TerminalPool, TerminalSlot, TerminalSlotId,
        TerminalSlotState,
    };
    pub use crate::bidi::{
        char_bidi_class, BidiResolution, BidiResolver, BidiRun, CharBidiClass, Direction,
        ParagraphDirection,
    };
    pub use crate::checkpoint::{
        CheckpointConfig, CheckpointHeader, CheckpointManager, CheckpointVersion,
    };
    pub use crate::coalesce::{
        CoalesceAction, CoalesceConfig, CoalesceError, CoalesceState, InputCoalescer,
        RenderCallback,
    };
    pub use crate::domain::{
        Domain, DomainError, DomainId, DomainRegistry, DomainResult, DomainState, DomainType, Pane,
        PaneId, SerialConfig, SerialFlowControl, SerialParity, SpawnConfig, SshConfig, WslConfig,
    };
    pub use crate::drcs::{
        DecdldParser, DrcsCharsetSize, DrcsEraseMode, DrcsFont, DrcsFontId, DrcsGlyph, DrcsStorage,
    };
    pub use crate::grapheme::{
        ascii_width, assign_cells, byte_to_column, classify_grapheme, column_to_byte,
        grapheme_at_byte, grapheme_at_column, grapheme_display_width, grapheme_width,
        has_skin_tone, has_zwj, is_ascii_only, is_flag_emoji, is_regional_indicator,
        is_skin_tone_modifier, pad_to_width, split_graphemes, truncate_to_width, Grapheme,
        GraphemeCells, GraphemeInfo, GraphemeSegmenter, GraphemeType, ZWJ,
    };
    pub use crate::grid::{
        Cell, CellFlags, Cursor, Damage, DamageTracker, Grid, LineDamageBounds, LineSize,
        PackedColor, PackedColors, Row, RowFlags,
    };
    pub use crate::integration::{AgentMediaBridge, AgentMediaBridgeConfig, AgentMediaBridgeError};
    pub use crate::iterm_image::{
        decode_base64, parse_file_command, Base64Error, DimensionSpec, ImageFileFormat,
        InlineImage, InlineImageParams, InlineImageStorage, ITERM_MAX_DIMENSION, MAX_PAYLOAD_SIZE,
    };
    pub use crate::kitty_graphics::{
        Action as KittyAction, DeleteAction, ImageFormat, KittyGraphicsCommand, KittyImage,
        KittyImageStorage, KittyPlacement, TransmissionType,
    };
    pub use crate::media::{
        AudioFormat, AudioStream, ClientId, MediaServer, MediaServerConfig, MediaServerError,
        MediaServerResult, PlatformCapabilities, Priority, StreamDirection, StreamId, StreamState,
        SttProvider, SttResult, SttSession, SttState, TtsProvider, TtsQueue, TtsState,
        TtsUtterance,
    };
    pub use crate::parser::{Action, ActionSink, Parser};
    pub use crate::plugins::{
        parse_manifest, validate_manifest, BridgeMetrics, KeyEvent as PluginKeyEvent, KeyModifiers,
        ManifestError, NativePluginProcessor, Permission, Plugin, PluginAction, PluginBridge,
        PluginBridgeConfig, PluginError, PluginEvent, PluginId, PluginManifest, PluginMetrics,
        PluginResult, PluginState, ProcessResult as PluginProcessResult,
        TerminalInfo as PluginTerminalInfo,
    };
    #[cfg(feature = "wasm-plugins")]
    pub use crate::plugins::{PluginExecutor, PluginInstance, WasmConfig, WasmError, WasmRuntime};
    pub use crate::render::{FrameAction, FrameSync, FrameSyncMode, TripleBuffer};
    pub use crate::rle::{
        CompressedStyle, Rle, RleIter, Run, StyleId, StyleRegistry, DEFAULT_STYLE_ID,
    };
    pub use crate::scrollback::{ColdTier, HotTier, Line, Scrollback, WarmBlock, WarmTier};
    pub use crate::search::{
        streaming::{
            Direction as SearchNavigationDirection, FilterMode, SearchContent, SearchError,
            SearchState, StreamingMatch, StreamingSearch, StreamingSearchConfig,
        },
        BloomFilter, SearchDirection, SearchIndex, SearchMatch, TerminalSearch,
    };
    pub use crate::selection::{
        BuiltinRules,
        RulePriority,
        SelectionAnchor,
        SelectionMatch,
        SelectionRule,
        SelectionRuleKind,
        SelectionSide,
        SelectionState,
        SelectionType,
        SmartSelection,
        // Text selection (mouse-based)
        TextSelection,
    };
    pub use crate::session::{
        CommandInfo, LoadedSession, PaneGeometry, PaneManifest, SessionInfo, SessionManager,
        SessionManifest, TabManifest, TerminalState,
    };
    pub use crate::sixel::{SixelDecoder, SixelImage, SixelImageHandle, SixelState};
    pub use crate::sync::{FairMutex, FairRwLock, Lease};
    pub use crate::terminal::{
        Annotation, ColorPalette, CommandMark, CurrentStyle, CursorStyle, Rgb, ShellEvent,
        ShellState, Terminal, TerminalMark, TerminalModes,
    };
    pub use crate::tmux::{
        decode_octal_output, TmuxBlockEvent, TmuxControlMode, TmuxControlParser, TmuxEventSink,
        TmuxNotification, TmuxPaneId, TmuxParseState, TmuxSessionId, TmuxWindowId,
    };
    pub use crate::triggers::{
        post_process_match, CaptureTarget, Trigger, TriggerAction, TriggerBuilder, TriggerError,
        TriggerEvaluator, TriggerMatch, TriggerResult, TriggerSet,
    };
    pub use crate::ui::{
        CallbackId, Event, EventData, EventId, EventKind, TerminalId,
        TerminalState as UITerminalState, UIBridge, UIError, UIResult, UIState, MAX_CALLBACKS,
        MAX_QUEUE, MAX_TERMINALS,
    };
    pub use crate::vi_mode::{ViModeCursor, ViMotion};
    pub use crate::vt_level::{
        min_vt_level_for_csi, min_vt_level_for_esc, DeviceAttributes, VtExtension, VtLevel,
    };
}
