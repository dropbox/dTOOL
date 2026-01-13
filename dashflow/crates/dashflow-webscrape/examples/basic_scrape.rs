use dashflow::core::tools::Tool;
use dashflow_webscrape::WebScrapeTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Web Scraping Tool - Basic Example ===\n");

    let scraper = WebScrapeTool::new();

    // Scrape a simple page
    let url = "https://example.com";
    println!("Scraping: {}\n", url);

    let content = scraper._call_str(url.to_string()).await?;

    println!("{}", content);

    Ok(())
}
