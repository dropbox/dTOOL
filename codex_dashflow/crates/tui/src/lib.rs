//! Codex DashFlow TUI
//!
//! Terminal UI for interactive agent sessions using Ratatui.
//! This crate provides the full-screen terminal interface for the Codex agent.
//!
//! ## Architecture
//!
//! The TUI follows the Elm architecture pattern:
//! - **Model**: `App` struct holding application state
//! - **View**: `render()` function that draws the UI
//! - **Update**: Event handlers that modify state
//!
//! ## Styling Convention (from Codex)
//!
//! - **Headers**: Use `bold`
//! - **Primary text**: Default
//! - **Secondary text**: Use `dim`
//! - **User input tips, selection**: Use ANSI `cyan`
//! - **Success and additions**: Use ANSI `green`
//! - **Errors and failures**: Use ANSI `red`
//! - **Agent responses**: Use ANSI `magenta`

mod app;
mod event;
mod history;
mod markdown;
pub mod session_log;
mod ui;
mod wrap;

pub use app::{run_app, App, AppConfig};
pub use event::TuiEvent;
pub use markdown::render_markdown;
