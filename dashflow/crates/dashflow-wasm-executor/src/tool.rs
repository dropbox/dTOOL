//! Tool integration for dashflow::core
//!
//! Implements the `Tool` trait for WASM code execution

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result as LangResult;
use serde_json::json;

use crate::executor::WasmExecutor;
use crate::Error;

/// WASM code execution tool for AI agents
///
/// Provides secure, HIPAA/SOC2 compliant code execution in a WebAssembly sandbox.
/// All code runs in isolation with comprehensive resource limits and audit logging.
///
/// # Example
///
/// ```no_run
/// use dashflow_wasm_executor::{WasmCodeExecutionTool, WasmExecutorConfig};
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let config = WasmExecutorConfig::new("your-jwt-secret-at-least-32-characters-long".to_string());
/// let tool = WasmCodeExecutionTool::new(config).unwrap();
///
/// // Execute WASM code via tool interface
/// let result = tool._call_str("execute fibonacci(10)".to_string()).await.unwrap();
/// println!("Result: {}", result);
/// # });
/// ```
pub struct WasmCodeExecutionTool {
    executor: WasmExecutor,
}

impl WasmCodeExecutionTool {
    /// Create a new WASM code execution tool
    ///
    /// # Arguments
    /// * `config` - Executor configuration with security settings
    ///
    /// # Errors
    /// Returns error if configuration is invalid or executor cannot be created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_wasm_executor::{WasmCodeExecutionTool, WasmExecutorConfig};
    ///
    /// let config = WasmExecutorConfig::new("your-jwt-secret-at-least-32-characters-long".to_string());
    /// let tool = WasmCodeExecutionTool::new(config).unwrap();
    /// ```
    pub fn new(config: crate::config::WasmExecutorConfig) -> Result<Self, Error> {
        let executor = WasmExecutor::new(config)?;
        Ok(Self { executor })
    }

    /// Get a reference to the underlying WASM executor
    ///
    /// Useful for advanced use cases that need direct access to the executor API
    #[must_use]
    pub fn executor(&self) -> &WasmExecutor {
        &self.executor
    }
}

#[async_trait]
impl Tool for WasmCodeExecutionTool {
    fn name(&self) -> &'static str {
        "wasm_code_execution"
    }

    fn description(&self) -> &'static str {
        "Execute code securely in a WebAssembly sandbox. Supports compiled WASM modules. \
         HIPAA/SOC2 compliant with comprehensive security controls. All data stays on your infrastructure. \
         Resource limits: 5 million fuel units, 64MB memory, 5 second timeout. \
         Zero WASI permissions by default (no filesystem, network, or system access). \
         Complete audit trail of all executions."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "wasm_bytes": {
                    "type": "string",
                    "description": "Base64-encoded WASM module bytecode",
                    "format": "base64"
                },
                "function": {
                    "type": "string",
                    "description": "Name of the function to call in the WASM module",
                    "default": "main"
                },
                "args": {
                    "type": "array",
                    "description": "Array of i32 arguments to pass to the function",
                    "items": {
                        "type": "integer",
                        "format": "int32"
                    },
                    "default": []
                }
            },
            "required": ["wasm_bytes"]
        })
    }

    async fn _call(&self, input: ToolInput) -> LangResult<String> {
        // Extract parameters from input
        let (wasm_bytes_b64, function, args) = match input {
            ToolInput::String(s) => {
                // Simple string input format: "base64_wasm_bytes"
                // or "base64_wasm_bytes|function_name"
                // or "base64_wasm_bytes|function_name|arg1,arg2,..."
                let parts: Vec<&str> = s.split('|').collect();
                let wasm_bytes_b64 = parts[0].to_string();
                let function = parts
                    .get(1)
                    .map_or_else(|| "main".to_string(), |s| (*s).to_string());
                let args = if let Some(args_str) = parts.get(2) {
                    args_str
                        .split(',')
                        .filter_map(|s| s.trim().parse::<i32>().ok())
                        .collect()
                } else {
                    vec![]
                };
                (wasm_bytes_b64, function, args)
            }
            ToolInput::Structured(v) => {
                let wasm_bytes_b64 = v
                    .get("wasm_bytes")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error(
                            "Missing 'wasm_bytes' field in structured input".to_string(),
                        )
                    })?
                    .to_string();

                let function = v
                    .get("function")
                    .and_then(|v| v.as_str())
                    .map_or_else(|| "main".to_string(), std::string::ToString::to_string);

                let args = v
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_i64().map(|i| i as i32))
                            .collect()
                    })
                    .unwrap_or_default();

                (wasm_bytes_b64, function, args)
            }
        };

        // Decode base64 WASM bytes
        use base64::prelude::*;
        let wasm_bytes = BASE64_STANDARD.decode(&wasm_bytes_b64).map_err(|e| {
            dashflow::core::Error::tool_error(format!("Invalid base64 WASM bytes: {e}"))
        })?;

        // Validate WASM size (max 10MB to prevent abuse)
        if wasm_bytes.len() > 10 * 1024 * 1024 {
            return Err(dashflow::core::Error::tool_error(
                "WASM module exceeds 10MB size limit".to_string(),
            ));
        }

        // Execute WASM code
        let result = self
            .executor
            .execute(&wasm_bytes, &function, &args)
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("WASM execution failed: {e}"))
            })?;

        Ok(result)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WasmExecutorConfig;

    /// Create a test config with proper temporary directory for audit logs
    fn test_config() -> WasmExecutorConfig {
        let mut config =
            WasmExecutorConfig::new("test-jwt-secret-at-least-32-characters-long!".to_string());
        // Disable audit logging for tests to avoid permission issues
        config.enable_audit_logging = false;
        config
    }

    #[tokio::test]
    async fn test_tool_creation() {
        let config = test_config();
        let tool = WasmCodeExecutionTool::new(config);
        assert!(tool.is_ok());
        let tool = tool.unwrap();
        // Verify the tool was created with expected properties
        assert_eq!(tool.name(), "wasm_code_execution");
    }

    #[tokio::test]
    async fn test_tool_name() {
        let config = test_config();
        let tool = WasmCodeExecutionTool::new(config).unwrap();
        assert_eq!(tool.name(), "wasm_code_execution");
    }

    #[tokio::test]
    async fn test_tool_description() {
        let config = test_config();
        let tool = WasmCodeExecutionTool::new(config).unwrap();
        let desc = tool.description();
        assert!(desc.contains("WebAssembly"));
        assert!(desc.contains("HIPAA"));
        assert!(desc.contains("SOC2"));
    }

    #[tokio::test]
    async fn test_tool_args_schema() {
        let config = test_config();
        let tool = WasmCodeExecutionTool::new(config).unwrap();
        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["wasm_bytes"].is_object());
        assert!(schema["properties"]["function"].is_object());
        assert!(schema["properties"]["args"].is_object());
    }

    #[tokio::test]
    async fn test_tool_invalid_base64() {
        let config = test_config();
        let tool = WasmCodeExecutionTool::new(config).unwrap();

        let input = ToolInput::String("not-valid-base64!!!".to_string());
        let result = tool._call(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid base64"));
    }

    #[tokio::test]
    async fn test_tool_wasm_too_large() {
        use base64::prelude::*;

        let config = test_config();
        let tool = WasmCodeExecutionTool::new(config).unwrap();

        // Create 11MB of data (exceeds 10MB limit)
        let large_data = vec![0u8; 11 * 1024 * 1024];
        let large_b64 = BASE64_STANDARD.encode(&large_data);

        let input = ToolInput::String(large_b64);
        let result = tool._call(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds 10MB"));
    }
}
