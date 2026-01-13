//! Example demonstrating structured output with LLMs
//!
//! This example shows how to use the `with_structured_output<T>()` API to get
//! type-safe, validated responses from language models.
//!
//! Run with:
//! ```bash
//! OPENAI_API_KEY=your-key cargo run --example structured_output
//! ```

use dashflow::core::language_models::structured::ChatModelStructuredExt;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Example output type: Extract information about a person
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PersonInfo {
    name: String,
    age: u32,
    email: String,
    occupation: String,
}

/// Example output type: Grade whether an answer contains hallucinations
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GradeHallucinations {
    /// True if the answer is grounded in the document, false if it contains hallucinations
    binary_score: bool,
    /// Brief explanation of the reasoning
    reasoning: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Extract structured information from text (provider-agnostic)
    println!("=== Example 1: Information Extraction (Provider-Agnostic) ===\n");

    // Provider-agnostic approach using Arc<dyn ChatModel>
    let llm: Arc<dyn ChatModel> = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4")
            .with_temperature(0.0),
    );

    let structured_llm = ChatModelStructuredExt::with_structured_output::<PersonInfo>(llm)?;

    let messages = vec![Message::human(
        "Extract information about: John Doe is a 30-year-old software engineer. \
         His email is john.doe@example.com",
    )];

    let result: PersonInfo = structured_llm.invoke(&messages).await?;
    println!("Extracted person info:");
    println!("  Name: {}", result.name);
    println!("  Age: {}", result.age);
    println!("  Email: {}", result.email);
    println!("  Occupation: {}", result.occupation);

    // Example 2: Grade hallucinations (provider-agnostic)
    println!("\n=== Example 2: Hallucination Grading (Provider-Agnostic) ===\n");

    let llm2: Arc<dyn ChatModel> = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4")
            .with_temperature(0.0),
    );

    let grader_llm = ChatModelStructuredExt::with_structured_output::<GradeHallucinations>(llm2)?;

    let document = "The Eiffel Tower was built in 1889 for the World's Fair in Paris.";
    let answer = "The Eiffel Tower was built in 1889.";

    let messages = vec![
        Message::system(
            "You are a grader assessing whether an answer is grounded in the document. \
             Grade the answer as 'yes' (grounded) or 'no' (hallucinated).",
        ),
        Message::human(format!(
            "Document: {}\n\nAnswer: {}\n\nIs the answer grounded in the document?",
            document, answer
        )),
    ];

    let result: GradeHallucinations = grader_llm.invoke(&messages).await?;
    println!("Hallucination grading:");
    println!("  Grounded: {}", result.binary_score);
    println!("  Reasoning: {}", result.reasoning);

    // Example 3: Using OpenAI's native structured output (OpenAI-specific)
    // Note: This uses OpenAI-specific API. For provider-agnostic code, use Examples 1 & 2.
    println!("\n=== Example 3: OpenAI Native Structured Output (OpenAI-specific) ===\n");

    use dashflow::core::schema::json_schema::json_schema;

    let schema = json_schema::<PersonInfo>()?;

    let llm3 = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_temperature(0.0)
        .with_structured_output(
            "person_info",
            schema,
            Some("Extract person information".to_string()),
            true, // strict mode
        );

    let messages = vec![Message::human(
        "Extract: Jane Smith, 28, jane@example.com, data scientist",
    )];

    let result = llm3.generate(&messages, None, None, None, None).await?;
    let content = result.first_content();
    println!("Raw response: {}", content);

    // Parse the response manually
    let person: PersonInfo = serde_json::from_str(&content)?;
    println!("Parsed person info:");
    println!("  Name: {}", person.name);
    println!("  Age: {}", person.age);

    println!("\n=== All examples completed successfully! ===");

    Ok(())
}
