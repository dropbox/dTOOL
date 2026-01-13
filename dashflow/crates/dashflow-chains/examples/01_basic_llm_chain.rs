//! Basic LLM Chain Example
//!
//! Demonstrates the simplest chain pattern: prompt formatting + LLM execution.
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY="your-key"
//! cargo run --package dashflow-chains --example 01_basic_llm_chain
//! ```

use dashflow::core::prompts::ChatPromptTemplate;
use dashflow_chains::ChatLLMChain;
use dashflow_openai::ChatOpenAI;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Basic LLM Chain Example ===\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set your OpenAI API key:");
        eprintln!("  export OPENAI_API_KEY='your-key-here'");
        std::process::exit(1);
    }

    // 1. Create a language model
    let model = Arc::new(
        ChatOpenAI::default()
            .with_model("gpt-3.5-turbo")
            .with_temperature(0.7),
    );

    // 2. Create a prompt template
    let prompt = ChatPromptTemplate::from_messages(vec![
        (
            "system",
            "You are a helpful assistant providing interesting facts.",
        ),
        ("human", "Tell me an interesting fact about {topic}."),
    ])?;

    println!("Prompt template: Chat with system + human messages");
    println!();

    // 3. Create the chain
    let chain = ChatLLMChain::new(model, prompt);

    // 4. Run the chain with different inputs
    let topics = vec!["Rust programming", "black holes", "ancient Rome"];

    for topic in topics {
        println!("Topic: {}", topic);
        println!("---");

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), topic.to_string());

        let result = chain.run(&inputs).await?;
        println!("{}", result.trim());
        println!();
    }

    println!("=== Example Complete ===");
    Ok(())
}
