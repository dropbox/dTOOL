use dashflow::core::tools::Tool;
use dashflow_wikipedia::WikipediaSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Wikipedia Search Tool Example ===\n");

    // Create a basic Wikipedia search tool
    let wiki = WikipediaSearchTool::new();

    // Example 1: Search for a programming language
    println!("Example 1: Searching for 'Rust programming language'");
    match wiki._call_str("Rust programming language".to_string()).await {
        Ok(result) => {
            // Print first 500 characters
            let preview = if result.len() > 500 {
                format!("{}...\n[Truncated for display]", &result[..500])
            } else {
                result
            };
            println!("{}\n", preview);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 2: Search with custom settings
    println!("Example 2: Searching with limited content (1000 chars max)");
    let wiki_short = WikipediaSearchTool::builder()
        .max_chars(1000)
        .load_all_available_meta(true)
        .build();

    match wiki_short._call_str("Albert Einstein".to_string()).await {
        Ok(result) => {
            println!("{}\n", result);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 3: Search for a historical event
    println!("Example 3: Searching for 'Apollo 11'");
    match wiki._call_str("Apollo 11".to_string()).await {
        Ok(result) => {
            let preview = if result.len() > 500 {
                format!("{}...\n[Truncated for display]", &result[..500])
            } else {
                result
            };
            println!("{}\n", preview);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    println!("=== Example Complete ===");
    Ok(())
}
