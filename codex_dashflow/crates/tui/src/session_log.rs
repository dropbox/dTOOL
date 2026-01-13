//! Session logging for TUI debugging and replay.
//!
//! Provides opt-in session recording for debugging TUI sessions.
//! Enable by setting `CODEX_TUI_RECORD_SESSION=1` environment variable.
//!
//! Session logs are written in JSONL format to `~/.codex-dashflow/logs/session-<timestamp>.jsonl`
//! or to a custom path via `CODEX_TUI_SESSION_LOG_PATH`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, OnceLock};

use codex_dashflow_core::streaming::AgentEvent;
use crossterm::event::Event as CrosstermEvent;
use serde::Serialize;
use serde_json::json;

use crate::event::TuiEvent;

static LOGGER: LazyLock<SessionLogger> = LazyLock::new(SessionLogger::new);

/// Session logger that writes JSONL records to a file.
struct SessionLogger {
    file: OnceLock<Mutex<File>>,
}

impl SessionLogger {
    fn new() -> Self {
        Self {
            file: OnceLock::new(),
        }
    }

    fn open(&self, path: PathBuf) -> std::io::Result<()> {
        let mut opts = OpenOptions::new();
        opts.create(true).truncate(true).write(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }

        let file = opts.open(path)?;
        self.file.get_or_init(|| Mutex::new(file));
        Ok(())
    }

    fn write_json_line(&self, value: serde_json::Value) {
        let Some(mutex) = self.file.get() else {
            return;
        };
        let mut guard = match mutex.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        match serde_json::to_string(&value) {
            Ok(serialized) => {
                if let Err(e) = guard.write_all(serialized.as_bytes()) {
                    tracing::warn!("session log write error: {}", e);
                    return;
                }
                if let Err(e) = guard.write_all(b"\n") {
                    tracing::warn!("session log write error: {}", e);
                    return;
                }
                if let Err(e) = guard.flush() {
                    tracing::warn!("session log flush error: {}", e);
                }
            }
            Err(e) => tracing::warn!("session log serialize error: {}", e),
        }
    }

    fn is_enabled(&self) -> bool {
        self.file.get().is_some()
    }
}

fn now_ts() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Initialize session logging if enabled.
///
/// Enable by setting `CODEX_TUI_RECORD_SESSION=1` environment variable.
/// Optionally specify output path with `CODEX_TUI_SESSION_LOG_PATH`.
pub fn maybe_init(model: &str, cwd: &str) {
    let enabled = std::env::var("CODEX_TUI_RECORD_SESSION")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let path = if let Ok(path) = std::env::var("CODEX_TUI_SESSION_LOG_PATH") {
        PathBuf::from(path)
    } else {
        let mut p = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
        p.push(".codex-dashflow");
        p.push("logs");
        if let Err(e) = std::fs::create_dir_all(&p) {
            tracing::error!("failed to create log directory {:?}: {}", p, e);
            return;
        }
        let filename = format!(
            "session-{}.jsonl",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        );
        p.push(filename);
        p
    };

    if let Err(e) = LOGGER.open(path.clone()) {
        tracing::error!("failed to open session log {:?}: {}", path, e);
        return;
    }

    // Write a header record with session context
    let header = json!({
        "ts": now_ts(),
        "dir": "meta",
        "kind": "session_start",
        "cwd": cwd,
        "model": model,
    });
    LOGGER.write_json_line(header);
    tracing::info!("Session recording enabled: {:?}", path);
}

/// Check if session logging is enabled.
pub fn is_enabled() -> bool {
    LOGGER.is_enabled()
}

/// Log an inbound TUI event.
pub fn log_inbound_event(event: &TuiEvent) {
    if !LOGGER.is_enabled() {
        return;
    }

    match event {
        TuiEvent::Terminal(crossterm_event) => {
            match crossterm_event {
                CrosstermEvent::Key(key_event) => {
                    let value = json!({
                        "ts": now_ts(),
                        "dir": "to_tui",
                        "kind": "key_event",
                        "code": format!("{:?}", key_event.code),
                        "modifiers": format!("{:?}", key_event.modifiers),
                    });
                    LOGGER.write_json_line(value);
                }
                CrosstermEvent::Resize(width, height) => {
                    let value = json!({
                        "ts": now_ts(),
                        "dir": "to_tui",
                        "kind": "resize",
                        "width": width,
                        "height": height,
                    });
                    LOGGER.write_json_line(value);
                }
                // Skip mouse and other events to reduce noise
                _ => {}
            }
        }
        TuiEvent::Agent(agent_event) => {
            let (kind, extra) = match agent_event {
                AgentEvent::UserTurn { content, .. } => {
                    ("user_turn", json!({ "content_len": content.len() }))
                }
                AgentEvent::ReasoningStart { turn, model, .. } => {
                    ("reasoning_start", json!({ "turn": turn, "model": model }))
                }
                AgentEvent::ReasoningComplete {
                    turn,
                    duration_ms,
                    has_tool_calls,
                    tool_count,
                    ..
                } => (
                    "reasoning_complete",
                    json!({
                        "turn": turn,
                        "duration_ms": duration_ms,
                        "has_tool_calls": has_tool_calls,
                        "tool_count": tool_count
                    }),
                ),
                AgentEvent::LlmMetrics {
                    model,
                    input_tokens,
                    output_tokens,
                    latency_ms,
                    ..
                } => (
                    "llm_metrics",
                    json!({
                        "model": model,
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "latency_ms": latency_ms
                    }),
                ),
                AgentEvent::ToolCallRequested { tool, .. } => {
                    ("tool_call_requested", json!({ "tool": tool }))
                }
                AgentEvent::ToolCallApproved { tool, .. } => {
                    ("tool_call_approved", json!({ "tool": tool }))
                }
                AgentEvent::ToolCallRejected { tool, reason, .. } => (
                    "tool_call_rejected",
                    json!({ "tool": tool, "reason": reason }),
                ),
                AgentEvent::ToolExecutionStart { tool, .. } => {
                    ("tool_execution_start", json!({ "tool": tool }))
                }
                AgentEvent::ToolExecutionComplete {
                    tool,
                    success,
                    duration_ms,
                    ..
                } => (
                    "tool_execution_complete",
                    json!({
                        "tool": tool,
                        "success": success,
                        "duration_ms": duration_ms
                    }),
                ),
                AgentEvent::TurnComplete { turn, status, .. } => {
                    ("turn_complete", json!({ "turn": turn, "status": status }))
                }
                AgentEvent::SessionComplete {
                    total_turns,
                    status,
                    ..
                } => (
                    "session_complete",
                    json!({ "total_turns": total_turns, "status": status }),
                ),
                AgentEvent::TokenChunk { is_final, .. } => {
                    ("token_chunk", json!({ "is_final": is_final }))
                }
                AgentEvent::Error { error, context, .. } => {
                    ("error", json!({ "error": error, "context": context }))
                }
                AgentEvent::ApprovalRequired { tool, reason, .. } => (
                    "approval_required",
                    json!({ "tool": tool, "reason": reason }),
                ),
                AgentEvent::EvalCapture { .. } => ("eval_capture", json!({})),
                AgentEvent::QualityGateStart {
                    attempt,
                    max_retries,
                    threshold,
                    ..
                } => (
                    "quality_gate_start",
                    json!({
                        "attempt": attempt,
                        "max_retries": max_retries,
                        "threshold": threshold,
                    }),
                ),
                AgentEvent::QualityGateResult {
                    attempt,
                    passed,
                    average_score,
                    is_final,
                    reason,
                    ..
                } => (
                    "quality_gate_result",
                    json!({
                        "attempt": attempt,
                        "passed": passed,
                        "average_score": average_score,
                        "is_final": is_final,
                        "reason": reason,
                    }),
                ),
                AgentEvent::SessionMetrics {
                    total_input_tokens,
                    total_output_tokens,
                    total_cost_usd,
                    ..
                } => (
                    "session_metrics",
                    json!({
                        "total_input_tokens": total_input_tokens,
                        "total_output_tokens": total_output_tokens,
                        "total_cost_usd": total_cost_usd,
                    }),
                ),
            };
            let value = json!({
                "ts": now_ts(),
                "dir": "to_tui",
                "kind": kind,
                "payload": extra,
            });
            LOGGER.write_json_line(value);
        }
        TuiEvent::Tick => {
            // Skip tick events to reduce noise
        }
        TuiEvent::Quit => {
            let value = json!({
                "ts": now_ts(),
                "dir": "to_tui",
                "kind": "quit_signal",
            });
            LOGGER.write_json_line(value);
        }
        TuiEvent::ApprovalRequest(req) => {
            let value = json!({
                "ts": now_ts(),
                "dir": "to_tui",
                "kind": "approval_request",
                "request_id": req.request_id,
                "tool": req.tool,
                "reason": req.reason,
            });
            LOGGER.write_json_line(value);
        }
    }
}

/// Log a user message submission.
pub fn log_user_message(message: &str) {
    if !LOGGER.is_enabled() {
        return;
    }
    let value = json!({
        "ts": now_ts(),
        "dir": "from_tui",
        "kind": "user_message",
        "content_len": message.len(),
        "is_slash_command": message.starts_with('/'),
    });
    LOGGER.write_json_line(value);
}

/// Log a slash command execution.
pub fn log_slash_command(command: &str, args: Option<&str>) {
    if !LOGGER.is_enabled() {
        return;
    }
    let value = json!({
        "ts": now_ts(),
        "dir": "from_tui",
        "kind": "slash_command",
        "command": command,
        "has_args": args.is_some(),
    });
    LOGGER.write_json_line(value);
}

/// Log session end.
pub fn log_session_end() {
    if !LOGGER.is_enabled() {
        return;
    }
    let value = json!({
        "ts": now_ts(),
        "dir": "meta",
        "kind": "session_end",
    });
    LOGGER.write_json_line(value);
}

/// Write a generic record with payload.
pub fn write_record<T>(dir: &str, kind: &str, payload: &T)
where
    T: Serialize,
{
    if !LOGGER.is_enabled() {
        return;
    }
    let value = json!({
        "ts": now_ts(),
        "dir": dir,
        "kind": kind,
        "payload": payload,
    });
    LOGGER.write_json_line(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::NamedTempFile;

    #[test]
    fn test_session_logger_disabled_by_default() {
        // Logger should not be enabled by default
        let logger = SessionLogger::new();
        assert!(!logger.is_enabled());
    }

    #[test]
    fn test_session_logger_open_creates_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let logger = SessionLogger::new();
        assert!(logger.open(path.clone()).is_ok());
        assert!(logger.is_enabled());
    }

    #[test]
    fn test_session_logger_writes_json_line() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let logger = SessionLogger::new();
        logger.open(path.clone()).unwrap();

        let value = json!({ "test": "value" });
        logger.write_json_line(value);

        let mut contents = String::new();
        File::open(path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();

        assert!(contents.contains("\"test\":\"value\""));
        assert!(contents.ends_with('\n'));
    }

    #[test]
    fn test_now_ts_returns_rfc3339() {
        let ts = now_ts();
        // Should be RFC3339 format: YYYY-MM-DDTHH:MM:SS.mmmZ
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert!(ts.len() > 20);
    }

    #[test]
    fn test_write_record_disabled_noop() {
        // When logger is not initialized, write_record should be a no-op
        #[derive(Serialize)]
        struct TestPayload {
            foo: &'static str,
        }
        // This should not panic
        write_record("test", "test_kind", &TestPayload { foo: "bar" });
    }

    #[test]
    fn test_log_user_message_tracks_slash_command() {
        // Just verifies the function runs without panicking
        log_user_message("regular message");
        log_user_message("/command arg");
    }

    #[test]
    fn test_log_slash_command_with_args() {
        // Just verifies the function runs without panicking
        log_slash_command("help", None);
        log_slash_command("model", Some("gpt-4"));
    }

    #[test]
    fn test_log_session_end_noop_when_disabled() {
        // Should not panic when logger is disabled
        log_session_end();
    }
}
