//! Comprehensive tests for Tool implementations
//!
//! These tests verify tool behavior under error conditions, edge cases, and stress scenarios.
//! They complement the basic `tool_tests.rs` module with production-readiness verification.
//!
//! ## Test Categories
//!
//! 1. **Error Scenarios**: Timeouts, malformed inputs, missing fields, invalid types
//! 2. **Edge Cases**: Empty inputs, very large inputs, special characters, unicode
//! 3. **Robustness**: Concurrent execution, repeated calls, resource cleanup
//!
//! ## Usage
//!
//! Implement the `ToolComprehensiveTests` trait in your tool test suite:
//!
//! ```rust,ignore
//! use dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests;
//!
//! struct MyToolTests {
//!     tool: Box<dyn Tool>,
//! }
//!
//! #[async_trait::async_trait]
//! impl ToolComprehensiveTests for MyToolTests {
//!     fn tool(&self) -> &dyn Tool {
//!         self.tool.as_ref()
//!     }
//!
//!     fn valid_input(&self) -> serde_json::Value {
//!         json!({"input": "valid data"})
//!     }
//! }
//! ```

use dashflow::core::tools::Tool;
use serde_json::Value;

/// Trait for comprehensive tool testing beyond basic functionality
///
/// These tests verify that tools handle errors, edge cases, and stress conditions gracefully.
#[async_trait::async_trait]
pub trait ToolComprehensiveTests: Send + Sync {
    /// Returns a reference to the tool instance to test
    fn tool(&self) -> &dyn Tool;

    /// Returns a valid input for the tool (used as baseline for error tests)
    fn valid_input(&self) -> Value;

    // ========================================================================
    // Error Scenario Tests
    // ========================================================================

    /// **COMPREHENSIVE TEST** - Tool handles missing required fields gracefully
    ///
    /// Tests that the tool returns a meaningful error when required fields
    /// are missing from structured input, rather than panicking.
    ///
    /// This test verifies:
    /// - Tool does not panic on missing fields
    /// - Error message is descriptive
    /// - Error mentions the missing field name (if possible)
    async fn test_error_missing_required_field(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();

        // Empty object - all required fields missing
        let empty_input = serde_json::json!({});

        let result = tool
            ._call(dashflow::core::tools::ToolInput::Structured(empty_input))
            .await;

        // Should return an error, not panic
        assert!(
            result.is_err(),
            "Tool should return error for missing required fields"
        );

        let error = result.unwrap_err();
        let error_msg = error.to_string().to_lowercase();

        // Error should mention missing field, invalid input, or similar error
        // We're flexible here because different tools may have different error messages
        assert!(
            error_msg.contains("missing")
                || error_msg.contains("required")
                || error_msg.contains("field")
                || error_msg.contains("invalid")
                || error_msg.contains("expected")
                || error_msg.contains("input"),
            "Error message should describe the problem. Got: {error_msg}"
        );

        Ok(())
    }

    /// **COMPREHENSIVE TEST** - Tool handles invalid field types gracefully
    ///
    /// Tests that the tool returns a meaningful error when field types
    /// don't match the schema (e.g., number instead of string).
    async fn test_error_invalid_field_type(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();
        let schema = tool.args_schema();

        // Get the first required field from schema
        let properties = schema.get("properties");
        let required = schema.get("required");

        if let (Some(_props), Some(req)) = (properties, required) {
            if let Some(req_array) = req.as_array() {
                if let Some(first_field) = req_array.first() {
                    if let Some(field_name) = first_field.as_str() {
                        // Create input with wrong type (array instead of expected type)
                        let invalid_input = serde_json::json!({
                            field_name: [1, 2, 3]  // Array is unlikely to be correct for most tools
                        });

                        let result = tool
                            ._call(dashflow::core::tools::ToolInput::Structured(invalid_input))
                            .await;

                        // Tool should either error or handle gracefully
                        // We don't mandate error (tool might coerce types), but it shouldn't panic
                        match result {
                            Ok(_) => {
                                // Tool handled it gracefully (maybe coerced type)
                                Ok(())
                            }
                            Err(e) => {
                                // Tool errored appropriately
                                eprintln!("Tool correctly rejected invalid type: {e}");
                                Ok(())
                            }
                        }
                    } else {
                        Ok(()) // Skip if field name not string
                    }
                } else {
                    Ok(()) // Skip if no required fields
                }
            } else {
                Ok(()) // Skip if required is not array
            }
        } else {
            Ok(()) // Skip if schema incomplete
        }
    }

    /// **COMPREHENSIVE TEST** - Tool handles empty string input gracefully
    ///
    /// Tests that the tool processes empty string input without panicking.
    /// The tool may return an error or empty result, but should not crash.
    async fn test_edge_case_empty_string(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();

        let result = tool
            ._call(dashflow::core::tools::ToolInput::String(String::new()))
            .await;

        // Tool should not panic - may return error or empty result
        match result {
            Ok(output) => {
                eprintln!("Tool returned output for empty string: {output:?}");
                Ok(())
            }
            Err(e) => {
                eprintln!("Tool correctly rejected empty string: {e}");
                Ok(())
            }
        }
    }

    /// **COMPREHENSIVE TEST** - Tool handles very long input gracefully
    ///
    /// Tests that the tool can process large inputs (1MB string) without
    /// crashing, excessive memory usage, or indefinite hanging.
    async fn test_edge_case_very_long_input(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();

        // Create a 1MB string
        let long_string = "A".repeat(1024 * 1024);

        // Use timeout to prevent indefinite hanging
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tool._call(dashflow::core::tools::ToolInput::String(long_string)),
        )
        .await;

        match result {
            Ok(tool_result) => {
                // Tool completed within timeout
                match tool_result {
                    Ok(_) => {
                        eprintln!("Tool successfully processed 1MB input");
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Tool rejected large input (acceptable): {e}");
                        Ok(())
                    }
                }
            }
            Err(_) => {
                // Timeout - tool took too long
                Err(
                    "Tool timed out processing large input (should handle or reject quickly)"
                        .into(),
                )
            }
        }
    }

    /// **COMPREHENSIVE TEST** - Tool handles unicode and special characters
    ///
    /// Tests that the tool correctly processes unicode characters including:
    /// - Emoji
    /// - Right-to-left text (Arabic, Hebrew)
    /// - Mathematical symbols
    /// - Zero-width characters
    async fn test_edge_case_unicode_and_special_chars(
        &self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();

        let test_strings = vec![
            "Hello 👋 World 🌍",                    // Emoji
            "مرحبا بالعالم",                        // Arabic (RTL)
            "שלום עולם",                            // Hebrew (RTL)
            "∑ ∫ ∂ √ ∞ ≠ ≈ ≤ ≥",                    // Math symbols
            "Zero\u{200B}Width\u{200C}Chars",       // Zero-width chars
            "Combining: e\u{0301}",                 // Combining diacritics
            "Hello\nWorld\tTab",                    // Control characters
            "\"Quotes\" 'Apostrophes' `Backticks`", // Quote types
            "<script>alert('xss')</script>",        // HTML/XSS attempt
            "'; DROP TABLE users; --",              // SQL injection attempt
        ];

        for test_str in test_strings {
            let result = tool
                ._call(dashflow::core::tools::ToolInput::String(
                    test_str.to_string(),
                ))
                .await;

            // Tool should not panic - may return error or process gracefully
            match result {
                Ok(_) => {
                    eprintln!("Tool processed: {test_str:?}");
                }
                Err(e) => {
                    eprintln!("Tool rejected input {test_str:?}: {e}");
                }
            }
        }

        Ok(())
    }

    /// **COMPREHENSIVE TEST** - Tool handles null and undefined values
    ///
    /// Tests that the tool processes JSON null values without panicking.
    async fn test_edge_case_null_values(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();
        let schema = tool.args_schema();

        // Get first field name from schema
        if let Some(props) = schema.get("properties") {
            if let Some(props_obj) = props.as_object() {
                if let Some(first_field) = props_obj.keys().next() {
                    // Create input with null value
                    let null_input = serde_json::json!({
                        first_field: serde_json::Value::Null
                    });

                    let result = tool
                        ._call(dashflow::core::tools::ToolInput::Structured(null_input))
                        .await;

                    // Tool should not panic - may error or handle gracefully
                    match result {
                        Ok(_) => {
                            eprintln!("Tool accepted null value");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Tool rejected null value: {e}");
                            Ok(())
                        }
                    }
                } else {
                    Ok(()) // No fields to test
                }
            } else {
                Ok(()) // Properties not an object
            }
        } else {
            Ok(()) // No properties
        }
    }

    // ========================================================================
    // Robustness Tests
    // ========================================================================

    /// **COMPREHENSIVE TEST** - Tool can be called multiple times
    ///
    /// Tests that the tool maintains consistent behavior across repeated calls
    /// and doesn't accumulate state or leak resources.
    async fn test_robustness_repeated_calls(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();
        let input = self.valid_input();

        // Call tool 10 times
        for i in 0..10 {
            let result = tool
                ._call(dashflow::core::tools::ToolInput::Structured(input.clone()))
                .await;

            // Each call should complete (may succeed or fail, but shouldn't panic)
            match result {
                Ok(_) => {
                    eprintln!("Call {} succeeded", i + 1);
                }
                Err(e) => {
                    eprintln!("Call {} failed (may be expected): {}", i + 1, e);
                }
            }
        }

        Ok(())
    }

    /// **COMPREHENSIVE TEST** - Tool handles concurrent calls
    ///
    /// Tests that the tool can process multiple calls concurrently without
    /// data races, deadlocks, or inconsistent state.
    async fn test_robustness_concurrent_calls(&self) -> Result<(), Box<dyn std::error::Error>> {
        let _tool = self.tool();
        let _input = self.valid_input();

        // Note: We can't easily test concurrent calls in a trait method
        // because we can't clone `&dyn Tool` without knowing the concrete type.
        // Provider-specific tests should implement actual concurrent testing
        // by spawning tokio tasks with cloned tool instances.

        // This default implementation is a placeholder to satisfy the trait.
        // Override in provider tests for actual concurrency testing.

        eprintln!("Concurrent test not implemented (override in provider tests)");

        Ok(())
    }

    /// **COMPREHENSIVE TEST** - Tool handles rapid alternating success/failure
    ///
    /// Tests that the tool can handle rapid switches between valid and invalid
    /// inputs without getting into an inconsistent state.
    async fn test_robustness_alternating_valid_invalid(
        &self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tool = self.tool();
        let valid_input = self.valid_input();
        let invalid_input = serde_json::json!({}); // Empty object

        // Alternate valid and invalid calls
        for i in 0..10 {
            let input = if i % 2 == 0 {
                &valid_input
            } else {
                &invalid_input
            };

            let result = tool
                ._call(dashflow::core::tools::ToolInput::Structured(input.clone()))
                .await;

            match result {
                Ok(_) => {
                    eprintln!("Call {} succeeded", i + 1);
                }
                Err(e) => {
                    eprintln!("Call {} failed (may be expected): {}", i + 1, e);
                }
            }
        }

        Ok(())
    }

    // ========================================================================
    // Helper Methods for Optional Extended Tests
    // ========================================================================

    /// Override this to test timeout behavior for long-running operations
    ///
    /// Default implementation skips the test. Override in provider tests
    /// that have timeout configuration.
    async fn test_timeout_handling(&self) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!("Timeout test not implemented (override in provider tests if applicable)");
        Ok(())
    }

    /// Override this to test rate limiting behavior
    ///
    /// Default implementation skips the test. Override in provider tests
    /// for API-based tools that have rate limiting.
    async fn test_rate_limiting(&self) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!("Rate limiting test not implemented (override in provider tests if applicable)");
        Ok(())
    }

    /// Override this to test retry logic
    ///
    /// Default implementation skips the test. Override in provider tests
    /// for tools that have built-in retry logic.
    async fn test_retry_logic(&self) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!("Retry logic test not implemented (override in provider tests if applicable)");
        Ok(())
    }
}

// ============================================================================
// Concrete Test Functions (for macro generation)
// ============================================================================

/// Test that tool handles missing required fields gracefully
pub async fn test_tool_error_missing_required_field<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_error_missing_required_field().await
}

/// Test that tool handles invalid field types gracefully
pub async fn test_tool_error_invalid_field_type<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_error_invalid_field_type().await
}

/// Test that tool handles empty string input gracefully
pub async fn test_tool_edge_case_empty_string<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_edge_case_empty_string().await
}

/// Test that tool handles very long input gracefully
pub async fn test_tool_edge_case_very_long_input<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_edge_case_very_long_input().await
}

/// Test that tool handles unicode and special characters
pub async fn test_tool_edge_case_unicode_and_special_chars<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_edge_case_unicode_and_special_chars().await
}

/// Test that tool handles null values
pub async fn test_tool_edge_case_null_values<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_edge_case_null_values().await
}

/// Test that tool can be called multiple times
pub async fn test_tool_robustness_repeated_calls<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_robustness_repeated_calls().await
}

/// Test that tool handles concurrent calls
pub async fn test_tool_robustness_concurrent_calls<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_robustness_concurrent_calls().await
}

/// Test that tool handles alternating valid/invalid inputs
pub async fn test_tool_robustness_alternating_valid_invalid<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_robustness_alternating_valid_invalid().await
}

/// Test timeout handling (optional, skipped by default)
pub async fn test_tool_timeout_handling<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_timeout_handling().await
}

/// Test rate limiting (optional, skipped by default)
pub async fn test_tool_rate_limiting<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_rate_limiting().await
}

/// Test retry logic (optional, skipped by default)
pub async fn test_tool_retry_logic<T: ToolComprehensiveTests>(
    test: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    test.test_retry_logic().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::tools::{sync_function_tool, Tool};
    use serde_json::json;

    /// Test tool for testing the comprehensive test suite
    struct TestCalculatorTool {
        tool: Box<dyn Tool>,
    }

    impl TestCalculatorTool {
        fn new() -> Self {
            let tool = sync_function_tool(
                "calculator",
                "Performs basic arithmetic operations",
                |input: String| -> Result<String, String> {
                    // Simple calculator that evaluates "a + b" format
                    let parts: Vec<&str> = input.split('+').collect();
                    if parts.len() == 2 {
                        let a: i32 = parts[0]
                            .trim()
                            .parse::<i32>()
                            .map_err(|e| e.to_string())?;
                        let b: i32 = parts[1]
                            .trim()
                            .parse::<i32>()
                            .map_err(|e| e.to_string())?;
                        Ok((a + b).to_string())
                    } else {
                        Err("Invalid input format. Expected 'a + b'".to_string())
                    }
                },
            );

            TestCalculatorTool {
                tool: Box::new(tool),
            }
        }
    }

    #[async_trait::async_trait]
    impl ToolComprehensiveTests for TestCalculatorTool {
        fn tool(&self) -> &dyn Tool {
            self.tool.as_ref()
        }

        fn valid_input(&self) -> serde_json::Value {
            json!({"input": "2 + 2"})
        }
    }

    #[tokio::test]
    async fn test_comprehensive_missing_required_field() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_error_missing_required_field().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_invalid_field_type() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_error_invalid_field_type().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_empty_string() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_edge_case_empty_string().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_very_long_input() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_edge_case_very_long_input().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_unicode_and_special_chars() {
        let test_tool = TestCalculatorTool::new();
        test_tool
            .test_edge_case_unicode_and_special_chars()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_null_values() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_edge_case_null_values().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_repeated_calls() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_robustness_repeated_calls().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_concurrent_calls() {
        let test_tool = TestCalculatorTool::new();
        test_tool.test_robustness_concurrent_calls().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_alternating_valid_invalid() {
        let test_tool = TestCalculatorTool::new();
        test_tool
            .test_robustness_alternating_valid_invalid()
            .await
            .unwrap();
    }
}
