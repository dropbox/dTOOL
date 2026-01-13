//! Recursive character text splitting example

use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

fn main() {
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(150)
        .with_chunk_overlap(30);

    let text = r#"This is a long document that needs to be split into smaller chunks.

The recursive text splitter will try to split on double newlines first, then single newlines, then spaces, and finally individual characters if needed.

This ensures that we keep semantically related text together as much as possible while respecting the chunk size limit.

Here's another paragraph to demonstrate the splitting behavior. The splitter maintains chunk overlap to ensure context isn't lost between chunks.

And a final paragraph to round out the example."#;

    let chunks = splitter.split_text(text);

    println!("Split text into {} chunks:", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("\nChunk {}:", i + 1);
        println!("{}", chunk);
        println!("Length: {}", chunk.len());
    }
}
