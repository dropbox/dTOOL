use dashflow::core::tools::Tool;
use dashflow_tavily::TavilySearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key = match std::env::var("TAVILY_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("TAVILY_API_KEY environment variable not set.");
            eprintln!("Set it to run this example: `export TAVILY_API_KEY=...`");
            return Ok(());
        }
    };

    // Create the Tavily search tool
    let tavily = TavilySearchTool::new(api_key);

    println!("=== Tavily Search Tool - Basic Example ===\n");

    // Perform a search
    let query = "Who is Leo Messi?";
    println!("Searching for: {}\n", query);

    let results = tavily._call_str(query.to_string()).await?;

    println!("{}", results);

    Ok(())
}
