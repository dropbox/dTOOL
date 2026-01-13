use dashflow::core::tools::Tool;
use dashflow_exa::ExaSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key = match std::env::var("EXA_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("EXA_API_KEY environment variable not set.");
            println!("Run: export EXA_API_KEY=\"your-api-key\"");
            return Ok(());
        }
    };

    // Create the Exa search tool
    let exa = ExaSearchTool::new(api_key);

    println!("=== Exa Search Tool - Basic Example ===\n");

    // Perform a search
    let query = "latest developments in large language models";
    println!("Searching for: {}\n", query);

    let results = exa._call_str(query.to_string()).await?;

    println!("{}", results);

    Ok(())
}
