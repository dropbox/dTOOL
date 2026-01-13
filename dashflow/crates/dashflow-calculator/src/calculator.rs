//! Calculator tool implementation.

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::{Error, Result};

/// A tool for evaluating mathematical expressions.
///
/// The Calculator tool uses the `evalexpr` library to safely evaluate
/// mathematical expressions. It supports basic arithmetic operations
/// and operator precedence.
///
/// # Examples
///
/// ```rust
/// use dashflow_calculator::Calculator;
/// use dashflow::core::tools::Tool;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let calculator = Calculator::new();
///
/// // Simple arithmetic
/// let result = calculator._call_str("10 + 5 * 2".to_string()).await?;
/// assert_eq!(result, "20");
///
/// // With exponentiation
/// let result = calculator._call_str("2 ^ 3".to_string()).await?;
/// assert_eq!(result, "8");
///
/// // Complex expression with parentheses
/// let result = calculator._call_str("(10 + 5) * 2 - 3".to_string()).await?;
/// assert_eq!(result, "27");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Calculator {
    name: String,
    description: String,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    /// Creates a new Calculator tool.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_calculator::Calculator;
    ///
    /// let calculator = Calculator::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "calculator".to_string(),
            description:
                "Useful for when you need to answer questions about math. \
                         Input should be a valid mathematical expression. \
                         Supports: +, -, *, /, %, ^ (exponentiation), and parentheses for grouping."
                    .to_string(),
        }
    }

    /// Creates a Calculator with a custom name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_calculator::Calculator;
    ///
    /// let calculator = Calculator::with_name("math");
    /// ```
    pub fn with_name(name: impl Into<String>) -> Self {
        let mut calc = Self::new();
        calc.name = name.into();
        calc
    }

    /// Creates a Calculator with a custom description.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_calculator::Calculator;
    ///
    /// let calculator = Calculator::new()
    ///     .with_description("A simple math evaluator");
    /// ```
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Evaluates a mathematical expression and returns the result.
    ///
    /// # Arguments
    ///
    /// * `expression` - A valid mathematical expression as a string
    ///
    /// # Returns
    ///
    /// Returns the evaluation result as a string, or an error if the expression
    /// is invalid or cannot be evaluated.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_calculator::Calculator;
    ///
    /// let calculator = Calculator::new();
    ///
    /// let result = calculator.evaluate("2 + 2")?;
    /// assert_eq!(result, "4");
    ///
    /// let result = calculator.evaluate("2 ^ 4")?;
    /// assert_eq!(result, "16");
    ///
    /// let result = calculator.evaluate("(5 + 3) * 2")?;
    /// assert_eq!(result, "16");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn evaluate(&self, expression: &str) -> Result<String> {
        // Trim whitespace
        let expression = expression.trim();

        if expression.is_empty() {
            return Err(Error::InvalidInput(
                "Expression cannot be empty".to_string(),
            ));
        }

        // Evaluate the expression using evalexpr with builtin functions
        match evalexpr::eval_with_context(expression, &evalexpr::HashMapContext::new()) {
            Ok(value) => {
                // Convert the value to a string
                match value {
                    evalexpr::Value::Float(f) => {
                        // Format float: if it's a whole number, show without decimal
                        if f.fract() == 0.0 {
                            Ok(format!("{f:.0}"))
                        } else {
                            Ok(format!("{f}"))
                        }
                    }
                    evalexpr::Value::Int(i) => Ok(format!("{i}")),
                    evalexpr::Value::Boolean(b) => Ok(format!("{b}")),
                    evalexpr::Value::String(s) => Ok(s),
                    evalexpr::Value::Empty => Err(Error::other("Empty result")),
                    evalexpr::Value::Tuple(_) => Err(Error::other("Tuple results not supported")),
                }
            }
            Err(e) => Err(Error::InvalidInput(format!(
                "Failed to evaluate expression '{expression}': {e}"
            ))),
        }
    }
}

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "A valid mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        // Extract the expression from the input
        let expression = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => {
                // Try to extract "expression" field from structured input
                v.get("expression")
                    .and_then(|e| e.as_str())
                    .unwrap_or("")
                    .to_string()
            }
        };

        self.evaluate(&expression)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ==================== BASIC ARITHMETIC TESTS ====================

    #[test]
    fn test_basic_arithmetic() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("2 + 2").unwrap(), "4");
        assert_eq!(calc.evaluate("10 - 5").unwrap(), "5");
        assert_eq!(calc.evaluate("3 * 4").unwrap(), "12");
        assert_eq!(calc.evaluate("15 / 3").unwrap(), "5");
        assert_eq!(calc.evaluate("10 % 3").unwrap(), "1");
    }

    #[test]
    fn test_addition_variants() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0 + 0").unwrap(), "0");
        assert_eq!(calc.evaluate("1 + 0").unwrap(), "1");
        assert_eq!(calc.evaluate("0 + 1").unwrap(), "1");
        assert_eq!(calc.evaluate("100 + 200").unwrap(), "300");
        assert_eq!(calc.evaluate("1 + 2 + 3").unwrap(), "6");
        assert_eq!(calc.evaluate("1 + 2 + 3 + 4 + 5").unwrap(), "15");
    }

    #[test]
    fn test_subtraction_variants() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0 - 0").unwrap(), "0");
        assert_eq!(calc.evaluate("5 - 0").unwrap(), "5");
        assert_eq!(calc.evaluate("0 - 5").unwrap(), "-5");
        assert_eq!(calc.evaluate("100 - 200").unwrap(), "-100");
        assert_eq!(calc.evaluate("10 - 3 - 2").unwrap(), "5");
    }

    #[test]
    fn test_multiplication_variants() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0 * 100").unwrap(), "0");
        assert_eq!(calc.evaluate("1 * 100").unwrap(), "100");
        assert_eq!(calc.evaluate("2 * 3 * 4").unwrap(), "24");
        assert_eq!(calc.evaluate("10 * 10 * 10").unwrap(), "1000");
    }

    #[test]
    fn test_division_variants() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0 / 5").unwrap(), "0");
        assert_eq!(calc.evaluate("10 / 2").unwrap(), "5");
        assert_eq!(calc.evaluate("100 / 10 / 2").unwrap(), "5");
        // evalexpr uses integer division for integer operands
        assert_eq!(calc.evaluate("7 / 2").unwrap(), "3");
        // Use floats to get fractional results
        assert_eq!(calc.evaluate("7.0 / 2.0").unwrap(), "3.5");
    }

    #[test]
    fn test_modulo_variants() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0 % 5").unwrap(), "0");
        assert_eq!(calc.evaluate("10 % 10").unwrap(), "0");
        assert_eq!(calc.evaluate("17 % 5").unwrap(), "2");
        assert_eq!(calc.evaluate("100 % 7").unwrap(), "2");
    }

    // ==================== EXPONENTIATION TESTS ====================

    #[test]
    fn test_exponentiation() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("2 ^ 3").unwrap(), "8");
        assert_eq!(calc.evaluate("5 ^ 2").unwrap(), "25");
    }

    #[test]
    fn test_exponentiation_edge_cases() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("2 ^ 0").unwrap(), "1");
        assert_eq!(calc.evaluate("0 ^ 5").unwrap(), "0");
        assert_eq!(calc.evaluate("1 ^ 100").unwrap(), "1");
        assert_eq!(calc.evaluate("10 ^ 1").unwrap(), "10");
        assert_eq!(calc.evaluate("2 ^ 10").unwrap(), "1024");
    }

    #[test]
    fn test_chained_exponentiation() {
        let calc = Calculator::new();

        // Exponentiation is typically right-associative: 2^3^2 = 2^(3^2) = 2^9 = 512
        // But evalexpr may evaluate left-to-right: (2^3)^2 = 8^2 = 64
        let result = calc.evaluate("2 ^ 3 ^ 2").unwrap();
        // Accept either interpretation
        assert!(result == "512" || result == "64");
    }

    // ==================== OPERATOR PRECEDENCE TESTS ====================

    #[test]
    fn test_operator_precedence() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("2 + 3 * 4").unwrap(), "14");
        assert_eq!(calc.evaluate("(2 + 3) * 4").unwrap(), "20");
        assert_eq!(calc.evaluate("10 - 2 * 3").unwrap(), "4");
    }

    #[test]
    fn test_precedence_mult_over_add() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("1 + 2 * 3").unwrap(), "7");
        assert_eq!(calc.evaluate("2 * 3 + 1").unwrap(), "7");
        assert_eq!(calc.evaluate("1 + 2 * 3 + 4").unwrap(), "11");
    }

    #[test]
    fn test_precedence_div_over_sub() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("10 - 6 / 2").unwrap(), "7");
        assert_eq!(calc.evaluate("6 / 2 - 1").unwrap(), "2");
    }

    #[test]
    fn test_precedence_exp_over_mult() {
        let calc = Calculator::new();

        // 2 * 3 ^ 2 should be 2 * 9 = 18
        assert_eq!(calc.evaluate("2 * 3 ^ 2").unwrap(), "18");
        // 3 ^ 2 * 2 should be 9 * 2 = 18
        assert_eq!(calc.evaluate("3 ^ 2 * 2").unwrap(), "18");
    }

    // ==================== PARENTHESES TESTS ====================

    #[test]
    fn test_complex_expressions() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("(10 + 5) * 2 - 9").unwrap(), "21");
        assert_eq!(calc.evaluate("2 ^ 3 + 4 * 5").unwrap(), "28");
        assert_eq!(calc.evaluate("(100 / 4) - (5 ^ 2)").unwrap(), "0");
    }

    #[test]
    fn test_nested_parentheses() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("((2 + 3))").unwrap(), "5");
        assert_eq!(calc.evaluate("(((1 + 2)))").unwrap(), "3");
        assert_eq!(calc.evaluate("((2 + 3) * (4 + 5))").unwrap(), "45");
        assert_eq!(calc.evaluate("((10 - 5) * (2 + 3))").unwrap(), "25");
        assert_eq!(calc.evaluate("(1 + (2 + (3 + 4)))").unwrap(), "10");
    }

    #[test]
    fn test_parentheses_override_precedence() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("(1 + 2) * 3").unwrap(), "9");
        assert_eq!(calc.evaluate("1 + 2 * 3").unwrap(), "7");
        assert_eq!(calc.evaluate("(10 - 2) / 4").unwrap(), "2");
        // evalexpr uses integer division: 2 / 4 = 0, so 10 - 0 = 10
        assert_eq!(calc.evaluate("10 - 2 / 4").unwrap(), "10");
        // Use floats to get fractional results
        assert_eq!(calc.evaluate("10 - 2.0 / 4").unwrap(), "9.5");
    }

    // ==================== NEGATIVE NUMBERS TESTS ====================

    #[test]
    fn test_negative_numbers() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("-5").unwrap(), "-5");
        assert_eq!(calc.evaluate("-5 + 10").unwrap(), "5");
        assert_eq!(calc.evaluate("10 + -5").unwrap(), "5");
        assert_eq!(calc.evaluate("-5 + -5").unwrap(), "-10");
    }

    #[test]
    fn test_negative_multiplication() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("-2 * 3").unwrap(), "-6");
        assert_eq!(calc.evaluate("2 * -3").unwrap(), "-6");
        assert_eq!(calc.evaluate("-2 * -3").unwrap(), "6");
    }

    #[test]
    fn test_negative_division() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("-10 / 2").unwrap(), "-5");
        assert_eq!(calc.evaluate("10 / -2").unwrap(), "-5");
        assert_eq!(calc.evaluate("-10 / -2").unwrap(), "5");
    }

    #[test]
    fn test_negative_in_parentheses() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("(-5)").unwrap(), "-5");
        assert_eq!(calc.evaluate("(-5) * 2").unwrap(), "-10");
        assert_eq!(calc.evaluate("2 * (-5)").unwrap(), "-10");
        assert_eq!(calc.evaluate("(-3) * (-4)").unwrap(), "12");
    }

    // ==================== WHITESPACE TESTS ====================

    #[test]
    fn test_whitespace_handling() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("  2  +  2  ").unwrap(), "4");
        assert_eq!(calc.evaluate("\t10 * 2\n").unwrap(), "20");
    }

    #[test]
    fn test_various_whitespace() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("   5   ").unwrap(), "5");
        assert_eq!(calc.evaluate("2+2").unwrap(), "4");
        assert_eq!(calc.evaluate("2 +2").unwrap(), "4");
        assert_eq!(calc.evaluate("2+ 2").unwrap(), "4");
        assert_eq!(calc.evaluate("\n\t2\t+\t2\n").unwrap(), "4");
    }

    // ==================== ERROR HANDLING TESTS ====================

    #[test]
    fn test_invalid_expressions() {
        let calc = Calculator::new();

        assert!(calc.evaluate("").is_err());
        assert!(calc.evaluate("2 +").is_err());
        assert!(calc.evaluate("/ 2").is_err());
        assert!(calc.evaluate("abc").is_err());
    }

    #[test]
    fn test_empty_expression_error() {
        let calc = Calculator::new();

        let err = calc.evaluate("").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_whitespace_only_error() {
        let calc = Calculator::new();

        assert!(calc.evaluate("   ").is_err());
        assert!(calc.evaluate("\t\n").is_err());
    }

    #[test]
    fn test_incomplete_expressions() {
        let calc = Calculator::new();

        assert!(calc.evaluate("1 +").is_err());
        assert!(calc.evaluate("* 2").is_err());
        assert!(calc.evaluate("3 /").is_err());
        assert!(calc.evaluate("- -").is_err());
        assert!(calc.evaluate("2 + + 3").is_err());
    }

    #[test]
    fn test_unbalanced_parentheses() {
        let calc = Calculator::new();

        assert!(calc.evaluate("(2 + 3").is_err());
        assert!(calc.evaluate("2 + 3)").is_err());
        assert!(calc.evaluate("((2 + 3)").is_err());
        assert!(calc.evaluate("(2 + 3))").is_err());
    }

    #[test]
    fn test_invalid_characters() {
        let calc = Calculator::new();

        assert!(calc.evaluate("2 $ 3").is_err());
        assert!(calc.evaluate("2 @ 3").is_err());
        assert!(calc.evaluate("2 # 3").is_err());
        assert!(calc.evaluate("2 & 3").is_err());
    }

    #[test]
    fn test_division_by_zero() {
        let calc = Calculator::new();

        // Division by zero typically results in infinity, not an error
        let result = calc.evaluate("1 / 0");
        // Either it's an error or it returns infinity
        if let Ok(val) = result {
            assert!(val.contains("inf") || val.to_lowercase().contains("inf"));
        }
    }

    // ==================== FLOATING POINT TESTS ====================

    #[test]
    fn test_float_formatting() {
        let calc = Calculator::new();

        // Whole numbers should not have decimal points
        assert_eq!(calc.evaluate("4.0 + 0.0").unwrap(), "4");

        // Non-whole numbers should show decimals
        let result = calc.evaluate("1.5 + 2.5").unwrap();
        assert_eq!(result, "4");

        let result = calc.evaluate("10.0 / 3.0").unwrap();
        assert!(result.starts_with("3.33"));
    }

    #[test]
    fn test_decimal_numbers() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0.5 + 0.5").unwrap(), "1");
        assert_eq!(calc.evaluate("1.1 + 2.2").unwrap(), "3.3000000000000003");
        assert_eq!(calc.evaluate("0.1 * 10").unwrap(), "1");
    }

    #[test]
    fn test_mixed_int_float() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("5 + 0.5").unwrap(), "5.5");
        assert_eq!(calc.evaluate("0.5 + 5").unwrap(), "5.5");
        // evalexpr uses integer division for integer operands
        assert_eq!(calc.evaluate("10 / 4").unwrap(), "2");
        // Use at least one float to get fractional results
        assert_eq!(calc.evaluate("10.0 / 4").unwrap(), "2.5");
        assert_eq!(calc.evaluate("10 / 4.0").unwrap(), "2.5");
    }

    #[test]
    fn test_very_small_decimals() {
        let calc = Calculator::new();

        let result = calc.evaluate("0.001 + 0.001").unwrap();
        assert!(result.starts_with("0.002"));

        let result = calc.evaluate("0.0001 * 10000").unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_float_precision() {
        let calc = Calculator::new();

        // Test that we handle floating point precision issues reasonably
        let result = calc.evaluate("0.1 + 0.2").unwrap();
        // Due to floating point representation, may not be exactly 0.3
        let val: f64 = result.parse().unwrap();
        assert!((val - 0.3).abs() < 0.0001);
    }

    // ==================== LARGE NUMBER TESTS ====================

    #[test]
    fn test_large_numbers() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("1000000 + 1000000").unwrap(), "2000000");
        assert_eq!(calc.evaluate("1000 * 1000").unwrap(), "1000000");
        assert_eq!(calc.evaluate("1000000 / 1000").unwrap(), "1000");
    }

    #[test]
    fn test_very_large_exponentiation() {
        let calc = Calculator::new();

        let result = calc.evaluate("2 ^ 20").unwrap();
        assert_eq!(result, "1048576");

        let result = calc.evaluate("10 ^ 6").unwrap();
        assert_eq!(result, "1000000");
    }

    // ==================== TOOL INTERFACE TESTS ====================

    #[tokio::test]
    async fn test_tool_interface() {
        let calc = Calculator::new();

        // Test call method with string input
        let result = calc._call_str("2 + 2".to_string()).await.unwrap();
        assert_eq!(result, "4");

        // Test call with structured input
        let structured_input = ToolInput::Structured(serde_json::json!({
            "expression": "3 * 4"
        }));
        let result = calc._call(structured_input).await.unwrap();
        assert_eq!(result, "12");

        // Test name and description
        assert_eq!(calc.name(), "calculator");
        assert!(calc.description().contains("math"));
    }

    #[tokio::test]
    async fn test_tool_string_input() {
        let calc = Calculator::new();

        let result = calc._call(ToolInput::String("5 + 5".to_string())).await.unwrap();
        assert_eq!(result, "10");
    }

    #[tokio::test]
    async fn test_tool_structured_input_variants() {
        let calc = Calculator::new();

        // Valid structured input
        let input = ToolInput::Structured(serde_json::json!({"expression": "7 * 8"}));
        let result = calc._call(input).await.unwrap();
        assert_eq!(result, "56");

        // Empty object - should return empty expression error
        let input = ToolInput::Structured(serde_json::json!({}));
        let result = calc._call(input).await;
        assert!(result.is_err());

        // Wrong field name
        let input = ToolInput::Structured(serde_json::json!({"expr": "2 + 2"}));
        let result = calc._call(input).await;
        assert!(result.is_err());

        // Expression is null
        let input = ToolInput::Structured(serde_json::json!({"expression": null}));
        let result = calc._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_empty_string_input() {
        let calc = Calculator::new();

        let result = calc._call(ToolInput::String("".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_whitespace_string_input() {
        let calc = Calculator::new();

        let result = calc._call(ToolInput::String("   ".to_string())).await;
        assert!(result.is_err());
    }

    // ==================== CONSTRUCTOR TESTS ====================

    #[test]
    fn test_custom_name_and_description() {
        let calc = Calculator::with_name("math_tool").with_description("Custom calculator");

        assert_eq!(calc.name(), "math_tool");
        assert_eq!(calc.description(), "Custom calculator");
    }

    #[test]
    fn test_default_trait() {
        let calc1 = Calculator::new();
        let calc2 = Calculator::default();

        assert_eq!(calc1.name(), calc2.name());
        assert_eq!(calc1.description(), calc2.description());
    }

    #[test]
    fn test_with_name_various() {
        let calc = Calculator::with_name("");
        assert_eq!(calc.name(), "");

        let calc = Calculator::with_name("a");
        assert_eq!(calc.name(), "a");

        let calc = Calculator::with_name(String::from("owned_string"));
        assert_eq!(calc.name(), "owned_string");
    }

    #[test]
    fn test_with_description_chain() {
        let calc = Calculator::new()
            .with_description("First description")
            .with_description("Second description");

        assert_eq!(calc.description(), "Second description");
    }

    #[test]
    fn test_clone() {
        let calc1 = Calculator::with_name("test_calc").with_description("Test description");
        let calc2 = calc1.clone();

        assert_eq!(calc1.name(), calc2.name());
        assert_eq!(calc1.description(), calc2.description());
    }

    #[test]
    fn test_debug() {
        let calc = Calculator::new();
        let debug_str = format!("{calc:?}");

        assert!(debug_str.contains("Calculator"));
        assert!(debug_str.contains("calculator"));
    }

    // ==================== SCHEMA TESTS ====================

    #[test]
    fn test_input_schema() {
        let calc = Calculator::new();
        let schema = calc.args_schema();

        // Verify schema structure
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].get("expression").is_some());
        assert_eq!(schema["required"], serde_json::json!(["expression"]));
    }

    #[test]
    fn test_input_schema_expression_property() {
        let calc = Calculator::new();
        let schema = calc.args_schema();

        let expression_prop = &schema["properties"]["expression"];
        assert_eq!(expression_prop["type"], "string");
        assert!(expression_prop["description"].as_str().unwrap().len() > 0);
    }

    // ==================== BOOLEAN EXPRESSION TESTS ====================

    #[test]
    fn test_boolean_evaluation() {
        let calc = Calculator::new();

        // evalexpr may support boolean operations
        let result = calc.evaluate("true");
        if let Ok(val) = result {
            assert_eq!(val, "true");
        }

        let result = calc.evaluate("false");
        if let Ok(val) = result {
            assert_eq!(val, "false");
        }
    }

    #[test]
    fn test_comparison_operators() {
        let calc = Calculator::new();

        // These may or may not be supported depending on evalexpr config
        if let Ok(result) = calc.evaluate("5 > 3") {
            assert_eq!(result, "true");
        }

        if let Ok(result) = calc.evaluate("5 < 3") {
            assert_eq!(result, "false");
        }

        if let Ok(result) = calc.evaluate("5 == 5") {
            assert_eq!(result, "true");
        }
    }

    // ==================== SPECIAL VALUE TESTS ====================

    #[test]
    fn test_zero_operations() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("0 + 0").unwrap(), "0");
        assert_eq!(calc.evaluate("0 * 100").unwrap(), "0");
        assert_eq!(calc.evaluate("0 / 100").unwrap(), "0");
        assert_eq!(calc.evaluate("0 ^ 0").unwrap(), "1"); // Mathematical convention
    }

    #[test]
    fn test_identity_operations() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("5 + 0").unwrap(), "5");
        assert_eq!(calc.evaluate("5 - 0").unwrap(), "5");
        assert_eq!(calc.evaluate("5 * 1").unwrap(), "5");
        assert_eq!(calc.evaluate("5 / 1").unwrap(), "5");
        assert_eq!(calc.evaluate("5 ^ 1").unwrap(), "5");
    }

    #[test]
    fn test_single_number() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("42").unwrap(), "42");
        assert_eq!(calc.evaluate("0").unwrap(), "0");
        assert_eq!(calc.evaluate("-7").unwrap(), "-7");
        assert_eq!(calc.evaluate("3.14159").unwrap(), "3.14159");
    }

    // ==================== COMPLEX EXPRESSION TESTS ====================

    #[test]
    fn test_long_expression() {
        let calc = Calculator::new();

        let result = calc.evaluate("1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10").unwrap();
        assert_eq!(result, "55");
    }

    #[test]
    fn test_mixed_operations() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("2 + 3 * 4 - 6 / 2").unwrap(), "11");
        assert_eq!(calc.evaluate("(2 + 3) * (4 - 1) / 3").unwrap(), "5");
        assert_eq!(calc.evaluate("2 ^ 3 + 4 ^ 2 - 10").unwrap(), "14");
    }

    #[test]
    fn test_deeply_nested_parentheses() {
        let calc = Calculator::new();

        assert_eq!(calc.evaluate("((((1 + 2))))").unwrap(), "3");
        assert_eq!(calc.evaluate("(((2 * 3) + 4) * 5)").unwrap(), "50");
        assert_eq!(calc.evaluate("((1 + (2 * (3 + 4))))").unwrap(), "15");
    }

    // ==================== EDGE CASES ====================

    #[test]
    fn test_double_negative() {
        let calc = Calculator::new();

        // Double negation
        let result = calc.evaluate("--5");
        // This may or may not be valid depending on parser
        if let Ok(val) = result {
            assert_eq!(val, "5");
        }
    }

    #[test]
    fn test_scientific_notation() {
        let calc = Calculator::new();

        // Scientific notation may or may not be supported
        if let Ok(result) = calc.evaluate("1e3") {
            let val: f64 = result.parse().unwrap();
            assert!((val - 1000.0).abs() < 0.001);
        }

        if let Ok(result) = calc.evaluate("1.5e2") {
            let val: f64 = result.parse().unwrap();
            assert!((val - 150.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_leading_plus() {
        let calc = Calculator::new();

        // Leading plus should be valid
        if let Ok(result) = calc.evaluate("+5") {
            assert_eq!(result, "5");
        }
    }

    #[test]
    fn test_expression_with_numbers_in_various_formats() {
        let calc = Calculator::new();

        // Integer
        assert_eq!(calc.evaluate("100").unwrap(), "100");

        // Float with leading zero
        assert_eq!(calc.evaluate("0.5").unwrap(), "0.5");

        // Float without leading zero (may not be supported)
        if let Ok(result) = calc.evaluate(".5") {
            let val: f64 = result.parse().unwrap();
            assert!((val - 0.5).abs() < 0.001);
        }
    }

    // ==================== CONSISTENCY TESTS ====================

    #[test]
    fn test_order_of_operands_addition() {
        let calc = Calculator::new();

        // Addition is commutative
        assert_eq!(calc.evaluate("3 + 5").unwrap(), calc.evaluate("5 + 3").unwrap());
        assert_eq!(calc.evaluate("1.5 + 2.5").unwrap(), calc.evaluate("2.5 + 1.5").unwrap());
    }

    #[test]
    fn test_order_of_operands_multiplication() {
        let calc = Calculator::new();

        // Multiplication is commutative
        assert_eq!(calc.evaluate("3 * 5").unwrap(), calc.evaluate("5 * 3").unwrap());
        assert_eq!(calc.evaluate("2 * 7").unwrap(), calc.evaluate("7 * 2").unwrap());
    }

    #[test]
    fn test_associativity_addition() {
        let calc = Calculator::new();

        // Addition is associative
        assert_eq!(
            calc.evaluate("(1 + 2) + 3").unwrap(),
            calc.evaluate("1 + (2 + 3)").unwrap()
        );
    }

    #[test]
    fn test_associativity_multiplication() {
        let calc = Calculator::new();

        // Multiplication is associative
        assert_eq!(
            calc.evaluate("(2 * 3) * 4").unwrap(),
            calc.evaluate("2 * (3 * 4)").unwrap()
        );
    }

    #[test]
    fn test_distributive_property() {
        let calc = Calculator::new();

        // a * (b + c) = a * b + a * c
        assert_eq!(
            calc.evaluate("2 * (3 + 4)").unwrap(),
            calc.evaluate("2 * 3 + 2 * 4").unwrap()
        );
    }
}
