//! Mock Tool for testing tool-using agents

use async_trait::async_trait;
use dashflow::core::{
    error::{Error, Result},
    tools::{Tool, ToolInput},
};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Handler function type for mock tool execution
pub type MockToolHandler = Arc<dyn Fn(&str) -> Result<String> + Send + Sync>;

/// A configurable mock tool for testing
///
/// # Example
///
/// ```rust
/// use dashflow_testing::MockTool;
///
/// let tool = MockTool::new("calculator")
///     .with_description("Performs calculations")
///     .with_handler(|input| Ok(format!("Result: {}", input)));
/// ```
#[derive(Clone)]
pub struct MockTool {
    /// Tool name
    name: String,
    /// Tool description
    description: String,
    /// JSON schema for parameters
    schema: Value,
    /// Handler function
    handler: Option<MockToolHandler>,
    /// Fixed response (used when no handler is set)
    fixed_response: String,
    /// Call history (input -> output)
    call_history: Arc<Mutex<Vec<(String, String)>>>,
    /// Number of times call was called
    call_count: Arc<Mutex<usize>>,
    /// Whether to fail on next invocation
    should_fail: Arc<Mutex<bool>>,
    /// Error message when failing
    error_message: String,
}

impl std::fmt::Debug for MockTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("call_count", &self.call_count())
            .finish()
    }
}

impl MockTool {
    /// Create a new MockTool with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: "A mock tool for testing".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input to the tool"
                    }
                },
                "required": ["input"]
            }),
            handler: None,
            fixed_response: "Mock tool response".to_string(),
            call_history: Arc::new(Mutex::new(Vec::new())),
            call_count: Arc::new(Mutex::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
            error_message: "Mock tool error".to_string(),
        }
    }

    /// Set the tool description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the parameter schema
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.schema = schema;
        self
    }

    /// Set a handler function for dynamic responses
    pub fn with_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> Result<String> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        self
    }

    /// Set a fixed response (used when no handler is set)
    pub fn with_response(mut self, response: impl Into<String>) -> Self {
        self.fixed_response = response.into();
        self
    }

    /// Configure the tool to fail on the next invocation
    pub fn fail_next(&self) {
        *self.should_fail.lock().unwrap() = true;
    }

    /// Set the error message when failing
    pub fn with_error_message(mut self, message: impl Into<String>) -> Self {
        self.error_message = message.into();
        self
    }

    /// Get the number of times call was called
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }

    /// Get the call history as (input, output) pairs
    pub fn call_history(&self) -> Vec<(String, String)> {
        self.call_history.lock().unwrap().clone()
    }

    /// Get just the inputs from call history
    pub fn inputs(&self) -> Vec<String> {
        self.call_history
            .lock()
            .unwrap()
            .iter()
            .map(|(input, _)| input.clone())
            .collect()
    }

    /// Reset the call count and history
    pub fn reset(&self) {
        *self.call_count.lock().unwrap() = 0;
        self.call_history.lock().unwrap().clear();
        *self.should_fail.lock().unwrap() = false;
    }

    /// Check if a specific input was received
    pub fn was_called_with(&self, input: &str) -> bool {
        self.call_history
            .lock()
            .unwrap()
            .iter()
            .any(|(i, _)| i.contains(input))
    }
}

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn args_schema(&self) -> Value {
        self.schema.clone()
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        // Increment call count
        *self.call_count.lock().unwrap() += 1;

        // Check if we should fail
        {
            let mut should_fail = self.should_fail.lock().unwrap();
            if *should_fail {
                *should_fail = false;
                return Err(Error::tool_error(self.error_message.clone()));
            }
        }

        // Get input string
        let input_str = match &input {
            ToolInput::String(s) => s.clone(),
            ToolInput::Structured(v) => v.to_string(),
        };

        // Execute handler or return fixed response
        let output = if let Some(handler) = &self.handler {
            handler(&input_str)?
        } else {
            self.fixed_response.clone()
        };

        // Record in history
        self.call_history
            .lock()
            .unwrap()
            .push((input_str, output.clone()));

        Ok(output)
    }
}

/// A builder for creating MockTools with specific behaviors
pub struct MockToolBuilder {
    tool: MockTool,
}

impl MockToolBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            tool: MockTool::new(name),
        }
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.tool = self.tool.with_description(desc);
        self
    }

    /// Set the parameters schema
    pub fn schema(mut self, schema: Value) -> Self {
        self.tool = self.tool.with_schema(schema);
        self
    }

    /// Set a fixed response
    pub fn response(mut self, resp: impl Into<String>) -> Self {
        self.tool = self.tool.with_response(resp);
        self
    }

    /// Set a handler function
    pub fn handler<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<String> + Send + Sync + 'static,
    {
        self.tool = self.tool.with_handler(f);
        self
    }

    /// Build the MockTool
    pub fn build(self) -> MockTool {
        self.tool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_tool_fixed_response() {
        let tool = MockTool::new("test").with_response("Fixed output");

        let result = tool
            ._call(ToolInput::String("anything".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Fixed output");
    }

    #[tokio::test]
    async fn test_mock_tool_handler() {
        let tool =
            MockTool::new("calculator").with_handler(|input| Ok(format!("Calculated: {}", input)));

        let result = tool
            ._call(ToolInput::String("2+2".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Calculated: 2+2");
    }

    #[tokio::test]
    async fn test_mock_tool_call_count() {
        let tool = MockTool::new("test");

        assert_eq!(tool.call_count(), 0);
        tool._call(ToolInput::String("first".to_string()))
            .await
            .unwrap();
        assert_eq!(tool.call_count(), 1);
        tool._call(ToolInput::String("second".to_string()))
            .await
            .unwrap();
        assert_eq!(tool.call_count(), 2);
    }

    #[tokio::test]
    async fn test_mock_tool_call_history() {
        let tool = MockTool::new("test").with_response("output");

        tool._call(ToolInput::String("input1".to_string()))
            .await
            .unwrap();
        tool._call(ToolInput::String("input2".to_string()))
            .await
            .unwrap();

        let history = tool.call_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, "input1");
        assert_eq!(history[1].0, "input2");
    }

    #[tokio::test]
    async fn test_mock_tool_fail_next() {
        let tool = MockTool::new("test").with_error_message("Test error");

        tool.fail_next();
        let result = tool._call(ToolInput::String("test".to_string())).await;
        assert!(result.is_err());

        // Should succeed next time
        let result = tool._call(ToolInput::String("test".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_tool_was_called_with() {
        let tool = MockTool::new("search");

        tool._call(ToolInput::String("search for rust".to_string()))
            .await
            .unwrap();

        assert!(tool.was_called_with("rust"));
        assert!(!tool.was_called_with("python"));
    }

    #[test]
    fn test_mock_tool_builder() {
        let tool = MockToolBuilder::new("builder_test")
            .description("Built tool")
            .response("Built response")
            .build();

        assert_eq!(tool.name(), "builder_test");
        assert_eq!(tool.description(), "Built tool");
    }

    // ==========================================================================
    // MockTool Construction Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_new_default_values() {
        let tool = MockTool::new("test_tool");
        assert_eq!(tool.name(), "test_tool");
        assert_eq!(tool.description(), "A mock tool for testing");
        assert_eq!(tool.call_count(), 0);
        assert!(tool.call_history().is_empty());
    }

    #[test]
    fn test_mock_tool_empty_name() {
        let tool = MockTool::new("");
        assert_eq!(tool.name(), "");
    }

    #[test]
    fn test_mock_tool_unicode_name() {
        let tool = MockTool::new("工具_测试");
        assert_eq!(tool.name(), "工具_测试");
    }

    #[test]
    fn test_mock_tool_special_chars_name() {
        let tool = MockTool::new("tool-with_special.chars:v1");
        assert_eq!(tool.name(), "tool-with_special.chars:v1");
    }

    #[test]
    fn test_mock_tool_name_with_spaces() {
        let tool = MockTool::new("my tool name");
        assert_eq!(tool.name(), "my tool name");
    }

    #[test]
    fn test_mock_tool_from_string() {
        let name = String::from("string_tool");
        let tool = MockTool::new(name);
        assert_eq!(tool.name(), "string_tool");
    }

    // ==========================================================================
    // Description Builder Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_with_description_empty() {
        let tool = MockTool::new("test").with_description("");
        assert_eq!(tool.description(), "");
    }

    #[test]
    fn test_mock_tool_with_description_unicode() {
        let tool = MockTool::new("test").with_description("这是一个测试工具");
        assert_eq!(tool.description(), "这是一个测试工具");
    }

    #[test]
    fn test_mock_tool_with_description_multiline() {
        let desc = "Line 1\nLine 2\nLine 3";
        let tool = MockTool::new("test").with_description(desc);
        assert_eq!(tool.description(), desc);
    }

    #[test]
    fn test_mock_tool_with_description_from_string() {
        let desc = String::from("A description");
        let tool = MockTool::new("test").with_description(desc);
        assert_eq!(tool.description(), "A description");
    }

    // ==========================================================================
    // Schema Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_default_schema() {
        let tool = MockTool::new("test");
        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["input"].is_object());
        assert_eq!(schema["required"][0], "input");
    }

    #[test]
    fn test_mock_tool_with_custom_schema() {
        let custom_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer"}
            },
            "required": ["query"]
        });
        let tool = MockTool::new("test").with_schema(custom_schema.clone());
        assert_eq!(tool.args_schema(), custom_schema);
    }

    #[test]
    fn test_mock_tool_with_empty_schema() {
        let tool = MockTool::new("test").with_schema(serde_json::json!({}));
        assert_eq!(tool.args_schema(), serde_json::json!({}));
    }

    #[test]
    fn test_mock_tool_with_nested_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "properties": {
                        "nested": {"type": "string"}
                    }
                }
            }
        });
        let tool = MockTool::new("test").with_schema(schema.clone());
        assert_eq!(tool.args_schema()["properties"]["config"]["type"], "object");
    }

    // ==========================================================================
    // Response Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_with_response_empty() {
        let _tool = MockTool::new("test").with_response("");
        // fixed_response is private, verify via call in async test
    }

    #[tokio::test]
    async fn test_mock_tool_with_response_empty_call() {
        let tool = MockTool::new("test").with_response("");
        let result = tool
            ._call(ToolInput::String("input".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_mock_tool_with_response_unicode() {
        let tool = MockTool::new("test").with_response("响应: 成功");
        let result = tool
            ._call(ToolInput::String("input".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "响应: 成功");
    }

    #[tokio::test]
    async fn test_mock_tool_with_response_json() {
        let json_response = r#"{"status": "ok", "count": 42}"#;
        let tool = MockTool::new("test").with_response(json_response);
        let result = tool
            ._call(ToolInput::String("input".to_string()))
            .await
            .unwrap();
        assert_eq!(result, json_response);
    }

    #[tokio::test]
    async fn test_mock_tool_default_response() {
        let tool = MockTool::new("test");
        let result = tool
            ._call(ToolInput::String("input".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Mock tool response");
    }

    // ==========================================================================
    // Handler Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_handler_returns_error() {
        let tool = MockTool::new("test")
            .with_handler(|_| Err(Error::tool_error("Handler error")));

        let result = tool._call(ToolInput::String("input".to_string())).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Handler error"));
    }

    #[tokio::test]
    async fn test_mock_tool_handler_processes_input() {
        let tool = MockTool::new("reverse")
            .with_handler(|input| Ok(input.chars().rev().collect::<String>()));

        let result = tool
            ._call(ToolInput::String("hello".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "olleh");
    }

    #[tokio::test]
    async fn test_mock_tool_handler_uppercase() {
        let tool =
            MockTool::new("upper").with_handler(|input| Ok(input.to_uppercase()));

        let result = tool
            ._call(ToolInput::String("hello world".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "HELLO WORLD");
    }

    #[tokio::test]
    async fn test_mock_tool_handler_with_state() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let tool = MockTool::new("counting").with_handler(move |_| {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(format!("Call #{}", count))
        });

        let r1 = tool
            ._call(ToolInput::String("a".to_string()))
            .await
            .unwrap();
        let r2 = tool
            ._call(ToolInput::String("b".to_string()))
            .await
            .unwrap();

        assert_eq!(r1, "Call #0");
        assert_eq!(r2, "Call #1");
    }

    #[tokio::test]
    async fn test_mock_tool_handler_overrides_response() {
        let tool = MockTool::new("test")
            .with_response("Fixed response")
            .with_handler(|_| Ok("Handler response".to_string()));

        let result = tool
            ._call(ToolInput::String("input".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Handler response");
    }

    // ==========================================================================
    // ToolInput Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_structured_input() {
        let tool = MockTool::new("test").with_response("ok");

        let structured = serde_json::json!({"key": "value", "num": 42});
        let result = tool._call(ToolInput::Structured(structured)).await.unwrap();
        assert_eq!(result, "ok");

        let history = tool.call_history();
        assert_eq!(history.len(), 1);
        // Structured input is converted to JSON string
        assert!(history[0].0.contains("key"));
        assert!(history[0].0.contains("value"));
    }

    #[tokio::test]
    async fn test_mock_tool_structured_input_complex() {
        let tool = MockTool::new("test")
            .with_handler(|input| Ok(format!("Got: {}", input)));

        let structured = serde_json::json!({
            "nested": {
                "array": [1, 2, 3],
                "object": {"a": "b"}
            }
        });
        let result = tool
            ._call(ToolInput::Structured(structured))
            .await
            .unwrap();
        assert!(result.contains("nested"));
        assert!(result.contains("array"));
    }

    #[tokio::test]
    async fn test_mock_tool_empty_string_input() {
        let tool = MockTool::new("test").with_response("ok");

        let result = tool
            ._call(ToolInput::String(String::new()))
            .await
            .unwrap();
        assert_eq!(result, "ok");

        let history = tool.call_history();
        assert_eq!(history[0].0, "");
    }

    // ==========================================================================
    // Error Handling Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_error_message_default() {
        let tool = MockTool::new("test");
        tool.fail_next();

        let result = tool._call(ToolInput::String("test".to_string())).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Mock tool error"));
    }

    #[tokio::test]
    async fn test_mock_tool_error_message_custom() {
        let tool = MockTool::new("test")
            .with_error_message("Custom failure message");
        tool.fail_next();

        let result = tool._call(ToolInput::String("test".to_string())).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Custom failure message"));
    }

    #[tokio::test]
    async fn test_mock_tool_fail_next_only_once() {
        let tool = MockTool::new("test");

        tool.fail_next();
        let r1 = tool._call(ToolInput::String("1".to_string())).await;
        let r2 = tool._call(ToolInput::String("2".to_string())).await;
        let r3 = tool._call(ToolInput::String("3".to_string())).await;

        assert!(r1.is_err());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }

    #[tokio::test]
    async fn test_mock_tool_fail_next_multiple_calls() {
        let tool = MockTool::new("test");

        tool.fail_next();
        let _ = tool._call(ToolInput::String("1".to_string())).await;

        tool.fail_next();
        let r2 = tool._call(ToolInput::String("2".to_string())).await;
        assert!(r2.is_err());
    }

    #[tokio::test]
    async fn test_mock_tool_fail_increments_count() {
        let tool = MockTool::new("test");

        tool.fail_next();
        let _ = tool._call(ToolInput::String("test".to_string())).await;

        // Call count should still be incremented even on failure
        assert_eq!(tool.call_count(), 1);
    }

    // ==========================================================================
    // Reset Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_reset_clears_count() {
        let tool = MockTool::new("test");

        tool._call(ToolInput::String("1".to_string())).await.unwrap();
        tool._call(ToolInput::String("2".to_string())).await.unwrap();
        assert_eq!(tool.call_count(), 2);

        tool.reset();
        assert_eq!(tool.call_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_tool_reset_clears_history() {
        let tool = MockTool::new("test");

        tool._call(ToolInput::String("1".to_string())).await.unwrap();
        tool._call(ToolInput::String("2".to_string())).await.unwrap();
        assert_eq!(tool.call_history().len(), 2);

        tool.reset();
        assert!(tool.call_history().is_empty());
    }

    #[tokio::test]
    async fn test_mock_tool_reset_clears_fail_flag() {
        let tool = MockTool::new("test");

        tool.fail_next();
        tool.reset();

        // Should not fail after reset
        let result = tool._call(ToolInput::String("test".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_tool_reset_allows_new_calls() {
        let tool = MockTool::new("test").with_response("ok");

        tool._call(ToolInput::String("old".to_string()))
            .await
            .unwrap();
        tool.reset();
        tool._call(ToolInput::String("new".to_string()))
            .await
            .unwrap();

        assert_eq!(tool.call_count(), 1);
        let history = tool.call_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0, "new");
    }

    // ==========================================================================
    // Call History Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_inputs_method() {
        let tool = MockTool::new("test").with_response("out");

        tool._call(ToolInput::String("in1".to_string()))
            .await
            .unwrap();
        tool._call(ToolInput::String("in2".to_string()))
            .await
            .unwrap();
        tool._call(ToolInput::String("in3".to_string()))
            .await
            .unwrap();

        let inputs = tool.inputs();
        assert_eq!(inputs, vec!["in1", "in2", "in3"]);
    }

    #[tokio::test]
    async fn test_mock_tool_inputs_empty() {
        let tool = MockTool::new("test");
        assert!(tool.inputs().is_empty());
    }

    #[tokio::test]
    async fn test_mock_tool_history_preserves_order() {
        let tool = MockTool::new("test").with_handler(|i| Ok(format!("R{}", i)));

        for i in 0..5 {
            tool._call(ToolInput::String(format!("{}", i)))
                .await
                .unwrap();
        }

        let history = tool.call_history();
        assert_eq!(history.len(), 5);
        for (i, (input, output)) in history.iter().enumerate() {
            assert_eq!(input, &format!("{}", i));
            assert_eq!(output, &format!("R{}", i));
        }
    }

    #[tokio::test]
    async fn test_mock_tool_was_called_with_partial_match() {
        let tool = MockTool::new("test");

        tool._call(ToolInput::String(
            "The quick brown fox jumps over".to_string(),
        ))
        .await
        .unwrap();

        assert!(tool.was_called_with("quick"));
        assert!(tool.was_called_with("brown fox"));
        assert!(tool.was_called_with("The quick"));
        assert!(!tool.was_called_with("lazy dog"));
    }

    #[tokio::test]
    async fn test_mock_tool_was_called_with_case_sensitive() {
        let tool = MockTool::new("test");

        tool._call(ToolInput::String("Hello World".to_string()))
            .await
            .unwrap();

        assert!(tool.was_called_with("Hello"));
        assert!(!tool.was_called_with("hello"));
    }

    // ==========================================================================
    // Debug and Clone Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_debug_format() {
        let tool = MockTool::new("debug_test")
            .with_description("A tool for debugging");

        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("MockTool"));
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("call_count"));
    }

    #[tokio::test]
    async fn test_mock_tool_debug_shows_call_count() {
        let tool = MockTool::new("test");

        tool._call(ToolInput::String("a".to_string())).await.unwrap();
        tool._call(ToolInput::String("b".to_string())).await.unwrap();

        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("call_count: 2"));
    }

    #[test]
    fn test_mock_tool_clone() {
        let original = MockTool::new("original")
            .with_description("Original description")
            .with_response("Original response");

        let cloned = original.clone();

        assert_eq!(cloned.name(), "original");
        assert_eq!(cloned.description(), "Original description");
    }

    #[tokio::test]
    async fn test_mock_tool_clone_shares_state() {
        let original = MockTool::new("test");
        let cloned = original.clone();

        original
            ._call(ToolInput::String("via_original".to_string()))
            .await
            .unwrap();

        // Clone shares state via Arc
        assert_eq!(cloned.call_count(), 1);
        assert!(cloned.was_called_with("via_original"));
    }

    #[tokio::test]
    async fn test_mock_tool_clone_both_can_call() {
        let original = MockTool::new("test");
        let cloned = original.clone();

        original
            ._call(ToolInput::String("from_original".to_string()))
            .await
            .unwrap();
        cloned
            ._call(ToolInput::String("from_clone".to_string()))
            .await
            .unwrap();

        // Both contribute to same state
        assert_eq!(original.call_count(), 2);
        assert_eq!(cloned.call_count(), 2);
    }

    // ==========================================================================
    // MockToolBuilder Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_builder_default() {
        let tool = MockToolBuilder::new("builder").build();
        assert_eq!(tool.name(), "builder");
        assert_eq!(tool.description(), "A mock tool for testing");
    }

    #[test]
    fn test_mock_tool_builder_all_options() {
        let tool = MockToolBuilder::new("full")
            .description("Full description")
            .response("Full response")
            .schema(serde_json::json!({"custom": true}))
            .build();

        assert_eq!(tool.name(), "full");
        assert_eq!(tool.description(), "Full description");
        assert_eq!(tool.args_schema()["custom"], true);
    }

    #[tokio::test]
    async fn test_mock_tool_builder_with_handler() {
        let tool = MockToolBuilder::new("handler")
            .handler(|input| Ok(format!("Handled: {}", input)))
            .build();

        let result = tool
            ._call(ToolInput::String("test".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Handled: test");
    }

    #[tokio::test]
    async fn test_mock_tool_builder_response_method() {
        let tool = MockToolBuilder::new("resp")
            .response("Builder response")
            .build();

        let result = tool
            ._call(ToolInput::String("any".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Builder response");
    }

    #[test]
    fn test_mock_tool_builder_chaining() {
        // Test that all methods return Self for chaining
        let _tool = MockToolBuilder::new("chain")
            .description("d")
            .schema(serde_json::json!({}))
            .response("r")
            .handler(|_| Ok("h".to_string()))
            .build();
    }

    // ==========================================================================
    // Thread Safety / Concurrent Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_concurrent_calls() {
        let tool = Arc::new(MockTool::new("concurrent").with_response("ok"));

        let mut handles = Vec::new();
        for i in 0..10 {
            let tool_clone = Arc::clone(&tool);
            handles.push(tokio::spawn(async move {
                tool_clone
                    ._call(ToolInput::String(format!("call_{}", i)))
                    .await
                    .unwrap()
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert_eq!(result, "ok");
        }

        assert_eq!(tool.call_count(), 10);
    }

    #[tokio::test]
    async fn test_mock_tool_concurrent_read_count() {
        let tool = Arc::new(MockTool::new("test").with_response("ok"));

        // Make some calls
        for i in 0..5 {
            tool._call(ToolInput::String(format!("{}", i)))
                .await
                .unwrap();
        }

        // Concurrent reads of call_count
        let mut handles = Vec::new();
        for _ in 0..10 {
            let tool_clone = Arc::clone(&tool);
            handles.push(tokio::spawn(async move { tool_clone.call_count() }));
        }

        for handle in handles {
            let count = handle.await.unwrap();
            assert_eq!(count, 5);
        }
    }

    // ==========================================================================
    // Edge Cases
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_very_long_input() {
        let tool = MockTool::new("test").with_response("ok");
        let long_input = "x".repeat(100_000);

        let result = tool
            ._call(ToolInput::String(long_input.clone()))
            .await
            .unwrap();
        assert_eq!(result, "ok");
        assert!(tool.was_called_with(&long_input));
    }

    #[tokio::test]
    async fn test_mock_tool_very_long_response() {
        let long_response = "y".repeat(100_000);
        let tool = MockTool::new("test").with_response(long_response.clone());

        let result = tool
            ._call(ToolInput::String("input".to_string()))
            .await
            .unwrap();
        assert_eq!(result, long_response);
    }

    #[tokio::test]
    async fn test_mock_tool_binary_like_content() {
        let tool = MockTool::new("test").with_response("ok");
        // Use null and control characters (valid 7-bit ASCII escapes)
        let binary_like = "\x00\x01\x02\x03\x7f\x1b\x0a";

        let result = tool
            ._call(ToolInput::String(binary_like.to_string()))
            .await
            .unwrap();
        assert_eq!(result, "ok");
        assert!(tool.was_called_with(binary_like));
    }

    #[tokio::test]
    async fn test_mock_tool_newlines_in_io() {
        let tool = MockTool::new("test").with_response("line1\nline2\nline3");

        let result = tool
            ._call(ToolInput::String("in1\nin2".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "line1\nline2\nline3");

        let history = tool.call_history();
        assert_eq!(history[0].0, "in1\nin2");
    }

    #[tokio::test]
    async fn test_mock_tool_whitespace_preservation() {
        let tool = MockTool::new("test").with_response("  spaced  ");

        let result = tool
            ._call(ToolInput::String("  input  ".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "  spaced  ");

        let inputs = tool.inputs();
        assert_eq!(inputs[0], "  input  ");
    }

    // ==========================================================================
    // Tool Trait Implementation Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_implements_tool_trait() {
        fn assert_tool<T: Tool>(_t: &T) {}
        let tool = MockTool::new("test");
        assert_tool(&tool);
    }

    #[test]
    fn test_mock_tool_name_trait_method() {
        let tool = MockTool::new("trait_name");
        let name: &str = Tool::name(&tool);
        assert_eq!(name, "trait_name");
    }

    #[test]
    fn test_mock_tool_description_trait_method() {
        let tool = MockTool::new("test").with_description("Trait description");
        let desc: &str = Tool::description(&tool);
        assert_eq!(desc, "Trait description");
    }

    #[test]
    fn test_mock_tool_args_schema_trait_method() {
        let tool = MockTool::new("test");
        let schema: Value = Tool::args_schema(&tool);
        assert_eq!(schema["type"], "object");
    }
}
