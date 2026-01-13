//! Reasoning node
//!
//! This node calls the LLM API with the current conversation context
//! and parses the response for tool calls or final answers.
//!
//! ## Quality Gate Validation
//!
//! When `quality_gate_config` is set on the agent state, the reasoning node
//! will validate LLM responses using a quality gate retry loop. If the response
//! quality is below the configured threshold, the node will retry the LLM call
//! up to `max_retries` times before accepting the best response.
//!
//! ### Heuristic Scoring (Default)
//!
//! Quality is evaluated based on:
//! - **Completeness**: Does the response address the user's request?
//! - **Relevance**: Is the response on-topic?
//! - **Tool Usage**: Are tools used appropriately when needed?
//!
//! ### LLM-as-Judge Scoring (Optional)
//!
//! When `use_llm_judge` is enabled (requires `llm-judge` feature), quality is
//! evaluated using DashFlow's MultiDimensionalJudge across 6 dimensions:
//! - **Accuracy**: Factual correctness (25% weight)
//! - **Relevance**: How well it addresses the query (25% weight)
//! - **Completeness**: Coverage of necessary aspects (20% weight)
//! - **Safety**: Absence of harmful content (15% weight)
//! - **Coherence**: Logical flow and readability (10% weight)
//! - **Conciseness**: Efficiency without verbosity (5% weight)

use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use dashflow::quality::{QualityGate, QualityScore as DashFlowQualityScore};

use crate::context::{messages_token_count, TruncationConfig};
use crate::llm::{LlmClient, TokenUsage};
use crate::state::{AgentState, Message, ToolCall};
use crate::streaming::AgentEvent;
use crate::tools::{get_tool_definitions, get_tool_definitions_with_mcp};

/// Reasoning node - calls LLM and processes response
///
/// This node:
/// 1. Ensures system prompt is present
/// 2. Calls the LLM API with conversation history
/// 3. Parses tool calls from the response
/// 4. Updates state with assistant response or pending tool calls
pub fn reasoning_node(
    mut state: AgentState,
) -> Pin<Box<dyn Future<Output = Result<AgentState, dashflow::Error>> + Send>> {
    Box::pin(async move {
        tracing::debug!(
            session_id = %state.session_id,
            turn = state.turn_count,
            use_mock = state.use_mock_llm,
            "Starting reasoning"
        );

        // Emit reasoning start event
        state.emit_event(AgentEvent::ReasoningStart {
            session_id: state.session_id.clone(),
            turn: state.turn_count,
            model: state.llm_config.model.clone(),
        });

        let start = Instant::now();

        // Ensure system prompt is present
        ensure_system_prompt(&mut state);

        // Call LLM with optional quality gate validation
        let (response_text, tool_calls, usage, quality_result) =
            if let Some(qg_config) = state.quality_gate_config.clone() {
                // Quality gate enabled - use retry loop
                call_llm_with_quality_gate(&mut state, qg_config).await?
            } else {
                // No quality gate - direct LLM call
                let (text, calls, usage) = if state.use_mock_llm {
                    mock_llm_response(&state)
                } else {
                    call_llm(&state).await?
                };
                (text, calls, usage, None)
            };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Emit quality gate result if validation was performed
        if let Some((passed, score, attempts, is_final, reason)) = quality_result {
            state.emit_event(AgentEvent::QualityGateResult {
                session_id: state.session_id.clone(),
                attempt: attempts,
                passed,
                accuracy: score.accuracy,
                relevance: score.relevance,
                completeness: score.completeness,
                average_score: score.average(),
                is_final,
                reason,
            });
        }

        // Extract token counts from usage
        let (input_tokens, output_tokens) = usage
            .as_ref()
            .map(|u| (Some(u.prompt_tokens), Some(u.completion_tokens)))
            .unwrap_or((None, None));

        // Emit reasoning complete event with token counts
        state.emit_event(AgentEvent::ReasoningComplete {
            session_id: state.session_id.clone(),
            turn: state.turn_count,
            duration_ms,
            has_tool_calls: !tool_calls.is_empty(),
            tool_count: tool_calls.len(),
            input_tokens,
            output_tokens,
        });

        // Emit detailed LLM metrics event if we have usage data
        // Audit #39: Accumulate token usage on state for session-level tracking
        // Audit #77: Accumulate cost on state for session-level cost aggregation
        if let Some(ref u) = usage {
            state.accumulate_token_usage(u);

            let cost = estimate_cost(
                &state.llm_config.model,
                u.prompt_tokens,
                u.completion_tokens,
            );
            state.accumulate_cost(cost);

            state.emit_event(AgentEvent::LlmMetrics {
                session_id: state.session_id.clone(),
                request_id: uuid::Uuid::new_v4().to_string(),
                model: state.llm_config.model.clone(),
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
                latency_ms: duration_ms,
                cost_usd: cost,
                cached: u.cached_tokens > 0,
            });
        }

        // Emit final TokenChunk event (audit #73) to signal response completion
        // Note: True streaming would require LLM layer refactoring to emit during SSE parsing.
        // This provides the event type for consumers even without per-token streaming.
        if let Some(ref text) = response_text {
            state.emit_event(AgentEvent::TokenChunk {
                session_id: state.session_id.clone(),
                chunk: text.clone(),
                is_final: true,
            });
        }

        // Update state with response and/or tool calls
        if !tool_calls.is_empty() {
            // Emit tool call requested events
            for tc in &tool_calls {
                state.emit_event(AgentEvent::ToolCallRequested {
                    session_id: state.session_id.clone(),
                    tool_call_id: tc.id.clone(),
                    tool: tc.tool.clone(),
                    args: tc.args.clone(),
                });
            }

            // Assistant message with tool calls - store both the message and pending calls
            // OpenAI API requires the assistant message with tool_calls to be in history
            let msg = Message::assistant_with_tool_calls(response_text.clone(), tool_calls.clone());
            state.messages.push(msg);
            state.pending_tool_calls.extend(tool_calls);
            if let Some(ref text) = response_text {
                state.last_response = Some(text.clone());
            }
        } else if let Some(text) = response_text {
            // Regular assistant message (no tool calls)
            state.add_assistant_message(text);
        }

        // If no tool calls and we have a response, mark as potentially complete
        if state.pending_tool_calls.is_empty() && state.last_response.is_some() {
            state.mark_complete();
        }

        tracing::debug!(
            session_id = %state.session_id,
            turn = state.turn_count,
            pending_tools = state.pending_tool_calls.len(),
            "Reasoning complete"
        );

        Ok(state)
    })
}

/// Ensure the system prompt is present in the messages
///
/// If introspection is enabled (graph_manifest is set), appends AI self-awareness
/// information to the system prompt so the AI can understand its own structure.
fn ensure_system_prompt(state: &mut AgentState) {
    let has_system = state
        .messages
        .iter()
        .any(|m| matches!(m.role, crate::state::MessageRole::System));

    if !has_system {
        // Insert system prompt at the beginning - use custom prompt if set
        let mut prompt = state.get_system_prompt().to_string();

        // Append introspection info if available (AI self-awareness)
        if let Some(ref manifest) = state.graph_manifest {
            prompt.push_str("\n\n## AI Self-Awareness (Introspection)\n\n");
            prompt.push_str("You are an AI agent built with DashFlow. Here is your structure:\n\n");
            prompt.push_str(&format!(
                "**Graph**: {} ({})\n",
                manifest.graph_name.as_deref().unwrap_or("unknown"),
                manifest.graph_id.as_deref().unwrap_or("unknown")
            ));
            prompt.push_str(&format!("**Entry Point**: {}\n\n", manifest.entry_point));

            prompt.push_str("**Available Nodes**:\n");
            for (name, node) in &manifest.nodes {
                prompt.push_str(&format!("- `{}` ({:?})", name, node.node_type));
                if let Some(ref desc) = node.description {
                    prompt.push_str(&format!(": {}", desc));
                }
                prompt.push('\n');
            }

            // List available tools from reasoning node
            if let Some(reasoning) = manifest.nodes.get("reasoning") {
                if !reasoning.tools_available.is_empty() {
                    prompt.push_str("\n**Your Available Tools**:\n");
                    for tool in &reasoning.tools_available {
                        prompt.push_str(&format!("- `{}`\n", tool));
                    }
                }
            }

            prompt.push_str("\nYou can use this information to understand your own capabilities and workflow.\n");
        }

        state.messages.insert(0, Message::system(prompt));
    }
}

/// Call the actual LLM API
async fn call_llm(
    state: &AgentState,
) -> Result<(Option<String>, Vec<ToolCall>, Option<TokenUsage>), dashflow::Error> {
    // Audit #89: Include MCP tools in LLM tool list so models can invoke them
    let tools = if let Some(ref mcp_client) = state.mcp_client() {
        let mcp_tools = mcp_client.list_tools().await;
        if !mcp_tools.is_empty() {
            tracing::debug!(
                count = mcp_tools.len(),
                "Including MCP tools in LLM request"
            );
        }
        get_tool_definitions_with_mcp(&mcp_tools)
    } else {
        get_tool_definitions()
    };

    // Pass stream callback and session ID to LLM client for token streaming (Audit #73)
    let client = LlmClient::with_config(state.llm_config.clone())
        .with_tools(tools)
        .with_stream_callback(state.stream_callback())
        .with_session_id(&state.session_id);

    // Truncate messages to fit context window
    // Audit #40: Use model-specific context limits instead of hardcoded default
    let truncation_config = TruncationConfig::for_model(&state.llm_config.model);
    let truncated_messages =
        truncation_config.truncate_messages_for_model(&state.messages, &state.llm_config.model);

    let original_tokens = messages_token_count(&state.messages);
    let truncated_tokens = messages_token_count(&truncated_messages);

    if truncated_messages.len() < state.messages.len() {
        tracing::info!(
            original_messages = state.messages.len(),
            truncated_messages = truncated_messages.len(),
            original_tokens,
            truncated_tokens,
            "Context truncated to fit model window"
        );
    }

    let response = client
        .generate(&truncated_messages, None)
        .await
        .map_err(|e| dashflow::Error::Validation(format!("LLM API error: {}", e)))?;

    tracing::debug!(
        model = %state.llm_config.model,
        finish_reason = ?response.finish_reason,
        tool_calls = response.tool_calls.len(),
        has_content = response.content.is_some(),
        "LLM response received"
    );

    if let Some(ref usage) = response.usage {
        tracing::debug!(
            prompt_tokens = usage.prompt_tokens,
            completion_tokens = usage.completion_tokens,
            total_tokens = usage.total_tokens,
            "Token usage"
        );
    }

    Ok((response.content, response.tool_calls, response.usage))
}

/// Type alias for quality gate result tuple
/// (passed, score, attempts, is_final, reason)
type QualityResultTuple = (bool, DashFlowQualityScore, usize, bool, Option<String>);

/// Call LLM with quality gate validation
///
/// Uses DashFlow's QualityGate to ensure LLM responses meet a quality threshold.
/// Will retry the LLM call up to max_retries times if quality is below threshold.
///
/// Quality can be evaluated in two modes:
/// 1. **Heuristic** (default): Fast, local evaluation based on response structure
/// 2. **LLM-as-Judge** (optional): Uses MultiDimensionalJudge for 6-dimension scoring
async fn call_llm_with_quality_gate(
    state: &mut AgentState,
    config: dashflow::quality::QualityGateConfig,
) -> Result<
    (
        Option<String>,
        Vec<ToolCall>,
        Option<TokenUsage>,
        Option<QualityResultTuple>,
    ),
    dashflow::Error,
> {
    // Check if LLM-as-judge is enabled
    #[cfg(feature = "llm-judge")]
    let use_llm_judge = state.use_llm_judge;
    #[cfg(not(feature = "llm-judge"))]
    let use_llm_judge = false;

    if use_llm_judge {
        #[cfg(feature = "llm-judge")]
        {
            call_llm_with_llm_judge_quality_gate(state, config).await
        }
        #[cfg(not(feature = "llm-judge"))]
        {
            // This branch is unreachable but needed for compilation
            call_llm_with_heuristic_quality_gate(state, config).await
        }
    } else {
        call_llm_with_heuristic_quality_gate(state, config).await
    }
}

/// Call LLM with heuristic quality gate validation (fast, local evaluation)
async fn call_llm_with_heuristic_quality_gate(
    state: &mut AgentState,
    config: dashflow::quality::QualityGateConfig,
) -> Result<
    (
        Option<String>,
        Vec<ToolCall>,
        Option<TokenUsage>,
        Option<QualityResultTuple>,
    ),
    dashflow::Error,
> {
    let gate = QualityGate::new(config.clone());
    let session_id = state.session_id.clone();

    // Emit quality gate start event
    state.emit_event(AgentEvent::QualityGateStart {
        session_id: session_id.clone(),
        attempt: 1,
        max_retries: config.max_retries,
        threshold: config.threshold,
    });

    // Get the last user message for quality evaluation context
    let last_user_message = state
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, crate::state::MessageRole::User))
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Track whether we need tool calls based on user message
    let likely_needs_tools = message_likely_needs_tools(&last_user_message);

    // Clone state for the closure (we need to avoid borrowing issues)
    let use_mock = state.use_mock_llm;

    // Use quality gate retry loop
    let result = gate
        .check_with_retry(
            |attempt| {
                // Clone what we need for the async block
                let state_clone = state.clone();
                let session_id_clone = session_id.clone();

                Box::pin(async move {
                    // Emit event for each attempt after the first
                    if attempt > 1 {
                        tracing::debug!(
                            session_id = %session_id_clone,
                            attempt,
                            "Quality gate retry attempt"
                        );
                    }

                    let (text, calls, usage) = if use_mock {
                        mock_llm_response(&state_clone)
                    } else {
                        call_llm(&state_clone).await?
                    };

                    Ok::<_, dashflow::Error>(LlmResponse {
                        text,
                        tool_calls: calls,
                        usage,
                    })
                })
            },
            |response| {
                let user_msg = last_user_message.clone();
                let needs_tools = likely_needs_tools;
                // Clone response data to avoid lifetime issues
                let response_clone = response.clone();

                Box::pin(async move {
                    // Evaluate quality heuristically
                    let score = evaluate_response_quality(&response_clone, &user_msg, needs_tools);
                    Ok::<_, dashflow::Error>(score)
                })
            },
        )
        .await?;

    // Extract final response and quality info
    let (response, passed, score, attempts, reason) = match result {
        dashflow::quality::QualityGateResult::Passed {
            response,
            score,
            attempts,
        } => (response, true, score, attempts, None),
        dashflow::quality::QualityGateResult::Failed {
            response,
            score,
            attempts,
            reason,
        } => (response, false, score, attempts, Some(reason)),
    };

    let quality_result = Some((passed, score, attempts, true, reason));

    Ok((
        response.text,
        response.tool_calls,
        response.usage,
        quality_result,
    ))
}

/// Call LLM with LLM-as-judge quality gate validation (6-dimension scoring)
///
/// Uses DashFlow's MultiDimensionalJudge to evaluate response quality across:
/// - Accuracy (25% weight)
/// - Relevance (25% weight)
/// - Completeness (20% weight)
/// - Safety (15% weight)
/// - Coherence (10% weight)
/// - Conciseness (5% weight)
#[cfg(feature = "llm-judge")]
async fn call_llm_with_llm_judge_quality_gate(
    state: &mut AgentState,
    config: dashflow::quality::QualityGateConfig,
) -> Result<
    (
        Option<String>,
        Vec<ToolCall>,
        Option<TokenUsage>,
        Option<QualityResultTuple>,
    ),
    dashflow::Error,
> {
    use dashflow_evals::MultiDimensionalJudge;
    use dashflow_openai::ChatOpenAI;

    let gate = QualityGate::new(config.clone());
    let session_id = state.session_id.clone();

    // Get the judge model name (will create judge inside closure since it's not Clone)
    let judge_model = state.llm_judge_model().to_string();

    tracing::info!(
        session_id = %session_id,
        judge_model = %judge_model,
        "Using LLM-as-judge for quality evaluation"
    );

    // Emit quality gate start event
    state.emit_event(AgentEvent::QualityGateStart {
        session_id: session_id.clone(),
        attempt: 1,
        max_retries: config.max_retries,
        threshold: config.threshold,
    });

    // Get the last user message for quality evaluation context
    let last_user_message = state
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, crate::state::MessageRole::User))
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Clone state for the closure
    let use_mock = state.use_mock_llm;

    // Use quality gate retry loop
    let result = gate
        .check_with_retry(
            |attempt| {
                let state_clone = state.clone();
                let session_id_clone = session_id.clone();

                Box::pin(async move {
                    if attempt > 1 {
                        tracing::debug!(
                            session_id = %session_id_clone,
                            attempt,
                            "Quality gate retry attempt (LLM-as-judge)"
                        );
                    }

                    let (text, calls, usage) = if use_mock {
                        mock_llm_response(&state_clone)
                    } else {
                        call_llm(&state_clone).await?
                    };

                    Ok::<_, dashflow::Error>(LlmResponse {
                        text,
                        tool_calls: calls,
                        usage,
                    })
                })
            },
            |response| {
                let user_msg = last_user_message.clone();
                let judge_model_name = judge_model.clone();
                let response_clone = response.clone();

                Box::pin(async move {
                    // Create judge inside the closure (MultiDimensionalJudge is not Clone)
                    let judge = MultiDimensionalJudge::new(
                        ChatOpenAI::new()
                            .with_model(&judge_model_name)
                            .with_temperature(0.0),
                    );

                    // Get response text for judging
                    let response_text = if let Some(ref text) = response_clone.text {
                        text.clone()
                    } else if !response_clone.tool_calls.is_empty() {
                        // For tool calls, create a description for the judge
                        let tools: Vec<String> = response_clone
                            .tool_calls
                            .iter()
                            .map(|tc| format!("Tool call: {} with args: {}", tc.tool, tc.args))
                            .collect();
                        tools.join("\n")
                    } else {
                        String::new()
                    };

                    // Use LLM judge to score the response
                    let judge_result = judge.score(&user_msg, &response_text, "").await;

                    let score = match judge_result {
                        Ok(quality_score) => {
                            // Convert 6-dimensional score to 3-dimensional DashFlowQualityScore
                            convert_evals_score_to_quality_score(&quality_score)
                        }
                        Err(e) => {
                            // If judge fails, fall back to heuristic scoring
                            tracing::warn!(
                                error = %e,
                                "LLM judge failed, falling back to heuristic scoring"
                            );
                            evaluate_response_quality_for_llm_judge(&response_clone, &user_msg)
                        }
                    };

                    Ok::<_, dashflow::Error>(score)
                })
            },
        )
        .await?;

    // Extract final response and quality info
    let (response, passed, score, attempts, reason) = match result {
        dashflow::quality::QualityGateResult::Passed {
            response,
            score,
            attempts,
        } => (response, true, score, attempts, None),
        dashflow::quality::QualityGateResult::Failed {
            response,
            score,
            attempts,
            reason,
        } => (response, false, score, attempts, Some(reason)),
    };

    let quality_result = Some((passed, score, attempts, true, reason));

    Ok((
        response.text,
        response.tool_calls,
        response.usage,
        quality_result,
    ))
}

/// Convert DashFlow Evals QualityScore to DashFlow Quality QualityScore
///
/// Maps the 6-dimensional score to the 3-dimensional score used by QualityGate:
/// - accuracy -> accuracy
/// - relevance -> relevance
/// - completeness -> completeness (weighted average of completeness, safety, coherence, conciseness)
#[cfg(feature = "llm-judge")]
fn convert_evals_score_to_quality_score(
    evals_score: &dashflow_evals::QualityScore,
) -> DashFlowQualityScore {
    // For completeness, use a weighted combination that captures the remaining dimensions
    // Completeness (50%) + Safety (20%) + Coherence (20%) + Conciseness (10%)
    let combined_completeness = (evals_score.completeness * 0.5
        + evals_score.safety * 0.2
        + evals_score.coherence * 0.2
        + evals_score.conciseness * 0.1) as f32;

    DashFlowQualityScore {
        accuracy: evals_score.accuracy as f32,
        relevance: evals_score.relevance as f32,
        completeness: combined_completeness,
    }
}

/// Heuristic quality evaluation for LLM-as-judge fallback
#[cfg(feature = "llm-judge")]
fn evaluate_response_quality_for_llm_judge(
    response: &LlmResponse,
    user_message: &str,
) -> DashFlowQualityScore {
    let likely_needs_tools = message_likely_needs_tools(user_message);
    evaluate_response_quality(response, user_message, likely_needs_tools)
}

/// Check if a user message likely requires tool calls
fn message_likely_needs_tools(message: &str) -> bool {
    let lower = message.to_lowercase();

    // File operations
    if lower.contains("read")
        || lower.contains("write")
        || lower.contains("create")
        || lower.contains("delete")
        || lower.contains("file")
    {
        return true;
    }

    // Shell operations
    if lower.contains("run")
        || lower.contains("execute")
        || lower.contains("shell")
        || lower.contains("command")
        || lower.contains("ls")
        || lower.contains("list")
    {
        return true;
    }

    // Search operations
    if lower.contains("search") || lower.contains("find") || lower.contains("grep") {
        return true;
    }

    false
}

/// Evaluate response quality heuristically
///
/// Scores the response on three dimensions:
/// - Accuracy: Did we generate a response at all?
/// - Relevance: Does the response type match expectations?
/// - Completeness: Is the response substantive?
fn evaluate_response_quality(
    response: &LlmResponse,
    user_message: &str,
    likely_needs_tools: bool,
) -> DashFlowQualityScore {
    let has_text = response.text.is_some() && !response.text.as_ref().unwrap().is_empty();
    let has_tools = !response.tool_calls.is_empty();

    // Accuracy: Did we get a response?
    let accuracy = if has_text || has_tools {
        1.0f32
    } else {
        0.0f32
    };

    // Relevance: Does response type match expectations?
    let relevance = if likely_needs_tools {
        if has_tools {
            1.0f32
        } else if has_text {
            // Text response when tools expected - might be refusal or explanation
            0.7
        } else {
            0.0
        }
    } else {
        // Conversational request
        if has_text {
            1.0
        } else if has_tools {
            // Tools when just conversation expected - still reasonable
            0.8
        } else {
            0.0
        }
    };

    // Completeness: Is the response substantive?
    let completeness = if let Some(ref text) = response.text {
        let word_count = text.split_whitespace().count();
        // Longer responses are generally more complete (up to a point)
        let base_completeness = (word_count as f32 / 20.0).min(1.0);

        // Boost if response mentions key terms from user message
        let user_words: Vec<&str> = user_message.split_whitespace().collect();
        let text_lower = text.to_lowercase();
        let keyword_matches = user_words
            .iter()
            .filter(|w| w.len() > 3 && text_lower.contains(&w.to_lowercase()))
            .count();
        if keyword_matches > 0 {
            (base_completeness + 0.2).min(1.0)
        } else {
            base_completeness
        }
    } else if has_tools {
        // Tool calls are complete responses
        1.0
    } else {
        0.0
    };

    DashFlowQualityScore {
        accuracy,
        relevance,
        completeness,
    }
}

/// Struct to hold LLM response data for quality gate evaluation
#[derive(Clone)]
struct LlmResponse {
    text: Option<String>,
    tool_calls: Vec<ToolCall>,
    usage: Option<TokenUsage>,
}

/// Estimate cost in USD based on model and token counts
///
/// Uses approximate pricing as of late 2024. Returns None for unknown models.
fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
    // Pricing per 1M tokens (input, output) - approximate as of late 2024
    let (input_price, output_price) = match model.to_lowercase().as_str() {
        // GPT-4 Turbo
        m if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") || m.contains("gpt-4-0125") => {
            (10.0, 30.0)
        }
        // GPT-4o
        m if m.contains("gpt-4o-mini") => (0.15, 0.6),
        m if m.contains("gpt-4o") => (2.5, 10.0),
        // GPT-4 (original)
        m if m.contains("gpt-4") => (30.0, 60.0),
        // GPT-3.5
        m if m.contains("gpt-3.5") => (0.5, 1.5),
        // Claude 3.5 Sonnet
        m if m.contains("claude-3-5-sonnet") || m.contains("claude-3.5-sonnet") => (3.0, 15.0),
        // Claude 3 Opus
        m if m.contains("claude-3-opus") => (15.0, 75.0),
        // Claude 3 Sonnet
        m if m.contains("claude-3-sonnet") => (3.0, 15.0),
        // Claude 3 Haiku
        m if m.contains("claude-3-haiku") => (0.25, 1.25),
        // Unknown model
        _ => return None,
    };

    let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price;

    Some(input_cost + output_cost)
}

/// Mock LLM response for development and testing
///
/// This simulates different LLM behaviors based on the user input.
/// Returns (response_text, tool_calls, token_usage) to simulate a real LLM response.
fn mock_llm_response(state: &AgentState) -> (Option<String>, Vec<ToolCall>, Option<TokenUsage>) {
    // Check if we have tool result messages - if so, generate a summary response
    let has_tool_results = state
        .messages
        .iter()
        .any(|m| matches!(m.role, crate::state::MessageRole::Tool));

    if has_tool_results {
        // After tool execution, return a summary response (no more tool calls)
        let tool_outputs: Vec<String> = state
            .messages
            .iter()
            .filter(|m| matches!(m.role, crate::state::MessageRole::Tool))
            .map(|m| m.content.clone())
            .collect();

        let response = format!("Here's what I found:\n\n{}", tool_outputs.join("\n\n"));
        // Simulate realistic token usage: ~200 prompt tokens for context, ~50 for response
        let usage = TokenUsage {
            prompt_tokens: 200,
            completion_tokens: 50,
            total_tokens: 250,
            cached_tokens: 0, // Mock mode doesn't simulate caching
        };
        return (Some(response), vec![], Some(usage));
    }

    // Get the last user message
    let last_user_message = state
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, crate::state::MessageRole::User))
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let lower = last_user_message.to_lowercase();

    // Simulate tool calling for file/shell operations
    // All tool calls use ~150 prompt tokens for context, ~30 for tool call output
    let tool_usage = TokenUsage {
        prompt_tokens: 150,
        completion_tokens: 30,
        total_tokens: 180,
        cached_tokens: 0, // Mock mode doesn't simulate caching
    };

    if lower.contains("list") || lower.contains("ls") || lower.contains("files") {
        let tool_call = ToolCall::new(
            "shell",
            serde_json::json!({
                "command": "ls -la"
            }),
        );
        (None, vec![tool_call], Some(tool_usage))
    } else if lower.contains("read") || lower.contains("cat") || lower.contains("show") {
        // Extract a file path if mentioned
        let file_path = if lower.contains("readme") {
            "README.md"
        } else {
            "file.txt"
        };
        let tool_call = ToolCall::new(
            "read_file",
            serde_json::json!({
                "path": file_path
            }),
        );
        (None, vec![tool_call], Some(tool_usage))
    } else if lower.contains("write") || lower.contains("create") {
        let tool_call = ToolCall::new(
            "write_file",
            serde_json::json!({
                "path": "new_file.txt",
                "content": "Hello, World!"
            }),
        );
        (None, vec![tool_call], Some(tool_usage))
    } else {
        // Simple text response - ~100 prompt tokens, ~40 for response
        let response = format!(
            "I understand you said: \"{}\". How can I help you further?",
            last_user_message
        );
        let text_usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 40,
            total_tokens: 140,
            cached_tokens: 0, // Mock mode doesn't simulate caching
        };
        (Some(response), vec![], Some(text_usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CompletionStatus, MessageRole};

    #[tokio::test]
    async fn test_reasoning_node_text_response() {
        // Use mock LLM for testing
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello there"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(state.last_response.is_some());
        assert!(state.pending_tool_calls.is_empty());
    }

    #[tokio::test]
    async fn test_reasoning_node_tool_call() {
        // Use mock LLM for testing
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("List the files"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "shell");
    }

    #[tokio::test]
    async fn test_system_prompt_added() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // System prompt should be the first message
        assert!(matches!(
            state.messages[0].role,
            crate::state::MessageRole::System
        ));
        assert!(state.messages[0].content.contains("coding assistant"));
    }

    #[tokio::test]
    async fn test_system_prompt_preserved_if_exists() {
        let mut state = AgentState::new().with_mock_llm();
        let custom_system = "You are a specialized assistant.";
        state.messages.push(Message::system(custom_system));
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Only one system message should exist (the original one)
        let system_count = state
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::System))
            .count();
        assert_eq!(system_count, 1);

        // The system message should be our custom one
        assert_eq!(state.messages[0].content, custom_system);
    }

    #[tokio::test]
    async fn test_reasoning_node_read_file_tool_call() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Read the README"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "read_file");

        // Check args contain the file path
        let args = &state.pending_tool_calls[0].args;
        assert_eq!(args["path"], "README.md");
    }

    #[tokio::test]
    async fn test_reasoning_node_write_file_tool_call() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Create a new file"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "write_file");

        // Check args contain the file path and content
        let args = &state.pending_tool_calls[0].args;
        assert_eq!(args["path"], "new_file.txt");
        assert!(args["content"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_reasoning_node_tool_result_summary() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("List files"));

        // Add a tool result message (as if shell command was run)
        state
            .messages
            .push(Message::tool("file1.txt\nfile2.txt", "call_123"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Should return a summary response, not more tool calls
        assert!(state.pending_tool_calls.is_empty());
        assert!(state.last_response.is_some());
        let response = state.last_response.unwrap();
        assert!(response.contains("file1.txt"));
    }

    #[tokio::test]
    async fn test_reasoning_node_preserves_session_id() {
        let mut state = AgentState::new().with_mock_llm();
        let original_session_id = state.session_id.clone();
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.session_id, original_session_id);
    }

    #[tokio::test]
    async fn test_reasoning_node_marks_complete_on_text_response() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello there")); // triggers text response

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // No tool calls and has response = complete
        assert!(state.pending_tool_calls.is_empty());
        assert!(state.last_response.is_some());
        assert_eq!(state.status, CompletionStatus::Complete);
    }

    #[tokio::test]
    async fn test_reasoning_node_not_complete_with_tool_calls() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("List the files")); // triggers tool call

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Has tool calls = not complete yet
        assert!(!state.pending_tool_calls.is_empty());
        assert_eq!(state.status, CompletionStatus::InProgress);
    }

    #[tokio::test]
    async fn test_reasoning_node_assistant_message_added() {
        let mut state = AgentState::new().with_mock_llm();
        let initial_message_count = state.messages.len();
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Should have system prompt + user message + assistant response
        assert!(state.messages.len() > initial_message_count + 1);

        // Last message should be assistant
        let last_msg = state.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::Assistant));
    }

    #[tokio::test]
    async fn test_reasoning_node_turn_count_not_modified() {
        let mut state = AgentState::new().with_mock_llm();
        state.turn_count = 5;
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Reasoning node doesn't modify turn count (that's user_input's job)
        assert_eq!(state.turn_count, 5);
    }

    #[tokio::test]
    async fn test_mock_llm_cat_triggers_read_file() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("cat the config"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "read_file");
    }

    #[tokio::test]
    async fn test_mock_llm_show_triggers_read_file() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("show me the code"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "read_file");
    }

    #[tokio::test]
    async fn test_mock_llm_ls_triggers_shell() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("ls -la"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "shell");
    }

    #[tokio::test]
    async fn test_mock_llm_files_triggers_shell() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("what files are here"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].tool, "shell");
    }

    #[test]
    fn test_ensure_system_prompt_direct() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello"));

        // No system prompt initially (besides default)
        let system_before = state
            .messages
            .iter()
            .any(|m| matches!(m.role, MessageRole::System));
        assert!(!system_before);

        ensure_system_prompt(&mut state);

        // Now should have system prompt at index 0
        assert!(matches!(state.messages[0].role, MessageRole::System));
        assert!(state.messages[0].content.contains("coding assistant"));
    }

    #[test]
    fn test_ensure_system_prompt_preserves_existing() {
        let mut state = AgentState::new();
        let custom = "Custom system prompt";
        state.messages.push(Message::system(custom));
        state.messages.push(Message::user("Hello"));

        ensure_system_prompt(&mut state);

        // Should still have only one system message
        let system_count = state
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::System))
            .count();
        assert_eq!(system_count, 1);
        assert_eq!(state.messages[0].content, custom);
    }

    #[test]
    fn test_mock_llm_response_text_only() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello there"));

        let (text, tool_calls, usage) = mock_llm_response(&state);

        assert!(text.is_some());
        assert!(text.unwrap().contains("Hello there"));
        assert!(tool_calls.is_empty());
        // Should have simulated token usage
        assert!(usage.is_some());
        let u = usage.unwrap();
        assert!(u.prompt_tokens > 0);
        assert!(u.completion_tokens > 0);
    }

    #[test]
    fn test_mock_llm_response_shell_tool() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("List the directory"));

        let (text, tool_calls, usage) = mock_llm_response(&state);

        assert!(text.is_none());
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "shell");
        // Should have simulated token usage for tool calls too
        assert!(usage.is_some());
    }

    #[test]
    fn test_mock_llm_response_read_file_tool() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Read the file"));

        let (text, tool_calls, usage) = mock_llm_response(&state);

        assert!(text.is_none());
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "read_file");
        assert!(usage.is_some());
    }

    #[test]
    fn test_mock_llm_response_write_file_tool() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Write a file"));

        let (text, tool_calls, usage) = mock_llm_response(&state);

        assert!(text.is_none());
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "write_file");
        assert!(usage.is_some());
    }

    #[test]
    fn test_mock_llm_response_with_tool_results() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("List files"));

        // Add tool result
        state.messages.push(Message::tool("output.txt", "call_1"));

        let (text, tool_calls, usage) = mock_llm_response(&state);

        // Should summarize, not call more tools
        assert!(text.is_some());
        assert!(text.unwrap().contains("output.txt"));
        assert!(tool_calls.is_empty());
        assert!(usage.is_some());
    }

    #[test]
    fn test_mock_llm_response_empty_user_message() {
        let state = AgentState::new();
        // No user message

        let (text, tool_calls, usage) = mock_llm_response(&state);

        // Default text response
        assert!(text.is_some());
        assert!(tool_calls.is_empty());
        assert!(usage.is_some());
    }

    #[test]
    fn test_mock_llm_response_token_usage_values() {
        // Test that different response types have different token usage
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello there"));

        let (_, _, text_usage) = mock_llm_response(&state);

        let mut state2 = AgentState::new();
        state2.messages.push(Message::user("List files"));

        let (_, _, tool_usage) = mock_llm_response(&state2);

        // Both should have usage
        let text_u = text_usage.unwrap();
        let tool_u = tool_usage.unwrap();

        // Tool calls typically use more prompt tokens for context
        // but fewer completion tokens
        assert!(text_u.total_tokens > 0);
        assert!(tool_u.total_tokens > 0);
    }

    // === Additional tests for improved coverage ===

    // --- estimate_cost tests for various models ---

    #[test]
    fn test_estimate_cost_gpt4_turbo() {
        let cost = estimate_cost("gpt-4-turbo", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $10 per 1M input + $30 per 1M output = $40
        assert!((c - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_gpt4_turbo_1106() {
        let cost = estimate_cost("gpt-4-1106-preview", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        assert!((c - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_gpt4_turbo_0125() {
        let cost = estimate_cost("gpt-4-0125-preview", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        assert!((c - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_gpt4o() {
        let cost = estimate_cost("gpt-4o", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $2.5 per 1M input + $10 per 1M output = $12.5
        assert!((c - 12.5).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_gpt4o_mini() {
        let cost = estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $0.15 per 1M input + $0.6 per 1M output = $0.75
        assert!((c - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_gpt4_original() {
        let cost = estimate_cost("gpt-4", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $30 per 1M input + $60 per 1M output = $90
        assert!((c - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_gpt35() {
        let cost = estimate_cost("gpt-3.5-turbo", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $0.5 per 1M input + $1.5 per 1M output = $2
        assert!((c - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_claude_35_sonnet() {
        let cost = estimate_cost("claude-3-5-sonnet-20240620", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $3 per 1M input + $15 per 1M output = $18
        assert!((c - 18.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_claude_35_sonnet_alt() {
        let cost = estimate_cost("claude-3.5-sonnet", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        assert!((c - 18.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_claude_3_opus() {
        let cost = estimate_cost("claude-3-opus-20240229", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $15 per 1M input + $75 per 1M output = $90
        assert!((c - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_claude_3_sonnet() {
        let cost = estimate_cost("claude-3-sonnet-20240229", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $3 per 1M input + $15 per 1M output = $18
        assert!((c - 18.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_claude_3_haiku() {
        let cost = estimate_cost("claude-3-haiku-20240307", 1_000_000, 1_000_000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $0.25 per 1M input + $1.25 per 1M output = $1.5
        assert!((c - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_unknown_model() {
        let cost = estimate_cost("unknown-model-xyz", 1_000_000, 1_000_000);
        assert!(cost.is_none());
    }

    #[test]
    fn test_estimate_cost_zero_tokens() {
        let cost = estimate_cost("gpt-4o", 0, 0);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 0.0).abs() < 0.0001);
    }

    #[test]
    fn test_estimate_cost_typical_request() {
        // Typical request: 500 input, 1000 output tokens
        let cost = estimate_cost("gpt-4o", 500, 1000);
        assert!(cost.is_some());
        let c = cost.unwrap();
        // $2.5 per 1M input: 500/1M * 2.5 = 0.00125
        // $10 per 1M output: 1000/1M * 10 = 0.01
        // Total = 0.01125
        assert!(c > 0.0);
        assert!(c < 0.02);
    }

    #[test]
    fn test_estimate_cost_case_insensitive() {
        // Model matching is case insensitive
        let cost1 = estimate_cost("GPT-4o", 1000, 1000);
        let cost2 = estimate_cost("gpt-4o", 1000, 1000);
        assert!(cost1.is_some());
        assert!(cost2.is_some());
        assert!((cost1.unwrap() - cost2.unwrap()).abs() < 0.0001);
    }

    // --- Additional mock_llm_response edge cases ---

    #[test]
    fn test_mock_llm_response_multiple_tool_results() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Do multiple things"));
        state.messages.push(Message::tool("result1", "call_1"));
        state.messages.push(Message::tool("result2", "call_2"));

        let (text, tool_calls, usage) = mock_llm_response(&state);

        assert!(text.is_some());
        let t = text.unwrap();
        assert!(t.contains("result1"));
        assert!(t.contains("result2"));
        assert!(tool_calls.is_empty());
        assert!(usage.is_some());
    }

    #[test]
    fn test_mock_llm_response_case_insensitive_triggers() {
        // "LIST" should still trigger shell tool
        let mut state = AgentState::new();
        state.messages.push(Message::user("LIST the directory"));

        let (_, tool_calls, _) = mock_llm_response(&state);
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "shell");
    }

    #[test]
    fn test_mock_llm_response_read_readme() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("read the README please"));

        let (_, tool_calls, _) = mock_llm_response(&state);
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "read_file");
        assert_eq!(tool_calls[0].args["path"], "README.md");
    }

    #[test]
    fn test_mock_llm_response_generic_read() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("read this file"));

        let (_, tool_calls, _) = mock_llm_response(&state);
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "read_file");
        // Generic read defaults to "file.txt"
        assert_eq!(tool_calls[0].args["path"], "file.txt");
    }

    #[test]
    fn test_mock_llm_response_create_triggers_write() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("create a new test file"));

        let (_, tool_calls, _) = mock_llm_response(&state);
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].tool, "write_file");
    }

    // --- Additional ensure_system_prompt tests ---

    #[test]
    fn test_ensure_system_prompt_empty_messages() {
        let mut state = AgentState::new();
        assert!(state.messages.is_empty());

        ensure_system_prompt(&mut state);

        assert_eq!(state.messages.len(), 1);
        assert!(matches!(state.messages[0].role, MessageRole::System));
    }

    #[test]
    fn test_ensure_system_prompt_system_not_first() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("First"));
        state.messages.push(Message::system("System after user"));

        // System exists but not first - should not add another
        ensure_system_prompt(&mut state);

        let system_count = state
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::System))
            .count();
        assert_eq!(system_count, 1);
    }

    // --- reasoning_node additional tests ---

    #[tokio::test]
    async fn test_reasoning_node_preserves_existing_messages() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("First message"));
        state.messages.push(Message::assistant("First response"));
        state.messages.push(Message::user("Second message"));

        let original_len = state.messages.len();
        let result = reasoning_node(state).await;

        assert!(result.is_ok());
        let state = result.unwrap();
        // Should have original messages + system prompt + new assistant response
        assert!(state.messages.len() > original_len);
    }

    #[tokio::test]
    async fn test_reasoning_node_with_custom_system_prompt() {
        let mut state = AgentState::new().with_mock_llm();
        state = state.with_system_prompt("You are a test bot");
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Custom system prompt should be used
        assert_eq!(state.messages[0].content, "You are a test bot");
    }

    #[tokio::test]
    async fn test_reasoning_node_emits_events() {
        // This tests that events are emitted during reasoning
        // (Full event verification would need a callback, but we can check state changes)
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        // If events failed to emit, this would panic (tests event emission doesn't crash)
    }

    #[tokio::test]
    async fn test_reasoning_node_tool_call_adds_assistant_message() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("List files")); // triggers tool call

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Should have assistant message with tool calls in history
        let has_assistant_with_tools = state
            .messages
            .iter()
            .any(|m| matches!(m.role, MessageRole::Assistant) && !m.tool_calls.is_empty());
        assert!(has_assistant_with_tools);
    }

    #[tokio::test]
    async fn test_reasoning_node_multiple_consecutive_calls() {
        // Simulates multiple reasoning steps
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        // First reasoning
        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let mut state = result.unwrap();

        // Add another user message
        state.messages.push(Message::user("What else?"));

        // Second reasoning
        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Should have multiple assistant messages
        let assistant_count = state
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::Assistant))
            .count();
        assert!(assistant_count >= 2);
    }

    // --- Token usage structure tests ---

    #[test]
    fn test_mock_llm_token_usage_cached_is_zero() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Hello"));

        let (_, _, usage) = mock_llm_response(&state);
        assert!(usage.is_some());
        assert_eq!(usage.unwrap().cached_tokens, 0);
    }

    #[test]
    fn test_mock_llm_response_tool_result_usage() {
        let mut state = AgentState::new();
        state.messages.push(Message::user("Do something"));
        state.messages.push(Message::tool("result", "call_id"));

        let (_, _, usage) = mock_llm_response(&state);
        let u = usage.unwrap();

        // Summary response uses ~200 prompt tokens, ~50 completion
        assert_eq!(u.prompt_tokens, 200);
        assert_eq!(u.completion_tokens, 50);
        assert_eq!(u.total_tokens, 250);
    }

    // --- Quality gate tests ---

    #[test]
    fn test_message_likely_needs_tools_file_operations() {
        assert!(message_likely_needs_tools("read the file"));
        assert!(message_likely_needs_tools("please write this to a file"));
        assert!(message_likely_needs_tools("create a new config"));
        assert!(message_likely_needs_tools("delete the old files"));
        assert!(message_likely_needs_tools("check the file contents"));
    }

    #[test]
    fn test_message_likely_needs_tools_shell_operations() {
        assert!(message_likely_needs_tools("run npm install"));
        assert!(message_likely_needs_tools("execute the tests"));
        assert!(message_likely_needs_tools("use the shell to check"));
        assert!(message_likely_needs_tools("run this command"));
        assert!(message_likely_needs_tools("ls -la"));
        assert!(message_likely_needs_tools("list the directory"));
    }

    #[test]
    fn test_message_likely_needs_tools_search_operations() {
        assert!(message_likely_needs_tools("search for the function"));
        assert!(message_likely_needs_tools("find the config file"));
        assert!(message_likely_needs_tools("grep for errors"));
    }

    #[test]
    fn test_message_likely_needs_tools_conversational() {
        assert!(!message_likely_needs_tools("hello"));
        assert!(!message_likely_needs_tools("what is rust?"));
        assert!(!message_likely_needs_tools("explain this code"));
        assert!(!message_likely_needs_tools("thank you"));
    }

    #[test]
    fn test_evaluate_response_quality_text_response() {
        let response = LlmResponse {
            text: Some(
                "This is a detailed response with multiple words to make it substantive enough"
                    .to_string(),
            ),
            tool_calls: vec![],
            usage: None,
        };

        let score = evaluate_response_quality(&response, "explain something", false);

        assert!((score.accuracy - 1.0).abs() < f32::EPSILON);
        assert!((score.relevance - 1.0).abs() < f32::EPSILON);
        assert!(score.completeness > 0.5); // Long enough response
    }

    #[test]
    fn test_evaluate_response_quality_tool_response() {
        let response = LlmResponse {
            text: None,
            tool_calls: vec![ToolCall::new("shell", serde_json::json!({"command": "ls"}))],
            usage: None,
        };

        let score = evaluate_response_quality(&response, "list files", true);

        assert!((score.accuracy - 1.0).abs() < f32::EPSILON);
        assert!((score.relevance - 1.0).abs() < f32::EPSILON);
        assert!((score.completeness - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evaluate_response_quality_empty_response() {
        let response = LlmResponse {
            text: None,
            tool_calls: vec![],
            usage: None,
        };

        let score = evaluate_response_quality(&response, "do something", false);

        assert!((score.accuracy - 0.0).abs() < f32::EPSILON);
        assert!((score.relevance - 0.0).abs() < f32::EPSILON);
        assert!((score.completeness - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evaluate_response_quality_text_when_tools_expected() {
        let response = LlmResponse {
            text: Some("I cannot perform that action".to_string()),
            tool_calls: vec![],
            usage: None,
        };

        let score = evaluate_response_quality(&response, "list files", true);

        assert!((score.accuracy - 1.0).abs() < f32::EPSILON);
        // Lower relevance when text provided but tools expected
        assert!((score.relevance - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evaluate_response_quality_tools_when_text_expected() {
        let response = LlmResponse {
            text: None,
            tool_calls: vec![ToolCall::new(
                "read_file",
                serde_json::json!({"path": "test.txt"}),
            )],
            usage: None,
        };

        let score = evaluate_response_quality(&response, "hello there", false);

        assert!((score.accuracy - 1.0).abs() < f32::EPSILON);
        // Still reasonable relevance
        assert!((score.relevance - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evaluate_response_quality_keyword_boost() {
        let response = LlmResponse {
            text: Some("The Rust programming language is great".to_string()),
            tool_calls: vec![],
            usage: None,
        };

        let score_with_keyword =
            evaluate_response_quality(&response, "tell me about Rust programming", false);

        let response_no_keyword = LlmResponse {
            text: Some("This is some generic response here".to_string()),
            tool_calls: vec![],
            usage: None,
        };
        let score_without_keyword = evaluate_response_quality(
            &response_no_keyword,
            "tell me about Rust programming",
            false,
        );

        // Response with keywords should have higher completeness
        assert!(score_with_keyword.completeness >= score_without_keyword.completeness);
    }

    #[tokio::test]
    async fn test_reasoning_node_with_quality_gate() {
        use dashflow::quality::QualityGateConfig;

        let mut state = AgentState::new()
            .with_mock_llm()
            .with_quality_gate(QualityGateConfig {
                threshold: 0.80,
                max_retries: 2,
                ..Default::default()
            });
        state.messages.push(Message::user("Hello there"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Should have a response
        assert!(state.last_response.is_some());
    }

    #[tokio::test]
    async fn test_reasoning_node_with_quality_gate_tool_call() {
        use dashflow::quality::QualityGateConfig;

        let mut state = AgentState::new()
            .with_mock_llm()
            .with_quality_gate(QualityGateConfig {
                threshold: 0.80,
                max_retries: 2,
                ..Default::default()
            });
        state.messages.push(Message::user("List the files"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Should have tool calls
        assert!(!state.pending_tool_calls.is_empty());
        assert_eq!(state.pending_tool_calls[0].tool, "shell");
    }

    #[tokio::test]
    async fn test_reasoning_node_without_quality_gate() {
        // Ensure reasoning still works without quality gate
        let mut state = AgentState::new().with_mock_llm();
        assert!(!state.has_quality_gate());
        state.messages.push(Message::user("Hello"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(state.last_response.is_some());
    }

    // --- LLM-as-judge tests ---

    #[test]
    fn test_state_llm_judge_configuration() {
        // Test with_llm_judge builder method
        let state = AgentState::new().with_llm_judge("gpt-4o");

        assert!(state.has_llm_judge());
        assert!(state.use_llm_judge);
        assert_eq!(state.llm_judge_model(), "gpt-4o");
    }

    #[test]
    fn test_state_default_llm_judge() {
        // Test with_default_llm_judge builder method
        let state = AgentState::new().with_default_llm_judge();

        assert!(state.has_llm_judge());
        assert!(state.use_llm_judge);
        assert_eq!(state.llm_judge_model(), "gpt-4o-mini"); // default model
    }

    #[test]
    fn test_state_llm_judge_disabled_by_default() {
        // LLM judge should be disabled by default
        let state = AgentState::new();

        assert!(!state.has_llm_judge());
        assert!(!state.use_llm_judge);
        assert_eq!(state.llm_judge_model(), "gpt-4o-mini"); // still returns default
    }

    #[test]
    fn test_state_llm_judge_with_quality_gate() {
        // Test combining LLM judge with quality gate
        use dashflow::quality::QualityGateConfig;

        let state = AgentState::new()
            .with_quality_gate(QualityGateConfig {
                threshold: 0.80,
                max_retries: 2,
                ..Default::default()
            })
            .with_llm_judge("gpt-4o");

        assert!(state.has_quality_gate());
        assert!(state.has_llm_judge());
        assert_eq!(state.llm_judge_model(), "gpt-4o");

        let qg_config = state.quality_gate_config.unwrap();
        assert!((qg_config.threshold - 0.80).abs() < f32::EPSILON);
        assert_eq!(qg_config.max_retries, 2);
    }

    #[tokio::test]
    async fn test_reasoning_node_with_llm_judge_disabled() {
        // When llm-judge feature is not enabled or use_llm_judge is false,
        // should fall back to heuristic scoring
        use dashflow::quality::QualityGateConfig;

        let mut state = AgentState::new()
            .with_mock_llm()
            .with_quality_gate(QualityGateConfig {
                threshold: 0.80,
                max_retries: 2,
                ..Default::default()
            });
        // Note: NOT calling .with_llm_judge(), so it should use heuristic

        assert!(state.has_quality_gate());
        assert!(!state.has_llm_judge());

        state.messages.push(Message::user("Hello there"));

        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(state.last_response.is_some());
    }

    // ===================================================================
    // AI Self-Awareness Tests (GraphManifest injection)
    // ===================================================================

    #[test]
    fn test_graph_manifest_injected_into_system_prompt() {
        use crate::graph::build_agent_graph_manifest;
        use std::sync::Arc;

        // Create state with graph manifest
        let manifest = Arc::new(build_agent_graph_manifest());
        let mut state = AgentState::new().with_graph_manifest(manifest);
        state
            .messages
            .push(Message::user("What are your capabilities?"));

        // Call ensure_system_prompt to inject manifest
        ensure_system_prompt(&mut state);

        // Verify system prompt was added with AI self-awareness section
        assert!(!state.messages.is_empty());
        assert!(matches!(state.messages[0].role, MessageRole::System));

        let system_content = &state.messages[0].content;

        // Check for AI self-awareness header
        assert!(
            system_content.contains("AI Self-Awareness"),
            "System prompt should contain 'AI Self-Awareness' section"
        );

        // Check for graph name
        assert!(
            system_content.contains("codex_dashflow_agent"),
            "System prompt should contain graph name"
        );

        // Check for node listings
        assert!(
            system_content.contains("user_input"),
            "System prompt should list user_input node"
        );
        assert!(
            system_content.contains("reasoning"),
            "System prompt should list reasoning node"
        );
        assert!(
            system_content.contains("tool_execution"),
            "System prompt should list tool_execution node"
        );

        // Check for tool listings
        assert!(
            system_content.contains("shell"),
            "System prompt should list shell tool"
        );
        assert!(
            system_content.contains("read_file"),
            "System prompt should list read_file tool"
        );
    }

    #[test]
    fn test_system_prompt_without_manifest_has_no_introspection() {
        // State without graph manifest
        let mut state = AgentState::new();
        state
            .messages
            .push(Message::user("What are your capabilities?"));

        // Call ensure_system_prompt
        ensure_system_prompt(&mut state);

        // Verify system prompt was added but WITHOUT AI self-awareness
        assert!(!state.messages.is_empty());
        assert!(matches!(state.messages[0].role, MessageRole::System));

        let system_content = &state.messages[0].content;

        // Should NOT contain AI self-awareness section
        assert!(
            !system_content.contains("AI Self-Awareness"),
            "System prompt should NOT contain 'AI Self-Awareness' section when manifest not set"
        );
    }

    #[tokio::test]
    async fn test_reasoning_with_manifest_includes_introspection() {
        use crate::graph::build_agent_graph_manifest;
        use std::sync::Arc;

        // Create state with graph manifest
        let manifest = Arc::new(build_agent_graph_manifest());
        let mut state = AgentState::new()
            .with_mock_llm()
            .with_graph_manifest(manifest);
        state
            .messages
            .push(Message::user("What are your capabilities?"));

        // Run reasoning node
        let result = reasoning_node(state).await;
        assert!(result.is_ok());
        let state = result.unwrap();

        // Verify system prompt contains introspection info
        let system_msg = state
            .messages
            .iter()
            .find(|m| matches!(m.role, MessageRole::System));
        assert!(system_msg.is_some(), "Should have system message");

        let system_content = &system_msg.unwrap().content;
        assert!(
            system_content.contains("AI Self-Awareness"),
            "System prompt should contain AI self-awareness section after reasoning"
        );
        assert!(
            system_content.contains("Available Nodes"),
            "System prompt should list available nodes"
        );
    }

    #[cfg(feature = "llm-judge")]
    mod llm_judge_tests {
        use super::*;

        #[test]
        fn test_convert_evals_score_to_quality_score() {
            let evals_score = dashflow_evals::QualityScore {
                accuracy: 0.9,
                relevance: 0.85,
                completeness: 0.8,
                safety: 1.0,
                coherence: 0.9,
                conciseness: 0.7,
                overall: 0.88,
                reasoning: "Test reasoning".to_string(),
                issues: vec![],
                suggestions: vec![],
            };

            let quality_score = convert_evals_score_to_quality_score(&evals_score);

            // accuracy and relevance are directly mapped
            assert!((quality_score.accuracy - 0.9).abs() < f32::EPSILON);
            assert!((quality_score.relevance - 0.85).abs() < f32::EPSILON);

            // completeness is weighted: 0.8*0.5 + 1.0*0.2 + 0.9*0.2 + 0.7*0.1 = 0.4 + 0.2 + 0.18 + 0.07 = 0.85
            let expected_completeness = 0.8 * 0.5 + 1.0 * 0.2 + 0.9 * 0.2 + 0.7 * 0.1;
            assert!((quality_score.completeness - expected_completeness as f32).abs() < 0.01);
        }

        #[test]
        fn test_convert_evals_score_perfect() {
            let evals_score = dashflow_evals::QualityScore {
                accuracy: 1.0,
                relevance: 1.0,
                completeness: 1.0,
                safety: 1.0,
                coherence: 1.0,
                conciseness: 1.0,
                overall: 1.0,
                reasoning: "Perfect".to_string(),
                issues: vec![],
                suggestions: vec![],
            };

            let quality_score = convert_evals_score_to_quality_score(&evals_score);

            assert!((quality_score.accuracy - 1.0).abs() < f32::EPSILON);
            assert!((quality_score.relevance - 1.0).abs() < f32::EPSILON);
            assert!((quality_score.completeness - 1.0).abs() < f32::EPSILON);
        }

        #[test]
        fn test_convert_evals_score_zero() {
            let evals_score = dashflow_evals::QualityScore {
                accuracy: 0.0,
                relevance: 0.0,
                completeness: 0.0,
                safety: 0.0,
                coherence: 0.0,
                conciseness: 0.0,
                overall: 0.0,
                reasoning: "Bad".to_string(),
                issues: vec![],
                suggestions: vec![],
            };

            let quality_score = convert_evals_score_to_quality_score(&evals_score);

            assert!((quality_score.accuracy - 0.0).abs() < f32::EPSILON);
            assert!((quality_score.relevance - 0.0).abs() < f32::EPSILON);
            assert!((quality_score.completeness - 0.0).abs() < f32::EPSILON);
        }

        #[test]
        fn test_evaluate_response_quality_for_llm_judge_fallback() {
            let response = LlmResponse {
                text: Some("This is a test response".to_string()),
                tool_calls: vec![],
                usage: None,
            };

            let score = evaluate_response_quality_for_llm_judge(&response, "hello");

            // Should have non-zero scores
            assert!(score.accuracy > 0.0);
            assert!(score.relevance > 0.0);
            assert!(score.completeness > 0.0);
        }
    }
}
