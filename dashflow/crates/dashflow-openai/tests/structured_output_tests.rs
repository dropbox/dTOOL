//! Integration tests for OpenAI structured output support
//!
//! These tests verify that ChatOpenAI's structured output methods work correctly
//! with the OpenAI API using different methods (JSON mode, JSON schema, function calling).
//!
//! Note: These tests require a valid OPENAI_API_KEY environment variable.

#![allow(clippy::expect_used)]

use dashflow::core::messages::Message;
use dashflow_openai::structured::ChatOpenAIStructuredExt;
use dashflow_openai::{ChatOpenAI, StructuredOutputMethod};
use dashflow_standard_tests::chat_model_tests::test_json_mode_typed;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Test schema matching Python's Joke class from standard tests
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
struct Joke {
    /// Question to set up the joke
    setup: String,
    /// Answer to resolve the joke
    punchline: String,
}

/// Test that JSON mode structured output works with typed schema (Pydantic class equivalent)
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_openai_json_mode_typed() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_structured_output_typed::<Joke>(StructuredOutputMethod::JsonMode)
        .expect("Failed to create structured model");

    // Run standard test
    test_json_mode_typed(
        &model,
        "Tell me a joke about cats. Return the result as a JSON with 'setup' and 'punchline' keys. Return nothing other than JSON.",
        |joke: &Joke| !joke.setup.is_empty() && !joke.punchline.is_empty(),
    )
    .await;
}

/// Test that JSON schema structured output works
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_openai_json_schema_typed() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_structured_output_typed::<Joke>(StructuredOutputMethod::JsonSchema)
        .expect("Failed to create structured model");

    test_json_mode_typed(&model, "Tell me a joke about cats.", |joke: &Joke| {
        !joke.setup.is_empty() && !joke.punchline.is_empty()
    })
    .await;
}

/// Test parsing with the invoke() helper method
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_openai_invoke_with_parsing() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_structured_output_typed::<Joke>(StructuredOutputMethod::JsonMode)
        .expect("Failed to create structured model");

    let messages = vec![Message::human(
        "Tell me a joke about dogs. Return the result as a JSON with 'setup' and 'punchline' keys. Return nothing other than JSON."
    )];

    let result: Joke = model.invoke(&messages).await.expect("Invoke failed");

    // Verify we got a valid joke
    assert!(!result.setup.is_empty(), "Setup should not be empty");
    assert!(
        !result.punchline.is_empty(),
        "Punchline should not be empty"
    );

    println!("Joke setup: {}", result.setup);
    println!("Joke punchline: {}", result.punchline);
}

/// Test with a more complex nested schema
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct Person {
    name: String,
    age: u32,
    address: Address,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct Address {
    street: String,
    city: String,
    country: String,
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_openai_nested_schema() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_structured_output_typed::<Person>(StructuredOutputMethod::JsonMode)
        .expect("Failed to create structured model");

    let messages = vec![Message::human(
        "Extract person info: John Doe, 30 years old, lives at 123 Main St, San Francisco, USA. \
         Return as JSON with name, age, and address (with street, city, country). Return nothing other than JSON."
    )];

    let result: Person = model.invoke(&messages).await.expect("Invoke failed");

    assert_eq!(result.name, "John Doe");
    assert_eq!(result.age, 30);
    assert_eq!(result.address.city, "San Francisco");

    println!("Person: {:?}", result);
}
