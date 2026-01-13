//! Terminal emulation core for DashTerm
//!
//! This crate provides high-performance terminal emulation using the VTE parser.
//! It handles ANSI escape sequences, maintains terminal state, and provides
//! efficient cell-based rendering data for the Swift UI layer.

pub mod agent_parser;
pub mod cell;
pub mod grid;
pub mod parser;
pub mod pty;
pub mod term;

pub use agent_parser::{AgentEvent, AgentNodeType, AgentParser, AgentStatus};
pub use cell::{Cell, CellAttributes, Color};
pub use grid::Grid;
pub use term::{Terminal, TerminalEvent, TerminalSize};

/// Terminal emulation error types
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("PTY error: {0}")]
    Pty(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TerminalError>;
