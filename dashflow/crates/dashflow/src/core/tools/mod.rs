//! # Tools
//!
//! Tools are components that agents can use to interact with the world.
//! Each tool has a name, description, and input schema that the agent uses
//! to determine when and how to use the tool.
//!
//! ## Overview
//!
//! The tool system provides:
//! - **Tool trait**: Base interface for all tools, extends Runnable
//! - **`FunctionTool`**: Wraps simple functions as tools
//! - **`ToolException`**: Custom error type for tool-specific errors
//! - **Schema support**: JSON schema generation for tool inputs
//!
//! ## Creating Tools
//!
//! ### Function Tool (Simple)
//!
//! ```rust,ignore
//! use dashflow::core::tools::{sync_function_tool, Tool};
//!
//! // Create a simple tool from a synchronous function
//! let calculator = sync_function_tool(
//!     "calculator",
//!     "Performs basic arithmetic operations",
//!     |input: String| -> Result<String, String> {
//!         // Parse and evaluate expression
//!         Ok(format!("Result: {}", input))
//!     }
//! );
//!
//! // Use the tool
//! # async fn example() {
//! let result = calculator._call_str("2 + 2".to_string()).await.unwrap();
//! println!("Calculator result: {}", result);
//! # }
//! ```
//!
//! ## Tool Trait
//!
//! All tools implement the `Tool` trait, which extends `Runnable`:
//!
//! ```rust,ignore
//! #[async_trait]
//! pub trait Tool: Runnable<Input = ToolInput, Output = String> {
//!     /// Get the tool's name
//!     fn name(&self) -> &str;
//!
//!     /// Get the tool's description
//!     fn description(&self) -> &str;
//!
//!     /// Get the tool's input schema (JSON Schema)
//!     fn args_schema(&self) -> serde_json::Value;
//!
//!     /// Execute the tool with given input
//!     async fn call(&self, input: String) -> Result<String, ToolError>;
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::constants::{REGEX_DFA_SIZE_LIMIT, REGEX_SIZE_LIMIT};
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};
use crate::core::language_models::ToolDefinition;
use crate::core::runnable::Runnable;

// =============================================================================
// Type Aliases for Complex Types (improves readability, removes clippy warnings)
// =============================================================================

/// Boxed async function that takes a string and returns a Result<String, String>
pub type AsyncStringFn = Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>;

/// Type-erased async tool callback: takes JSON input, returns string output
pub type AsyncToolCallback =
    Box<dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

// =============================================================================

/// Input type for tools - can be a string or structured data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolInput {
    /// Simple string input
    String(String),
    /// Structured input with named arguments
    Structured(serde_json::Value),
}

impl From<String> for ToolInput {
    fn from(s: String) -> Self {
        ToolInput::String(s)
    }
}

impl From<&str> for ToolInput {
    fn from(s: &str) -> Self {
        ToolInput::String(s.to_string())
    }
}

impl From<serde_json::Value> for ToolInput {
    fn from(v: serde_json::Value) -> Self {
        ToolInput::Structured(v)
    }
}

/// Custom error type for tool execution errors
///
/// This error type allows tools to signal errors without stopping the agent.
/// The error is handled according to the tool's `handle_tool_error` setting,
/// and the result is returned as an observation to the agent.
#[derive(Debug, Clone)]
pub struct ToolException {
    /// The error message describing what went wrong.
    pub message: String,
}

impl ToolException {
    /// Creates a new tool exception with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        ToolException {
            message: message.into(),
        }
    }
}

impl fmt::Display for ToolException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tool execution error: {}", self.message)
    }
}

impl StdError for ToolException {}

// ============================================================================
// Tool Input Schema Validation (M-226)
// ============================================================================

/// Validates a `ToolInput` against a JSON Schema.
///
/// This function is used to validate tool inputs before execution, ensuring
/// that the input matches the tool's declared `args_schema`. This prevents:
/// - Malformed inputs from LLM-generated tool calls
/// - Type mismatches that would otherwise cause runtime errors
/// - Security issues from unexpected input shapes
///
/// # Arguments
///
/// * `input` - The tool input to validate
/// * `schema` - The JSON Schema to validate against
///
/// # Returns
///
/// * `Ok(())` if validation passes
/// * `Err(Error)` with validation details if it fails
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{validate_tool_input, ToolInput};
/// use serde_json::json;
///
/// let schema = json!({
///     "type": "object",
///     "properties": {
///         "query": { "type": "string", "minLength": 1 }
///     },
///     "required": ["query"]
/// });
///
/// let input = ToolInput::Structured(json!({"query": "test"}));
/// assert!(validate_tool_input(&input, &schema).is_ok());
///
/// let bad_input = ToolInput::Structured(json!({"query": ""}));
/// assert!(validate_tool_input(&bad_input, &schema).is_err()); // minLength violation
/// ```
pub fn validate_tool_input(input: &ToolInput, schema: &serde_json::Value) -> Result<()> {
    // Convert ToolInput to JSON Value for validation
    let input_value = match input {
        ToolInput::String(s) => {
            // For string inputs, wrap in object if schema expects object with "input" field
            if schema.get("type") == Some(&json!("object")) {
                if let Some(props) = schema.get("properties") {
                    if props.get("input").is_some() {
                        json!({"input": s})
                    } else {
                        // Schema expects object but doesn't have "input" field
                        // Try to parse string as JSON object
                        serde_json::from_str(s).unwrap_or_else(|_| json!({"input": s}))
                    }
                } else {
                    json!({"input": s})
                }
            } else if schema.get("type") == Some(&json!("string")) {
                json!(s)
            } else {
                // Try to parse string as JSON
                serde_json::from_str(s).unwrap_or_else(|_| json!(s))
            }
        }
        ToolInput::Structured(v) => v.clone(),
    };

    // Compile and validate the schema
    let compiled_schema = match jsonschema::validator_for(schema) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Invalid JSON Schema in tool definition, skipping validation"
            );
            // If the schema itself is invalid, skip validation rather than fail
            // This maintains backwards compatibility with tools that have incomplete schemas
            return Ok(());
        }
    };

    // Collect validation errors using iter_errors
    let errors: Vec<String> = compiled_schema
        .iter_errors(&input_value)
        .map(|e| {
            let path = e.instance_path.to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{}: {}", path, e)
            }
        })
        .collect();

    if !errors.is_empty() {
        return Err(Error::tool_error(format!(
            "Tool input validation failed: {}",
            errors.join("; ")
        )));
    }

    Ok(())
}

/// Validates tool input with a tool name for better error messages.
///
/// This is a convenience wrapper around `validate_tool_input` that includes
/// the tool name in error messages for easier debugging.
pub fn validate_tool_input_for(
    tool_name: &str,
    input: &ToolInput,
    schema: &serde_json::Value,
) -> Result<()> {
    validate_tool_input(input, schema).map_err(|e| {
        Error::tool_error(format!(
            "Tool '{}' input validation failed: {}",
            tool_name, e
        ))
    })
}

/// Base trait for all DashFlow tools
///
/// Tools are components that can be called by agents to perform specific actions.
/// All tools must have a name, description, and input schema.
///
/// Tools extend the `Runnable` trait, allowing them to be composed in chains
/// and used with the LCEL (DashFlow Expression Language) system.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool's unique name
    ///
    /// The name should clearly communicate the tool's purpose and be
    /// easily distinguishable from other tools.
    fn name(&self) -> &str;

    /// Get the tool's description
    ///
    /// Used to tell the model how/when/why to use the tool.
    /// You can provide few-shot examples as part of the description.
    fn description(&self) -> &str;

    /// Get the tool's input schema (JSON Schema format)
    ///
    /// Returns a JSON Schema object describing the tool's expected inputs.
    /// If the tool accepts a single string input, this should return a simple
    /// string schema.
    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Tool input"
                }
            },
            "required": ["input"]
        })
    }

    /// Internal method - use `dashflow::call_tool()` instead.
    ///
    /// Application code should use the framework API which provides:
    /// - ExecutionTrace collection for optimizers
    /// - Streaming events for live progress
    /// - Introspection capabilities
    /// - Metrics collection (latency, success rate)
    ///
    /// ```rust,ignore
    /// use dashflow::call_tool;
    /// let result = call_tool(tool, "input").await?;
    /// ```
    #[doc(hidden)]
    async fn _call(&self, input: ToolInput) -> Result<String>;

    /// Internal method - use `dashflow::call_tool()` instead.
    #[doc(hidden)]
    async fn _call_str(&self, input: String) -> Result<String> {
        self._call(ToolInput::String(input)).await
    }

    /// Convert this tool to a `ToolDefinition` for passing to language models.
    ///
    /// This method creates a `ToolDefinition` that can be used by language models
    /// to understand when and how to call this tool. The `ToolDefinition` includes
    /// the tool's name, description, and parameter schema.
    ///
    /// # Returns
    ///
    /// A `ToolDefinition` containing:
    /// - `name`: The tool's unique identifier
    /// - `description`: Human-readable description of what the tool does
    /// - `parameters`: JSON Schema describing the tool's expected inputs
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::tools::{sync_function_tool, Tool};
    ///
    /// let calculator = sync_function_tool(
    ///     "calculator",
    ///     "Performs basic arithmetic operations",
    ///     |input: String| -> Result<String, String> {
    ///         Ok(format!("Result: {}", input))
    ///     }
    /// );
    ///
    /// let definition = calculator.to_definition();
    /// assert_eq!(definition.name, "calculator");
    /// assert_eq!(definition.description, "Performs basic arithmetic operations");
    /// ```
    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.args_schema(),
        }
    }
}

/// A tool that wraps a simple function
///
/// This is the simplest way to create a tool - just wrap a function that
/// takes a string input and returns a string output.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{sync_function_tool, Tool};
///
/// let echo = sync_function_tool(
///     "echo",
///     "Returns the input unchanged",
///     |input: String| -> Result<String, String> {
///         Ok(input)
///     }
/// );
///
/// # async fn example() {
/// let result = echo._call_str("hello".to_string()).await.unwrap();
/// assert_eq!(result, "hello");
/// # }
/// ```
pub struct FunctionTool<F>
where
    F: Fn(String) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: String,
    func: Arc<F>,
    args_schema: Option<serde_json::Value>,
}

impl<F> FunctionTool<F>
where
    F: Fn(String) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
{
    /// Create a new function tool from an async function
    pub fn new(name: impl Into<String>, description: impl Into<String>, func: F) -> Self {
        FunctionTool {
            name: name.into(),
            description: description.into(),
            func: Arc::new(func),
            args_schema: None,
        }
    }

    /// Set a custom input schema for the tool
    #[must_use]
    pub fn with_args_schema(mut self, schema: serde_json::Value) -> Self {
        self.args_schema = Some(schema);
        self
    }
}

#[async_trait]
impl<F> Tool for FunctionTool<F>
where
    F: Fn(String) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn args_schema(&self) -> serde_json::Value {
        self.args_schema.clone().unwrap_or_else(|| {
            json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": self.description
                    }
                },
                "required": ["input"]
            })
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_str = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => {
                // Try to extract "input" field from structured input
                if let Some(input_value) = v.get("input") {
                    // Handle various JSON types
                    match input_value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Null => "null".to_string(),
                        _ => {
                            // For arrays/objects, serialize to JSON string
                            serde_json::to_string(input_value).map_err(|e| {
                                Error::tool_error(format!("Failed to serialize input: {e}"))
                            })?
                        }
                    }
                } else {
                    // No "input" field, serialize entire value
                    serde_json::to_string(&v)
                        .map_err(|e| Error::tool_error(format!("Invalid tool input: {e}")))?
                }
            }
        };

        (self.func)(input_str).await.map_err(Error::tool_error)
    }
}

// Implement Runnable for FunctionTool
#[async_trait]
impl<F> Runnable for FunctionTool<F>
where
    F: Fn(String) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
{
    type Input = ToolInput;
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._call(input).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self._call(input).await?);
        }
        Ok(results)
    }
}

/// Creates a simple synchronous function tool.
///
/// This helper wraps a synchronous function as a [`FunctionTool`] that can be used
/// with agents and tool executors. The function runs directly on the async runtime
/// without spawning a blocking task.
///
/// # Warning
///
/// This function is suitable for quick, non-blocking computations. For functions
/// that perform I/O operations or long-running computations, use [`blocking_function_tool`]
/// instead to avoid blocking the async runtime.
///
/// # Arguments
///
/// * `name` - The name of the tool (must be unique among tools used with an agent)
/// * `description` - A description of what the tool does (used by LLMs to select tools)
/// * `func` - The function to wrap, taking a `String` input and returning `Result<String, String>`
///
/// # Example
///
/// ```rust
/// use dashflow::core::tools::sync_function_tool;
///
/// let add_tool = sync_function_tool(
///     "add",
///     "Add two numbers separated by comma",
///     |input| {
///         let parts: Vec<&str> = input.split(',').collect();
///         if parts.len() != 2 {
///             return Err("Expected two comma-separated numbers".to_string());
///         }
///         let a: i32 = parts[0].trim().parse().map_err(|e| format!("{}", e))?;
///         let b: i32 = parts[1].trim().parse().map_err(|e| format!("{}", e))?;
///         Ok((a + b).to_string())
///     }
/// );
/// ```
///
/// # See Also
///
/// * [`blocking_function_tool`] - For functions that perform blocking I/O
/// * [`FunctionTool`] - The underlying tool type
#[allow(clippy::type_complexity)] // Return type includes boxed future with full async bounds
pub fn sync_function_tool<F>(
    name: impl Into<String>,
    description: impl Into<String>,
    func: F,
) -> FunctionTool<
    impl Fn(String) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
>
where
    F: Fn(String) -> std::result::Result<String, String> + Send + Sync + 'static,
{
    let func = Arc::new(func);
    FunctionTool::new(name, description, move |input: String| {
        let func = Arc::clone(&func);
        Box::pin(async move { func(input) })
    })
}

/// Helper function to create a tool from a blocking function.
///
/// Unlike [`sync_function_tool`], this wraps the function in `tokio::task::spawn_blocking`
/// to avoid blocking the async runtime. Use this for functions that perform I/O operations
/// (filesystem, network, etc.) that could block for extended periods.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{blocking_function_tool, Tool};
///
/// let file_reader = blocking_function_tool(
///     "read_file",
///     "Reads a file from disk",
///     |path: String| -> Result<String, String> {
///         std::fs::read_to_string(&path)
///             .map_err(|e| format!("Failed to read file: {e}"))
///     }
/// );
/// ```
#[allow(clippy::type_complexity)] // Return type includes boxed future with full async bounds
pub fn blocking_function_tool<F>(
    name: impl Into<String>,
    description: impl Into<String>,
    func: F,
) -> FunctionTool<
    impl Fn(String) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
>
where
    F: Fn(String) -> std::result::Result<String, String> + Send + Sync + 'static,
{
    let func = Arc::new(func);
    FunctionTool::new(name, description, move |input: String| {
        let func = Arc::clone(&func);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || func(input))
                .await
                .map_err(|e| format!("Task panicked: {e}"))?
        })
    })
}

/// A tool that accepts structured input with multiple named arguments
///
/// `StructuredTool` allows you to define tools that accept multiple typed arguments
/// rather than a single string. Arguments are parsed from JSON according to a schema.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{StructuredTool, Tool};
/// use serde::{Deserialize, Serialize};
/// use serde_json::json;
///
/// #[derive(Debug, Deserialize)]
/// struct SearchArgs {
///     query: String,
///     max_results: i32,
/// }
///
/// let search_tool = StructuredTool::new(
///     "web_search",
///     "Searches the web for a query",
///     json!({
///         "type": "object",
///         "properties": {
///             "query": {
///                 "type": "string",
///                 "description": "The search query"
///             },
///             "max_results": {
///                 "type": "integer",
///                 "description": "Maximum number of results",
///                 "default": 10
///             }
///         },
///         "required": ["query"]
///     }),
///     |args: SearchArgs| async move {
///         Ok(format!("Found {} results for: {}", args.max_results, args.query))
///     }
/// );
///
/// # async fn example() {
/// use dashflow::core::tools::ToolInput;
/// let input = json!({"query": "rust programming", "max_results": 5});
/// let result = search_tool._call(ToolInput::Structured(input)).await.unwrap();
/// assert!(result.contains("rust programming"));
/// # }
/// ```
pub struct StructuredTool<Args, F, Fut>
where
    Args: for<'de> Deserialize<'de> + Send + Sync + 'static,
    F: Fn(Args) -> Fut + Send + Sync,
    Fut: Future<Output = std::result::Result<String, String>> + Send,
{
    name: String,
    description: String,
    args_schema: serde_json::Value,
    func: Arc<F>,
    _phantom: std::marker::PhantomData<Args>,
}

impl<Args, F, Fut> StructuredTool<Args, F, Fut>
where
    Args: for<'de> Deserialize<'de> + Send + Sync + 'static,
    F: Fn(Args) -> Fut + Send + Sync,
    Fut: Future<Output = std::result::Result<String, String>> + Send,
{
    /// Create a new structured tool
    ///
    /// # Arguments
    ///
    /// * `name` - The tool's unique name
    /// * `description` - Description of what the tool does
    /// * `args_schema` - JSON Schema describing the tool's arguments
    /// * `func` - The async function that implements the tool
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        args_schema: serde_json::Value,
        func: F,
    ) -> Self {
        StructuredTool {
            name: name.into(),
            description: description.into(),
            args_schema,
            func: Arc::new(func),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<Args, F, Fut> Tool for StructuredTool<Args, F, Fut>
where
    Args: for<'de> Deserialize<'de> + Send + Sync + 'static,
    F: Fn(Args) -> Fut + Send + Sync,
    Fut: Future<Output = std::result::Result<String, String>> + Send + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn args_schema(&self) -> serde_json::Value {
        self.args_schema.clone()
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let json_value = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => {
                // Try to parse string as JSON
                serde_json::from_str(&s).map_err(|e| {
                    Error::tool_error(format!(
                        "StructuredTool requires structured input, got string that couldn't parse as JSON: {e}"
                    ))
                })?
            }
        };

        // Deserialize to the expected argument type
        let args: Args = serde_json::from_value(json_value)
            .map_err(|e| Error::tool_error(format!("Failed to parse tool arguments: {e}")))?;

        // Call the function
        (self.func)(args).await.map_err(Error::tool_error)
    }
}

// Implement Runnable for StructuredTool
#[async_trait]
impl<Args, F, Fut> Runnable for StructuredTool<Args, F, Fut>
where
    Args: for<'de> Deserialize<'de> + Send + Sync + 'static,
    F: Fn(Args) -> Fut + Send + Sync,
    Fut: Future<Output = std::result::Result<String, String>> + Send + 'static,
{
    type Input = ToolInput;
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._call(input).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self._call(input).await?);
        }
        Ok(results)
    }
}

/// Helper to create a `StructuredTool` from a sync function
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{sync_structured_tool, Tool};
/// use serde::{Deserialize, Serialize};
/// use serde_json::json;
///
/// #[derive(Debug, Deserialize)]
/// struct AddArgs {
///     a: i32,
///     b: i32,
/// }
///
/// let add_tool = sync_structured_tool(
///     "add",
///     "Adds two numbers",
///     json!({
///         "type": "object",
///         "properties": {
///             "a": {"type": "integer"},
///             "b": {"type": "integer"}
///         },
///         "required": ["a", "b"]
///     }),
///     |args: AddArgs| -> Result<String, String> {
///         Ok(format!("{}", args.a + args.b))
///     }
/// );
///
/// # async fn example() {
/// use dashflow::core::tools::ToolInput;
/// let input = json!({"a": 5, "b": 3});
/// let result = add_tool._call(ToolInput::Structured(input)).await.unwrap();
/// assert_eq!(result, "8");
/// # }
/// ```
#[allow(clippy::type_complexity)] // Return type includes boxed future with full async bounds
pub fn sync_structured_tool<Args, F>(
    name: impl Into<String>,
    description: impl Into<String>,
    args_schema: serde_json::Value,
    func: F,
) -> StructuredTool<
    Args,
    impl Fn(Args) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
    Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>,
>
where
    Args: for<'de> Deserialize<'de> + Send + Sync + 'static,
    F: Fn(Args) -> std::result::Result<String, String> + Send + Sync + 'static,
{
    let func = Arc::new(func);
    StructuredTool::new(
        name,
        description,
        args_schema,
        move |args: Args| -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        {
            let func = Arc::clone(&func);
            Box::pin(async move { func(args) })
        },
    )
}

/// Helper function to create a structured tool from a blocking function.
///
/// Unlike [`sync_structured_tool`], this wraps the function in `tokio::task::spawn_blocking`
/// to avoid blocking the async runtime. Use this for structured tools that perform I/O operations.
#[allow(clippy::type_complexity)] // Return type includes boxed future with full async bounds
pub fn blocking_structured_tool<Args, F>(
    name: impl Into<String>,
    description: impl Into<String>,
    args_schema: serde_json::Value,
    func: F,
) -> StructuredTool<
    Args,
    impl Fn(Args) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        + Send
        + Sync,
    Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>,
>
where
    Args: for<'de> Deserialize<'de> + Send + Sync + 'static,
    F: Fn(Args) -> std::result::Result<String, String> + Send + Sync + 'static,
{
    let func = Arc::new(func);
    StructuredTool::new(
        name,
        description,
        args_schema,
        move |args: Args| -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send>>
        {
            let func = Arc::clone(&func);
            Box::pin(async move {
                tokio::task::spawn_blocking(move || func(args))
                    .await
                    .map_err(|e| format!("Task panicked: {e}"))?
            })
        },
    )
}

/// Convert a collection of tools to `ToolDefinitions` for passing to language models.
///
/// This is a convenience function that converts a slice of trait objects
/// (`Arc<dyn Tool>`) into a vector of `ToolDefinition` structs that can be
/// passed to language models for function/tool calling.
///
/// # Arguments
///
/// * `tools` - A slice of boxed Tool trait objects
///
/// # Returns
///
/// A vector of `ToolDefinitions`, one for each tool in the input slice.
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use dashflow::core::tools::{sync_function_tool, Tool, tools_to_definitions};
///
/// let calculator = sync_function_tool(
///     "calculator",
///     "Performs arithmetic operations",
///     |input: String| -> Result<String, String> {
///         Ok(format!("Result: {}", input))
///     }
/// );
///
/// let search = sync_function_tool(
///     "search",
///     "Search the web",
///     |input: String| -> Result<String, String> {
///         Ok(format!("Search results for: {}", input))
///     }
/// );
///
/// let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(calculator), Arc::new(search)];
/// let definitions = tools_to_definitions(&tools);
/// assert_eq!(definitions.len(), 2);
/// assert_eq!(definitions[0].name, "calculator");
/// assert_eq!(definitions[1].name, "search");
/// ```
pub fn tools_to_definitions(tools: &[Arc<dyn Tool>]) -> Vec<ToolDefinition> {
    tools.iter().map(|tool| tool.to_definition()).collect()
}

#[cfg(test)]
mod tests {
    use super::{validate_tool_input, validate_tool_input_for, ToolInput};
    use crate::test_prelude::*;
    use serde::Deserialize;

    #[tokio::test]
    async fn test_function_tool_string_input() {
        let echo = sync_function_tool("echo", "Returns the input unchanged", |input: String| {
            Ok(input)
        });

        let result = echo._call_str("hello".to_string()).await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_function_tool_structured_input() {
        let echo = sync_function_tool("echo", "Returns the input unchanged", |input: String| {
            Ok(input)
        });

        let input = json!({"input": "hello"});
        let result = echo._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_function_tool_metadata() {
        let tool = sync_function_tool(
            "calculator",
            "Performs arithmetic operations",
            |input: String| Ok(format!("Result: {}", input)),
        );

        assert_eq!(Tool::name(&tool), "calculator");
        assert_eq!(Tool::description(&tool), "Performs arithmetic operations");

        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }

    #[tokio::test]
    async fn test_function_tool_error() {
        let failing_tool = sync_function_tool("fail", "Always fails", |_input: String| {
            Err("Tool failed".into())
        });

        let result = failing_tool._call_str("test".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Tool failed"));
    }

    #[tokio::test]
    async fn test_tool_input_from_string() {
        let input: ToolInput = "hello".into();
        match input {
            ToolInput::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected String variant"),
        }
    }

    #[tokio::test]
    async fn test_tool_input_from_json() {
        let value = json!({"key": "value"});
        let input: ToolInput = value.clone().into();
        match input {
            ToolInput::Structured(v) => assert_eq!(v, value),
            _ => panic!("Expected Structured variant"),
        }
    }

    #[tokio::test]
    async fn test_custom_args_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            }
        });

        let tool = sync_function_tool("search", "Searches the web", |input: String| {
            Ok(format!("Search results for: {}", input))
        })
        .with_args_schema(schema.clone());

        assert_eq!(tool.args_schema(), schema);
    }

    #[tokio::test]
    async fn test_tool_as_runnable() {
        let tool = sync_function_tool("upper", "Converts to uppercase", |input: String| {
            Ok(input.to_uppercase())
        });

        // Use tool as Runnable directly (no need for dyn trait object)
        let result = tool
            .invoke(ToolInput::String("hello".to_string()), None)
            .await
            .unwrap();
        assert_eq!(result, "HELLO");
    }

    #[tokio::test]
    async fn test_tool_batch() {
        let tool = sync_function_tool("double", "Doubles the input", |input: String| {
            Ok(format!("{}{}", input, input))
        });

        let inputs = vec![
            ToolInput::String("a".to_string()),
            ToolInput::String("b".to_string()),
            ToolInput::String("c".to_string()),
        ];

        let results = tool.batch(inputs, None).await.unwrap();
        assert_eq!(results, vec!["aa", "bb", "cc"]);
    }

    // StructuredTool tests
    #[derive(Debug, Deserialize)]
    struct AddArgs {
        a: i32,
        b: i32,
    }

    #[tokio::test]
    async fn test_structured_tool_basic() {
        let add_tool = sync_structured_tool(
            "add",
            "Adds two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                },
                "required": ["a", "b"]
            }),
            |args: AddArgs| -> std::result::Result<String, String> {
                Ok(format!("{}", args.a + args.b))
            },
        );

        let input = json!({"a": 5, "b": 3});
        let result = add_tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "8");
    }

    #[tokio::test]
    async fn test_structured_tool_metadata() {
        let tool = sync_structured_tool(
            "multiply",
            "Multiplies two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                }
            }),
            |args: AddArgs| -> std::result::Result<String, String> {
                Ok(format!("{}", args.a * args.b))
            },
        );

        assert_eq!(Tool::name(&tool), "multiply");
        assert_eq!(Tool::description(&tool), "Multiplies two numbers");

        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }

    #[derive(Debug, Deserialize)]
    struct SearchArgs {
        query: String,
        max_results: Option<i32>,
    }

    #[tokio::test]
    async fn test_structured_tool_with_optional_args() {
        let search_tool = sync_structured_tool(
            "search",
            "Searches for content",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum results",
                        "default": 10
                    }
                },
                "required": ["query"]
            }),
            |args: SearchArgs| -> std::result::Result<String, String> {
                let max = args.max_results.unwrap_or(10);
                Ok(format!("Searching for '{}' (max: {})", args.query, max))
            },
        );

        // Test with both arguments
        let input1 = json!({"query": "rust", "max_results": 5});
        let result1 = search_tool
            ._call(ToolInput::Structured(input1))
            .await
            .unwrap();
        assert_eq!(result1, "Searching for 'rust' (max: 5)");

        // Test with only required argument
        let input2 = json!({"query": "programming"});
        let result2 = search_tool
            ._call(ToolInput::Structured(input2))
            .await
            .unwrap();
        assert_eq!(result2, "Searching for 'programming' (max: 10)");
    }

    #[tokio::test]
    async fn test_structured_tool_json_string_input() {
        let add_tool = sync_structured_tool(
            "add",
            "Adds two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                }
            }),
            |args: AddArgs| -> std::result::Result<String, String> {
                Ok(format!("{}", args.a + args.b))
            },
        );

        // Pass JSON as a string - should parse it
        let input = ToolInput::String(r#"{"a": 10, "b": 20}"#.to_string());
        let result = add_tool._call(input).await.unwrap();
        assert_eq!(result, "30");
    }

    #[tokio::test]
    async fn test_structured_tool_error_handling() {
        let failing_tool = sync_structured_tool(
            "fail",
            "Always fails",
            json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                }
            }),
            |_args: serde_json::Value| -> std::result::Result<String, String> {
                Err("Tool execution failed".to_string())
            },
        );

        let input = json!({"message": "test"});
        let result = failing_tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Tool execution failed"));
    }

    #[tokio::test]
    async fn test_structured_tool_invalid_input() {
        let add_tool = sync_structured_tool(
            "add",
            "Adds two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                }
            }),
            |args: AddArgs| -> std::result::Result<String, String> {
                Ok(format!("{}", args.a + args.b))
            },
        );

        // Missing required field
        let input = json!({"a": 5});
        let result = add_tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse tool arguments"));
    }

    #[tokio::test]
    async fn test_structured_tool_as_runnable() {
        let tool = sync_structured_tool(
            "subtract",
            "Subtracts b from a",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                }
            }),
            |args: AddArgs| -> std::result::Result<String, String> {
                Ok(format!("{}", args.a - args.b))
            },
        );

        let input = json!({"a": 10, "b": 3});
        let result = tool
            .invoke(ToolInput::Structured(input), None)
            .await
            .unwrap();
        assert_eq!(result, "7");
    }

    #[tokio::test]
    async fn test_structured_tool_batch() {
        let tool = sync_structured_tool(
            "multiply",
            "Multiplies two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                }
            }),
            |args: AddArgs| -> std::result::Result<String, String> {
                Ok(format!("{}", args.a * args.b))
            },
        );

        let inputs = vec![
            ToolInput::Structured(json!({"a": 2, "b": 3})),
            ToolInput::Structured(json!({"a": 4, "b": 5})),
            ToolInput::Structured(json!({"a": 6, "b": 7})),
        ];

        let results = tool.batch(inputs, None).await.unwrap();
        assert_eq!(results, vec!["6", "20", "42"]);
    }

    #[tokio::test]
    async fn test_structured_tool_async_function() {
        let tool = StructuredTool::new(
            "async_add",
            "Asynchronously adds two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": {"type": "integer"},
                    "b": {"type": "integer"}
                }
            }),
            |args: AddArgs| async move {
                // Simulate async work
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                Ok(format!("{}", args.a + args.b))
            },
        );

        let input = json!({"a": 100, "b": 200});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "300");
    }

    // ========================================================================
    // Tool Input Schema Validation Tests (M-226)
    // ========================================================================

    #[test]
    fn test_validate_tool_input_valid_structured() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "count": {"type": "integer", "minimum": 1}
            },
            "required": ["query"]
        });

        let input = ToolInput::Structured(json!({"query": "test", "count": 5}));
        assert!(validate_tool_input(&input, &schema).is_ok());
    }

    #[test]
    fn test_validate_tool_input_missing_required() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "count": {"type": "integer"}
            },
            "required": ["query"]
        });

        let input = ToolInput::Structured(json!({"count": 5}));
        let result = validate_tool_input(&input, &schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    #[test]
    fn test_validate_tool_input_wrong_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer"}
            }
        });

        let input = ToolInput::Structured(json!({"count": "not a number"}));
        let result = validate_tool_input(&input, &schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("type"));
    }

    #[test]
    fn test_validate_tool_input_minimum_violation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 1}
            }
        });

        let input = ToolInput::Structured(json!({"count": 0}));
        let result = validate_tool_input(&input, &schema);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_tool_input_string_with_input_field() {
        let schema = json!({
            "type": "object",
            "properties": {
                "input": {"type": "string"}
            },
            "required": ["input"]
        });

        let input = ToolInput::String("hello world".to_string());
        assert!(validate_tool_input(&input, &schema).is_ok());
    }

    #[test]
    fn test_validate_tool_input_string_schema() {
        let schema = json!({
            "type": "string",
            "minLength": 1
        });

        let input = ToolInput::String("test".to_string());
        assert!(validate_tool_input(&input, &schema).is_ok());

        let empty_input = ToolInput::String("".to_string());
        assert!(validate_tool_input(&empty_input, &schema).is_err());
    }

    #[test]
    fn test_validate_tool_input_invalid_schema_skipped() {
        // Invalid schema should not cause failure (backwards compatibility)
        let invalid_schema = json!({
            "type": "invalid_type_that_does_not_exist"
        });

        let input = ToolInput::String("test".to_string());
        // Should not error - invalid schema is skipped with warning
        assert!(validate_tool_input(&input, &invalid_schema).is_ok());
    }

    #[test]
    fn test_validate_tool_input_for_with_name() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"]
        });

        let input = ToolInput::Structured(json!({}));
        let result = validate_tool_input_for("search_tool", &input, &schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("search_tool"));
    }
}

/// Built-in tools that can be used with agents
pub mod builtin;

/// Convert a Tool into a JSON schema suitable for LLM tool calling
///
/// This helper function converts a Tool trait object into the JSON schema format
/// that LLM providers (`OpenAI`, Anthropic, etc.) expect when binding tools.
///
/// # Arguments
///
/// * `tool` - A reference to a Tool trait object
///
/// # Returns
///
/// A JSON value with the following structure:
/// ```json
/// {
///   "name": "tool_name",
///   "description": "tool description",
///   "input_schema": { ... }  // or "parameters" for OpenAI-style
/// }
/// ```
///
/// # Example
///
/// ```
/// use dashflow::core::tools::{tool_to_json_schema, builtin::calculator_tool};
///
/// let tool = calculator_tool();
/// let schema = tool_to_json_schema(&tool);
///
/// assert_eq!(schema["name"], "calculator");
/// assert!(schema["description"].is_string());
/// ```
pub fn tool_to_json_schema(tool: &dyn Tool) -> serde_json::Value {
    json!({
        "name": tool.name(),
        "description": tool.description(),
        "input_schema": tool.args_schema()
    })
}

/// Convert multiple Tools into JSON schemas for LLM tool calling
///
/// Convenience function that maps `tool_to_json_schema` over a collection of tools.
///
/// # Arguments
///
/// * `tools` - A slice of Tool trait object references
///
/// # Returns
///
/// A vector of JSON values, one for each tool
///
/// # Example
///
/// ```
/// use dashflow::core::tools::{tools_to_json_schemas, builtin::{calculator_tool, echo_tool}};
///
/// let tools: Vec<Box<dyn dashflow::core::tools::Tool>> = vec![
///     Box::new(calculator_tool()),
///     Box::new(echo_tool()),
/// ];
///
/// let tool_refs: Vec<&dyn dashflow::core::tools::Tool> = tools.iter()
///     .map(|t| t.as_ref())
///     .collect();
///
/// let schemas = tools_to_json_schemas(&tool_refs);
/// assert_eq!(schemas.len(), 2);
/// ```
pub fn tools_to_json_schemas(tools: &[&dyn Tool]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| tool_to_json_schema(*tool))
        .collect()
}

/// Create a tool that wraps a retriever for use with agents.
///
/// This function creates a Tool that queries the retriever and formats the results
/// for the agent. Documents are formatted using a template and joined with a separator.
///
/// # Arguments
///
/// * `retriever` - The retriever to wrap (must be `Send + Sync + 'static`)
/// * `name` - Name of the tool (passed to the language model)
/// * `description` - Description of what the tool does (passed to the language model)
/// * `document_separator` - String to join documents with (default: "\n\n")
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::create_retriever_tool;
/// use dashflow::core::retrievers::VectorStoreRetriever;
///
/// let retriever = VectorStoreRetriever::new(vector_store, ...);
/// let tool = create_retriever_tool(
///     retriever,
///     "search_docs",
///     "Search for relevant documentation",
///     "\n\n".to_string(),
/// );
///
/// // Use tool with an agent
/// let result = tool._call_str("How do I use embeddings?".to_string()).await?;
/// ```
pub fn create_retriever_tool<R>(
    retriever: R,
    name: impl Into<String>,
    description: impl Into<String>,
    document_separator: Option<String>,
) -> impl Tool
where
    R: crate::core::retrievers::Retriever + 'static,
{
    let separator = document_separator.unwrap_or_else(|| "\n\n".to_string());
    let name_str = name.into();
    let description_str = description.into();

    sync_function_tool(
        name_str,
        description_str,
        move |query: String| -> std::result::Result<String, String> {
            // Use tokio runtime to run async retriever in sync context
            let runtime = tokio::runtime::Handle::try_current()
                .or_else(|_| tokio::runtime::Runtime::new().map(|rt| rt.handle().clone()))
                .map_err(|e| format!("Failed to get tokio runtime: {e}"))?;

            let docs = runtime
                .block_on(retriever._get_relevant_documents(&query, None))
                .map_err(|e| format!("Retriever error: {e}"))?;

            // Format documents: join page_content with separator
            let content = docs
                .iter()
                .map(|doc| doc.page_content.clone())
                .collect::<Vec<_>>()
                .join(&separator);

            Ok(content)
        },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Query to search for in the retriever"
            }
        },
        "required": ["query"]
    }))
}

/// Base trait for tool kits
///
/// A toolkit is a collection of related tools that can be used together.
/// Toolkits provide a convenient way to group and initialize multiple tools
/// that share common configuration or resources.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{BaseToolkit, Tool};
///
/// struct MyToolkit {
///     api_key: String,
/// }
///
/// impl BaseToolkit for MyToolkit {
///     fn get_tools(&self) -> Vec<Arc<dyn Tool>> {
///         vec![
///             Arc::new(tool1_with_key(&self.api_key)),
///             Arc::new(tool2_with_key(&self.api_key)),
///         ]
///     }
/// }
/// ```
pub trait BaseToolkit: Send + Sync {
    /// Get the tools in the toolkit
    ///
    /// Returns a vector of Tool trait objects that can be used with agents.
    /// Each toolkit implementation decides which tools to provide and how
    /// to configure them.
    ///
    /// # Returns
    ///
    /// A vector of Arc-wrapped Tool trait objects
    fn get_tools(&self) -> Vec<Arc<dyn Tool>>;
}

/// A tool that wraps a Runnable, allowing any runnable chain to be used as a tool.
///
/// This enables the Python `DashFlow` pattern of `chain.as_tool()`, where a prompt+LLM+parser
/// chain can be converted to a tool that other LLMs can call.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::Runnable;
/// use dashflow::core::tools::{RunnableTool, Tool};
///
/// // Create a runnable chain (e.g., prompt | llm | parser)
/// let chain = my_prompt.pipe(my_llm).pipe(my_parser);
///
/// // Convert it to a tool
/// let tool = RunnableTool::new(
///     "greeting_generator",
///     "Generate a greeting in a particular style of speaking.",
///     chain,
///     args_schema // JSON schema for the tool's parameters
/// );
///
/// // Use it like any other tool
/// let result = tool._call_str("style: pirate").await?;
/// ```
pub struct RunnableTool {
    /// The tool's name
    name: String,
    /// The tool's description
    description: String,
    /// The wrapped runnable (type-erased to accept JSON and return String)
    #[allow(clippy::type_complexity)] // Type-erased async callback: JSON → Future<String>
    runnable: Box<
        dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>
            + Send
            + Sync,
    >,
    /// JSON Schema for the tool's parameters
    args_schema: serde_json::Value,
}

impl RunnableTool {
    /// Create a new `RunnableTool` from a runnable
    ///
    /// # Arguments
    ///
    /// * `name` - The tool's name (must be unique among tools)
    /// * `description` - Description of what the tool does
    /// * `runnable` - A function that wraps the runnable's invoke method
    /// * `args_schema` - JSON Schema describing the tool's expected parameters
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tool = RunnableTool::new(
    ///     "calculator",
    ///     "Performs arithmetic calculations",
    ///     Box::new(move |input| {
    ///         let chain = chain.clone();
    ///         Box::pin(async move {
    ///             chain.invoke(input, None).await
    ///         })
    ///     }),
    ///     json!({"type": "object", "properties": {"expression": {"type": "string"}}})
    /// );
    /// ```
    #[allow(clippy::type_complexity)] // Constructor accepts type-erased async callback
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        runnable: Box<
            dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>
                + Send
                + Sync,
        >,
        args_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            runnable,
            args_schema,
        }
    }
}

#[async_trait]
impl Tool for RunnableTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn args_schema(&self) -> serde_json::Value {
        self.args_schema.clone()
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        // Convert ToolInput to JSON
        let json_input = match input {
            ToolInput::String(s) => {
                // If string input, wrap in a generic {"input": "..."} object
                json!({"input": s})
            }
            ToolInput::Structured(v) => v,
        };

        // Call the wrapped runnable
        (self.runnable)(json_input).await
    }
}

// ============================================================================
// Tool Output Shaping Middleware
// ============================================================================

/// Compile a regex pattern with size limits to prevent resource exhaustion.
/// Uses centralized constants from `crate::constants`.
fn compile_bounded_regex(pattern: &str) -> std::result::Result<regex::Regex, regex::Error> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
}

/// Configuration for output shaping middleware
///
/// Controls how tool outputs are processed before being returned to the caller.
/// This includes truncation for large outputs and redaction of sensitive patterns.
#[derive(Debug, Clone)]
pub struct OutputShapingConfig {
    /// Maximum output size in bytes
    pub max_output_bytes: usize,

    /// Message to append when output is truncated
    pub truncation_message: String,

    /// Regex patterns to redact from output
    pub redact_patterns: Vec<regex::Regex>,

    /// Replacement text for redacted patterns
    pub redaction_replacement: String,
}

impl Default for OutputShapingConfig {
    fn default() -> Self {
        Self {
            max_output_bytes: 32_000,
            truncation_message: "[output truncated, {remaining} bytes omitted]".to_string(),
            redact_patterns: Vec::new(),
            redaction_replacement: "[REDACTED]".to_string(),
        }
    }
}

impl OutputShapingConfig {
    /// Create a new output shaping configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum output size in bytes
    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_output_bytes = max_bytes;
        self
    }

    /// Set the truncation message (use `{remaining}` as placeholder for byte count)
    #[must_use]
    pub fn with_truncation_message(mut self, message: impl Into<String>) -> Self {
        self.truncation_message = message.into();
        self
    }

    /// Add a pattern to redact from output
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern is not a valid regex or exceeds size limits
    pub fn with_redact_pattern(mut self, pattern: &str) -> std::result::Result<Self, regex::Error> {
        self.redact_patterns.push(compile_bounded_regex(pattern)?);
        Ok(self)
    }

    /// Set the replacement text for redacted patterns
    #[must_use]
    pub fn with_redaction_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.redaction_replacement = replacement.into();
        self
    }

    /// Shape the output according to this configuration
    ///
    /// Applies redaction first, then truncation.
    #[must_use]
    pub fn shape(&self, output: &str) -> String {
        // First apply redaction
        let mut result = output.to_string();
        for pattern in &self.redact_patterns {
            result = pattern
                .replace_all(&result, &self.redaction_replacement)
                .to_string();
        }

        // Then apply truncation if needed
        if result.len() > self.max_output_bytes {
            let remaining = result.len() - self.max_output_bytes;
            let truncation_msg = self
                .truncation_message
                .replace("{remaining}", &remaining.to_string());

            // Truncate and append message
            let truncate_at = self
                .max_output_bytes
                .saturating_sub(truncation_msg.len() + 1);
            let mut truncated = result[..truncate_at].to_string();
            truncated.push('\n');
            truncated.push_str(&truncation_msg);
            result = truncated;
        }

        result
    }
}

/// Tool wrapper that shapes output before returning
///
/// This middleware wraps any tool and applies output shaping:
/// - Truncates output exceeding max size
/// - Redacts sensitive patterns
/// - Adds truncation messages
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::{ToolOutputShaper, OutputShapingConfig};
///
/// let config = OutputShapingConfig::new()
///     .with_max_bytes(32_000)
///     .with_redact_pattern(r"/Users/\w+")
///     .unwrap()
///     .with_redact_pattern(r"api_key=\w+")
///     .unwrap();
///
/// let shaped_tool = ToolOutputShaper::new(shell_tool, config);
/// ```
pub struct ToolOutputShaper<T: Tool> {
    inner: T,
    config: OutputShapingConfig,
}

impl<T: Tool> ToolOutputShaper<T> {
    /// Create a new output shaper wrapping the given tool
    pub fn new(inner: T, config: OutputShapingConfig) -> Self {
        Self { inner, config }
    }

    /// Wrap a tool with default output shaping configuration
    pub fn wrap(inner: T) -> Self {
        Self::new(inner, OutputShapingConfig::default())
    }
}

#[async_trait]
impl<T: Tool> Tool for ToolOutputShaper<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn args_schema(&self) -> serde_json::Value {
        self.inner.args_schema()
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let output = self.inner._call(input).await?;
        Ok(self.config.shape(&output))
    }
}

// Make ToolOutputShaper implement Debug if the inner tool does
impl<T: Tool + std::fmt::Debug> std::fmt::Debug for ToolOutputShaper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolOutputShaper")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod output_shaper_tests {
    use super::*;

    #[test]
    fn test_output_shaping_config_default() {
        let config = OutputShapingConfig::default();
        assert_eq!(config.max_output_bytes, 32_000);
        assert!(config.redact_patterns.is_empty());
    }

    #[test]
    fn test_output_shaping_no_truncation() {
        let config = OutputShapingConfig::new().with_max_bytes(100);
        let output = "short output";
        let shaped = config.shape(output);
        assert_eq!(shaped, output);
    }

    #[test]
    fn test_output_shaping_truncation() {
        let config = OutputShapingConfig::new()
            .with_max_bytes(50)
            .with_truncation_message("[truncated: {remaining} bytes]");

        let output = "a".repeat(100);
        let shaped = config.shape(&output);

        assert!(shaped.len() <= 50);
        assert!(shaped.contains("[truncated:"));
        assert!(shaped.contains("bytes]"));
    }

    #[test]
    fn test_output_shaping_redaction() {
        let config = OutputShapingConfig::new()
            .with_max_bytes(1000)
            .with_redact_pattern(r"api_key=\w+")
            .unwrap()
            .with_redaction_replacement("[SECRET]");

        let output = "Response: api_key=abc123 data=ok";
        let shaped = config.shape(output);

        assert_eq!(shaped, "Response: [SECRET] data=ok");
        assert!(!shaped.contains("abc123"));
    }

    #[test]
    fn test_output_shaping_redaction_home_paths() {
        let config = OutputShapingConfig::new()
            .with_max_bytes(1000)
            .with_redact_pattern(r"/Users/\w+")
            .unwrap()
            .with_redact_pattern(r"/home/\w+")
            .unwrap();

        let output = "File at /Users/john/docs and /home/jane/files";
        let shaped = config.shape(output);

        assert_eq!(shaped, "File at [REDACTED]/docs and [REDACTED]/files");
    }

    #[test]
    fn test_output_shaping_redaction_then_truncation() {
        let config = OutputShapingConfig::new()
            .with_max_bytes(50)
            .with_redact_pattern(r"secret")
            .unwrap()
            .with_redaction_replacement("*****");

        let output = format!("secret {} secret", "x".repeat(100));
        let shaped = config.shape(&output);

        // Redaction happens first, then truncation
        assert!(!shaped.contains("secret"));
        assert!(shaped.len() <= 50);
    }

    #[test]
    fn test_output_shaping_builder_chain() {
        let config = OutputShapingConfig::new()
            .with_max_bytes(64_000)
            .with_truncation_message("[...{remaining} more bytes]")
            .with_redaction_replacement("***");

        assert_eq!(config.max_output_bytes, 64_000);
        assert_eq!(config.truncation_message, "[...{remaining} more bytes]");
        assert_eq!(config.redaction_replacement, "***");
    }
}
