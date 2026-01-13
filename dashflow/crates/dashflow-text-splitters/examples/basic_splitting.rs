//! Basic character text splitting example

use dashflow_text_splitters::{CharacterTextSplitter, TextSplitter};

fn main() {
    let splitter = CharacterTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20)
        .with_separator("\n\n");

    let text = r#"This is the first paragraph. It contains some text that we want to split.

This is the second paragraph. It also contains text that needs to be chunked.

This is the third paragraph. We're demonstrating how the text splitter works.

And here's a fourth paragraph to make sure we have enough content."#;

    let chunks = splitter.split_text(text);

    println!("Split text into {} chunks:", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("\nChunk {}:", i + 1);
        println!("{}", chunk);
        println!("Length: {}", chunk.len());
    }
}
