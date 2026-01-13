// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Standard conformance tests for ChatOpenAI.
//!
//! These tests verify that ChatOpenAI behaves consistently with other
//! ChatModel implementations across the DashFlow ecosystem.

use super::*;
use dashflow_standard_tests::chat_model_tests::*;
use dashflow_test_utils::init_test_env;

/// Helper function to create a test model with standard settings
///
/// Uses gpt-3.5-turbo for fast, cost-effective testing
fn create_test_model() -> ChatOpenAI {
    ChatOpenAI::with_config(Default::default())
        .with_model("gpt-3.5-turbo")
        .with_temperature(0.0) // Deterministic for testing
        .with_max_tokens(100) // Limit tokens for cost control
}

/// Standard Test 1: Basic invoke
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_invoke_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_invoke(&model).await;
}

/// Standard Test 2: Streaming
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stream_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_stream(&model).await;
}

/// Standard Test 3: Batch processing
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_batch_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_batch(&model).await;
}

/// Standard Test 4: Multi-turn conversation
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_conversation_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_conversation(&model).await;
}

/// Standard Test 4b: Double messages conversation
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_double_messages_conversation_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_double_messages_conversation(&model).await;
}

/// Standard Test 4c: Message with name field
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_message_with_name_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_message_with_name(&model).await;
}

/// Standard Test 5: Stop sequences
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stop_sequence_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_stop_sequence(&model).await;
}

/// Standard Test 6: Usage metadata
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_usage_metadata_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_usage_metadata(&model).await;
}

/// Standard Test 7: Empty messages
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_empty_messages_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_empty_messages(&model).await;
}

/// Standard Test 8: Long conversation
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_long_conversation_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_long_conversation(&model).await;
}

/// Standard Test 9: Special characters
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_special_characters_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_special_characters(&model).await;
}

/// Standard Test 10: Unicode and emoji
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_unicode_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_unicode(&model).await;
}

/// Standard Test 11: Tool calling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_tool_calling(&model).await;
}

/// Standard Test 12: Structured output
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_structured_output_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_structured_output(&model).await;
}

/// Standard Test 13: JSON mode
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_json_mode_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_json_mode(&model).await;
}

/// Standard Test 14: Usage metadata in streaming
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_usage_metadata_streaming_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_usage_metadata_streaming(&model).await;
}

/// Standard Test 15: System message handling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_system_message_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_system_message(&model).await;
}

/// Standard Test 16: Empty content handling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_empty_content_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_empty_content(&model).await;
}

/// Standard Test 17: Large input handling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_large_input_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_large_input(&model).await;
}

/// Standard Test 18: Concurrent generation
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_concurrent_generation_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_concurrent_generation(&model).await;
}

/// Standard Test 19: Error recovery
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_error_recovery_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_error_recovery(&model).await;
}

/// Standard Test 20: Response consistency
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_response_consistency_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_response_consistency(&model).await;
}

/// Standard Test 21: Tool calling with no arguments
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_with_no_arguments_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_tool_calling_with_no_arguments(&model).await;
}

// ========================================================================
// COMPREHENSIVE TESTS - Advanced Edge Cases
// ========================================================================

/// Comprehensive Test 1: Streaming with timeout
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stream_with_timeout_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_stream_with_timeout(&model).await;
}

/// Comprehensive Test 2: Streaming interruption handling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stream_interruption_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_stream_interruption(&model).await;
}

/// Comprehensive Test 3: Empty stream handling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stream_empty_response_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_stream_empty_response(&model).await;
}

/// Comprehensive Test 4: Multiple system messages
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_multiple_system_messages_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_multiple_system_messages(&model).await;
}

/// Comprehensive Test 5: Empty system message
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_empty_system_message_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_empty_system_message(&model).await;
}

/// Comprehensive Test 6: Temperature edge cases
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_temperature_extremes_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_temperature_extremes(&model).await;
}

/// Comprehensive Test 7: Max tokens enforcement
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_max_tokens_limit_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_max_tokens_limit(&model).await;
}

/// Comprehensive Test 8: Invalid stop sequences
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_invalid_stop_sequences_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_invalid_stop_sequences(&model).await;
}

/// Comprehensive Test 9: Context window overflow
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_context_window_overflow_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_context_window_overflow(&model).await;
}

/// Comprehensive Test 10: Rapid consecutive calls
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_rapid_consecutive_calls_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_rapid_consecutive_calls(&model).await;
}

/// Comprehensive Test 11: Network error handling
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_network_error_handling_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_network_error_handling(&model).await;
}

/// Comprehensive Test 12: Malformed input recovery
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_malformed_input_recovery_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_malformed_input_recovery(&model).await;
}

/// Comprehensive Test 13: Very long single message
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_very_long_single_message_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_very_long_single_message(&model).await;
}

/// Comprehensive Test 14: Response format consistency
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_response_format_consistency_comprehensive() {
    init_test_env().ok();
    let model = create_test_model();
    test_response_format_consistency(&model).await;
}

// ========================================================================
// NEW STANDARD TESTS - Tool Calling
// ========================================================================

/// Standard Test: Agent loop with tool calling
/// Verifies tool calling works in multi-step agent reasoning scenarios
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_loop_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_agent_loop(&model).await;
}

/// Standard Test: Tool message with error status
/// Verifies models handle tool execution errors gracefully (ToolMessage with status="error")
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_message_error_status_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_tool_message_error_status(&model).await;
}

/// Standard Test: Tool message histories with string content
/// Verifies model handles ToolMessage with string content (OpenAI format)
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_message_histories_string_content_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_tool_message_histories_string_content(&model).await;
}

/// Standard Test: Tool message histories with list content
/// Verifies model handles AIMessage with structured content blocks (Anthropic format)
#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_message_histories_list_content_standard() {
    init_test_env().ok();
    let model = create_test_model();
    test_tool_message_histories_list_content(&model).await;
}
