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

    println!("=== Tavily Search Tool - Advanced Example ===\n");

    // Example 1: Basic search with LLM answer
    println!("1. Search with AI-generated answer:");
    let tavily_answer = TavilySearchTool::builder()
        .api_key(&api_key)
        .max_results(3)
        .include_answer(true)
        .build()?;

    let results = tavily_answer
        ._call_str("What are the latest developments in AI?".to_string())
        .await?;
    println!("{}\n", results);

    // Example 2: News search with images
    println!("2. News search with images:");
    let tavily_news = TavilySearchTool::builder()
        .api_key(&api_key)
        .max_results(5)
        .topic("news")
        .include_images(true)
        .build()?;

    let results = tavily_news
        ._call_str("artificial intelligence breakthroughs 2024".to_string())
        .await?;
    println!("{}\n", results);

    // Example 3: Advanced depth search for comprehensive results
    println!("3. Advanced search for in-depth information:");
    let tavily_advanced = TavilySearchTool::builder()
        .api_key(&api_key)
        .max_results(5)
        .search_depth("advanced")
        .include_answer(true)
        .build()?;

    let results = tavily_advanced
        ._call_str("How do transformer models work?".to_string())
        .await?;
    println!("{}", results);

    Ok(())
}
