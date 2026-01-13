//! Compatibility layers for gradual migration from other TUI frameworks.
//!
//! This module provides adapters to use inky alongside or as a replacement for
//! other terminal UI libraries like ratatui.

pub mod ratatui;

pub use ratatui::{InkyBackend, TerminalBackend};
