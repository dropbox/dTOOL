//! Example demonstrating OpenWeatherMap tool usage for weather queries.
//!
//! This example shows how to use the OpenWeatherMap tool to get current weather data
//! for various locations using different query formats.
//!
//! # Setup
//!
//! 1. Get a free API key from: https://openweathermap.org/api
//! 2. Set the environment variable:
//!    ```bash
//!    export OPENWEATHERMAP_API_KEY="your-api-key"
//!    ```
//! 3. Run the example:
//!    ```bash
//!    cargo run --example weather_query
//!    ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_openweathermap::OpenWeatherMapTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key = match std::env::var("OPENWEATHERMAP_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("OPENWEATHERMAP_API_KEY environment variable must be set.");
            println!("Run: export OPENWEATHERMAP_API_KEY=\"your-api-key\"");
            return Ok(());
        }
    };

    println!("=== OpenWeatherMap Tool Demo ===\n");

    // Create tool with metric units (Celsius)
    let tool = OpenWeatherMapTool::new(api_key.clone()).with_units("metric");

    println!("Tool: {}", tool.name());
    println!("Description: {}\n", tool.description());

    // Example 1: Query by city name
    println!("1. Weather in London, UK:");
    println!("   Query: 'London,UK'");
    match tool._call(ToolInput::String("London,UK".to_string())).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Query by city name (US)
    println!("2. Weather in New York, US:");
    println!("   Query: 'New York,US'");
    match tool
        ._call(ToolInput::String("New York,US".to_string()))
        .await
    {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Query by coordinates
    println!("3. Weather at specific coordinates (Paris):");
    println!("   Query: 'lat=48.8566&lon=2.3522'");
    match tool
        ._call(ToolInput::String("lat=48.8566&lon=2.3522".to_string()))
        .await
    {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 4: Query by zip code
    println!("4. Weather by ZIP code (San Francisco):");
    println!("   Query: 'zip=94102,US'");
    match tool
        ._call(ToolInput::String("zip=94102,US".to_string()))
        .await
    {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 5: Imperial units (Fahrenheit)
    println!("5. Weather with Imperial units (Fahrenheit):");
    let tool_imperial = OpenWeatherMapTool::new(api_key.clone()).with_units("imperial");
    println!("   Query: 'Miami,US'");
    match tool_imperial
        ._call(ToolInput::String("Miami,US".to_string()))
        .await
    {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 6: Multiple cities comparison
    println!("6. Comparing weather across multiple cities:");
    let cities = vec!["Tokyo,JP", "Sydney,AU", "Toronto,CA", "Mumbai,IN"];

    for city in cities {
        println!("   {}", city);
        match tool._call(ToolInput::String(city.to_string())).await {
            Ok(result) => {
                // Extract just the temperature line for concise output
                if let Some(temp_line) = result.lines().nth(2) {
                    println!("   {}", temp_line);
                }
            }
            Err(e) => eprintln!("   Error: {}", e),
        }
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}
