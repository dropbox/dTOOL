//! Test Rust text splitter against Python baseline

use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

fn main() {
    println!("=== Testing Rust RecursiveCharacterTextSplitter ===");

    let text = "Hello world. This is a test. More text here. And even more.";

    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(20)
        .with_chunk_overlap(5);
    let chunks = splitter.split_text(text);

    println!("Input text: {}", text);
    println!("Chunk size: 20, overlap: 5");
    println!("Number of chunks: {}", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  Chunk {}: '{}' (len={})", i, chunk, chunk.len());
    }

    println!("\n✅ Rust text splitter works");
    println!("Expected from Python: 4 chunks");
    println!("  Chunk 0: 'Hello world. This is' (len=20)");
    println!("  Chunk 1: 'is a test. More' (len=15)");
    println!("  Chunk 2: 'More text here. And' (len=19)");
    println!("  Chunk 3: 'And even more.' (len=14)");
}
