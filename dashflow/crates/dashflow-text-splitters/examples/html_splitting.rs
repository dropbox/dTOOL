//! Example demonstrating HTMLTextSplitter
//!
//! This example shows how to split HTML documents while preserving tag structure.

use dashflow_text_splitters::{HTMLTextSplitter, TextSplitter};

fn main() {
    // Create an HTMLTextSplitter with custom configuration
    let splitter = HTMLTextSplitter::new()
        .with_chunk_size(150)
        .with_chunk_overlap(30);

    // Sample HTML document
    let html = r#"<html>
<body>
    <h1>Welcome to Web Development</h1>
    <p>Web development is the work involved in developing websites for the Internet or an intranet.</p>

    <h2>Frontend Development</h2>
    <p>Frontend development focuses on the visual elements of a website that users interact with.</p>
    <ul>
        <li>HTML - Structure</li>
        <li>CSS - Styling</li>
        <li>JavaScript - Interactivity</li>
    </ul>

    <h2>Backend Development</h2>
    <p>Backend development focuses on server-side logic, databases, and application architecture.</p>
    <ul>
        <li>Server programming</li>
        <li>Database management</li>
        <li>API development</li>
    </ul>

    <h2>Full Stack Development</h2>
    <p>Full stack developers work on both frontend and backend aspects of web applications.</p>
</body>
</html>"#;

    println!("Original HTML ({} characters):\n{}\n", html.len(), html);
    println!("{}", "=".repeat(80));

    // Split the HTML
    let chunks = splitter.split_text(html);

    println!("\nSplit into {} chunks:\n", chunks.len());

    for (i, chunk) in chunks.iter().enumerate() {
        println!("--- Chunk {} ({} characters) ---", i + 1, chunk.len());
        println!("{}", chunk);
        println!();
    }
}
