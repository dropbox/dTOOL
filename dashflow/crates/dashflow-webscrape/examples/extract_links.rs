use dashflow::core::tools::Tool;
use dashflow_webscrape::WebScrapeTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Web Scraping Tool - Link Extraction Example ===\n");

    // Create scraper with link extraction enabled
    let scraper = WebScrapeTool::builder()
        .include_links(true)
        .max_content_length(2000)
        .build();

    // Scrape a page and extract links
    let url = "https://example.com";
    println!("Scraping with link extraction: {}\n", url);

    let content = scraper._call_str(url.to_string()).await?;

    println!("{}", content);

    Ok(())
}
