use dashflow::core::tools::Tool;
use dashflow_calculator::Calculator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Calculator Tool Example ===\n");

    // Create a calculator tool
    let calc = Calculator::new();

    // Example 1: Basic arithmetic
    println!("Example 1: Basic Arithmetic");
    println!("Expression: 2 + 3 * 4");
    match calc._call_str("2 + 3 * 4".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 2: Complex expression with parentheses
    println!("Example 2: Complex Expression");
    println!("Expression: (10 + 5) * 2 - 8 / 4");
    match calc._call_str("(10 + 5) * 2 - 8 / 4".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 3: Exponentiation
    println!("Example 3: Exponentiation");
    println!("Expression: 2^8");
    match calc._call_str("2^8".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 4: Division with floating point result
    println!("Example 4: Division");
    println!("Expression: 22 / 7");
    match calc._call_str("22 / 7".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 5: Modulo operation
    println!("Example 5: Modulo");
    println!("Expression: 17 % 5");
    match calc._call_str("17 % 5".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 6: Error handling - invalid expression
    println!("Example 6: Error Handling");
    println!("Expression: 2 +* 3 (invalid)");
    match calc._call_str("2 +* 3".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error (as expected): {}\n", e);
        }
    }

    // Example 7: Custom calculator with different name/description
    println!("Example 7: Custom Calculator");
    let custom_calc = Calculator::with_name("MathEvaluator")
        .with_description("Evaluates mathematical expressions with custom settings");

    println!("Tool Name: {}", custom_calc.name());
    println!("Description: {}", custom_calc.description());
    println!("Expression: 100 / (2 + 3)");
    match custom_calc._call_str("100 / (2 + 3)".to_string()).await {
        Ok(result) => {
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 8: Using the Tool interface with ToolInput
    println!("Example 8: Using ToolInput");
    use dashflow::core::tools::ToolInput;
    let input = ToolInput::String("3.14 * 2".to_string());
    match calc._call(input).await {
        Ok(result) => {
            println!("Expression: 3.14 * 2");
            println!("Result: {}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    println!("=== Example Complete ===");
    Ok(())
}
