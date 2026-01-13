//! Agent Output Parser
//!
//! Parses terminal output to detect AI agent events and patterns.
//! Supports multiple agent output formats and emits structured events
//! for graph visualization updates.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::LazyLock;

/// Events detected from agent output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum AgentEvent {
    /// A node/step has started execution
    NodeStart {
        node_id: String,
        label: String,
        node_type: AgentNodeType,
    },
    /// A node/step has completed
    NodeComplete {
        node_id: String,
        status: AgentStatus,
        duration_ms: Option<u64>,
    },
    /// A tool is being invoked
    ToolUse {
        tool_name: String,
        tool_id: Option<String>,
        input_preview: Option<String>,
    },
    /// Tool completed with result
    ToolResult {
        tool_name: String,
        tool_id: Option<String>,
        success: bool,
    },
    /// Agent is thinking/reasoning
    Thinking {
        preview: Option<String>,
    },
    /// Agent produced output
    Output {
        content: String,
    },
    /// Error occurred
    Error {
        message: String,
        node_id: Option<String>,
    },
    /// Agent flow started
    FlowStart {
        name: String,
    },
    /// Agent flow completed
    FlowComplete {
        name: String,
        success: bool,
    },
}

/// Types of agent nodes matching graph node types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentNodeType {
    Start,
    End,
    Model,
    Tool,
    Condition,
    Parallel,
    Join,
    Human,
    Custom,
}

/// Execution status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    Success,
    Failed,
    Skipped,
}

/// Compiled regex patterns for agent output detection
struct AgentPatterns {
    // Claude Code patterns
    claude_tool_use: Regex,
    claude_tool_result: Regex,
    claude_thinking: Regex,

    // Generic agent patterns
    node_start: Regex,
    node_complete: Regex,
    step_start: Regex,
    step_complete: Regex,

    // Tool patterns
    tool_call: Regex,
    tool_complete: Regex,

    // Flow patterns
    flow_start: Regex,
    flow_complete: Regex,

    // Error patterns
    error_line: Regex,

    // LangChain patterns
    langchain_chain_start: Regex,
    langchain_chain_end: Regex,
    langchain_thought: Regex,
    langchain_action: Regex,
    langchain_action_input: Regex,
    langchain_observation: Regex,
    langchain_final_answer: Regex,

    // AutoGPT patterns
    autogpt_thoughts: Regex,
    autogpt_reasoning: Regex,
    autogpt_plan: Regex,
    autogpt_criticism: Regex,
    autogpt_next_action: Regex,
    autogpt_command: Regex,
    autogpt_system: Regex,
}

static PATTERNS: LazyLock<AgentPatterns> = LazyLock::new(|| {
    AgentPatterns {
        // Claude Code specific patterns
        // Match: "using tool: Bash", "calling `Read`", "invoking 'Write'"
        claude_tool_use: Regex::new(r#"(?i)(?:using|calling|invoking)\s+(?:tool:?\s*)?[`'"]?(\w+)[`'"]?"#).unwrap(),
        claude_tool_result: Regex::new(r#"(?i)(?:tool|function)\s+[`'"]?(\w+)[`'"]?\s+(?:returned|completed|finished)"#).unwrap(),
        claude_thinking: Regex::new(r"(?i)(?:thinking|reasoning|analyzing)\.{3}").unwrap(),

        // Generic node/step patterns (LangGraph style)
        node_start: Regex::new(r"\[(?:NODE|STEP)\]\s*(\w+):\s*(?:start(?:ing|ed)?|running|executing)").unwrap(),
        node_complete: Regex::new(r"\[(?:NODE|STEP)\]\s*(\w+):\s*(?:complete[d]?|finish(?:ed)?|done)(?:\s*\((\w+)\))?").unwrap(),
        step_start: Regex::new(r"(?:^|\n)(?:>>>|-->|==>)\s*(?:Step|Stage|Phase)\s+(\d+|[\w_]+):\s*(.+?)(?:\n|$)").unwrap(),
        step_complete: Regex::new(r"(?:^|\n)(?:<<<|<--|<==)\s*(?:Step|Stage|Phase)\s+(\d+|[\w_]+):\s*(?:complete[d]?|done|finish(?:ed)?)\s*(?:\((\w+)\))?").unwrap(),

        // Tool patterns
        tool_call: Regex::new(r#"(?i)(?:^|\n)\s*(?:\[TOOL\]|\*\*Tool:\*\*|Tool:)\s*[`'"]?(\w+)[`'"]?(?:\s*\((.*?)\))?"#).unwrap(),
        tool_complete: Regex::new(r#"(?i)(?:^|\n)\s*(?:\[TOOL_RESULT\]|\*\*Result:\*\*|Result:)\s*[`'"]?(\w+)[`'"]?\s*(?:->|:)\s*(\w+)"#).unwrap(),

        // Flow patterns - use \S+ for non-whitespace to match flow names
        flow_start: Regex::new(r#"(?i)(?:^|\n)\s*(?:\[FLOW\]|\*\*Flow:\*\*|Starting flow:?)\s*[`'"]?(\S+?)[`'"]?(?:\s|$)"#).unwrap(),
        flow_complete: Regex::new(r#"(?i)(?:^|\n)\s*(?:\[FLOW_END\]|\*\*Flow Complete:\*\*|Flow completed:?)\s*[`'"]?(\S+)[`'"]?\s*(?:\((\w+)\))?"#).unwrap(),

        // Error patterns
        error_line: Regex::new(r"(?i)(?:^|\n)\s*(?:error|exception|failed|failure):\s*(.+?)(?:\n|$)").unwrap(),

        // LangChain patterns
        // > Entering new AgentExecutor chain...
        langchain_chain_start: Regex::new(r"(?i)>\s*Entering\s+new\s+(\w+)\s+chain").unwrap(),
        // > Finished chain.
        langchain_chain_end: Regex::new(r"(?i)>\s*Finished\s+chain\.?").unwrap(),
        // Thought: I should search for...
        langchain_thought: Regex::new(r"(?i)^Thought:\s*(.+)").unwrap(),
        // Action: search
        langchain_action: Regex::new(r"(?i)^Action:\s*(\w+)").unwrap(),
        // Action Input: query string
        langchain_action_input: Regex::new(r"(?i)^Action\s*Input:\s*(.+)").unwrap(),
        // Observation: result from tool
        langchain_observation: Regex::new(r"(?i)^Observation:\s*(.+)").unwrap(),
        // Final Answer: The answer is...
        langchain_final_answer: Regex::new(r"(?i)^Final\s*Answer:\s*(.+)").unwrap(),

        // AutoGPT patterns
        // THOUGHTS: I need to analyze...
        autogpt_thoughts: Regex::new(r"(?i)^THOUGHTS?:\s*(.+)").unwrap(),
        // REASONING: Based on the information...
        autogpt_reasoning: Regex::new(r"(?i)^REASONING:\s*(.+)").unwrap(),
        // PLAN:
        autogpt_plan: Regex::new(r"(?i)^PLAN:\s*(.*)").unwrap(),
        // CRITICISM: I should have...
        autogpt_criticism: Regex::new(r"(?i)^CRITICISM:\s*(.+)").unwrap(),
        // NEXT ACTION: COMMAND = browse_website
        autogpt_next_action: Regex::new(r"(?i)^NEXT\s+ACTION:\s*(?:COMMAND\s*=\s*)?(\w+)").unwrap(),
        // EXECUTING COMMAND: browse_website
        autogpt_command: Regex::new(r"(?i)^(?:EXECUTING\s+)?COMMAND:\s*(\w+)(?:\s+ARGS:\s*(.+))?").unwrap(),
        // SYSTEM: Command executed successfully
        autogpt_system: Regex::new(r"(?i)^SYSTEM:\s*(.+)").unwrap(),
    }
});

/// Parser for detecting agent events in terminal output
pub struct AgentParser {
    /// Buffer for accumulating incomplete lines
    line_buffer: String,
    /// Recent events for deduplication
    recent_events: VecDeque<AgentEvent>,
    /// Maximum recent events to track
    max_recent: usize,
    /// Current active node (for context)
    active_node: Option<String>,
    /// Current active tool
    active_tool: Option<String>,
    /// Event counter for generating IDs
    event_counter: u64,
}

impl Default for AgentParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentParser {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            recent_events: VecDeque::with_capacity(20),
            max_recent: 20,
            active_node: None,
            active_tool: None,
            event_counter: 0,
        }
    }

    /// Process terminal output bytes and extract agent events
    pub fn process(&mut self, text: &str) -> Vec<AgentEvent> {
        let mut events = Vec::new();

        // Add to buffer and process complete lines
        self.line_buffer.push_str(text);

        // Process complete lines
        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line: String = self.line_buffer.drain(..=newline_pos).collect();
            let line = line.trim_end_matches('\n').trim_end_matches('\r');

            if let Some(event) = self.parse_line(line) {
                if !self.is_duplicate(&event) {
                    self.add_recent(event.clone());
                    events.push(event);
                }
            }
        }

        // Also check partial buffer for multi-line patterns
        if self.line_buffer.len() > 500 {
            // Buffer too large, process as-is to prevent memory issues
            let text = std::mem::take(&mut self.line_buffer);
            if let Some(event) = self.parse_line(&text) {
                if !self.is_duplicate(&event) {
                    self.add_recent(event.clone());
                    events.push(event);
                }
            }
        }

        events
    }

    /// Parse a single line for agent patterns
    fn parse_line(&mut self, line: &str) -> Option<AgentEvent> {
        let patterns = &*PATTERNS;

        // Check for flow start
        if let Some(caps) = patterns.flow_start.captures(line) {
            let name = caps.get(1)?.as_str().to_string();
            return Some(AgentEvent::FlowStart { name });
        }

        // Check for flow complete
        if let Some(caps) = patterns.flow_complete.captures(line) {
            let name = caps.get(1)?.as_str().to_string();
            let success = caps.get(2)
                .map(|m| !m.as_str().eq_ignore_ascii_case("failed"))
                .unwrap_or(true);
            return Some(AgentEvent::FlowComplete { name, success });
        }

        // Check for node start
        if let Some(caps) = patterns.node_start.captures(line) {
            let node_id = caps.get(1)?.as_str().to_string();
            self.active_node = Some(node_id.clone());
            return Some(AgentEvent::NodeStart {
                node_id: node_id.clone(),
                label: node_id,
                node_type: AgentNodeType::Custom,
            });
        }

        // Check for node complete
        if let Some(caps) = patterns.node_complete.captures(line) {
            let node_id = caps.get(1)?.as_str().to_string();
            let status = caps.get(2)
                .map(|m| match m.as_str().to_lowercase().as_str() {
                    "success" | "ok" | "done" => AgentStatus::Success,
                    "failed" | "error" => AgentStatus::Failed,
                    "skipped" | "skip" => AgentStatus::Skipped,
                    _ => AgentStatus::Success,
                })
                .unwrap_or(AgentStatus::Success);

            if self.active_node.as_ref() == Some(&node_id) {
                self.active_node = None;
            }

            return Some(AgentEvent::NodeComplete {
                node_id,
                status,
                duration_ms: None,
            });
        }

        // Check for step start
        if let Some(caps) = patterns.step_start.captures(line) {
            let step_id = caps.get(1)?.as_str().to_string();
            let label = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| step_id.clone());
            self.active_node = Some(step_id.clone());
            return Some(AgentEvent::NodeStart {
                node_id: format!("step_{}", step_id),
                label,
                node_type: AgentNodeType::Custom,
            });
        }

        // Check for step complete
        if let Some(caps) = patterns.step_complete.captures(line) {
            let step_id = caps.get(1)?.as_str().to_string();
            let status = caps.get(2)
                .map(|m| match m.as_str().to_lowercase().as_str() {
                    "success" | "ok" | "done" => AgentStatus::Success,
                    "failed" | "error" => AgentStatus::Failed,
                    _ => AgentStatus::Success,
                })
                .unwrap_or(AgentStatus::Success);

            let node_id = format!("step_{}", step_id);
            if self.active_node.as_ref() == Some(&node_id) {
                self.active_node = None;
            }

            return Some(AgentEvent::NodeComplete {
                node_id,
                status,
                duration_ms: None,
            });
        }

        // Check for tool call
        if let Some(caps) = patterns.tool_call.captures(line) {
            let tool_name = caps.get(1)?.as_str().to_string();
            let input_preview = caps.get(2).map(|m| m.as_str().to_string());
            self.event_counter += 1;
            let tool_id = format!("tool_{}_{}", tool_name, self.event_counter);
            self.active_tool = Some(tool_name.clone());
            return Some(AgentEvent::ToolUse {
                tool_name,
                tool_id: Some(tool_id),
                input_preview,
            });
        }

        // Check for Claude-style tool use
        if let Some(caps) = patterns.claude_tool_use.captures(line) {
            let tool_name = caps.get(1)?.as_str().to_string();
            // Avoid matching common words
            if !["the", "a", "an", "is", "are", "was", "were"].contains(&tool_name.to_lowercase().as_str()) {
                self.event_counter += 1;
                let tool_id = format!("tool_{}_{}", tool_name, self.event_counter);
                self.active_tool = Some(tool_name.clone());
                return Some(AgentEvent::ToolUse {
                    tool_name,
                    tool_id: Some(tool_id),
                    input_preview: None,
                });
            }
        }

        // Check for tool result
        if let Some(caps) = patterns.tool_complete.captures(line) {
            let tool_name = caps.get(1)?.as_str().to_string();
            let result = caps.get(2).map(|m| m.as_str().to_lowercase());
            let success = result.map(|r| r != "failed" && r != "error").unwrap_or(true);

            if self.active_tool.as_ref() == Some(&tool_name) {
                self.active_tool = None;
            }

            return Some(AgentEvent::ToolResult {
                tool_name,
                tool_id: None,
                success,
            });
        }

        // Check for Claude tool result
        if let Some(caps) = patterns.claude_tool_result.captures(line) {
            let tool_name = caps.get(1)?.as_str().to_string();
            if self.active_tool.as_ref() == Some(&tool_name) {
                self.active_tool = None;
            }
            return Some(AgentEvent::ToolResult {
                tool_name,
                tool_id: None,
                success: true,
            });
        }

        // Check for thinking
        if patterns.claude_thinking.is_match(line) {
            return Some(AgentEvent::Thinking { preview: None });
        }

        // Check for errors
        if let Some(caps) = patterns.error_line.captures(line) {
            let message = caps.get(1)?.as_str().to_string();
            return Some(AgentEvent::Error {
                message,
                node_id: self.active_node.clone(),
            });
        }

        // LangChain patterns
        // > Entering new AgentExecutor chain...
        if let Some(caps) = patterns.langchain_chain_start.captures(line) {
            let chain_type = caps.get(1)?.as_str().to_string();
            return Some(AgentEvent::FlowStart { name: chain_type });
        }

        // > Finished chain.
        if patterns.langchain_chain_end.is_match(line) {
            return Some(AgentEvent::FlowComplete {
                name: "chain".to_string(),
                success: true,
            });
        }

        // Thought: ...
        if let Some(caps) = patterns.langchain_thought.captures(line) {
            let preview = caps.get(1).map(|m| m.as_str().to_string());
            return Some(AgentEvent::Thinking { preview });
        }

        // Action: tool_name
        if let Some(caps) = patterns.langchain_action.captures(line) {
            let tool_name = caps.get(1)?.as_str().to_string();
            self.event_counter += 1;
            let tool_id = format!("langchain_tool_{}_{}", tool_name, self.event_counter);
            self.active_tool = Some(tool_name.clone());
            return Some(AgentEvent::ToolUse {
                tool_name,
                tool_id: Some(tool_id),
                input_preview: None,
            });
        }

        // Action Input: ... (update the active tool with input preview)
        if let Some(caps) = patterns.langchain_action_input.captures(line) {
            if let Some(tool_name) = &self.active_tool {
                let input = caps.get(1)?.as_str().to_string();
                self.event_counter += 1;
                let tool_id = format!("langchain_tool_{}_{}", tool_name, self.event_counter);
                return Some(AgentEvent::ToolUse {
                    tool_name: tool_name.clone(),
                    tool_id: Some(tool_id),
                    input_preview: Some(input),
                });
            }
        }

        // Observation: ... (tool result)
        if let Some(caps) = patterns.langchain_observation.captures(line) {
            if let Some(tool_name) = self.active_tool.take() {
                let _result = caps.get(1)?.as_str();
                return Some(AgentEvent::ToolResult {
                    tool_name,
                    tool_id: None,
                    success: true,
                });
            }
        }

        // Final Answer: ...
        if let Some(caps) = patterns.langchain_final_answer.captures(line) {
            let content = caps.get(1)?.as_str().to_string();
            return Some(AgentEvent::Output { content });
        }

        // AutoGPT patterns
        // THOUGHTS: ...
        if let Some(caps) = patterns.autogpt_thoughts.captures(line) {
            let preview = caps.get(1).map(|m| m.as_str().to_string());
            return Some(AgentEvent::Thinking { preview });
        }

        // REASONING: ... (also thinking)
        if let Some(caps) = patterns.autogpt_reasoning.captures(line) {
            let preview = caps.get(1).map(|m| m.as_str().to_string());
            return Some(AgentEvent::Thinking { preview });
        }

        // PLAN: ... (treat as thinking/output)
        if let Some(caps) = patterns.autogpt_plan.captures(line) {
            let preview = caps.get(1).map(|m| m.as_str().to_string());
            return Some(AgentEvent::Thinking { preview });
        }

        // CRITICISM: ... (self-reflection, treat as thinking)
        if let Some(caps) = patterns.autogpt_criticism.captures(line) {
            let preview = caps.get(1).map(|m| m.as_str().to_string());
            return Some(AgentEvent::Thinking { preview });
        }

        // NEXT ACTION / COMMAND: ...
        if let Some(caps) = patterns.autogpt_next_action.captures(line) {
            let command = caps.get(1)?.as_str().to_string();
            self.event_counter += 1;
            let tool_id = format!("autogpt_cmd_{}_{}", command, self.event_counter);
            self.active_tool = Some(command.clone());
            return Some(AgentEvent::ToolUse {
                tool_name: command,
                tool_id: Some(tool_id),
                input_preview: None,
            });
        }

        if let Some(caps) = patterns.autogpt_command.captures(line) {
            let command = caps.get(1)?.as_str().to_string();
            let args = caps.get(2).map(|m| m.as_str().to_string());
            self.event_counter += 1;
            let tool_id = format!("autogpt_cmd_{}_{}", command, self.event_counter);
            self.active_tool = Some(command.clone());
            return Some(AgentEvent::ToolUse {
                tool_name: command,
                tool_id: Some(tool_id),
                input_preview: args,
            });
        }

        // SYSTEM: ... (result/output)
        if let Some(caps) = patterns.autogpt_system.captures(line) {
            let message = caps.get(1)?.as_str().to_string();
            // Check if it indicates success/failure
            let is_success = !message.to_lowercase().contains("error")
                && !message.to_lowercase().contains("failed");
            if let Some(tool_name) = self.active_tool.take() {
                return Some(AgentEvent::ToolResult {
                    tool_name,
                    tool_id: None,
                    success: is_success,
                });
            } else {
                return Some(AgentEvent::Output { content: message });
            }
        }

        None
    }

    /// Check if event is a duplicate of a recent event
    fn is_duplicate(&self, event: &AgentEvent) -> bool {
        self.recent_events.iter().any(|e| e == event)
    }

    /// Add event to recent events buffer
    fn add_recent(&mut self, event: AgentEvent) {
        if self.recent_events.len() >= self.max_recent {
            self.recent_events.pop_front();
        }
        self.recent_events.push_back(event);
    }

    /// Clear parser state
    pub fn clear(&mut self) {
        self.line_buffer.clear();
        self.recent_events.clear();
        self.active_node = None;
        self.active_tool = None;
    }

    /// Get currently active node
    pub fn active_node(&self) -> Option<&str> {
        self.active_node.as_deref()
    }

    /// Get currently active tool
    pub fn active_tool(&self) -> Option<&str> {
        self.active_tool.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_new() {
        let parser = AgentParser::new();
        assert!(parser.line_buffer.is_empty());
        assert!(parser.active_node.is_none());
    }

    #[test]
    fn test_flow_start() {
        let mut parser = AgentParser::new();
        let events = parser.process("[FLOW] MyAgentFlow\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::FlowStart { name } => {
                assert_eq!(name, "MyAgentFlow");
            }
            _ => panic!("Expected FlowStart event"),
        }
    }

    #[test]
    fn test_flow_complete() {
        let mut parser = AgentParser::new();
        let events = parser.process("[FLOW_END] MyAgentFlow (success)\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::FlowComplete { name, success } => {
                assert_eq!(name, "MyAgentFlow");
                assert!(*success);
            }
            _ => panic!("Expected FlowComplete event"),
        }
    }

    #[test]
    fn test_node_start() {
        let mut parser = AgentParser::new();
        let events = parser.process("[NODE] process_input: starting\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::NodeStart { node_id, .. } => {
                assert_eq!(node_id, "process_input");
            }
            _ => panic!("Expected NodeStart event"),
        }

        assert_eq!(parser.active_node(), Some("process_input"));
    }

    #[test]
    fn test_node_complete() {
        let mut parser = AgentParser::new();
        let events = parser.process("[NODE] process_input: complete (success)\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::NodeComplete { node_id, status, .. } => {
                assert_eq!(node_id, "process_input");
                assert_eq!(*status, AgentStatus::Success);
            }
            _ => panic!("Expected NodeComplete event"),
        }
    }

    #[test]
    fn test_tool_call() {
        let mut parser = AgentParser::new();
        let events = parser.process("[TOOL] read_file (path/to/file.rs)\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolUse { tool_name, input_preview, .. } => {
                assert_eq!(tool_name, "read_file");
                assert_eq!(input_preview.as_deref(), Some("path/to/file.rs"));
            }
            _ => panic!("Expected ToolUse event"),
        }
    }

    #[test]
    fn test_claude_tool_use() {
        let mut parser = AgentParser::new();
        let events = parser.process("Using tool: `Bash`\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolUse { tool_name, .. } => {
                assert_eq!(tool_name, "Bash");
            }
            _ => panic!("Expected ToolUse event"),
        }
    }

    #[test]
    fn test_step_start() {
        let mut parser = AgentParser::new();
        let events = parser.process(">>> Step 1: Initialize database\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::NodeStart { node_id, label, .. } => {
                assert_eq!(node_id, "step_1");
                assert_eq!(label, "Initialize database");
            }
            _ => panic!("Expected NodeStart event"),
        }
    }

    #[test]
    fn test_error_detection() {
        let mut parser = AgentParser::new();
        let events = parser.process("Error: Connection refused\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Error { message, .. } => {
                assert_eq!(message, "Connection refused");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_thinking_detection() {
        let mut parser = AgentParser::new();
        let events = parser.process("Thinking...\n");

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], AgentEvent::Thinking { .. }));
    }

    #[test]
    fn test_multiline_buffer() {
        let mut parser = AgentParser::new();

        // First chunk without newline
        let events1 = parser.process("[NODE] test");
        assert!(events1.is_empty()); // No newline yet

        // Second chunk completes the line
        let events2 = parser.process(": starting\n");
        assert_eq!(events2.len(), 1);
        assert!(matches!(events2[0], AgentEvent::NodeStart { .. }));
    }

    #[test]
    fn test_deduplication() {
        let mut parser = AgentParser::new();

        // Same event twice
        let events1 = parser.process("[NODE] test: starting\n");
        let events2 = parser.process("[NODE] test: starting\n");

        assert_eq!(events1.len(), 1);
        assert_eq!(events2.len(), 0); // Deduplicated
    }

    #[test]
    fn test_clear() {
        let mut parser = AgentParser::new();
        parser.process("[NODE] test: starting\n");

        assert!(parser.active_node().is_some());

        parser.clear();

        assert!(parser.active_node().is_none());
        assert!(parser.line_buffer.is_empty());
    }

    #[test]
    fn test_multiple_events() {
        let mut parser = AgentParser::new();
        let events = parser.process(
            "[FLOW] TestFlow\n\
             [NODE] step1: starting\n\
             [TOOL] read_file\n\
             [NODE] step1: complete\n"
        );

        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], AgentEvent::FlowStart { .. }));
        assert!(matches!(events[1], AgentEvent::NodeStart { .. }));
        assert!(matches!(events[2], AgentEvent::ToolUse { .. }));
        assert!(matches!(events[3], AgentEvent::NodeComplete { .. }));
    }

    // LangChain tests
    #[test]
    fn test_langchain_chain_start() {
        let mut parser = AgentParser::new();
        let events = parser.process("> Entering new AgentExecutor chain...\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::FlowStart { name } => {
                assert_eq!(name, "AgentExecutor");
            }
            _ => panic!("Expected FlowStart event, got {:?}", events[0]),
        }
    }

    #[test]
    fn test_langchain_chain_end() {
        let mut parser = AgentParser::new();
        let events = parser.process("> Finished chain.\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::FlowComplete { name, success } => {
                assert_eq!(name, "chain");
                assert!(*success);
            }
            _ => panic!("Expected FlowComplete event"),
        }
    }

    #[test]
    fn test_langchain_thought() {
        let mut parser = AgentParser::new();
        let events = parser.process("Thought: I should search for relevant information\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Thinking { preview } => {
                assert_eq!(preview.as_deref(), Some("I should search for relevant information"));
            }
            _ => panic!("Expected Thinking event"),
        }
    }

    #[test]
    fn test_langchain_action() {
        let mut parser = AgentParser::new();
        let events = parser.process("Action: search\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolUse { tool_name, .. } => {
                assert_eq!(tool_name, "search");
            }
            _ => panic!("Expected ToolUse event"),
        }
    }

    #[test]
    fn test_langchain_action_input() {
        let mut parser = AgentParser::new();
        // First set up the active tool
        parser.process("Action: search\n");
        // Then process action input
        let events = parser.process("Action Input: python programming\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolUse { tool_name, input_preview, .. } => {
                assert_eq!(tool_name, "search");
                assert_eq!(input_preview.as_deref(), Some("python programming"));
            }
            _ => panic!("Expected ToolUse event with input"),
        }
    }

    #[test]
    fn test_langchain_observation() {
        let mut parser = AgentParser::new();
        // Set up active tool first
        parser.process("Action: search\n");
        let events = parser.process("Observation: Found 5 results\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolResult { tool_name, success, .. } => {
                assert_eq!(tool_name, "search");
                assert!(*success);
            }
            _ => panic!("Expected ToolResult event"),
        }
    }

    #[test]
    fn test_langchain_final_answer() {
        let mut parser = AgentParser::new();
        let events = parser.process("Final Answer: The answer is 42\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Output { content } => {
                assert_eq!(content, "The answer is 42");
            }
            _ => panic!("Expected Output event"),
        }
    }

    #[test]
    fn test_langchain_full_flow() {
        let mut parser = AgentParser::new();
        let events = parser.process(
            "> Entering new AgentExecutor chain...\n\
             Thought: I need to search for this\n\
             Action: search\n\
             Action Input: query\n\
             Observation: Found results\n\
             Final Answer: Here is the answer\n\
             > Finished chain.\n"
        );

        assert_eq!(events.len(), 7);
        assert!(matches!(events[0], AgentEvent::FlowStart { .. }));
        assert!(matches!(events[1], AgentEvent::Thinking { .. }));
        assert!(matches!(events[2], AgentEvent::ToolUse { .. }));
        assert!(matches!(events[3], AgentEvent::ToolUse { .. })); // Action Input
        assert!(matches!(events[4], AgentEvent::ToolResult { .. }));
        assert!(matches!(events[5], AgentEvent::Output { .. }));
        assert!(matches!(events[6], AgentEvent::FlowComplete { .. }));
    }

    // AutoGPT tests
    #[test]
    fn test_autogpt_thoughts() {
        let mut parser = AgentParser::new();
        let events = parser.process("THOUGHTS: I need to analyze the data\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Thinking { preview } => {
                assert_eq!(preview.as_deref(), Some("I need to analyze the data"));
            }
            _ => panic!("Expected Thinking event"),
        }
    }

    #[test]
    fn test_autogpt_reasoning() {
        let mut parser = AgentParser::new();
        let events = parser.process("REASONING: Based on the data\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Thinking { preview } => {
                assert_eq!(preview.as_deref(), Some("Based on the data"));
            }
            _ => panic!("Expected Thinking event"),
        }
    }

    #[test]
    fn test_autogpt_next_action() {
        let mut parser = AgentParser::new();
        let events = parser.process("NEXT ACTION: COMMAND = browse_website\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolUse { tool_name, .. } => {
                assert_eq!(tool_name, "browse_website");
            }
            _ => panic!("Expected ToolUse event"),
        }
    }

    #[test]
    fn test_autogpt_command_with_args() {
        let mut parser = AgentParser::new();
        let events = parser.process("COMMAND: write_file ARGS: path/to/file.txt\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolUse { tool_name, input_preview, .. } => {
                assert_eq!(tool_name, "write_file");
                assert_eq!(input_preview.as_deref(), Some("path/to/file.txt"));
            }
            _ => panic!("Expected ToolUse event with args"),
        }
    }

    #[test]
    fn test_autogpt_system_success() {
        let mut parser = AgentParser::new();
        // Set up active tool
        parser.process("COMMAND: browse_website\n");
        let events = parser.process("SYSTEM: Command executed successfully\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolResult { tool_name, success, .. } => {
                assert_eq!(tool_name, "browse_website");
                assert!(*success);
            }
            _ => panic!("Expected ToolResult event"),
        }
    }

    #[test]
    fn test_autogpt_system_failure() {
        let mut parser = AgentParser::new();
        // Set up active tool
        parser.process("COMMAND: write_file\n");
        let events = parser.process("SYSTEM: Error - permission denied\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::ToolResult { tool_name, success, .. } => {
                assert_eq!(tool_name, "write_file");
                assert!(!*success);
            }
            _ => panic!("Expected ToolResult event with failure"),
        }
    }

    #[test]
    fn test_autogpt_full_flow() {
        let mut parser = AgentParser::new();
        let events = parser.process(
            "THOUGHTS: I need to find information\n\
             REASONING: Searching the web is the best approach\n\
             NEXT ACTION: COMMAND = browse_website\n\
             SYSTEM: Command executed successfully\n"
        );

        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], AgentEvent::Thinking { .. }));
        assert!(matches!(events[1], AgentEvent::Thinking { .. }));
        assert!(matches!(events[2], AgentEvent::ToolUse { .. }));
        assert!(matches!(events[3], AgentEvent::ToolResult { .. }));
    }
}
