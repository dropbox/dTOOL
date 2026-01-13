//! Calculator tool for `DashFlow` Rust.
//!
//! This crate provides a `Calculator` tool that can evaluate mathematical expressions
//! using the `evalexpr` library. It's designed to be used by LLMs to perform
//! mathematical calculations they cannot do natively.
//!
//! # Features
//!
//! - Evaluate basic arithmetic operations (+, -, *, /, %, ^)
//! - Support for parentheses and operator precedence
//! - Safe evaluation with error handling
//! - Async/await API
//!
//! # Example
//!
//! ```rust
//! use dashflow_calculator::Calculator;
//! use dashflow::core::tools::Tool;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let calculator = Calculator::new();
//!
//!     // Evaluate a simple expression
//!     let result = calculator._call_str("2 + 2 * 3".to_string()).await?;
//!     assert_eq!(result, "8");
//!
//!     // More complex expressions with parentheses
//!     let result = calculator._call_str("(10 + 5) * 2".to_string()).await?;
//!     assert_eq!(result, "30");
//!
//!     // Exponentiation
//!     let result = calculator._call_str("2 ^ 3".to_string()).await?;
//!     assert_eq!(result, "8");
//!
//!     Ok(())
//! }
//! ```
//!
//! # Supported Operations
//!
//! ## Arithmetic Operators
//! - Addition: `+`
//! - Subtraction: `-`
//! - Multiplication: `*`
//! - Division: `/`
//! - Modulo: `%`
//! - Exponentiation: `^`
//!
//! ## Grouping
//! - Parentheses: `(` and `)` for controlling order of operations
//!
//! # Safety
//!
//! The calculator is designed to safely evaluate mathematical expressions without
//! allowing arbitrary code execution. It uses the `evalexpr` library which provides
//! safe expression evaluation.
//!
//! # See Also
//!
//! - [`dashflow::core::tools::Tool`] - The trait this implements
//! - [`dashflow-shell-tool`](https://docs.rs/dashflow-shell-tool) - Shell command execution tool
//! - [`dashflow-json-tool`](https://docs.rs/dashflow-json-tool) - JSON parsing and querying tool
//! - [evalexpr Documentation](https://docs.rs/evalexpr/) - Underlying expression evaluator

mod calculator;

pub use calculator::Calculator;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // This test just ensures the crate compiles
    }
}
