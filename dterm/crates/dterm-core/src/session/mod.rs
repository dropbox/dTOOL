//! Session serialization and resurrection.
//!
//! ## Overview
//!
//! Session resurrection enables saving and restoring complete terminal sessions,
//! including terminal state, layout, and running commands. This is inspired by
//! Zellij's session serialization.
//!
//! ## Architecture
//!
//! Sessions are saved in two parts:
//! 1. **Terminal checkpoints** (.dtck files) - Grid content and scrollback
//! 2. **Session manifest** (.dtsession files) - Layout, metadata, terminal state
//!
//! ```text
//! ~/.dterm/sessions/
//! ├── my-session/
//! │   ├── manifest.dtsession      # Session layout and metadata
//! │   ├── terminal_0.dtck         # First terminal checkpoint
//! │   ├── terminal_1.dtck         # Second terminal checkpoint
//! │   └── state_0.dtstate         # Terminal state (modes, style, etc.)
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use dterm_core::session::{Session, SessionManager};
//!
//! // Save a session
//! let manager = SessionManager::new(&sessions_dir);
//! manager.save_session("my-session", &terminals)?;
//!
//! // List available sessions
//! for session in manager.list_sessions()? {
//!     println!("{}: {} panes", session.name, session.pane_count);
//! }
//!
//! // Restore a session
//! let session = manager.load_session("my-session")?;
//! for pane in session.panes() {
//!     let terminal = pane.restore_terminal()?;
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

// ----------------------------------------------------------------------------
// Serialization helpers for safe length encoding
// ----------------------------------------------------------------------------

/// Encode a usize length as u32 for wire format.
///
/// Session files limit lengths to u32::MAX (~4GB), which is sufficient for
/// terminal data. Lengths exceeding this are clamped (extremely unlikely in practice).
#[inline]
fn encode_len(len: usize) -> u32 {
    len.try_into().unwrap_or(u32::MAX)
}

/// Decode a u32 length from wire format to usize.
///
/// On 32-bit systems this is lossless. On 64-bit systems it's safe (widening).
/// On hypothetical 16-bit systems it could truncate, but we don't target those.
#[inline]
#[allow(clippy::cast_possible_truncation)] // u32 always fits in usize on supported platforms
fn decode_len(len: u32) -> usize {
    len as usize
}

use crate::checkpoint::{CheckpointConfig, CheckpointManager};
use crate::grid::Cursor;
use crate::grid::{CellFlags, Grid, PackedColor};
use crate::iterm_image::{DimensionSpec, InlineImage, InlineImageParams};
use crate::scrollback::Scrollback;
use crate::terminal::{
    CharacterSet, CharacterSetState, CurrentStyle, CursorStyle, GlMapping, MouseEncoding,
    MouseMode, SavedCursorState, SingleShift, Terminal, TerminalModes,
};

/// Magic bytes for session manifest files.
pub const SESSION_MAGIC: [u8; 4] = *b"DTSS";

/// Magic bytes for terminal state files.
pub const STATE_MAGIC: [u8; 4] = *b"DTST";

/// Terminal state file format version.
const TERMINAL_STATE_VERSION: u32 = 2;

/// Session manifest version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SessionVersion {
    /// Version 1: Initial format.
    V1 = 1,
}

impl SessionVersion {
    /// Get version number.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Create from version number.
    #[must_use]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::V1),
            _ => None,
        }
    }
}

/// Session manifest containing layout and metadata.
#[derive(Debug, Clone)]
pub struct SessionManifest {
    /// Session name.
    pub name: String,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last modified timestamp.
    pub modified_at: u64,
    /// Global working directory.
    pub global_cwd: Option<String>,
    /// Tabs in this session.
    pub tabs: Vec<TabManifest>,
    /// Index of focused tab.
    pub focused_tab: usize,
}

impl SessionManifest {
    /// Create a new session manifest.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            name: name.to_string(),
            created_at: now,
            modified_at: now,
            global_cwd: None,
            tabs: Vec::new(),
            focused_tab: 0,
        }
    }

    /// Add a tab to the session.
    pub fn add_tab(&mut self, tab: TabManifest) {
        self.tabs.push(tab);
    }

    /// Get total pane count across all tabs.
    #[must_use]
    pub fn pane_count(&self) -> usize {
        self.tabs.iter().map(|t| t.panes.len()).sum()
    }

    /// Serialize manifest to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Magic
        data.extend_from_slice(&SESSION_MAGIC);

        // Version
        data.extend_from_slice(&SessionVersion::V1.as_u32().to_le_bytes());

        // Name (length-prefixed)
        let name_bytes = self.name.as_bytes();
        data.extend_from_slice(&encode_len(name_bytes.len()).to_le_bytes());
        data.extend_from_slice(name_bytes);

        // Timestamps
        data.extend_from_slice(&self.created_at.to_le_bytes());
        data.extend_from_slice(&self.modified_at.to_le_bytes());

        // Global CWD (length-prefixed, 0 for None)
        if let Some(cwd) = &self.global_cwd {
            let cwd_bytes = cwd.as_bytes();
            data.extend_from_slice(&encode_len(cwd_bytes.len()).to_le_bytes());
            data.extend_from_slice(cwd_bytes);
        } else {
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        // Focused tab
        data.extend_from_slice(&encode_len(self.focused_tab).to_le_bytes());

        // Tab count
        data.extend_from_slice(&encode_len(self.tabs.len()).to_le_bytes());

        // Tabs
        for tab in &self.tabs {
            let tab_data = tab.serialize();
            data.extend_from_slice(&encode_len(tab_data.len()).to_le_bytes());
            data.extend_from_slice(&tab_data);
        }

        data
    }

    /// Deserialize manifest from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        if data.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "session data too short",
            ));
        }

        let mut offset = 0;

        // Check magic
        if data[0..4] != SESSION_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid session magic",
            ));
        }
        offset += 4;

        // Check version
        let version = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        if SessionVersion::from_u32(version).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported session version",
            ));
        }
        offset += 4;

        // Name
        let name_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + name_len > data.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "name truncated"));
        }
        let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
        offset += name_len;

        // Timestamps
        if offset + 16 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "timestamps truncated",
            ));
        }
        let created_at = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        let modified_at = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        // Global CWD
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cwd length truncated",
            ));
        }
        let cwd_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        let global_cwd = if cwd_len > 0 {
            if offset + cwd_len > data.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "cwd truncated"));
            }
            let cwd = String::from_utf8_lossy(&data[offset..offset + cwd_len]).to_string();
            offset += cwd_len;
            Some(cwd)
        } else {
            None
        };

        // Focused tab
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "focused tab truncated",
            ));
        }
        let focused_tab = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;

        // Tab count
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "tab count truncated",
            ));
        }
        let tab_count = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;

        // Tabs
        let mut tabs = Vec::with_capacity(tab_count);
        for _ in 0..tab_count {
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "tab length truncated",
                ));
            }
            let tab_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + tab_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "tab data truncated",
                ));
            }
            let tab = TabManifest::deserialize(&data[offset..offset + tab_len])?;
            tabs.push(tab);
            offset += tab_len;
        }

        Ok(Self {
            name,
            created_at,
            modified_at,
            global_cwd,
            tabs,
            focused_tab,
        })
    }
}

/// Tab within a session.
#[derive(Debug, Clone)]
pub struct TabManifest {
    /// Tab name.
    pub name: String,
    /// Panes within this tab.
    pub panes: Vec<PaneManifest>,
    /// Index of focused pane.
    pub focused_pane: Option<usize>,
}

impl TabManifest {
    /// Create a new tab manifest.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            panes: Vec::new(),
            focused_pane: None,
        }
    }

    /// Add a pane to the tab.
    pub fn add_pane(&mut self, pane: PaneManifest) {
        self.panes.push(pane);
    }

    /// Serialize tab to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Name
        let name_bytes = self.name.as_bytes();
        data.extend_from_slice(&encode_len(name_bytes.len()).to_le_bytes());
        data.extend_from_slice(name_bytes);

        // Focused pane (u32::MAX for None)
        let focused = self.focused_pane.map(encode_len).unwrap_or(u32::MAX);
        data.extend_from_slice(&focused.to_le_bytes());

        // Pane count
        data.extend_from_slice(&encode_len(self.panes.len()).to_le_bytes());

        // Panes
        for pane in &self.panes {
            let pane_data = pane.serialize();
            data.extend_from_slice(&encode_len(pane_data.len()).to_le_bytes());
            data.extend_from_slice(&pane_data);
        }

        data
    }

    /// Deserialize tab from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        let mut offset = 0;

        // Name
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "tab name length truncated",
            ));
        }
        let name_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + name_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "tab name truncated",
            ));
        }
        let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
        offset += name_len;

        // Focused pane
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "focused pane truncated",
            ));
        }
        let focused = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let focused_pane = if focused == u32::MAX {
            None
        } else {
            Some(decode_len(focused))
        };
        offset += 4;

        // Pane count
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pane count truncated",
            ));
        }
        let pane_count = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;

        // Panes
        let mut panes = Vec::with_capacity(pane_count);
        for _ in 0..pane_count {
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "pane length truncated",
                ));
            }
            let pane_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + pane_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "pane data truncated",
                ));
            }
            let pane = PaneManifest::deserialize(&data[offset..offset + pane_len])?;
            panes.push(pane);
            offset += pane_len;
        }

        Ok(Self {
            name,
            panes,
            focused_pane,
        })
    }
}

/// Pane within a tab.
#[derive(Debug, Clone)]
pub struct PaneManifest {
    /// Unique pane identifier within session.
    pub id: u32,
    /// Pane geometry (x, y, width, height).
    pub geometry: PaneGeometry,
    /// Terminal checkpoint file path (relative to session dir).
    pub checkpoint_file: String,
    /// Terminal state file path (relative to session dir).
    pub state_file: String,
    /// Pane title.
    pub title: Option<String>,
    /// Working directory.
    pub cwd: Option<String>,
    /// Command that was running.
    pub command: Option<CommandInfo>,
    /// Whether this pane was focused.
    pub is_focused: bool,
}

impl PaneManifest {
    /// Create a new pane manifest.
    #[must_use]
    pub fn new(id: u32) -> Self {
        Self {
            id,
            geometry: PaneGeometry::default(),
            checkpoint_file: format!("terminal_{}.dtck", id),
            state_file: format!("state_{}.dtstate", id),
            title: None,
            cwd: None,
            command: None,
            is_focused: false,
        }
    }

    /// Serialize pane to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // ID
        data.extend_from_slice(&self.id.to_le_bytes());

        // Geometry
        data.extend_from_slice(&self.geometry.x.to_le_bytes());
        data.extend_from_slice(&self.geometry.y.to_le_bytes());
        data.extend_from_slice(&self.geometry.width.to_le_bytes());
        data.extend_from_slice(&self.geometry.height.to_le_bytes());

        // Checkpoint file
        let checkpoint_bytes = self.checkpoint_file.as_bytes();
        data.extend_from_slice(&encode_len(checkpoint_bytes.len()).to_le_bytes());
        data.extend_from_slice(checkpoint_bytes);

        // State file
        let state_bytes = self.state_file.as_bytes();
        data.extend_from_slice(&encode_len(state_bytes.len()).to_le_bytes());
        data.extend_from_slice(state_bytes);

        // Title
        if let Some(title) = &self.title {
            let title_bytes = title.as_bytes();
            data.extend_from_slice(&encode_len(title_bytes.len()).to_le_bytes());
            data.extend_from_slice(title_bytes);
        } else {
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        // CWD
        if let Some(cwd) = &self.cwd {
            let cwd_bytes = cwd.as_bytes();
            data.extend_from_slice(&encode_len(cwd_bytes.len()).to_le_bytes());
            data.extend_from_slice(cwd_bytes);
        } else {
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        // Command
        if let Some(cmd) = &self.command {
            data.push(1); // Has command
            let cmd_data = cmd.serialize();
            data.extend_from_slice(&encode_len(cmd_data.len()).to_le_bytes());
            data.extend_from_slice(&cmd_data);
        } else {
            data.push(0); // No command
        }

        // Is focused
        data.push(u8::from(self.is_focused));

        data
    }

    /// Deserialize pane from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        let mut offset = 0;

        // ID
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pane id truncated",
            ));
        }
        let id = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Geometry
        if offset + 16 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "geometry truncated",
            ));
        }
        let x = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let y = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let width = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let height = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let geometry = PaneGeometry {
            x,
            y,
            width,
            height,
        };

        // Checkpoint file
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint path length truncated",
            ));
        }
        let checkpoint_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + checkpoint_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint path truncated",
            ));
        }
        let checkpoint_file =
            String::from_utf8_lossy(&data[offset..offset + checkpoint_len]).to_string();
        offset += checkpoint_len;

        // State file
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "state path length truncated",
            ));
        }
        let state_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + state_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "state path truncated",
            ));
        }
        let state_file = String::from_utf8_lossy(&data[offset..offset + state_len]).to_string();
        offset += state_len;

        // Title
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "title length truncated",
            ));
        }
        let title_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        let title = if title_len > 0 {
            if offset + title_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "title truncated",
                ));
            }
            let t = String::from_utf8_lossy(&data[offset..offset + title_len]).to_string();
            offset += title_len;
            Some(t)
        } else {
            None
        };

        // CWD
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cwd length truncated",
            ));
        }
        let cwd_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        let cwd = if cwd_len > 0 {
            if offset + cwd_len > data.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "cwd truncated"));
            }
            let c = String::from_utf8_lossy(&data[offset..offset + cwd_len]).to_string();
            offset += cwd_len;
            Some(c)
        } else {
            None
        };

        // Command
        if offset >= data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "command flag truncated",
            ));
        }
        let has_command = data[offset] != 0;
        offset += 1;
        let command = if has_command {
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "command length truncated",
                ));
            }
            let cmd_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + cmd_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "command truncated",
                ));
            }
            let cmd = CommandInfo::deserialize(&data[offset..offset + cmd_len])?;
            offset += cmd_len;
            Some(cmd)
        } else {
            None
        };

        // Is focused
        if offset >= data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "focused flag truncated",
            ));
        }
        let is_focused = data[offset] != 0;

        Ok(Self {
            id,
            geometry,
            checkpoint_file,
            state_file,
            title,
            cwd,
            command,
            is_focused,
        })
    }
}

/// Pane geometry.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaneGeometry {
    /// X position in cells.
    pub x: u32,
    /// Y position in cells.
    pub y: u32,
    /// Width in cells.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
}

/// Command information for a pane.
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Command executable.
    pub program: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Environment variables (key=value).
    pub env: HashMap<String, String>,
}

impl CommandInfo {
    /// Create a new command info.
    #[must_use]
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Program
        let program_bytes = self.program.as_bytes();
        data.extend_from_slice(&encode_len(program_bytes.len()).to_le_bytes());
        data.extend_from_slice(program_bytes);

        // Args count
        data.extend_from_slice(&encode_len(self.args.len()).to_le_bytes());
        for arg in &self.args {
            let arg_bytes = arg.as_bytes();
            data.extend_from_slice(&encode_len(arg_bytes.len()).to_le_bytes());
            data.extend_from_slice(arg_bytes);
        }

        // Env count
        data.extend_from_slice(&encode_len(self.env.len()).to_le_bytes());
        for (key, value) in &self.env {
            let key_bytes = key.as_bytes();
            data.extend_from_slice(&encode_len(key_bytes.len()).to_le_bytes());
            data.extend_from_slice(key_bytes);
            let value_bytes = value.as_bytes();
            data.extend_from_slice(&encode_len(value_bytes.len()).to_le_bytes());
            data.extend_from_slice(value_bytes);
        }

        data
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        let mut offset = 0;

        // Program
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "program length truncated",
            ));
        }
        let program_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + program_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "program truncated",
            ));
        }
        let program = String::from_utf8_lossy(&data[offset..offset + program_len]).to_string();
        offset += program_len;

        // Args
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "args count truncated",
            ));
        }
        let args_count = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;

        let mut args = Vec::with_capacity(args_count);
        for _ in 0..args_count {
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "arg length truncated",
                ));
            }
            let arg_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + arg_len > data.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "arg truncated"));
            }
            let arg = String::from_utf8_lossy(&data[offset..offset + arg_len]).to_string();
            args.push(arg);
            offset += arg_len;
        }

        // Env
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "env count truncated",
            ));
        }
        let env_count = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;

        let mut env = HashMap::with_capacity(env_count);
        for _ in 0..env_count {
            // Key
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "env key length truncated",
                ));
            }
            let key_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + key_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "env key truncated",
                ));
            }
            let key = String::from_utf8_lossy(&data[offset..offset + key_len]).to_string();
            offset += key_len;

            // Value
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "env value length truncated",
                ));
            }
            let value_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + value_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "env value truncated",
                ));
            }
            let value = String::from_utf8_lossy(&data[offset..offset + value_len]).to_string();
            offset += value_len;

            env.insert(key, value);
        }

        Ok(Self { program, args, env })
    }
}

/// Serialized inline image data for session resurrection.
#[derive(Debug, Clone)]
pub struct SerializedInlineImage {
    /// Raw image data (format-specific: PNG, JPEG, etc.).
    pub data: Vec<u8>,
    /// Filename (if provided).
    pub name: Option<String>,
    /// Display width specification.
    pub width: DimensionSpec,
    /// Display height specification.
    pub height: DimensionSpec,
    /// Whether to preserve aspect ratio.
    pub preserve_aspect_ratio: bool,
    /// Terminal cursor row where image should be placed.
    pub cursor_row: u16,
    /// Terminal cursor column where image should be placed.
    pub cursor_col: u16,
}

impl SerializedInlineImage {
    /// Extract inline image data for serialization.
    #[must_use]
    pub fn from_inline_image(image: &InlineImage) -> Self {
        Self {
            data: image.data().to_vec(),
            name: image.name().map(|name| name.to_string()),
            width: image.width(),
            height: image.height(),
            preserve_aspect_ratio: image.preserve_aspect_ratio(),
            cursor_row: image.cursor_row(),
            cursor_col: image.cursor_col(),
        }
    }

    /// Store this inline image in terminal storage.
    pub fn store_into(&self, storage: &mut crate::iterm_image::InlineImageStorage) {
        let mut params = InlineImageParams::new();
        params.name.clone_from(&self.name);
        params.width = self.width;
        params.height = self.height;
        params.preserve_aspect_ratio = self.preserve_aspect_ratio;
        params.inline = true;
        let _ = storage.store(self.data.clone(), &params, self.cursor_row, self.cursor_col);
    }
}

/// Terminal state for session resurrection.
///
/// Contains all terminal state that isn't captured in the grid checkpoint,
/// including modes, style, cursor state, etc.
#[derive(Debug, Clone)]
pub struct TerminalState {
    /// Terminal modes.
    pub modes: SerializedModes,
    /// Current text style.
    pub style: SerializedStyle,
    /// Character set state.
    pub charset: SerializedCharset,
    /// Saved cursor state (main screen).
    pub saved_cursor_main: Option<SerializedCursor>,
    /// Saved cursor state (alt screen).
    pub saved_cursor_alt: Option<SerializedCursor>,
    /// Mode 1049 cursor (main).
    pub mode_1049_cursor_main: Option<SerializedCursor>,
    /// Mode 1049 cursor (alt).
    pub mode_1049_cursor_alt: Option<SerializedCursor>,
    /// Window title.
    pub title: String,
    /// Icon name.
    pub icon_name: String,
    /// Current hyperlink URL.
    pub current_hyperlink: Option<String>,
    /// Current underline color.
    pub current_underline_color: Option<u32>,
    /// Current working directory.
    pub current_working_directory: Option<String>,
    /// Title stack (icon_name, window_title pairs).
    pub title_stack: Vec<(String, String)>,
    /// Kitty keyboard flags (raw bits).
    pub kitty_keyboard_flags: u8,
    /// Inline images stored via OSC 1337 File.
    pub inline_images: Vec<SerializedInlineImage>,
}

impl TerminalState {
    /// Extract state from a terminal.
    #[must_use]
    pub fn from_terminal(terminal: &Terminal) -> Self {
        Self {
            modes: SerializedModes::from_modes(terminal.modes()),
            style: SerializedStyle::from_style(terminal.style()),
            charset: SerializedCharset::from_charset(terminal.charset()),
            saved_cursor_main: terminal
                .saved_cursor_main()
                .map(SerializedCursor::from_cursor),
            saved_cursor_alt: terminal
                .saved_cursor_alt()
                .map(SerializedCursor::from_cursor),
            mode_1049_cursor_main: terminal
                .mode_1049_cursor_main()
                .map(SerializedCursor::from_cursor),
            mode_1049_cursor_alt: terminal
                .mode_1049_cursor_alt()
                .map(SerializedCursor::from_cursor),
            title: terminal.title().to_string(),
            icon_name: terminal.icon_name().to_string(),
            current_hyperlink: terminal.current_hyperlink().map(|s| s.to_string()),
            current_underline_color: terminal.current_underline_color(),
            current_working_directory: terminal.current_working_directory().map(|s| s.to_string()),
            title_stack: terminal
                .title_stack()
                .iter()
                .map(|(i, w)| (i.to_string(), w.to_string()))
                .collect(),
            kitty_keyboard_flags: terminal.kitty_keyboard().flags().bits(),
            inline_images: terminal
                .inline_images()
                .images()
                .iter()
                .map(SerializedInlineImage::from_inline_image)
                .collect(),
        }
    }

    /// Apply state to a terminal.
    pub fn apply_to(&self, terminal: &mut Terminal) {
        // Apply modes
        self.modes.apply_to_modes(terminal.modes_mut());

        // Apply style
        self.style.apply_to_style(terminal.style_mut());

        // Apply charset
        self.charset.apply_to_charset(terminal.charset_mut());

        // Apply saved cursors
        if let Some(ref cursor) = self.saved_cursor_main {
            terminal.set_saved_cursor_main(Some(cursor.to_cursor()));
        }
        if let Some(ref cursor) = self.saved_cursor_alt {
            terminal.set_saved_cursor_alt(Some(cursor.to_cursor()));
        }
        if let Some(ref cursor) = self.mode_1049_cursor_main {
            terminal.set_mode_1049_cursor_main(Some(cursor.to_cursor()));
        }
        if let Some(ref cursor) = self.mode_1049_cursor_alt {
            terminal.set_mode_1049_cursor_alt(Some(cursor.to_cursor()));
        }

        // Apply titles
        terminal.set_title(&self.title);
        terminal.set_icon_name(&self.icon_name);

        // Apply hyperlink
        if let Some(ref url) = self.current_hyperlink {
            terminal.set_hyperlink(Some(url));
        }

        // Apply underline color
        if let Some(color) = self.current_underline_color {
            terminal.set_underline_color(Some(color));
        }

        // Apply CWD
        if let Some(ref cwd) = self.current_working_directory {
            terminal.set_current_working_directory(Some(cwd.clone()));
        }

        // Apply title stack
        terminal.set_title_stack(
            self.title_stack
                .iter()
                .map(|(i, w)| (Arc::from(i.as_str()), Arc::from(w.as_str())))
                .collect(),
        );

        // Apply kitty keyboard flags (mode 0 = set directly)
        terminal
            .kitty_keyboard_mut()
            .set_flags(self.kitty_keyboard_flags, 0);

        // Apply inline images
        let storage = terminal.inline_images_mut();
        storage.clear();
        for image in &self.inline_images {
            image.store_into(storage);
        }
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Magic
        data.extend_from_slice(&STATE_MAGIC);

        // Version
        data.extend_from_slice(&TERMINAL_STATE_VERSION.to_le_bytes());

        // Modes
        let modes_data = self.modes.serialize();
        data.extend_from_slice(&encode_len(modes_data.len()).to_le_bytes());
        data.extend_from_slice(&modes_data);

        // Style
        let style_data = self.style.serialize();
        data.extend_from_slice(&encode_len(style_data.len()).to_le_bytes());
        data.extend_from_slice(&style_data);

        // Charset
        let charset_data = self.charset.serialize();
        data.extend_from_slice(&encode_len(charset_data.len()).to_le_bytes());
        data.extend_from_slice(&charset_data);

        // Saved cursors (using helper)
        serialize_optional_cursor(&mut data, self.saved_cursor_main.as_ref());
        serialize_optional_cursor(&mut data, self.saved_cursor_alt.as_ref());
        serialize_optional_cursor(&mut data, self.mode_1049_cursor_main.as_ref());
        serialize_optional_cursor(&mut data, self.mode_1049_cursor_alt.as_ref());

        // Title
        let title_bytes = self.title.as_bytes();
        data.extend_from_slice(&encode_len(title_bytes.len()).to_le_bytes());
        data.extend_from_slice(title_bytes);

        // Icon name
        let icon_bytes = self.icon_name.as_bytes();
        data.extend_from_slice(&encode_len(icon_bytes.len()).to_le_bytes());
        data.extend_from_slice(icon_bytes);

        // Current hyperlink
        if let Some(ref url) = self.current_hyperlink {
            let url_bytes = url.as_bytes();
            data.extend_from_slice(&encode_len(url_bytes.len()).to_le_bytes());
            data.extend_from_slice(url_bytes);
        } else {
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        // Current underline color
        if let Some(color) = self.current_underline_color {
            data.push(1);
            data.extend_from_slice(&color.to_le_bytes());
        } else {
            data.push(0);
        }

        // Current working directory
        if let Some(ref cwd) = self.current_working_directory {
            let cwd_bytes = cwd.as_bytes();
            data.extend_from_slice(&encode_len(cwd_bytes.len()).to_le_bytes());
            data.extend_from_slice(cwd_bytes);
        } else {
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        // Title stack
        data.extend_from_slice(&encode_len(self.title_stack.len()).to_le_bytes());
        for (icon, title) in &self.title_stack {
            let icon_bytes = icon.as_bytes();
            data.extend_from_slice(&encode_len(icon_bytes.len()).to_le_bytes());
            data.extend_from_slice(icon_bytes);
            let title_bytes = title.as_bytes();
            data.extend_from_slice(&encode_len(title_bytes.len()).to_le_bytes());
            data.extend_from_slice(title_bytes);
        }

        // Kitty keyboard flags
        data.push(self.kitty_keyboard_flags);

        // Inline images
        serialize_inline_images(&mut data, &self.inline_images);

        data
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        if data.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "state data too short",
            ));
        }

        let mut offset = 0;

        // Check magic
        if data[0..4] != STATE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid state magic",
            ));
        }
        offset += 4;

        // Version
        let version = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let has_inline_images = match version {
            1 => false,
            2 => true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported state version",
                ));
            }
        };
        offset += 4;

        // Modes
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "modes length truncated",
            ));
        }
        let modes_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + modes_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "modes truncated",
            ));
        }
        let modes = SerializedModes::deserialize(&data[offset..offset + modes_len])?;
        offset += modes_len;

        // Style
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "style length truncated",
            ));
        }
        let style_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + style_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "style truncated",
            ));
        }
        let style = SerializedStyle::deserialize(&data[offset..offset + style_len])?;
        offset += style_len;

        // Charset
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "charset length truncated",
            ));
        }
        let charset_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + charset_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "charset truncated",
            ));
        }
        let charset = SerializedCharset::deserialize(&data[offset..offset + charset_len])?;
        offset += charset_len;

        // Saved cursors
        let (saved_cursor_main, new_offset) = deserialize_optional_cursor(data, offset)?;
        offset = new_offset;
        let (saved_cursor_alt, new_offset) = deserialize_optional_cursor(data, offset)?;
        offset = new_offset;
        let (mode_1049_cursor_main, new_offset) = deserialize_optional_cursor(data, offset)?;
        offset = new_offset;
        let (mode_1049_cursor_alt, new_offset) = deserialize_optional_cursor(data, offset)?;
        offset = new_offset;

        // Title
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "title length truncated",
            ));
        }
        let title_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + title_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "title truncated",
            ));
        }
        let title = String::from_utf8_lossy(&data[offset..offset + title_len]).to_string();
        offset += title_len;

        // Icon name
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "icon name length truncated",
            ));
        }
        let icon_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + icon_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "icon name truncated",
            ));
        }
        let icon_name = String::from_utf8_lossy(&data[offset..offset + icon_len]).to_string();
        offset += icon_len;

        // Current hyperlink
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "hyperlink length truncated",
            ));
        }
        let hyperlink_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        let current_hyperlink = if hyperlink_len > 0 {
            if offset + hyperlink_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "hyperlink truncated",
                ));
            }
            let url = String::from_utf8_lossy(&data[offset..offset + hyperlink_len]).to_string();
            offset += hyperlink_len;
            Some(url)
        } else {
            None
        };

        // Current underline color
        if offset >= data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "underline color flag truncated",
            ));
        }
        let has_underline_color = data[offset] != 0;
        offset += 1;
        let current_underline_color = if has_underline_color {
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "underline color truncated",
                ));
            }
            let color = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
            Some(color)
        } else {
            None
        };

        // Current working directory
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cwd length truncated",
            ));
        }
        let cwd_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        let current_working_directory = if cwd_len > 0 {
            if offset + cwd_len > data.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "cwd truncated"));
            }
            let cwd = String::from_utf8_lossy(&data[offset..offset + cwd_len]).to_string();
            offset += cwd_len;
            Some(cwd)
        } else {
            None
        };

        // Title stack
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "title stack count truncated",
            ));
        }
        let stack_count = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;

        let mut title_stack = Vec::with_capacity(stack_count);
        for _ in 0..stack_count {
            // Icon
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "stack icon length truncated",
                ));
            }
            let icon_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + icon_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "stack icon truncated",
                ));
            }
            let icon = String::from_utf8_lossy(&data[offset..offset + icon_len]).to_string();
            offset += icon_len;

            // Title
            if offset + 4 > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "stack title length truncated",
                ));
            }
            let title_len = decode_len(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
            if offset + title_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "stack title truncated",
                ));
            }
            let title = String::from_utf8_lossy(&data[offset..offset + title_len]).to_string();
            offset += title_len;

            title_stack.push((icon, title));
        }

        // Kitty keyboard flags
        if offset >= data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "kitty flags truncated",
            ));
        }
        let kitty_keyboard_flags = data[offset];
        offset += 1;

        let inline_images = if has_inline_images {
            let (images, _) = deserialize_inline_images(data, offset)?;
            images
        } else {
            Vec::new()
        };

        Ok(Self {
            modes,
            style,
            charset,
            saved_cursor_main,
            saved_cursor_alt,
            mode_1049_cursor_main,
            mode_1049_cursor_alt,
            title,
            icon_name,
            current_hyperlink,
            current_underline_color,
            current_working_directory,
            title_stack,
            kitty_keyboard_flags,
            inline_images,
        })
    }
}

/// Helper to serialize optional cursor.
fn serialize_optional_cursor(data: &mut Vec<u8>, cursor: Option<&SerializedCursor>) {
    if let Some(c) = cursor {
        data.push(1);
        let cursor_data = c.serialize();
        data.extend_from_slice(&encode_len(cursor_data.len()).to_le_bytes());
        data.extend_from_slice(&cursor_data);
    } else {
        data.push(0);
    }
}

/// Helper to deserialize optional cursor.
fn deserialize_optional_cursor(
    data: &[u8],
    mut offset: usize,
) -> io::Result<(Option<SerializedCursor>, usize)> {
    if offset >= data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cursor flag truncated",
        ));
    }
    let has_cursor = data[offset] != 0;
    offset += 1;

    if has_cursor {
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cursor length truncated",
            ));
        }
        let cursor_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + cursor_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cursor truncated",
            ));
        }
        let cursor = SerializedCursor::deserialize(&data[offset..offset + cursor_len])?;
        offset += cursor_len;
        Ok((Some(cursor), offset))
    } else {
        Ok((None, offset))
    }
}

/// Serialize a DimensionSpec for session storage.
fn serialize_dimension_spec(data: &mut Vec<u8>, spec: DimensionSpec) {
    match spec {
        DimensionSpec::Auto => {
            data.push(0);
            data.extend_from_slice(&0u32.to_le_bytes());
        }
        DimensionSpec::Cells(value) => {
            data.push(1);
            data.extend_from_slice(&value.to_le_bytes());
        }
        DimensionSpec::Pixels(value) => {
            data.push(2);
            data.extend_from_slice(&value.to_le_bytes());
        }
        DimensionSpec::Percent(value) => {
            data.push(3);
            data.extend_from_slice(&u32::from(value).to_le_bytes());
        }
    }
}

/// Deserialize a DimensionSpec from session storage.
fn deserialize_dimension_spec(
    data: &[u8],
    mut offset: usize,
) -> io::Result<(DimensionSpec, usize)> {
    if offset >= data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "dimension spec tag truncated",
        ));
    }
    let tag = data[offset];
    offset += 1;

    if offset + 4 > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "dimension spec value truncated",
        ));
    }
    let value = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    offset += 4;

    let spec = match tag {
        0 => DimensionSpec::Auto,
        1 => DimensionSpec::Cells(value),
        2 => DimensionSpec::Pixels(value),
        3 => DimensionSpec::Percent(u8::try_from(value.min(100)).unwrap_or(100)),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown dimension spec tag",
            ));
        }
    };

    Ok((spec, offset))
}

/// Serialize inline images for session storage.
fn serialize_inline_images(data: &mut Vec<u8>, images: &[SerializedInlineImage]) {
    data.extend_from_slice(&encode_len(images.len()).to_le_bytes());
    for image in images {
        data.extend_from_slice(&encode_len(image.data.len()).to_le_bytes());
        data.extend_from_slice(&image.data);

        if let Some(ref name) = image.name {
            let name_bytes = name.as_bytes();
            data.extend_from_slice(&encode_len(name_bytes.len()).to_le_bytes());
            data.extend_from_slice(name_bytes);
        } else {
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        serialize_dimension_spec(data, image.width);
        serialize_dimension_spec(data, image.height);

        data.push(u8::from(image.preserve_aspect_ratio));
        data.extend_from_slice(&image.cursor_row.to_le_bytes());
        data.extend_from_slice(&image.cursor_col.to_le_bytes());
    }
}

/// Deserialize inline images from session storage.
fn deserialize_inline_images(
    data: &[u8],
    mut offset: usize,
) -> io::Result<(Vec<SerializedInlineImage>, usize)> {
    if offset + 4 > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "inline image count truncated",
        ));
    }
    let image_count = decode_len(u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]));
    offset += 4;

    let mut images = Vec::with_capacity(image_count);
    for _ in 0..image_count {
        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inline image data length truncated",
            ));
        }
        let data_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        if offset + data_len > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inline image data truncated",
            ));
        }
        let image_data = data[offset..offset + data_len].to_vec();
        offset += data_len;

        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inline image name length truncated",
            ));
        }
        let name_len = decode_len(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
        let name = if name_len > 0 {
            if offset + name_len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "inline image name truncated",
                ));
            }
            let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
            offset += name_len;
            Some(name)
        } else {
            None
        };

        let (width, new_offset) = deserialize_dimension_spec(data, offset)?;
        offset = new_offset;
        let (height, new_offset) = deserialize_dimension_spec(data, offset)?;
        offset = new_offset;

        if offset >= data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inline image aspect ratio truncated",
            ));
        }
        let preserve_aspect_ratio = data[offset] != 0;
        offset += 1;

        if offset + 4 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inline image cursor truncated",
            ));
        }
        let cursor_row = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let cursor_col = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;

        images.push(SerializedInlineImage {
            data: image_data,
            name,
            width,
            height,
            preserve_aspect_ratio,
            cursor_row,
            cursor_col,
        });
    }

    Ok((images, offset))
}

/// Serialized terminal modes.
#[derive(Debug, Clone)]
pub struct SerializedModes {
    /// Packed mode flags (13 boolean modes as bits).
    pub flags: u16,
    /// Cursor style (DECSCUSR).
    pub cursor_style: u8,
    /// Mouse mode.
    pub mouse_mode: u8,
    /// Mouse encoding.
    pub mouse_encoding: u8,
}

impl SerializedModes {
    /// Extract from TerminalModes.
    #[must_use]
    pub fn from_modes(modes: &TerminalModes) -> Self {
        let mut flags = 0u16;
        if modes.cursor_visible {
            flags |= 1 << 0;
        }
        if modes.application_cursor_keys {
            flags |= 1 << 1;
        }
        if modes.alternate_screen {
            flags |= 1 << 2;
        }
        if modes.auto_wrap {
            flags |= 1 << 3;
        }
        if modes.origin_mode {
            flags |= 1 << 4;
        }
        if modes.insert_mode {
            flags |= 1 << 5;
        }
        if modes.new_line_mode {
            flags |= 1 << 6;
        }
        if modes.bracketed_paste {
            flags |= 1 << 7;
        }
        if modes.focus_reporting {
            flags |= 1 << 8;
        }
        if modes.synchronized_output {
            flags |= 1 << 9;
        }

        let cursor_style = match modes.cursor_style {
            CursorStyle::BlinkingBlock => 1,
            CursorStyle::SteadyBlock => 2,
            CursorStyle::BlinkingUnderline => 3,
            CursorStyle::SteadyUnderline => 4,
            CursorStyle::BlinkingBar => 5,
            CursorStyle::SteadyBar => 6,
        };

        let mouse_mode = match modes.mouse_mode {
            MouseMode::None => 0,
            MouseMode::Normal => 1,
            MouseMode::ButtonEvent => 2,
            MouseMode::AnyEvent => 3,
        };

        let mouse_encoding = match modes.mouse_encoding {
            MouseEncoding::X10 => 0,
            MouseEncoding::Utf8 => 1,
            MouseEncoding::Sgr => 2,
            MouseEncoding::Urxvt => 3,
            MouseEncoding::SgrPixel => 4,
        };

        Self {
            flags,
            cursor_style,
            mouse_mode,
            mouse_encoding,
        }
    }

    /// Apply to TerminalModes.
    pub fn apply_to_modes(&self, modes: &mut TerminalModes) {
        modes.cursor_visible = (self.flags & (1 << 0)) != 0;
        modes.application_cursor_keys = (self.flags & (1 << 1)) != 0;
        modes.alternate_screen = (self.flags & (1 << 2)) != 0;
        modes.auto_wrap = (self.flags & (1 << 3)) != 0;
        modes.origin_mode = (self.flags & (1 << 4)) != 0;
        modes.insert_mode = (self.flags & (1 << 5)) != 0;
        modes.new_line_mode = (self.flags & (1 << 6)) != 0;
        modes.bracketed_paste = (self.flags & (1 << 7)) != 0;
        modes.focus_reporting = (self.flags & (1 << 8)) != 0;
        modes.synchronized_output = (self.flags & (1 << 9)) != 0;

        modes.cursor_style = match self.cursor_style {
            2 => CursorStyle::SteadyBlock,
            3 => CursorStyle::BlinkingUnderline,
            4 => CursorStyle::SteadyUnderline,
            5 => CursorStyle::BlinkingBar,
            6 => CursorStyle::SteadyBar,
            _ => CursorStyle::BlinkingBlock, // Default
        };

        modes.mouse_mode = match self.mouse_mode {
            1 => MouseMode::Normal,
            2 => MouseMode::ButtonEvent,
            3 => MouseMode::AnyEvent,
            _ => MouseMode::None,
        };

        modes.mouse_encoding = match self.mouse_encoding {
            1 => MouseEncoding::Utf8,
            2 => MouseEncoding::Sgr,
            3 => MouseEncoding::Urxvt,
            4 => MouseEncoding::SgrPixel,
            _ => MouseEncoding::X10,
        };
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.flags.to_le_bytes());
        data.push(self.cursor_style);
        data.push(self.mouse_mode);
        data.push(self.mouse_encoding);
        data
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "modes data too short",
            ));
        }
        Ok(Self {
            flags: u16::from_le_bytes([data[0], data[1]]),
            cursor_style: data[2],
            mouse_mode: data[3],
            mouse_encoding: data[4],
        })
    }
}

/// Serialized text style.
#[derive(Debug, Clone)]
pub struct SerializedStyle {
    /// Foreground color.
    pub fg: u32,
    /// Background color.
    pub bg: u32,
    /// Cell flags.
    pub flags: u16,
    /// Protected attribute.
    pub protected: bool,
}

impl SerializedStyle {
    /// Extract from CurrentStyle.
    #[must_use]
    pub fn from_style(style: &CurrentStyle) -> Self {
        Self {
            fg: style.fg.0,
            bg: style.bg.0,
            flags: style.flags.bits(),
            protected: style.protected,
        }
    }

    /// Apply to CurrentStyle.
    pub fn apply_to_style(&self, style: &mut CurrentStyle) {
        style.fg = PackedColor(self.fg);
        style.bg = PackedColor(self.bg);
        style.flags = CellFlags::from_bits(self.flags);
        style.protected = self.protected;
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.fg.to_le_bytes());
        data.extend_from_slice(&self.bg.to_le_bytes());
        data.extend_from_slice(&self.flags.to_le_bytes());
        data.push(u8::from(self.protected));
        data
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        if data.len() < 11 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "style data too short",
            ));
        }
        Ok(Self {
            fg: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            bg: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            flags: u16::from_le_bytes([data[8], data[9]]),
            protected: data[10] != 0,
        })
    }
}

/// Serialized character set state.
#[derive(Debug, Clone)]
pub struct SerializedCharset {
    /// G0 charset.
    pub g0: u8,
    /// G1 charset.
    pub g1: u8,
    /// G2 charset.
    pub g2: u8,
    /// G3 charset.
    pub g3: u8,
    /// GL mapping (0-3).
    pub gl: u8,
    /// Single shift (0=none, 2=SS2, 3=SS3).
    pub single_shift: u8,
}

impl SerializedCharset {
    /// Extract from CharacterSetState.
    #[must_use]
    pub fn from_charset(charset: &CharacterSetState) -> Self {
        Self {
            g0: charset.g0 as u8,
            g1: charset.g1 as u8,
            g2: charset.g2 as u8,
            g3: charset.g3 as u8,
            gl: charset.gl as u8,
            single_shift: match charset.single_shift {
                SingleShift::None => 0,
                SingleShift::Ss2 => 2,
                SingleShift::Ss3 => 3,
            },
        }
    }

    /// Apply to CharacterSetState.
    pub fn apply_to_charset(&self, charset: &mut CharacterSetState) {
        charset.g0 = CharacterSet::from_u8(self.g0);
        charset.g1 = CharacterSet::from_u8(self.g1);
        charset.g2 = CharacterSet::from_u8(self.g2);
        charset.g3 = CharacterSet::from_u8(self.g3);
        charset.gl = match self.gl {
            0 => GlMapping::G0,
            1 => GlMapping::G1,
            2 => GlMapping::G2,
            3 => GlMapping::G3,
            _ => GlMapping::G0,
        };
        charset.single_shift = match self.single_shift {
            2 => SingleShift::Ss2,
            3 => SingleShift::Ss3,
            _ => SingleShift::None,
        };
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        vec![
            self.g0,
            self.g1,
            self.g2,
            self.g3,
            self.gl,
            self.single_shift,
        ]
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        if data.len() < 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "charset data too short",
            ));
        }
        Ok(Self {
            g0: data[0],
            g1: data[1],
            g2: data[2],
            g3: data[3],
            gl: data[4],
            single_shift: data[5],
        })
    }
}

/// Serialized saved cursor state.
#[derive(Debug, Clone)]
pub struct SerializedCursor {
    /// Row position.
    pub row: u16,
    /// Column position.
    pub col: u16,
    /// Style at save time.
    pub style: SerializedStyle,
    /// Charset at save time.
    pub charset: SerializedCharset,
    /// Origin mode at save time.
    pub origin_mode: bool,
    /// Auto-wrap mode at save time.
    pub auto_wrap: bool,
}

impl SerializedCursor {
    /// Extract from SavedCursorState.
    #[must_use]
    pub fn from_cursor(cursor: &SavedCursorState) -> Self {
        Self {
            row: cursor.cursor.row,
            col: cursor.cursor.col,
            style: SerializedStyle {
                fg: cursor.style.fg.0,
                bg: cursor.style.bg.0,
                flags: cursor.style.flags.bits(),
                protected: cursor.style.protected,
            },
            charset: SerializedCharset::from_charset(&cursor.charset),
            origin_mode: cursor.origin_mode,
            auto_wrap: cursor.auto_wrap,
        }
    }

    /// Convert to SavedCursorState.
    #[must_use]
    pub fn to_cursor(&self) -> SavedCursorState {
        SavedCursorState {
            cursor: Cursor {
                row: self.row,
                col: self.col,
            },
            style: CurrentStyle {
                fg: PackedColor(self.style.fg),
                bg: PackedColor(self.style.bg),
                flags: CellFlags::from_bits(self.style.flags),
                protected: self.style.protected,
            },
            charset: CharacterSetState {
                g0: CharacterSet::from_u8(self.charset.g0),
                g1: CharacterSet::from_u8(self.charset.g1),
                g2: CharacterSet::from_u8(self.charset.g2),
                g3: CharacterSet::from_u8(self.charset.g3),
                gl: match self.charset.gl {
                    0 => GlMapping::G0,
                    1 => GlMapping::G1,
                    2 => GlMapping::G2,
                    3 => GlMapping::G3,
                    _ => GlMapping::G0,
                },
                single_shift: match self.charset.single_shift {
                    2 => SingleShift::Ss2,
                    3 => SingleShift::Ss3,
                    _ => SingleShift::None,
                },
            },
            origin_mode: self.origin_mode,
            auto_wrap: self.auto_wrap,
        }
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.row.to_le_bytes());
        data.extend_from_slice(&self.col.to_le_bytes());
        data.extend_from_slice(&self.style.serialize());
        data.extend_from_slice(&self.charset.serialize());
        data.push(u8::from(self.origin_mode));
        data.push(u8::from(self.auto_wrap));
        data
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> io::Result<Self> {
        if data.len() < 23 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cursor data too short",
            ));
        }
        let row = u16::from_le_bytes([data[0], data[1]]);
        let col = u16::from_le_bytes([data[2], data[3]]);
        let style = SerializedStyle::deserialize(&data[4..15])?;
        let charset = SerializedCharset::deserialize(&data[15..21])?;
        let origin_mode = data[21] != 0;
        let auto_wrap = data[22] != 0;

        Ok(Self {
            row,
            col,
            style,
            charset,
            origin_mode,
            auto_wrap,
        })
    }
}

/// Session manager for saving and restoring sessions.
#[derive(Debug)]
pub struct SessionManager {
    /// Base directory for sessions.
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager.
    #[must_use]
    pub fn new(sessions_dir: &Path) -> Self {
        Self {
            sessions_dir: sessions_dir.to_path_buf(),
        }
    }

    /// Get the sessions directory.
    #[must_use]
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// List available sessions.
    pub fn list_sessions(&self) -> io::Result<Vec<SessionInfo>> {
        let mut sessions = Vec::new();

        if !self.sessions_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("manifest.dtsession");
            if !manifest_path.exists() {
                continue;
            }

            // Try to load manifest to get info
            if let Ok(data) = fs::read(&manifest_path) {
                if let Ok(manifest) = SessionManifest::deserialize(&data) {
                    sessions.push(SessionInfo {
                        name: manifest.name.clone(),
                        path: path.clone(),
                        created_at: manifest.created_at,
                        modified_at: manifest.modified_at,
                        pane_count: manifest.pane_count(),
                        tab_count: manifest.tabs.len(),
                    });
                }
            }
        }

        // Sort by modification time (newest first)
        sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

        Ok(sessions)
    }

    /// Save a session.
    pub fn save_session(
        &self,
        name: &str,
        terminals: &[(&Terminal, Option<&Scrollback>)],
    ) -> io::Result<PathBuf> {
        let session_dir = self.sessions_dir.join(name);
        fs::create_dir_all(&session_dir)?;

        let mut manifest = SessionManifest::new(name);
        let mut tab = TabManifest::new("default");

        // Save each terminal
        for (idx, (terminal, scrollback)) in terminals.iter().enumerate() {
            let pane_id = encode_len(idx);
            let mut pane = PaneManifest::new(pane_id);

            // Set metadata from terminal
            pane.title = Some(terminal.title().to_string());
            pane.cwd = terminal.current_working_directory().map(|s| s.to_string());
            pane.is_focused = idx == 0; // First pane is focused by default

            // Save checkpoint (grid + scrollback)
            let checkpoint_path = session_dir.join(&pane.checkpoint_file);
            self.save_checkpoint_to(&checkpoint_path, terminal.grid(), *scrollback)?;

            // Save terminal state
            let state = TerminalState::from_terminal(terminal);
            let state_path = session_dir.join(&pane.state_file);
            fs::write(&state_path, state.serialize())?;

            tab.add_pane(pane);
        }

        if !tab.panes.is_empty() {
            tab.focused_pane = Some(0);
        }

        manifest.add_tab(tab);

        // Save manifest
        let manifest_path = session_dir.join("manifest.dtsession");
        fs::write(&manifest_path, manifest.serialize())?;

        Ok(session_dir)
    }

    /// Helper to save checkpoint to specific path.
    fn save_checkpoint_to(
        &self,
        path: &Path,
        grid: &Grid,
        scrollback: Option<&Scrollback>,
    ) -> io::Result<()> {
        // Use a temporary checkpoint manager to get the serialization logic
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut manager = CheckpointManager::with_config(
            dir,
            CheckpointConfig {
                compress: true,
                compression_level: 3,
                ..Default::default()
            },
        );

        manager.save(grid, scrollback)?;

        // Find the saved checkpoint and rename to our target path
        let checkpoints: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "dtck")
                    .unwrap_or(false)
            })
            .collect();

        if let Some(checkpoint) = checkpoints.into_iter().next() {
            fs::rename(checkpoint.path(), path)?;
        }

        Ok(())
    }

    /// Load a session.
    pub fn load_session(&self, name: &str) -> io::Result<LoadedSession> {
        let session_dir = self.sessions_dir.join(name);
        let manifest_path = session_dir.join("manifest.dtsession");

        let manifest_data = fs::read(&manifest_path)?;
        let manifest = SessionManifest::deserialize(&manifest_data)?;

        Ok(LoadedSession {
            manifest,
            session_dir,
        })
    }

    /// Delete a session.
    pub fn delete_session(&self, name: &str) -> io::Result<()> {
        let session_dir = self.sessions_dir.join(name);
        if session_dir.exists() {
            fs::remove_dir_all(session_dir)?;
        }
        Ok(())
    }

    /// Check if a session exists.
    #[must_use]
    pub fn session_exists(&self, name: &str) -> bool {
        let manifest_path = self.sessions_dir.join(name).join("manifest.dtsession");
        manifest_path.exists()
    }
}

/// Information about an available session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session name.
    pub name: String,
    /// Path to session directory.
    pub path: PathBuf,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last modification timestamp.
    pub modified_at: u64,
    /// Number of panes.
    pub pane_count: usize,
    /// Number of tabs.
    pub tab_count: usize,
}

/// A loaded session ready for restoration.
#[derive(Debug)]
pub struct LoadedSession {
    /// Session manifest.
    pub manifest: SessionManifest,
    /// Session directory.
    session_dir: PathBuf,
}

impl LoadedSession {
    /// Get all panes from the session.
    pub fn panes(&self) -> impl Iterator<Item = &PaneManifest> {
        self.manifest.tabs.iter().flat_map(|t| t.panes.iter())
    }

    /// Restore a terminal from a pane.
    pub fn restore_terminal(
        &self,
        pane: &PaneManifest,
    ) -> io::Result<(Terminal, Option<Scrollback>)> {
        // Load checkpoint
        let checkpoint_path = self.session_dir.join(&pane.checkpoint_file);
        let checkpoint_manager = CheckpointManager::new(&self.session_dir);
        let (grid, scrollback) = checkpoint_manager.restore_from(&checkpoint_path)?;

        // Create terminal with restored grid
        let mut terminal = Terminal::from_grid(grid);

        // Load and apply state
        let state_path = self.session_dir.join(&pane.state_file);
        if state_path.exists() {
            let state_data = fs::read(&state_path)?;
            let state = TerminalState::deserialize(&state_data)?;
            state.apply_to(&mut terminal);
        }

        Ok((terminal, scrollback))
    }

    /// Get the session directory.
    #[must_use]
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn session_manifest_roundtrip() {
        let mut manifest = SessionManifest::new("test-session");
        manifest.global_cwd = Some("/home/user".to_string());

        let mut tab = TabManifest::new("Tab 1");
        let mut pane = PaneManifest::new(0);
        pane.title = Some("Terminal".to_string());
        pane.cwd = Some("/home/user/project".to_string());
        tab.add_pane(pane);
        tab.focused_pane = Some(0);

        manifest.add_tab(tab);

        let data = manifest.serialize();
        let restored = SessionManifest::deserialize(&data).unwrap();

        assert_eq!(restored.name, "test-session");
        assert_eq!(restored.global_cwd, Some("/home/user".to_string()));
        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.tabs[0].name, "Tab 1");
        assert_eq!(restored.tabs[0].panes.len(), 1);
        assert_eq!(
            restored.tabs[0].panes[0].title,
            Some("Terminal".to_string())
        );
    }

    #[test]
    fn terminal_state_roundtrip() {
        let mut terminal = Terminal::new(24, 80);
        let mut params = InlineImageParams::new();
        params.name = Some("test.png".to_string());
        params.width = DimensionSpec::Pixels(120);
        params.height = DimensionSpec::Cells(3);
        params.preserve_aspect_ratio = false;
        params.inline = true;
        assert!(terminal
            .inline_images_mut()
            .store(vec![1, 2, 3], &params, 4, 5)
            .is_some());
        let state = TerminalState::from_terminal(&terminal);

        let data = state.serialize();
        let restored = TerminalState::deserialize(&data).unwrap();

        assert_eq!(restored.title, terminal.title());
        assert_eq!(restored.modes.flags, state.modes.flags);
        assert_eq!(restored.inline_images.len(), 1);
        let image = &restored.inline_images[0];
        assert_eq!(image.name.as_deref(), Some("test.png"));
        assert_eq!(image.width, DimensionSpec::Pixels(120));
        assert_eq!(image.height, DimensionSpec::Cells(3));
        assert!(!image.preserve_aspect_ratio);
        assert_eq!(image.cursor_row, 4);
        assert_eq!(image.cursor_col, 5);
    }

    #[test]
    fn serialized_modes_roundtrip() {
        let mut modes = TerminalModes::new();
        modes.bracketed_paste = true;
        modes.application_cursor_keys = true;
        modes.mouse_mode = MouseMode::ButtonEvent;

        let serialized = SerializedModes::from_modes(&modes);
        let data = serialized.serialize();
        let restored = SerializedModes::deserialize(&data).unwrap();

        let mut result_modes = TerminalModes::default();
        restored.apply_to_modes(&mut result_modes);

        assert!(result_modes.bracketed_paste);
        assert!(result_modes.application_cursor_keys);
        assert_eq!(result_modes.mouse_mode, MouseMode::ButtonEvent);
    }

    #[test]
    fn session_manager_list_empty() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path());
        let sessions = manager.list_sessions().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn session_manager_save_and_list() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path());

        let terminal = Terminal::new(24, 80);
        let terminals: Vec<(&Terminal, Option<&Scrollback>)> = vec![(&terminal, None)];

        manager.save_session("my-session", &terminals).unwrap();

        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "my-session");
        assert_eq!(sessions[0].pane_count, 1);
    }

    #[test]
    fn session_manager_save_and_load() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path());

        let mut terminal = Terminal::new(24, 80);
        terminal.set_title("Test Terminal");
        let mut params = InlineImageParams::new();
        params.inline = true;
        params.width = DimensionSpec::Pixels(10);
        params.height = DimensionSpec::Pixels(10);
        assert!(terminal
            .inline_images_mut()
            .store(b"Hello".to_vec(), &params, 1, 2)
            .is_some());

        let terminals: Vec<(&Terminal, Option<&Scrollback>)> = vec![(&terminal, None)];
        manager.save_session("test-restore", &terminals).unwrap();

        let loaded = manager.load_session("test-restore").unwrap();
        assert_eq!(loaded.manifest.name, "test-restore");

        let pane = loaded.panes().next().unwrap();
        let (restored_terminal, _) = loaded.restore_terminal(pane).unwrap();

        assert_eq!(restored_terminal.title(), "Test Terminal");
        assert_eq!(restored_terminal.inline_images().len(), 1);
        assert_eq!(
            restored_terminal.inline_images().images()[0].data(),
            b"Hello"
        );
    }

    #[test]
    fn session_manager_delete() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path());

        let terminal = Terminal::new(24, 80);
        let terminals: Vec<(&Terminal, Option<&Scrollback>)> = vec![(&terminal, None)];

        manager.save_session("to-delete", &terminals).unwrap();
        assert!(manager.session_exists("to-delete"));

        manager.delete_session("to-delete").unwrap();
        assert!(!manager.session_exists("to-delete"));
    }

    #[test]
    fn command_info_roundtrip() {
        let mut cmd = CommandInfo::new("/bin/bash");
        cmd.args = vec!["-c".to_string(), "echo hello".to_string()];
        cmd.env
            .insert("TERM".to_string(), "xterm-256color".to_string());

        let data = cmd.serialize();
        let restored = CommandInfo::deserialize(&data).unwrap();

        assert_eq!(restored.program, "/bin/bash");
        assert_eq!(restored.args, vec!["-c", "echo hello"]);
        assert_eq!(
            restored.env.get("TERM"),
            Some(&"xterm-256color".to_string())
        );
    }

    #[test]
    fn pane_geometry_default() {
        let geom = PaneGeometry::default();
        assert_eq!(geom.x, 0);
        assert_eq!(geom.y, 0);
        assert_eq!(geom.width, 0);
        assert_eq!(geom.height, 0);
    }

    #[test]
    fn serialized_cursor_roundtrip() {
        let cursor = SerializedCursor {
            row: 10,
            col: 20,
            style: SerializedStyle {
                fg: 0xFF_FF_FF_FF,
                bg: 0x00_00_00_00,
                flags: 0x0001, // Bold
                protected: true,
            },
            charset: SerializedCharset {
                g0: 0,
                g1: 1,
                g2: 0,
                g3: 0,
                gl: 0,
                single_shift: 0,
            },
            origin_mode: true,
            auto_wrap: false,
        };

        let data = cursor.serialize();
        let restored = SerializedCursor::deserialize(&data).unwrap();

        assert_eq!(restored.row, 10);
        assert_eq!(restored.col, 20);
        assert!(restored.origin_mode);
        assert!(!restored.auto_wrap);
        assert!(restored.style.protected);
    }
}
