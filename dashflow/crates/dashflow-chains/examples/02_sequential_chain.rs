//! Sequential Chain Example
//!
//! Demonstrates chaining multiple steps together where each step can have
//! named inputs and outputs, with outputs from previous steps available to later steps.
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY="your-key"
//! cargo run --package dashflow-chains --example 02_sequential_chain
//! ```

use dashflow_chains::sequential::SequentialChain;
use dashflow::core::error::Error;
use std::collections::HashMap;
use std::error::Error as StdError;

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    println!("=== Sequential Chain Example ===\n");

    // Example: Build a multi-step content creation pipeline
    // Step 1: Generate an outline from a topic
    // Step 2: Expand the outline into sections
    // Step 3: Add a summary and conclusion

    let chain = SequentialChain::builder()
        .input_variables(vec!["topic".to_string()])
        // Step 1: Create outline
        .add_step(
            vec!["topic".to_string()],
            vec!["outline".to_string()],
            |inputs: &HashMap<String, String>| {
                let topic = inputs
                    .get("topic")
                    .ok_or_else(|| Error::InvalidInput("Missing input: topic".to_string()))?;
                println!("Step 1: Generating outline for '{}'", topic);

                let outline = format!(
                    "Outline for {}:\n\
                    1. Introduction\n\
                    2. Key Concepts\n\
                    3. Practical Applications\n\
                    4. Conclusion",
                    topic
                );

                println!("  Generated outline with 4 sections\n");

                let mut result = HashMap::new();
                result.insert("outline".to_string(), outline);
                Ok(result)
            },
        )
        // Step 2: Expand outline into detailed sections
        .add_step(
            vec!["topic".to_string(), "outline".to_string()],
            vec!["sections".to_string()],
            |inputs: &HashMap<String, String>| {
                let topic = inputs
                    .get("topic")
                    .ok_or_else(|| Error::InvalidInput("Missing input: topic".to_string()))?;
                let outline = inputs
                    .get("outline")
                    .ok_or_else(|| Error::InvalidInput("Missing input: outline".to_string()))?;
                println!("Step 2: Expanding outline into detailed sections");

                let sections = format!(
                    "Detailed Sections for {}:\n\n\
                    INTRODUCTION:\n\
                    {} is a fascinating subject that has gained significant importance...\n\n\
                    KEY CONCEPTS:\n\
                    The fundamental principles of {} include...\n\n\
                    PRACTICAL APPLICATIONS:\n\
                    In real-world scenarios, {} can be applied to...\n\n\
                    Based on: {}",
                    topic, topic, topic, topic, outline
                );

                println!("  Expanded 4 sections\n");

                let mut result = HashMap::new();
                result.insert("sections".to_string(), sections);
                Ok(result)
            },
        )
        // Step 3: Add conclusion and metadata
        .add_step(
            vec!["topic".to_string(), "sections".to_string()],
            vec!["final_document".to_string(), "metadata".to_string()],
            |inputs: &HashMap<String, String>| {
                let topic = inputs
                    .get("topic")
                    .ok_or_else(|| Error::InvalidInput("Missing input: topic".to_string()))?;
                let sections = inputs
                    .get("sections")
                    .ok_or_else(|| Error::InvalidInput("Missing input: sections".to_string()))?;
                println!("Step 3: Adding conclusion and metadata");

                let final_doc = format!(
                    "{}\n\n\
                    CONCLUSION:\n\
                    In summary, {} represents an important area of study with wide-ranging implications...\n\n\
                    [END OF DOCUMENT]",
                    sections, topic
                );

                let metadata = format!(
                    "Document Metadata:\n\
                    - Topic: {}\n\
                    - Sections: 4\n\
                    - Word count: ~150\n\
                    - Generated: Example pipeline",
                    topic
                );

                println!("  Added conclusion and metadata\n");

                let mut result = HashMap::new();
                result.insert("final_document".to_string(), final_doc);
                result.insert("metadata".to_string(), metadata);
                Ok(result)
            },
        )
        .output_variables(vec![
            "final_document".to_string(),
            "metadata".to_string(),
        ])
        .build()?;

    // Run the chain
    let mut inputs = HashMap::new();
    inputs.insert("topic".to_string(), "Machine Learning".to_string());

    println!("Running pipeline...\n");
    let result = chain.run(&inputs).await?;

    // Display results
    println!("=== Final Results ===\n");

    if let Some(doc) = result.get("final_document") {
        println!("FINAL DOCUMENT:");
        println!("{}", doc);
        println!();
    }

    if let Some(meta) = result.get("metadata") {
        println!("{}", meta);
    }

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • Sequential chains enable multi-step processing");
    println!("  • Each step has named inputs and outputs");
    println!("  • Outputs from all previous steps are available to later steps");
    println!("  • Chain validates data flow at build time");

    Ok(())
}
