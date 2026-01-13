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

    println!("=== Exa Search Tool - Advanced Example ===\n");

    // Example 1: Neural search with category filter
    println!("1. Neural search for research papers:");
    let exa_research = ExaSearchTool::builder()
        .api_key(&api_key)
        .num_results(3)
        .search_type("neural")
        .category("research paper")
        .build()?;

    let results = exa_research
        ._call_str("attention mechanisms in transformers".to_string())
        .await?;
    println!("{}\n", results);

    // Example 2: Keyword search with domain filtering
    println!("2. Keyword search on specific domains:");
    let exa_github = ExaSearchTool::builder()
        .api_key(&api_key)
        .num_results(3)
        .search_type("keyword")
        .include_domains(vec!["github.com".to_string(), "docs.rs".to_string()])
        .build()?;

    let results = exa_github
        ._call_str("rust async web framework".to_string())
        .await?;
    println!("{}\n", results);

    // Example 3: Fast search for quick results
    println!("3. Fast search for recent news:");
    let exa_fast = ExaSearchTool::builder()
        .api_key(&api_key)
        .num_results(5)
        .search_type("fast")
        .category("news")
        .build()?;

    let results = exa_fast
        ._call_str("artificial intelligence breakthroughs 2024".to_string())
        .await?;
    println!("{}", results);

    Ok(())
}
