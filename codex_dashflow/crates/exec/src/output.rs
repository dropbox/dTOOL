//! Output handlers for exec mode

use std::io::Write;
use std::sync::{Arc, Mutex};

use codex_dashflow_core::streaming::{AgentEvent, StreamCallback};
use serde::Serialize;

/// Output from exec mode execution
#[derive(Debug, Clone, Serialize)]
pub struct ExecOutput {
    /// Session ID
    pub session_id: String,
    /// Final response text
    pub final_response: String,
    /// Number of turns executed
    pub turns: u32,
    /// Execution status
    pub status: String,
    /// Tool calls made during execution
    pub tool_calls: Vec<ToolCallRecord>,
}

/// Record of a tool call
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRecord {
    /// Tool name
    pub tool: String,
    /// Tool arguments
    pub args: serde_json::Value,
    /// Whether the call succeeded
    pub success: bool,
    /// Output preview
    pub output_preview: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Trait for output handlers
pub trait OutputHandler: Send + Sync {
    /// Handle a streaming event
    fn handle_event(&self, event: &AgentEvent);

    /// Print final output
    fn print_final(&self, output: &ExecOutput);
}

/// JSON output handler - outputs JSON Lines
pub struct JsonOutputHandler<W: Write + Send> {
    writer: Mutex<W>,
}

impl<W: Write + Send> JsonOutputHandler<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
        }
    }
}

impl<W: Write + Send + Sync> OutputHandler for JsonOutputHandler<W> {
    fn handle_event(&self, event: &AgentEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writeln!(writer, "{}", json);
                let _ = writer.flush();
            }
        }
    }

    fn print_final(&self, output: &ExecOutput) {
        if let Ok(json) = serde_json::to_string(output) {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writeln!(writer, "{}", json);
            }
        }
    }
}

/// Human-readable output handler
pub struct HumanOutputHandler<W: Write + Send> {
    writer: Mutex<W>,
    verbose: bool,
}

impl<W: Write + Send> HumanOutputHandler<W> {
    pub fn new(writer: W, verbose: bool) -> Self {
        Self {
            writer: Mutex::new(writer),
            verbose,
        }
    }
}

impl<W: Write + Send + Sync> OutputHandler for HumanOutputHandler<W> {
    fn handle_event(&self, event: &AgentEvent) {
        if !self.verbose {
            return;
        }

        let output = match event {
            AgentEvent::ReasoningStart { .. } => Some("[Agent] Thinking...".to_string()),
            AgentEvent::ReasoningComplete { duration_ms, .. } => {
                Some(format!("[Agent] Reasoning complete ({} ms)", duration_ms))
            }
            AgentEvent::ToolCallRequested { tool, args, .. } => {
                Some(format!("[Tool] Calling: {} with args: {}", tool, args))
            }
            AgentEvent::ToolExecutionStart { tool, .. } => {
                Some(format!("[Tool] Executing: {}", tool))
            }
            AgentEvent::ToolExecutionComplete {
                tool,
                success,
                output_preview,
                ..
            } => {
                let status = if *success { "+" } else { "x" };
                Some(format!("[Tool] {} {}: {}", status, tool, output_preview))
            }
            AgentEvent::TurnComplete { turn, .. } => {
                Some(format!("[Agent] Turn {} complete", turn))
            }
            _ => None,
        };

        if let Some(msg) = output {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writeln!(writer, "{}", msg);
                let _ = writer.flush();
            }
        }
    }

    fn print_final(&self, output: &ExecOutput) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writeln!(writer);
            let _ = writeln!(writer, "{}", output.final_response);
        }
    }
}

/// Streaming callback that collects events and forwards to an output handler
pub struct ExecStreamCallback {
    handler: Arc<dyn OutputHandler>,
    tool_calls: Mutex<Vec<ToolCallRecord>>,
}

impl ExecStreamCallback {
    pub fn new(handler: Arc<dyn OutputHandler>) -> Self {
        Self {
            handler,
            tool_calls: Mutex::new(Vec::new()),
        }
    }

    /// Get recorded tool calls
    pub fn tool_calls(&self) -> Vec<ToolCallRecord> {
        self.tool_calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl StreamCallback for ExecStreamCallback {
    async fn on_event(&self, event: AgentEvent) {
        // Forward to handler
        self.handler.handle_event(&event);

        // Record tool calls
        if let AgentEvent::ToolExecutionComplete {
            tool,
            success,
            output_preview,
            duration_ms,
            ..
        } = &event
        {
            if let Ok(mut calls) = self.tool_calls.lock() {
                calls.push(ToolCallRecord {
                    tool: tool.clone(),
                    args: serde_json::Value::Null, // Args captured from request event
                    success: *success,
                    output_preview: output_preview.clone(),
                    duration_ms: *duration_ms,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_output_serialize() {
        let output = ExecOutput {
            session_id: "test-123".to_string(),
            final_response: "Hello, world!".to_string(),
            turns: 2,
            status: "complete".to_string(),
            tool_calls: vec![],
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("test-123"));
        assert!(json.contains("Hello, world!"));
    }

    #[test]
    fn test_tool_call_record_serialize() {
        let record = ToolCallRecord {
            tool: "shell".to_string(),
            args: serde_json::json!({"command": "ls"}),
            success: true,
            output_preview: "file1.txt\nfile2.txt".to_string(),
            duration_ms: 50,
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("shell"));
        assert!(json.contains("ls"));
    }

    #[test]
    fn test_json_output_handler() {
        let buffer: Vec<u8> = Vec::new();
        let handler = JsonOutputHandler::new(buffer);

        let event = AgentEvent::ReasoningStart {
            session_id: "test".to_string(),
            turn: 1,
            model: "gpt-4".to_string(),
        };
        handler.handle_event(&event);

        // The buffer is inside the mutex, verify handler doesn't panic
    }

    #[test]
    fn test_human_output_handler_verbose() {
        let buffer: Vec<u8> = Vec::new();
        let handler = HumanOutputHandler::new(buffer, true);

        let event = AgentEvent::ReasoningStart {
            session_id: "test".to_string(),
            turn: 1,
            model: "gpt-4".to_string(),
        };
        handler.handle_event(&event);

        // Handler should process verbose events
    }

    #[test]
    fn test_human_output_handler_quiet() {
        let buffer: Vec<u8> = Vec::new();
        let handler = HumanOutputHandler::new(buffer, false);

        let event = AgentEvent::ReasoningStart {
            session_id: "test".to_string(),
            turn: 1,
            model: "gpt-4".to_string(),
        };
        handler.handle_event(&event);

        // Handler should skip events when not verbose
    }
}
