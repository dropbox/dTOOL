//! Console output tracer implementation
//!
//! Provides tracers that print run information to stdout with colored output
//! for debugging and monitoring DashFlow execution flows.

use super::{BaseTracer, RunTree, RunType};
use crate::core::callbacks::CallbackHandler;
use crate::core::error::Result;
use async_trait::async_trait;
use colored::Colorize;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const MILLISECONDS_IN_SECOND: f64 = 1000.0;

/// Try to stringify a value to JSON
fn try_json_stringify(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{:?}", value))
}

/// Try to stringify a HashMap to JSON
fn try_json_stringify_map(value: &HashMap<String, serde_json::Value>, fallback: &str) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| fallback.to_string())
}

/// Get elapsed time as a formatted string
fn elapsed(run: &RunTree) -> String {
    if let Some(end_time) = run.end_time {
        let duration = end_time.signed_duration_since(run.start_time);
        let seconds = duration.num_milliseconds() as f64 / MILLISECONDS_IN_SECOND;
        if seconds < 1.0 {
            format!("{:.0}ms", seconds * MILLISECONDS_IN_SECOND)
        } else {
            format!("{:.2}s", seconds)
        }
    } else {
        "pending".to_string()
    }
}

/// Tracer that calls a function with a single string parameter
///
/// This tracer formats run information and calls a provided callback function
/// with formatted strings. Useful for custom logging or monitoring.
pub struct FunctionCallbackHandler {
    /// The name of the tracer
    pub name: String,
    /// Map of run IDs to run trees
    pub(crate) run_map: Arc<Mutex<HashMap<Uuid, RunTree>>>,
    /// The callback function to call with formatted output
    callback: Arc<dyn Fn(&str) + Send + Sync>,
}

impl FunctionCallbackHandler {
    /// Create a new FunctionCallbackHandler with a custom callback
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        Self {
            name: "function_callback_handler".to_string(),
            run_map: Arc::new(Mutex::new(HashMap::new())),
            callback: Arc::new(callback),
        }
    }

    /// Get the parents of a run
    fn get_parents(&self, run: &RunTree) -> Vec<RunTree> {
        let mut parents = Vec::new();
        let run_map = self.run_map.lock();
        let mut current_run = run.clone();

        while let Some(parent_id) = current_run.parent_run_id {
            if let Some(parent) = run_map.get(&parent_id) {
                parents.push(parent.clone());
                current_run = parent.clone();
            } else {
                break;
            }
        }

        parents
    }

    /// Get breadcrumbs for a run showing the call hierarchy
    fn get_breadcrumbs(&self, run: &RunTree) -> String {
        let mut parents = self.get_parents(run);
        parents.reverse();
        parents.push(run.clone());

        parents
            .iter()
            .map(|r| format!("{}:{}", format!("{:?}", r.run_type).to_lowercase(), r.name))
            .collect::<Vec<_>>()
            .join(" > ")
    }

    /// Get or create a run tree
    fn get_or_create_run(&self, run_id: Uuid, name: &str, run_type: RunType) -> RunTree {
        let mut run_map = self.run_map.lock();
        run_map
            .entry(run_id)
            .or_insert_with(|| RunTree::new(run_id, name, run_type))
            .clone()
    }

    /// Update a run tree
    fn update_run<F>(&self, run_id: Uuid, update_fn: F)
    where
        F: FnOnce(&mut RunTree),
    {
        let mut run_map = self.run_map.lock();
        if let Some(run) = run_map.get_mut(&run_id) {
            update_fn(run);
        }
    }
}

#[async_trait]
impl CallbackHandler for FunctionCallbackHandler {
    async fn on_chain_start(
        &self,
        _serialized: &HashMap<String, serde_json::Value>,
        inputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        _metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut run = self.get_or_create_run(run_id, "Chain", RunType::Chain);
        run.inputs = Some(serde_json::to_value(inputs).unwrap_or(serde_json::json!({})));
        run.tags = Some(tags.to_vec());
        if let Some(parent) = parent_run_id {
            run.parent_run_id = Some(parent);
        }

        let crumbs = self.get_breadcrumbs(&run);
        let message = format!(
            "{} {} {}",
            "[chain/start]".green(),
            format!("[{}] Entering Chain run with input:", crumbs).bold(),
            try_json_stringify_map(inputs, "[inputs]")
        );
        (self.callback)(&message);

        self.run_map.lock().insert(run_id, run);
        Ok(())
    }

    async fn on_chain_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.update_run(run_id, |run| {
            run.end_time = Some(chrono::Utc::now());
            run.outputs = Some(serde_json::to_value(outputs).unwrap_or(serde_json::json!({})));
        });

        // Clone run to avoid deadlock when get_breadcrumbs() calls get_parents() which locks run_map
        let run = self.run_map.lock().get(&run_id).cloned();
        if let Some(run) = run {
            let crumbs = self.get_breadcrumbs(&run);
            let message = format!(
                "{} {} {}",
                "[chain/end]".blue(),
                format!(
                    "[{}] [{}] Exiting Chain run with output:",
                    crumbs,
                    elapsed(&run)
                )
                .bold(),
                try_json_stringify_map(outputs, "[outputs]")
            );
            (self.callback)(&message);
        }
        Ok(())
    }

    async fn on_chain_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.update_run(run_id, |run| {
            run.end_time = Some(chrono::Utc::now());
            run.error = Some(error.to_string());
        });

        // Clone run to avoid deadlock when get_breadcrumbs() calls get_parents() which locks run_map
        let run = self.run_map.lock().get(&run_id).cloned();
        if let Some(run) = run {
            let crumbs = self.get_breadcrumbs(&run);
            let message = format!(
                "{} {} {}",
                "[chain/error]".red(),
                format!(
                    "[{}] [{}] Chain run errored with error:",
                    crumbs,
                    elapsed(&run)
                )
                .bold(),
                error
            );
            (self.callback)(&message);
        }
        Ok(())
    }

    async fn on_llm_start(
        &self,
        _serialized: &HashMap<String, serde_json::Value>,
        prompts: &[String],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        _metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut run = self.get_or_create_run(run_id, "LLM", RunType::Llm);
        let inputs = serde_json::json!({"prompts": prompts});
        run.inputs = Some(inputs.clone());
        run.tags = Some(tags.to_vec());
        if let Some(parent) = parent_run_id {
            run.parent_run_id = Some(parent);
        }

        let crumbs = self.get_breadcrumbs(&run);
        let message = format!(
            "{} {} {}",
            "[llm/start]".green(),
            format!("[{}] Entering LLM run with input:", crumbs).bold(),
            try_json_stringify(&inputs)
        );
        (self.callback)(&message);

        self.run_map.lock().insert(run_id, run);
        Ok(())
    }

    async fn on_llm_end(
        &self,
        response: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.update_run(run_id, |run| {
            run.end_time = Some(chrono::Utc::now());
            run.outputs = Some(serde_json::to_value(response).unwrap_or(serde_json::json!({})));
        });

        // Clone run to avoid deadlock when get_breadcrumbs() calls get_parents() which locks run_map
        let run = self.run_map.lock().get(&run_id).cloned();
        if let Some(run) = run {
            let crumbs = self.get_breadcrumbs(&run);
            let message = format!(
                "{} {} {}",
                "[llm/end]".blue(),
                format!(
                    "[{}] [{}] Exiting LLM run with output:",
                    crumbs,
                    elapsed(&run)
                )
                .bold(),
                try_json_stringify_map(response, "[response]")
            );
            (self.callback)(&message);
        }
        Ok(())
    }

    async fn on_llm_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.update_run(run_id, |run| {
            run.end_time = Some(chrono::Utc::now());
            run.error = Some(error.to_string());
        });

        // Clone run to avoid deadlock when get_breadcrumbs() calls get_parents() which locks run_map
        let run = self.run_map.lock().get(&run_id).cloned();
        if let Some(run) = run {
            let crumbs = self.get_breadcrumbs(&run);
            let message = format!(
                "{} {} {}",
                "[llm/error]".red(),
                format!(
                    "[{}] [{}] LLM run errored with error:",
                    crumbs,
                    elapsed(&run)
                )
                .bold(),
                error
            );
            (self.callback)(&message);
        }
        Ok(())
    }

    async fn on_tool_start(
        &self,
        _serialized: &HashMap<String, serde_json::Value>,
        input_str: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        _metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut run = self.get_or_create_run(run_id, "Tool", RunType::Tool);
        run.inputs = Some(serde_json::json!({"input": input_str}));
        run.tags = Some(tags.to_vec());
        if let Some(parent) = parent_run_id {
            run.parent_run_id = Some(parent);
        }

        let crumbs = self.get_breadcrumbs(&run);
        let message = format!(
            "{} {} \"{}\"",
            "[tool/start]".green(),
            format!("[{}] Entering Tool run with input:", crumbs).bold(),
            input_str.trim()
        );
        (self.callback)(&message);

        self.run_map.lock().insert(run_id, run);
        Ok(())
    }

    async fn on_tool_end(
        &self,
        output: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.update_run(run_id, |run| {
            run.end_time = Some(chrono::Utc::now());
            run.outputs = Some(serde_json::json!({"output": output}));
        });

        // Clone run to avoid deadlock when get_breadcrumbs() calls get_parents() which locks run_map
        let run = self.run_map.lock().get(&run_id).cloned();
        if let Some(run) = run {
            let crumbs = self.get_breadcrumbs(&run);
            let message = format!(
                "{} {} \"{}\"",
                "[tool/end]".blue(),
                format!(
                    "[{}] [{}] Exiting Tool run with output:",
                    crumbs,
                    elapsed(&run)
                )
                .bold(),
                output.trim()
            );
            (self.callback)(&message);
        }
        Ok(())
    }

    async fn on_tool_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.update_run(run_id, |run| {
            run.end_time = Some(chrono::Utc::now());
            run.error = Some(error.to_string());
        });

        // Clone run to avoid deadlock when get_breadcrumbs() calls get_parents() which locks run_map
        let run = self.run_map.lock().get(&run_id).cloned();
        if let Some(run) = run {
            let crumbs = self.get_breadcrumbs(&run);
            let message = format!(
                "{} {} Tool run errored with error:\n{}",
                "[tool/error]".red(),
                format!("[{}] [{}]", crumbs, elapsed(&run)).bold(),
                error
            );
            (self.callback)(&message);
        }
        Ok(())
    }
}

#[async_trait]
impl BaseTracer for FunctionCallbackHandler {
    async fn persist_run(&self, _run: &RunTree) -> Result<()> {
        // FunctionCallbackHandler doesn't persist runs
        Ok(())
    }
}

/// Tracer that prints to the console (stdout)
///
/// This is a convenience wrapper around `FunctionCallbackHandler` that
/// uses `println!` as the callback function, providing colored console
/// output for debugging and monitoring.
///
/// # Example
///
/// ```
/// use dashflow::core::tracers::ConsoleCallbackHandler;
/// use dashflow::core::callbacks::CallbackManager;
///
/// let tracer = ConsoleCallbackHandler::new();
/// // Add to callback manager to use with DashFlow components
/// ```
pub struct ConsoleCallbackHandler {
    inner: FunctionCallbackHandler,
}

impl ConsoleCallbackHandler {
    /// Create a new ConsoleCallbackHandler
    pub fn new() -> Self {
        Self {
            inner: FunctionCallbackHandler::new(|msg| println!("{}", msg)),
        }
    }
}

impl Default for ConsoleCallbackHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CallbackHandler for ConsoleCallbackHandler {
    async fn on_chain_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        inputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        self.inner
            .on_chain_start(serialized, inputs, run_id, parent_run_id, tags, metadata)
            .await
    }

    async fn on_chain_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.inner
            .on_chain_end(outputs, run_id, parent_run_id)
            .await
    }

    async fn on_chain_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.inner
            .on_chain_error(error, run_id, parent_run_id)
            .await
    }

    async fn on_llm_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        prompts: &[String],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        self.inner
            .on_llm_start(serialized, prompts, run_id, parent_run_id, tags, metadata)
            .await
    }

    async fn on_llm_end(
        &self,
        response: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.inner.on_llm_end(response, run_id, parent_run_id).await
    }

    async fn on_llm_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.inner.on_llm_error(error, run_id, parent_run_id).await
    }

    async fn on_tool_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        input_str: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        self.inner
            .on_tool_start(serialized, input_str, run_id, parent_run_id, tags, metadata)
            .await
    }

    async fn on_tool_end(
        &self,
        output: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.inner.on_tool_end(output, run_id, parent_run_id).await
    }

    async fn on_tool_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.inner.on_tool_error(error, run_id, parent_run_id).await
    }
}

#[async_trait]
impl BaseTracer for ConsoleCallbackHandler {
    async fn persist_run(&self, run: &RunTree) -> Result<()> {
        self.inner.persist_run(run).await
    }
}

#[cfg(test)]
mod tests {
    use super::{elapsed, try_json_stringify, try_json_stringify_map, ConsoleCallbackHandler};
    use crate::core::tracers::base::BaseTracer;
    use crate::test_prelude::*;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    #[tokio::test]
    async fn test_function_callback_handler() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();
        let inputs = HashMap::new();

        handler
            .on_chain_start(&HashMap::new(), &inputs, run_id, None, &[], &HashMap::new())
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(!output_vec.is_empty());
        assert!(output_vec[0].contains("Chain"));
        assert!(output_vec[0].contains("Entering"));
    }

    #[tokio::test]
    async fn test_console_callback_handler() {
        let handler = ConsoleCallbackHandler::new();
        let run_id = Uuid::new_v4();

        // Should not panic
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
    }

    #[test]
    fn test_elapsed_formatting() {
        let run_id = Uuid::new_v4();
        let mut run = RunTree::new(run_id, "test", RunType::Chain);
        run.end_time = Some(run.start_time + chrono::Duration::milliseconds(500));

        let elapsed_str = elapsed(&run);
        assert!(elapsed_str.contains("ms") || elapsed_str.contains("500"));
    }

    #[test]
    fn test_breadcrumbs() {
        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        let handler = FunctionCallbackHandler::new(|_| {});

        let parent_run = RunTree::new(parent_id, "ParentChain", RunType::Chain);
        handler.run_map.lock().insert(parent_id, parent_run);

        let child_run = RunTree::new(child_id, "ChildTool", RunType::Tool).with_parent(parent_id);

        let breadcrumbs = handler.get_breadcrumbs(&child_run);
        assert!(breadcrumbs.contains("ParentChain"));
        assert!(breadcrumbs.contains("ChildTool"));
        assert!(breadcrumbs.contains(" > "));
    }

    #[test]
    fn test_try_json_stringify() {
        let value = serde_json::json!({"key": "value"});
        let result = try_json_stringify(&value);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_try_json_stringify_map() {
        let mut map = HashMap::new();
        map.insert("test".to_string(), serde_json::json!("value"));

        let result = try_json_stringify_map(&map, "fallback");
        assert!(result.contains("test") || result == "fallback");
    }

    #[test]
    fn test_elapsed_pending() {
        let run_id = Uuid::new_v4();
        let run = RunTree::new(run_id, "test", RunType::Chain);

        let elapsed_str = elapsed(&run);
        assert_eq!(elapsed_str, "pending");
    }

    #[test]
    fn test_elapsed_milliseconds() {
        let run_id = Uuid::new_v4();
        let mut run = RunTree::new(run_id, "test", RunType::Chain);
        run.end_time = Some(run.start_time + chrono::Duration::milliseconds(100));

        let elapsed_str = elapsed(&run);
        assert!(elapsed_str.contains("ms"));
        assert!(elapsed_str.contains("100"));
    }

    #[test]
    fn test_elapsed_seconds() {
        let run_id = Uuid::new_v4();
        let mut run = RunTree::new(run_id, "test", RunType::Chain);
        run.end_time = Some(run.start_time + chrono::Duration::seconds(2));

        let elapsed_str = elapsed(&run);
        assert!(elapsed_str.contains("s"));
        assert!(elapsed_str.contains("2."));
    }

    #[tokio::test]
    async fn test_function_callback_handler_llm_start() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();
        let prompts = vec!["Test prompt".to_string()];

        handler
            .on_llm_start(
                &HashMap::new(),
                &prompts,
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(!output_vec.is_empty());
        assert!(output_vec[0].contains("LLM"));
        assert!(output_vec[0].contains("Entering"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_llm_end() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();
        let prompts = vec!["Test prompt".to_string()];

        // Start first
        handler
            .on_llm_start(
                &HashMap::new(),
                &prompts,
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Then end
        let mut response = HashMap::new();
        response.insert("output".to_string(), serde_json::json!("response"));

        handler.on_llm_end(&response, run_id, None).await.unwrap();

        let output_vec = output.lock().unwrap();
        assert!(output_vec.len() >= 2);
        assert!(output_vec[1].contains("Exiting"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_llm_error() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();
        let prompts = vec!["Test prompt".to_string()];

        // Start first
        handler
            .on_llm_start(
                &HashMap::new(),
                &prompts,
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Then error
        handler
            .on_llm_error("Test error", run_id, None)
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(output_vec.len() >= 2);
        assert!(output_vec[1].contains("error"));
        assert!(output_vec[1].contains("Test error"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_tool_start() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();

        handler
            .on_tool_start(
                &HashMap::new(),
                "tool input",
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(!output_vec.is_empty());
        assert!(output_vec[0].contains("Tool"));
        assert!(output_vec[0].contains("tool input"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_tool_end() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();

        // Start first
        handler
            .on_tool_start(
                &HashMap::new(),
                "tool input",
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Then end
        handler
            .on_tool_end("tool output", run_id, None)
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(output_vec.len() >= 2);
        assert!(output_vec[1].contains("tool output"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_tool_error() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();

        // Start first
        handler
            .on_tool_start(
                &HashMap::new(),
                "tool input",
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Then error
        handler
            .on_tool_error("Tool error occurred", run_id, None)
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(output_vec.len() >= 2);
        assert!(output_vec[1].contains("error"));
        assert!(output_vec[1].contains("Tool error occurred"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_chain_end() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();

        // Start first
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Then end
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), serde_json::json!("success"));

        handler.on_chain_end(&outputs, run_id, None).await.unwrap();

        let output_vec = output.lock().unwrap();
        assert!(output_vec.len() >= 2);
        assert!(output_vec[1].contains("Exiting"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_chain_error() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();

        // Start first
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Then error
        handler
            .on_chain_error("Chain failed", run_id, None)
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert!(output_vec.len() >= 2);
        assert!(output_vec[1].contains("error"));
        assert!(output_vec[1].contains("Chain failed"));
    }

    #[tokio::test]
    async fn test_nested_runs_with_parent() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        // Start parent chain
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                parent_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Start child tool with parent
        handler
            .on_tool_start(
                &HashMap::new(),
                "child input",
                child_id,
                Some(parent_id),
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        let output_vec = output.lock().unwrap();
        assert_eq!(output_vec.len(), 2);
        // Child breadcrumbs should include parent
        assert!(output_vec[1].contains(" > "));
    }

    #[test]
    fn test_get_or_create_run() {
        let handler = FunctionCallbackHandler::new(|_| {});
        let run_id = Uuid::new_v4();

        // First call creates run
        let run1 = handler.get_or_create_run(run_id, "TestRun", RunType::Llm);
        assert_eq!(run1.id, run_id);
        assert_eq!(run1.name, "TestRun");

        // Second call returns existing run
        let run2 = handler.get_or_create_run(run_id, "DifferentName", RunType::Chain);
        assert_eq!(run2.id, run_id);
        assert_eq!(run2.name, "TestRun"); // Original name preserved
    }

    #[test]
    fn test_update_run() {
        let handler = FunctionCallbackHandler::new(|_| {});
        let run_id = Uuid::new_v4();

        // Create a run
        let _run = handler.get_or_create_run(run_id, "TestRun", RunType::Chain);

        // Update it
        handler.update_run(run_id, |run| {
            run.error = Some("Test error".to_string());
        });

        // Verify update
        let run_map = handler.run_map.lock();
        let run = run_map.get(&run_id).unwrap();
        assert_eq!(run.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_get_parents_empty() {
        let handler = FunctionCallbackHandler::new(|_| {});
        let run_id = Uuid::new_v4();
        let run = RunTree::new(run_id, "Solo", RunType::Chain);

        let parents = handler.get_parents(&run);
        assert!(parents.is_empty());
    }

    #[test]
    fn test_get_parents_single_parent() {
        let handler = FunctionCallbackHandler::new(|_| {});

        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        let parent_run = RunTree::new(parent_id, "Parent", RunType::Chain);
        handler.run_map.lock().insert(parent_id, parent_run.clone());

        let child_run = RunTree::new(child_id, "Child", RunType::Tool).with_parent(parent_id);

        let parents = handler.get_parents(&child_run);
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0].id, parent_id);
    }

    #[test]
    fn test_get_parents_multiple_ancestors() {
        let handler = FunctionCallbackHandler::new(|_| {});

        let grandparent_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        let grandparent_run = RunTree::new(grandparent_id, "Grandparent", RunType::Chain);
        let parent_run =
            RunTree::new(parent_id, "Parent", RunType::Chain).with_parent(grandparent_id);
        let child_run = RunTree::new(child_id, "Child", RunType::Tool).with_parent(parent_id);

        handler
            .run_map
            .lock()
            .insert(grandparent_id, grandparent_run);
        handler.run_map.lock().insert(parent_id, parent_run);

        let parents = handler.get_parents(&child_run);
        assert_eq!(parents.len(), 2);
        assert_eq!(parents[0].id, parent_id);
        assert_eq!(parents[1].id, grandparent_id);
    }

    #[tokio::test]
    async fn test_console_callback_handler_default() {
        let handler = ConsoleCallbackHandler::default();
        let run_id = Uuid::new_v4();

        // Should not panic - output goes to stdout
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_console_callback_handler_all_methods() {
        let handler = ConsoleCallbackHandler::new();
        let run_id = Uuid::new_v4();

        // Test all callback methods don't panic
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();
        handler.on_chain_error("error", run_id, None).await.unwrap();

        let llm_id = Uuid::new_v4();
        handler
            .on_llm_start(
                &HashMap::new(),
                &["prompt".to_string()],
                llm_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler
            .on_llm_end(&HashMap::new(), llm_id, None)
            .await
            .unwrap();
        handler.on_llm_error("error", llm_id, None).await.unwrap();

        let tool_id = Uuid::new_v4();
        handler
            .on_tool_start(
                &HashMap::new(),
                "input",
                tool_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler.on_tool_end("output", tool_id, None).await.unwrap();
        handler.on_tool_error("error", tool_id, None).await.unwrap();
    }

    #[tokio::test]
    async fn test_persist_run_no_op() {
        let handler = FunctionCallbackHandler::new(|_| {});
        let run = RunTree::new(Uuid::new_v4(), "Test", RunType::Chain);

        // persist_run should be a no-op and not fail
        let result = handler.persist_run(&run).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_console_persist_run_no_op() {
        let handler = ConsoleCallbackHandler::new();
        let run = RunTree::new(Uuid::new_v4(), "Test", RunType::Chain);

        // persist_run should be a no-op and not fail
        let result = handler.persist_run(&run).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_breadcrumbs_run_types() {
        let handler = FunctionCallbackHandler::new(|_| {});

        let llm_run = RunTree::new(Uuid::new_v4(), "MyLLM", RunType::Llm);
        let breadcrumbs = handler.get_breadcrumbs(&llm_run);
        assert!(breadcrumbs.contains("llm:"));
        assert!(breadcrumbs.contains("MyLLM"));

        let chain_run = RunTree::new(Uuid::new_v4(), "MyChain", RunType::Chain);
        let breadcrumbs = handler.get_breadcrumbs(&chain_run);
        assert!(breadcrumbs.contains("chain:"));

        let tool_run = RunTree::new(Uuid::new_v4(), "MyTool", RunType::Tool);
        let breadcrumbs = handler.get_breadcrumbs(&tool_run);
        assert!(breadcrumbs.contains("tool:"));
    }

    #[tokio::test]
    async fn test_function_callback_handler_with_tags() {
        let output = Arc::new(StdMutex::new(Vec::new()));
        let output_clone = output.clone();

        let handler = FunctionCallbackHandler::new(move |msg| {
            output_clone.lock().unwrap().push(msg.to_string());
        });

        let run_id = Uuid::new_v4();
        let tags = vec!["tag1".to_string(), "tag2".to_string()];

        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &tags,
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Verify tags are stored in run
        let run_map = handler.run_map.lock();
        let run = run_map.get(&run_id).unwrap();
        assert_eq!(run.tags, Some(tags));
    }

    #[test]
    fn test_function_callback_handler_name() {
        let handler = FunctionCallbackHandler::new(|_| {});
        assert_eq!(handler.name, "function_callback_handler");
    }
}
