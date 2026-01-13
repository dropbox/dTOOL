# dashflow-calculator

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)

Mathematical expression evaluation tool for DashFlow - designed for LLM-powered applications that need to perform calculations.

## Overview

The `dashflow-calculator` crate provides a safe calculator tool that evaluates mathematical expressions. It's designed to be used by Large Language Models (LLMs) to perform mathematical calculations they cannot do natively or reliably.

### Key Features

- **Safe Expression Evaluation**: Uses the `evalexpr` library for secure mathematical expression parsing and evaluation
- **Comprehensive Operator Support**: Addition, subtraction, multiplication, division, modulo, and exponentiation
- **Proper Precedence Handling**: Respects mathematical operator precedence rules
- **Parentheses Support**: Allows grouping and explicit order of operations control
- **Tool Trait Implementation**: Integrates seamlessly with DashFlow's tool system
- **Async/Await API**: Built for modern async Rust applications
- **Error Handling**: Provides clear error messages for invalid expressions

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
dashflow-calculator = "1.11"
dashflow = "1.11"
```

## Quick Start

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a calculator tool
    let calculator = Calculator::new();

    // Evaluate a mathematical expression
    let result = calculator.call_str("2 + 2 * 3".to_string()).await?;
    println!("Result: {}", result); // Output: "8"

    Ok(())
}
```

## Supported Operations

### Arithmetic Operators

| Operator | Description | Example | Result |
|----------|-------------|---------|--------|
| `+` | Addition | `5 + 3` | `8` |
| `-` | Subtraction | `10 - 4` | `6` |
| `*` | Multiplication | `3 * 4` | `12` |
| `/` | Division | `15 / 3` | `5` |
| `%` | Modulo (remainder) | `17 % 5` | `2` |
| `^` | Exponentiation | `2 ^ 3` | `8` |

### Grouping

- **Parentheses**: `(` and `)` for controlling order of operations

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator::new();

    // Without parentheses (multiplication first)
    let result = calc.call_str("2 + 3 * 4".to_string()).await?;
    assert_eq!(result, "14");

    // With parentheses (addition first)
    let result = calc.call_str("(2 + 3) * 4".to_string()).await?;
    assert_eq!(result, "20");

    Ok(())
}
```

## Usage Examples

### Basic Arithmetic

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator::new();

    // Simple operations
    let result = calc.call_str("10 + 5".to_string()).await?;
    assert_eq!(result, "15");

    let result = calc.call_str("20 / 4".to_string()).await?;
    assert_eq!(result, "5");

    let result = calc.call_str("7 % 3".to_string()).await?;
    assert_eq!(result, "1");

    Ok(())
}
```

### Complex Expressions

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator::new();

    // Complex expression with multiple operators
    let result = calc.call_str("(10 + 5) * 2 - 8 / 4".to_string()).await?;
    assert_eq!(result, "28");

    // Exponentiation in complex expression
    let result = calc.call_str("2 ^ 3 + 4 * 5".to_string()).await?;
    assert_eq!(result, "28");

    Ok(())
}
```

### Error Handling

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() {
    let calc = Calculator::new();

    // Invalid expression
    match calc.call_str("2 +".to_string()).await {
        Ok(result) => println!("Result: {}", result),
        Err(e) => println!("Error: {}", e), // Will print error message
    }

    // Empty expression
    match calc.call_str("".to_string()).await {
        Ok(result) => println!("Result: {}", result),
        Err(e) => println!("Error: Expression cannot be empty"),
    }
}
```

### Custom Name and Description

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator::with_name("math_evaluator")
        .with_description("A powerful mathematical expression evaluator");

    println!("Tool Name: {}", calc.name());
    println!("Description: {}", calc.description());

    let result = calc.call_str("100 / (2 + 3)".to_string()).await?;
    assert_eq!(result, "20");

    Ok(())
}
```

### Using ToolInput

The calculator supports both string and structured input formats:

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::{Tool, ToolInput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator::new();

    // String input
    let input = ToolInput::String("3.14 * 2".to_string());
    let result = calc.call(input).await?;
    println!("Result: {}", result);

    // Structured input
    let input = ToolInput::Structured(serde_json::json!({
        "expression": "5 + 3"
    }));
    let result = calc.call(input).await?;
    assert_eq!(result, "8");

    Ok(())
}
```

### Integration with LLM Agents

```rust
use dashflow_calculator::Calculator;
use dashflow::core::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create calculator tool
    let calculator = Calculator::new();

    // Get tool metadata for LLM
    println!("Name: {}", calculator.name());
    println!("Description: {}", calculator.description());

    // Get JSON schema for arguments
    let schema = calculator.args_schema();
    println!("Schema: {}", serde_json::to_string_pretty(&schema)?);

    // LLM can now call the tool with expressions
    let llm_expression = "(100 - 20) * 0.5";
    let result = calculator.call_str(llm_expression.to_string()).await?;
    println!("Calculation result: {}", result);

    Ok(())
}
```

## API Documentation

### `Calculator`

The main calculator struct that implements the `Tool` trait.

#### Constructors

- `Calculator::new()` - Creates a calculator with default name and description
- `Calculator::with_name(name)` - Creates a calculator with a custom name

#### Methods

- `with_description(description)` - Sets a custom description (builder pattern)
- `evaluate(expression: &str) -> Result<String>` - Evaluates a mathematical expression synchronously
- `call(input: ToolInput) -> Result<String>` - Async tool interface (from `Tool` trait)
- `call_str(input: String) -> Result<String>` - Convenience method for string input
- `name() -> &str` - Returns the tool name
- `description() -> &str` - Returns the tool description
- `args_schema() -> serde_json::Value` - Returns JSON schema for tool arguments

## Number Formatting

The calculator intelligently formats numbers:

- **Whole numbers**: Displayed without decimal points (e.g., `4` not `4.0`)
- **Decimal numbers**: Displayed with appropriate precision (e.g., `3.333333333333`)

```rust
use dashflow_calculator::Calculator;

let calc = Calculator::new();

// Whole number result
let result = calc.evaluate("4.0 + 0.0").unwrap();
assert_eq!(result, "4");

// Decimal result
let result = calc.evaluate("10.0 / 3.0").unwrap();
assert!(result.starts_with("3.33"));
```

## Safety

The calculator is designed to safely evaluate mathematical expressions without allowing arbitrary code execution. It uses the `evalexpr` library which provides:

- **No system access**: Cannot execute shell commands or access files
- **Sandboxed evaluation**: Expression parsing and evaluation happen in a controlled environment
- **Type safety**: Only mathematical operations on numeric types are allowed
- **Error handling**: Invalid expressions result in clear error messages, not panics

## Use Cases

1. **LLM Agent Tools**: Enable language models to perform accurate calculations
2. **Formula Evaluation**: Dynamically evaluate user-provided mathematical formulas
3. **Expression Parsing**: Parse and compute mathematical expressions from text
4. **Educational Applications**: Calculator backend for learning applications
5. **API Endpoints**: Safe calculation endpoints that accept user input

## Running Examples

The crate includes a comprehensive example demonstrating all features:

```bash
cargo run --example calculator_tool
```

This example demonstrates:
- Basic arithmetic operations
- Complex expressions with parentheses
- Exponentiation
- Floating-point division
- Modulo operations
- Error handling
- Custom names and descriptions
- Tool interface usage

## Testing

The crate includes comprehensive tests covering:
- Basic arithmetic operations
- Operator precedence
- Exponentiation
- Complex expressions
- Whitespace handling
- Error cases
- Float formatting
- Tool interface
- Custom configuration
- Input schemas

Run the tests:

```bash
cargo test --package dashflow-calculator
```

## Performance

The calculator uses the `evalexpr` library for expression evaluation, which provides:
- Fast parsing and evaluation
- Minimal memory overhead
- No compilation or JIT overhead
- Suitable for high-frequency calculations

## Limitations

- **No variables**: The calculator evaluates expressions with literal values only (no variable substitution)
- **No functions**: Built-in mathematical functions (sin, cos, log, etc.) are not supported in the default configuration
- **Numeric only**: Only numeric types are supported (no string concatenation or boolean logic)
- **Integer/Float operations**: Operations are performed using Rust's numeric types (standard precision)

## Error Messages

The calculator provides clear error messages for common issues:

```rust
use dashflow_calculator::Calculator;

let calc = Calculator::new();

// Empty expression
assert!(calc.evaluate("").is_err()); // "Expression cannot be empty"

// Incomplete expression
assert!(calc.evaluate("2 +").is_err()); // "Failed to evaluate expression..."

// Invalid syntax
assert!(calc.evaluate("2 +* 3").is_err()); // "Failed to evaluate expression..."

// Unknown identifier
assert!(calc.evaluate("x + 2").is_err()); // "Failed to evaluate expression..."
```

## Related Crates

- **dashflow**: Core DashFlow functionality including the `Tool` trait
- **dashflow-chains**: Chain composition utilities for building complex workflows
- **dashflow-openai**: OpenAI LLM integration for agent-based applications

## Contributing

Contributions are welcome! Please see the main [DashFlow repository](https://github.com/dropbox/dTOOL/dashflow) for contribution guidelines.

## License

This crate is part of the DashFlow project and is licensed under the MIT License. See the [LICENSE](../../LICENSE) file for details.

## Version

Current version: **1.11**

## Support

- **Documentation**: See source code and this README (crate not published to crates.io)
- **Issues**: [GitHub Issues](https://github.com/dropbox/dTOOL/dashflow/issues)
- **Discussions**: [GitHub Discussions](https://github.com/dropbox/dTOOL/dashflow/discussions)
