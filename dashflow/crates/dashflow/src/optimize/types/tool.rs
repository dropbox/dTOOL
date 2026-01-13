// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tool calling types for function calling in agentic workflows

use serde::{Deserialize, Serialize};

/// A single tool/function call
///
/// Represents a request from the LLM to execute a specific function
/// with given arguments.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::types::ToolCall;
///
/// let call = ToolCall::new("get_weather", serde_json::json!({
///     "location": "San Francisco",
///     "unit": "celsius"
/// }));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,

    /// Tool/function name
    pub name: String,

    /// Arguments as JSON
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call with auto-generated ID
    ///
    /// # Arguments
    /// * `name` - Tool/function name
    /// * `arguments` - Arguments as JSON value
    pub fn new(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: generate_tool_call_id(),
            name: name.into(),
            arguments,
        }
    }

    /// Create a tool call with specific ID
    pub fn with_id(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    /// Get argument value by key
    pub fn get_arg(&self, key: &str) -> Option<&serde_json::Value> {
        self.arguments.get(key)
    }

    /// Get argument as string
    pub fn get_arg_str(&self, key: &str) -> Option<&str> {
        self.arguments.get(key).and_then(|v| v.as_str())
    }

    /// Get argument as i64
    pub fn get_arg_i64(&self, key: &str) -> Option<i64> {
        self.arguments.get(key).and_then(|v| v.as_i64())
    }

    /// Get argument as f64
    pub fn get_arg_f64(&self, key: &str) -> Option<f64> {
        self.arguments.get(key).and_then(|v| v.as_f64())
    }

    /// Get argument as bool
    pub fn get_arg_bool(&self, key: &str) -> Option<bool> {
        self.arguments.get(key).and_then(|v| v.as_bool())
    }

    /// Check if arguments contain a key
    pub fn has_arg(&self, key: &str) -> bool {
        self.arguments.get(key).is_some()
    }
}

impl std::fmt::Display for ToolCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.name, self.arguments)
    }
}

/// Collection of tool calls
///
/// Represents multiple tool calls that may be made in parallel
/// or sequentially by the LLM.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCalls(Vec<ToolCall>);

impl ToolCalls {
    /// Create empty tool calls collection
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from vector
    pub fn from_vec(calls: Vec<ToolCall>) -> Self {
        Self(calls)
    }

    /// Create from a single call
    pub fn single(call: ToolCall) -> Self {
        Self(vec![call])
    }

    /// Add a tool call
    pub fn add(&mut self, call: ToolCall) {
        self.0.push(call);
    }

    /// Get number of calls
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get call by index
    pub fn get(&self, index: usize) -> Option<&ToolCall> {
        self.0.get(index)
    }

    /// Get call by ID
    pub fn get_by_id(&self, id: &str) -> Option<&ToolCall> {
        self.0.iter().find(|c| c.id == id)
    }

    /// Iterate over calls
    pub fn iter(&self) -> impl Iterator<Item = &ToolCall> {
        self.0.iter()
    }

    /// Filter calls by name
    pub fn filter_by_name(&self, name: &str) -> Self {
        Self(self.0.iter().filter(|c| c.name == name).cloned().collect())
    }

    /// Get all unique tool names
    pub fn tool_names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.0.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        names.dedup();
        names
    }

    /// Convert to OpenAI format
    pub fn to_openai_format(&self) -> Vec<serde_json::Value> {
        self.0
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "type": "function",
                    "function": {
                        "name": c.name,
                        "arguments": c.arguments.to_string()
                    }
                })
            })
            .collect()
    }

    /// Convert to Anthropic format
    pub fn to_anthropic_format(&self) -> Vec<serde_json::Value> {
        self.0
            .iter()
            .map(|c| {
                serde_json::json!({
                    "type": "tool_use",
                    "id": c.id,
                    "name": c.name,
                    "input": c.arguments
                })
            })
            .collect()
    }
}

impl IntoIterator for ToolCalls {
    type Item = ToolCall;
    type IntoIter = std::vec::IntoIter<ToolCall>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a ToolCalls {
    type Item = &'a ToolCall;
    type IntoIter = std::slice::Iter<'a, ToolCall>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<ToolCall> for ToolCalls {
    fn from_iter<I: IntoIterator<Item = ToolCall>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Result from executing a tool
///
/// Contains the output from running a tool/function call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool call ID this result is for
    pub tool_call_id: String,

    /// Result content (usually text or JSON)
    pub content: String,

    /// Whether the tool execution failed
    #[serde(default)]
    pub is_error: bool,

    /// Optional structured output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: false,
            output: None,
        }
    }

    /// Create an error tool result
    pub fn error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: error.into(),
            is_error: true,
            output: None,
        }
    }

    /// Create result with structured output
    #[must_use]
    pub fn with_output(tool_call_id: impl Into<String>, output: serde_json::Value) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: output.to_string(),
            is_error: false,
            output: Some(output),
        }
    }

    /// Add structured output
    #[must_use]
    pub fn set_output(mut self, output: serde_json::Value) -> Self {
        self.output = Some(output);
        self
    }

    /// Convert to OpenAI format message
    pub fn to_openai_format(&self) -> serde_json::Value {
        serde_json::json!({
            "role": "tool",
            "tool_call_id": self.tool_call_id,
            "content": self.content
        })
    }

    /// Convert to Anthropic format content block
    pub fn to_anthropic_format(&self) -> serde_json::Value {
        let mut block = serde_json::json!({
            "type": "tool_result",
            "tool_use_id": self.tool_call_id,
            "content": self.content
        });

        if self.is_error {
            block["is_error"] = serde_json::Value::Bool(true);
        }

        block
    }
}

/// Collection of tool results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolResults(Vec<ToolResult>);

impl ToolResults {
    /// Create empty results collection
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from vector
    pub fn from_vec(results: Vec<ToolResult>) -> Self {
        Self(results)
    }

    /// Add a result
    pub fn add(&mut self, result: ToolResult) {
        self.0.push(result);
    }

    /// Get number of results
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get result by index
    pub fn get(&self, index: usize) -> Option<&ToolResult> {
        self.0.get(index)
    }

    /// Get result by tool call ID
    pub fn get_by_id(&self, id: &str) -> Option<&ToolResult> {
        self.0.iter().find(|r| r.tool_call_id == id)
    }

    /// Iterate over results
    pub fn iter(&self) -> impl Iterator<Item = &ToolResult> {
        self.0.iter()
    }

    /// Check if any results are errors
    pub fn has_errors(&self) -> bool {
        self.0.iter().any(|r| r.is_error)
    }

    /// Get only error results
    pub fn errors(&self) -> Vec<&ToolResult> {
        self.0.iter().filter(|r| r.is_error).collect()
    }

    /// Get only successful results
    pub fn successes(&self) -> Vec<&ToolResult> {
        self.0.iter().filter(|r| !r.is_error).collect()
    }
}

impl IntoIterator for ToolResults {
    type Item = ToolResult;
    type IntoIter = std::vec::IntoIter<ToolResult>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a ToolResults {
    type Item = &'a ToolResult;
    type IntoIter = std::slice::Iter<'a, ToolResult>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<ToolResult> for ToolResults {
    fn from_iter<I: IntoIterator<Item = ToolResult>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Generate a unique tool call ID
fn generate_tool_call_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("call_{:016x}", count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_new() {
        let call = ToolCall::new("get_weather", serde_json::json!({"city": "SF"}));
        assert_eq!(call.name, "get_weather");
        assert!(call.id.starts_with("call_"));
    }

    #[test]
    fn test_tool_call_with_id() {
        let call = ToolCall::with_id("my-id", "test_func", serde_json::json!({}));
        assert_eq!(call.id, "my-id");
        assert_eq!(call.name, "test_func");
    }

    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is test data, not PI
    fn test_tool_call_get_args() {
        let call = ToolCall::new(
            "test",
            serde_json::json!({
                "str": "hello",
                "num": 42,
                "float": 3.14,
                "bool": true
            }),
        );

        assert_eq!(call.get_arg_str("str"), Some("hello"));
        assert_eq!(call.get_arg_i64("num"), Some(42));
        assert_eq!(call.get_arg_f64("float"), Some(3.14));
        assert_eq!(call.get_arg_bool("bool"), Some(true));
        assert!(call.has_arg("str"));
        assert!(!call.has_arg("missing"));
    }

    #[test]
    fn test_tool_calls_collection() {
        let mut calls = ToolCalls::new();
        calls.add(ToolCall::new("func1", serde_json::json!({})));
        calls.add(ToolCall::new("func2", serde_json::json!({})));

        assert_eq!(calls.len(), 2);
        assert!(!calls.is_empty());
    }

    #[test]
    fn test_tool_calls_filter() {
        let calls = ToolCalls::from_vec(vec![
            ToolCall::new("get_weather", serde_json::json!({"city": "SF"})),
            ToolCall::new("get_weather", serde_json::json!({"city": "LA"})),
            ToolCall::new("get_time", serde_json::json!({})),
        ]);

        let weather = calls.filter_by_name("get_weather");
        assert_eq!(weather.len(), 2);
    }

    #[test]
    fn test_tool_calls_openai_format() {
        let call = ToolCall::with_id("id1", "test", serde_json::json!({"a": 1}));
        let calls = ToolCalls::single(call);

        let format = calls.to_openai_format();
        assert_eq!(format.len(), 1);
        assert_eq!(format[0]["id"], "id1");
        assert_eq!(format[0]["type"], "function");
    }

    #[test]
    fn test_tool_calls_anthropic_format() {
        let call = ToolCall::with_id("id1", "test", serde_json::json!({"a": 1}));
        let calls = ToolCalls::single(call);

        let format = calls.to_anthropic_format();
        assert_eq!(format.len(), 1);
        assert_eq!(format[0]["type"], "tool_use");
        assert_eq!(format[0]["name"], "test");
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("call-1", "The weather is sunny");
        assert_eq!(result.tool_call_id, "call-1");
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("call-1", "API unavailable");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_result_with_output() {
        let output = serde_json::json!({"temp": 72, "unit": "F"});
        let result = ToolResult::with_output("call-1", output.clone());
        assert_eq!(result.output, Some(output));
    }

    #[test]
    fn test_tool_results_collection() {
        let mut results = ToolResults::new();
        results.add(ToolResult::success("1", "ok"));
        results.add(ToolResult::error("2", "fail"));

        assert_eq!(results.len(), 2);
        assert!(results.has_errors());
        assert_eq!(results.errors().len(), 1);
        assert_eq!(results.successes().len(), 1);
    }

    #[test]
    fn test_tool_result_openai_format() {
        let result = ToolResult::success("call-1", "Done");
        let format = result.to_openai_format();

        assert_eq!(format["role"], "tool");
        assert_eq!(format["tool_call_id"], "call-1");
        assert_eq!(format["content"], "Done");
    }

    #[test]
    fn test_tool_result_anthropic_format() {
        let result = ToolResult::error("call-1", "Failed");
        let format = result.to_anthropic_format();

        assert_eq!(format["type"], "tool_result");
        assert_eq!(format["is_error"], true);
    }

    #[test]
    fn test_serialization() {
        let call = ToolCall::with_id("id", "func", serde_json::json!({"x": 1}));
        let json = serde_json::to_string(&call).unwrap();

        let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "func");
        assert_eq!(deserialized.id, "id");
    }
}
