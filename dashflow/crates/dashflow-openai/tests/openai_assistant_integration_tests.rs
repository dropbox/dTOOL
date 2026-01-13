//! Integration tests for OpenAI Assistant API
//!
//! These tests verify the OpenAI Assistant integration with real API calls.
//!
//! ## Test Coverage
//!
//! 1. **Basic Operations:**
//!    - `test_assistant_basic_creation`: Verify assistant creation
//!    - `test_assistant_simple_invocation`: Simple query/response
//!
//! 2. **Built-in Tools:**
//!    - `test_assistant_with_code_interpreter`: Code execution capability
//!    - `test_assistant_with_file_search`: Document search capability
//!    - `test_assistant_multiple_tools`: Multiple tools in one assistant
//!
//! 3. **Conversation Management:**
//!    - `test_assistant_thread_continuation`: Thread reuse across invocations
//!
//! 4. **Agent Mode:**
//!    - `test_assistant_agent_mode`: Integration with AgentExecutor framework
//!
//! 5. **Configuration:**
//!    - `test_assistant_with_custom_check_interval`: Custom polling frequency
//!    - `test_assistant_serialization`: Serialization behavior
//!
//! 6. **Run Parameter Overrides:**
//!    - `test_assistant_run_parameter_overrides`: temperature, max_tokens, additional_instructions
//!    - `test_assistant_with_metadata`: run_metadata support
//!    - `test_assistant_parallel_tool_calls`: parallel_tool_calls parameter
//!
//! 7. **AgentExecutor Integration:**
//!    - `test_assistant_with_intermediate_steps`: Full intermediate_steps workflow
//!    - `test_assistant_intermediate_steps_error_handling`: Error handling for malformed steps
//!
//! 8. **Message Attachments:**
//!    - `test_assistant_with_message_attachments`: Empty attachments array acceptance
//!    - `test_assistant_attachments_format`: Attachment format validation
//!
//! 9. **Error Handling:**
//!    - `test_assistant_error_handling`: Validation of error cases
//!
//! ## Prerequisites
//! - OPENAI_API_KEY environment variable must be set
//!
//! ## Running Tests
//! ```bash
//! # Run all tests (ignored by default)
//! cargo test --test openai_assistant_integration_tests -- --ignored
//!
//! # Run specific test
//! cargo test --test openai_assistant_integration_tests test_assistant_simple_invocation -- --ignored
//! ```

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use dashflow::core::runnable::Runnable;
use dashflow_openai::{AssistantOutput, OpenAIAssistantRunnable};
use std::collections::HashMap;

/// Helper to check if OpenAI API key is available
fn has_openai_key() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_basic_creation() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant with code_interpreter
    let result = OpenAIAssistantRunnable::create_assistant(
        "Test Math Tutor",
        "You are a helpful math tutor.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await;

    assert!(result.is_ok(), "Failed to create assistant");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_simple_invocation() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Assistant",
        "You are a helpful assistant.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Simple query without tools
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("What is the capital of France?"),
    );

    let result = assistant.invoke(input, None).await;
    assert!(result.is_ok(), "Failed to invoke assistant: {:?}", result);

    let output = result.unwrap();
    match output {
        AssistantOutput::Messages(messages) => {
            assert!(!messages.is_empty(), "Expected non-empty messages");
            // Check for Paris in the response
            let messages_str = format!("{:?}", messages);
            assert!(
                messages_str.to_lowercase().contains("paris"),
                "Expected 'Paris' in response"
            );
        }
        _ => panic!("Expected Messages output, got: {:?}", output),
    }
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_with_code_interpreter() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant with code_interpreter tool
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Code Assistant",
        "You are a helpful assistant that can write and run code.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Ask to calculate something using code
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("Calculate the sum of all numbers from 1 to 100 using code"),
    );

    let result = assistant.invoke(input, None).await;
    assert!(result.is_ok(), "Failed to invoke assistant: {:?}", result);

    let output = result.unwrap();
    match output {
        AssistantOutput::Messages(messages) => {
            assert!(!messages.is_empty(), "Expected non-empty messages");
            // Check for correct answer (5050)
            let messages_str = format!("{:?}", messages);
            assert!(
                messages_str.contains("5050") || messages_str.contains("5,050"),
                "Expected '5050' in response"
            );
        }
        _ => panic!("Expected Messages output, got: {:?}", output),
    }
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_thread_continuation() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Conversation Assistant",
        "You are a helpful assistant that maintains conversation context.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // First message
    let mut input1 = HashMap::new();
    input1.insert(
        "content".to_string(),
        serde_json::json!("My favorite color is blue."),
    );

    let result1 = assistant.invoke(input1, None).await;
    assert!(result1.is_ok(), "First invocation failed: {:?}", result1);

    // Extract thread_id from first response
    let thread_id = match result1.unwrap() {
        AssistantOutput::Finish(finish) => finish.thread_id,
        AssistantOutput::Messages(messages) => {
            panic!(
                "Expected Finish output to extract thread_id, got Messages: {:?}",
                messages
            );
        }
        _ => panic!("Unexpected output type"),
    };

    // Second message in same thread
    let mut input2 = HashMap::new();
    input2.insert(
        "content".to_string(),
        serde_json::json!("What is my favorite color?"),
    );
    input2.insert("thread_id".to_string(), serde_json::json!(thread_id));

    let result2 = assistant.invoke(input2, None).await;
    assert!(result2.is_ok(), "Second invocation failed: {:?}", result2);

    // Verify the assistant remembers the context
    let output2 = result2.unwrap();
    let output_str = format!("{:?}", output2);
    assert!(
        output_str.to_lowercase().contains("blue"),
        "Expected assistant to remember favorite color is blue"
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_agent_mode() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant in agent mode
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Agent",
        "You are a helpful agent.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant")
    .with_as_agent(true);

    // Invoke in agent mode
    let mut input = HashMap::new();
    input.insert("content".to_string(), serde_json::json!("What is 2 + 2?"));

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke in agent mode: {:?}",
        result
    );

    let output = result.unwrap();
    match output {
        AssistantOutput::Finish(_) => {
            // Expected: agent finished with result
        }
        AssistantOutput::Actions(actions) => {
            // Also valid: agent wants to take actions
            assert!(!actions.is_empty(), "Expected non-empty actions");
        }
        _ => panic!(
            "Expected Finish or Actions output in agent mode, got: {:?}",
            output
        ),
    }
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_with_file_search() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant with file_search tool
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Search Assistant",
        "You are a helpful assistant that can search documents.",
        vec![serde_json::json!({"type": "file_search"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Simple query (without actual files, it should still work)
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("What can you help me with?"),
    );

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke assistant with file_search: {:?}",
        result
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_multiple_tools() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant with both code_interpreter and file_search
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Multi-Tool Assistant",
        "You are a helpful assistant with multiple capabilities.",
        vec![
            serde_json::json!({"type": "code_interpreter"}),
            serde_json::json!({"type": "file_search"}),
        ],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Query that might use code_interpreter
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("Calculate the factorial of 5"),
    );

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke multi-tool assistant: {:?}",
        result
    );

    let output = result.unwrap();
    let output_str = format!("{:?}", output);
    assert!(
        output_str.contains("120"),
        "Expected factorial result (120) in output"
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_error_handling() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Error Assistant",
        "You are a helpful assistant.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Try to invoke with missing content
    let input = HashMap::new();

    let result = assistant.invoke(input, None).await;
    // This should error because content is required for new threads
    assert!(result.is_err(), "Expected error for missing content");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_with_custom_check_interval() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant with custom polling interval
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Custom Interval",
        "You are a helpful assistant.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant")
    .with_check_every_ms(500); // Check every 500ms instead of default 1000ms

    let mut input = HashMap::new();
    input.insert("content".to_string(), serde_json::json!("Hello!"));

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke with custom interval: {:?}",
        result
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_serialization() {
    use dashflow::core::serialization::Serializable;

    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Serialization",
        "You are a helpful assistant.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Check serialization behavior
    assert!(
        !assistant.is_lc_serializable(),
        "Assistant should not be serializable (contains client)"
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_run_parameter_overrides() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Parameter Override",
        "You are a helpful assistant.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Invoke with custom run parameters
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("Say 'hello' once."),
    );
    input.insert("temperature".to_string(), serde_json::json!(0.1));
    input.insert("max_completion_tokens".to_string(), serde_json::json!(50));
    input.insert(
        "additional_instructions".to_string(),
        serde_json::json!("Be extremely brief."),
    );

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke with parameter overrides: {:?}",
        result
    );

    // Verify response exists (actual content will vary due to temperature)
    let output = result.unwrap();
    match output {
        AssistantOutput::Messages(messages) => {
            assert!(!messages.is_empty(), "Expected non-empty messages");
        }
        AssistantOutput::Finish(finish) => {
            assert!(!finish.output.is_empty(), "Expected non-empty output");
        }
        _ => {}
    }
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_with_metadata() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Metadata",
        "You are a helpful assistant.",
        vec![],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Invoke with run metadata
    let mut input = HashMap::new();
    input.insert("content".to_string(), serde_json::json!("Hello!"));
    input.insert(
        "run_metadata".to_string(),
        serde_json::json!({
            "test_case": "metadata_test",
            "version": "1.0"
        }),
    );

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke with metadata: {:?}",
        result
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_parallel_tool_calls() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create assistant with code_interpreter
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Parallel Tools",
        "You are a helpful assistant.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Invoke with parallel_tool_calls enabled
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("Calculate both 5+5 and 10*10"),
    );
    input.insert("parallel_tool_calls".to_string(), serde_json::json!(true));

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke with parallel_tool_calls: {:?}",
        result
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_with_intermediate_steps() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // This test simulates AgentExecutor's intermediate_steps workflow
    // Note: This is a unit-level test of the intermediate_steps parsing logic
    // Full AgentExecutor integration requires custom tools, which is tested separately

    // Create assistant in agent mode with code_interpreter
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Agent with Steps",
        "You are a helpful agent. Use code_interpreter to solve problems.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant")
    .with_as_agent(true);

    // Step 1: Initial invocation - ask a question that requires tool use
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("Calculate the square root of 144"),
    );

    let result = assistant.invoke(input, None).await;
    assert!(result.is_ok(), "Failed initial invocation: {:?}", result);

    let output = result.unwrap();
    match output {
        AssistantOutput::Actions(actions) => {
            // Got tool calls - this is what we expect
            assert!(!actions.is_empty(), "Expected at least one action");

            // Extract metadata from first action
            let first_action = &actions[0];
            let run_id = first_action.run_id.clone();
            let thread_id = first_action.thread_id.clone();
            let tool_call_id = first_action.tool_call_id.clone();

            // Step 2: Simulate AgentExecutor collecting intermediate steps
            // Format: array of [action, output] tuples
            let intermediate_steps = serde_json::json!([
                [
                    {
                        "tool": first_action.tool,
                        "tool_input": first_action.tool_input,
                        "tool_call_id": tool_call_id,
                        "run_id": run_id,
                        "thread_id": thread_id,
                        "log": "",
                    },
                    "12.0" // Simulated tool output
                ]
            ]);

            // Step 3: Submit intermediate_steps back to assistant
            let mut followup_input = HashMap::new();
            followup_input.insert("intermediate_steps".to_string(), intermediate_steps);

            let followup_result = assistant.invoke(followup_input, None).await;
            assert!(
                followup_result.is_ok(),
                "Failed to process intermediate_steps: {:?}",
                followup_result
            );

            // Should complete or request more actions
            match followup_result.unwrap() {
                AssistantOutput::Finish(finish) => {
                    // Expected: assistant finished with result
                    assert!(!finish.output.is_empty(), "Expected non-empty output");
                }
                AssistantOutput::Actions(_) => {
                    // Also valid: assistant needs more tool calls
                }
                _ => panic!("Expected Finish or Actions after submitting intermediate_steps"),
            }
        }
        AssistantOutput::Finish(_) => {
            // Assistant completed without needing tools - this is fine
            // (GPT-4 might answer sqrt(144) directly without code)
            eprintln!("Note: Assistant answered without using tools");
        }
        _ => panic!("Expected Actions or Finish output in agent mode"),
    }
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_intermediate_steps_error_handling() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Test error handling for malformed intermediate_steps

    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Error Handling",
        "You are a test assistant.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant")
    .with_as_agent(true);

    // Test 1: Empty intermediate_steps
    let mut input = HashMap::new();
    input.insert("intermediate_steps".to_string(), serde_json::json!([]));

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_err(),
        "Expected error for empty intermediate_steps"
    );

    // Test 2: Malformed intermediate_steps (not an array)
    let mut input = HashMap::new();
    input.insert(
        "intermediate_steps".to_string(),
        serde_json::json!("not an array"),
    );

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_err(),
        "Expected error for non-array intermediate_steps"
    );

    // Test 3: Malformed step structure (not [action, output])
    let mut input = HashMap::new();
    input.insert(
        "intermediate_steps".to_string(),
        serde_json::json!([["only_one_element"]]),
    );

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_err(),
        "Expected error for malformed step structure"
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY and file upload"]
async fn test_assistant_with_message_attachments() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Note: This test validates the attachments parameter is accepted
    // Full end-to-end testing would require file upload, which is beyond scope here

    // Create assistant with file_search tool
    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Attachments Assistant",
        "You are a helpful assistant that can search documents.",
        vec![serde_json::json!({"type": "file_search"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Test with empty attachments array (should be accepted)
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("What can you help me with?"),
    );
    input.insert("attachments".to_string(), serde_json::json!([]));

    let result = assistant.invoke(input, None).await;
    assert!(
        result.is_ok(),
        "Failed to invoke with empty attachments: {:?}",
        result
    );

    // Note: Testing with actual file_id would require:
    // 1. Upload file via OpenAI files API
    // 2. Get file_id from upload response
    // 3. Pass file_id in attachments
    // 4. Clean up file after test
    // This is left for manual testing with real files
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_assistant_attachments_format() {
    // This test validates attachment format parsing without API calls
    // The actual API behavior is tested in test_assistant_with_message_attachments

    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Test Format",
        "Test assistant.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await
    .expect("Failed to create assistant");

    // Test various attachment formats

    // Format 1: Single attachment with code_interpreter
    let mut input1 = HashMap::new();
    input1.insert("content".to_string(), serde_json::json!("Test"));
    input1.insert(
        "attachments".to_string(),
        serde_json::json!([
            {
                "file_id": "file-abc123",
                "tools": [{"type": "code_interpreter"}]
            }
        ]),
    );

    let result1 = assistant.invoke(input1, None).await;
    // Should either succeed or fail with OpenAI error (not parsing error)
    // Parsing errors would indicate our code is broken
    if let Err(e) = &result1 {
        let err_str = format!("{:?}", e);
        assert!(
            !err_str.contains("Failed to build message"),
            "Message building failed - attachment parsing broken: {:?}",
            e
        );
    }

    // Format 2: Multiple attachments with mixed tools
    let mut input2 = HashMap::new();
    input2.insert("content".to_string(), serde_json::json!("Test"));
    input2.insert(
        "attachments".to_string(),
        serde_json::json!([
            {
                "file_id": "file-abc123",
                "tools": [{"type": "code_interpreter"}]
            },
            {
                "file_id": "file-def456",
                "tools": [{"type": "file_search"}]
            }
        ]),
    );

    let result2 = assistant.invoke(input2, None).await;
    if let Err(e) = &result2 {
        let err_str = format!("{:?}", e);
        assert!(
            !err_str.contains("Failed to build message"),
            "Message building failed - attachment parsing broken: {:?}",
            e
        );
    }
}
