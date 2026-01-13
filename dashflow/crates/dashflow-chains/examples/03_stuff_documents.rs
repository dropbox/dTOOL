//! Stuff Documents Chain Example
//!
//! Demonstrates combining multiple documents by "stuffing" them all into a single prompt.
//! This is the simplest document combination strategy - concat all docs and send to LLM.
//!
//! Best for: Small numbers of documents that fit within context window
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY="your-key"
//! cargo run --package dashflow-chains --example 03_stuff_documents
//! ```

use dashflow::core::documents::Document;
use dashflow::core::prompts::PromptTemplate;
use dashflow_chains::StuffDocumentsChain;
use dashflow_openai::ChatOpenAI;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Stuff Documents Chain Example ===\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set your OpenAI API key:");
        eprintln!("  export OPENAI_API_KEY='your-key-here'");
        std::process::exit(1);
    }

    // 1. Create sample documents (e.g., research paper sections)
    let documents = vec![
        Document::new(
            "Rust is a systems programming language focused on safety, speed, and concurrency. \
            It achieves memory safety without garbage collection through its ownership system.",
        ),
        Document::new(
            "The ownership system ensures that each value has a single owner at any time. \
            When the owner goes out of scope, the value is dropped. This prevents memory leaks \
            and data races at compile time.",
        ),
        Document::new(
            "Rust's type system and compile-time checks make it ideal for building reliable \
            and efficient software. Major companies like Mozilla, Dropbox, and Microsoft use \
            Rust for production systems.",
        ),
        Document::new(
            "The language provides zero-cost abstractions, meaning high-level features \
            compile down to performant machine code. Rust code can be as fast as C or C++ \
            while providing stronger safety guarantees.",
        ),
    ];

    println!(
        "Created {} documents about Rust programming\n",
        documents.len()
    );

    // 2. Create a language model
    let llm = Arc::new(
        ChatOpenAI::default()
            .with_model("gpt-4o-mini")
            .with_temperature(0.3), // Lower temperature for factual summarization
    );

    // 3. Create the chain with custom prompt
    let prompt = PromptTemplate::from_template(
        "You are a technical writer creating concise documentation.\n\
        \n\
        Summarize the following technical content in 2-3 sentences:\n\
        \n\
        {context}\n\
        \n\
        Summary:",
    )?;

    let chain = StuffDocumentsChain::new_chat(llm)
        .with_prompt(prompt)
        .with_document_variable_name("context")
        .with_document_separator("\n\n---\n\n");

    // 4. Combine and summarize the documents
    println!("Running StuffDocumentsChain...");
    println!("  Strategy: Concatenate all docs → single LLM call\n");

    let (summary, _metadata) = chain.combine_docs(&documents, None).await?;

    println!("=== Results ===\n");
    println!(
        "Input: {} documents ({} total chars)",
        documents.len(),
        documents
            .iter()
            .map(|d| d.page_content.len())
            .sum::<usize>()
    );
    println!();
    println!("Output Summary:");
    println!("{}", summary.trim());
    println!();

    // 5. Demonstrate with additional inputs (e.g., instructions)
    println!("=== Custom Instructions Example ===\n");

    let prompt_with_instructions = PromptTemplate::from_template(
        "You are a {audience_level} technical writer.\n\
        \n\
        Create a summary for a {audience_type} audience:\n\
        \n\
        {context}\n\
        \n\
        Summary:",
    )?;

    let chain2 =
        StuffDocumentsChain::new_chat(Arc::new(ChatOpenAI::default().with_model("gpt-4o-mini")))
            .with_prompt(prompt_with_instructions)
            .with_document_variable_name("context");

    let mut additional_inputs = HashMap::new();
    additional_inputs.insert(
        "audience_level".to_string(),
        "beginner-friendly".to_string(),
    );
    additional_inputs.insert("audience_type".to_string(), "non-technical".to_string());

    let (beginner_summary, _) = chain2
        .combine_docs(&documents, Some(additional_inputs))
        .await?;

    println!("Beginner-friendly summary:");
    println!("{}", beginner_summary.trim());
    println!();

    println!("=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • StuffDocumentsChain concatenates all documents into one prompt");
    println!("  • Simple and efficient for small document sets");
    println!("  • May hit token limits with many/large documents");
    println!("  • Use MapReduceDocumentsChain for large document sets");
    println!("  • Supports additional prompt variables beyond documents");

    Ok(())
}
