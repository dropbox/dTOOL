//! Session persistence for Codex DashFlow agent runs.
//!
//! Stores agent message history on disk so users can resume later.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use dashflow::prebuilt::AgentState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const SESSION_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub working_dir: Option<PathBuf>,
    pub state: AgentState,
}

impl Session {
    pub fn new(state: AgentState, working_dir: Option<&Path>) -> Self {
        let now = Utc::now();
        Self {
            version: SESSION_VERSION,
            created_at: now,
            updated_at: now,
            working_dir: working_dir.map(|p| p.to_path_buf()),
            state,
        }
    }

    pub fn update_state(&mut self, state: AgentState) {
        self.updated_at = Utc::now();
        self.state = state;
    }
}

pub fn default_session_path() -> Result<PathBuf> {
    let home =
        dirs::home_dir().context("Could not determine home directory for session storage")?;
    Ok(home
        .join(".codex-dashflow")
        .join("sessions")
        .join("default.json"))
}

pub async fn load_session(path: &Path) -> Result<Session> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("Failed to read session file: {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("Failed to parse session JSON from file: {}", path.display()))
}

pub async fn save_session(path: &Path, session: &Session) -> Result<()> {
    let parent = path
        .parent()
        .context("Session path must have a parent directory")?;
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("Failed to create session directory: {}", parent.display()))?;

    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("session"),
        Uuid::new_v4()
    ));

    let bytes = serde_json::to_vec_pretty(session).context("Failed to serialize session JSON")?;
    tokio::fs::write(&tmp_path, bytes)
        .await
        .with_context(|| format!("Failed to write temp session file: {}", tmp_path.display()))?;

    match tokio::fs::rename(&tmp_path, path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            // Windows rename fails if destination exists.
            if path.exists() {
                let _ = tokio::fs::remove_file(path).await;
            }
            tokio::fs::rename(&tmp_path, path).await.with_context(|| {
                format!(
                    "Failed to move session into place: {} ({})",
                    path.display(),
                    e
                )
            })?;
            Ok(())
        }
    }
}

pub async fn load_or_create_session(
    path: &Path,
    resume: bool,
    initial_state: AgentState,
    working_dir: Option<&Path>,
) -> Result<Session> {
    if path.exists() {
        let session = load_session(path).await?;
        if session.version != SESSION_VERSION {
            bail!(
                "Unsupported session version {} (expected {})",
                session.version,
                SESSION_VERSION
            );
        }
        Ok(session)
    } else if resume {
        bail!("Session file does not exist: {}", path.display());
    } else {
        Ok(Session::new(initial_state, working_dir))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    #[tokio::test]
    async fn test_session_roundtrip_save_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session.json");

        let state = AgentState::new(Message::system("system"));
        let session = Session::new(state.clone(), None);

        save_session(&path, &session).await.expect("save");
        let loaded = load_session(&path).await.expect("load");

        assert_eq!(loaded.version, SESSION_VERSION);
        assert_eq!(loaded.state.messages.len(), state.messages.len());
        assert_eq!(loaded.state.messages[0].message_type(), "system");
    }

    #[tokio::test]
    async fn test_load_or_create_session_resume_missing_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.json");

        let err = load_or_create_session(
            &path,
            true,
            AgentState::new(Message::system("system")),
            None,
        )
        .await
        .expect_err("should fail");

        assert!(err.to_string().contains("does not exist"));
    }
}
