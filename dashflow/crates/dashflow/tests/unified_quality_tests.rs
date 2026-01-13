//! Comprehensive Test Suite for Unified Quality Agent
//!
//! This test suite validates all 15 innovations working together:
//! - Self-correcting retry loops
//! - Quality gate enforcement
//! - Multi-model cascade
//! - Response validation
//! - Confidence calibration
//! - Tool context management
//!
//! Test Categories:
//! 1. Basic functionality (happy path)
//! 2. Retry behavior (quality below threshold)
//! 3. Model cascade (fast → premium)
//! 4. Validation (tool ignorance detection)
//! 5. Edge cases (max retries, tool failures)
//! 6. Performance (cost, latency)

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Test State Structure
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TestQualityState {
    query: String,
    response: Option<String>,
    tool_called: bool,
    tool_results: Option<String>,
    quality_score: Option<f64>,
    retry_count: usize,
    max_retries: usize,
    validation_issues: Vec<String>,
    models_tried: Vec<String>,
    current_model: String,
    total_cost: f64,
    strategy: String,
}

impl MergeableState for TestQualityState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            self.query = other.query.clone();
        }
        if other.response.is_some() {
            self.response = other.response.clone();
        }
        self.tool_called = self.tool_called || other.tool_called;
        if other.tool_results.is_some() {
            self.tool_results = other.tool_results.clone();
        }
        if other.quality_score.is_some() {
            self.quality_score = other.quality_score;
        }
        self.retry_count = self.retry_count.max(other.retry_count);
        self.max_retries = self.max_retries.max(other.max_retries);
        self.validation_issues
            .extend(other.validation_issues.clone());
        self.models_tried.extend(other.models_tried.clone());
        if !other.current_model.is_empty() {
            self.current_model = other.current_model.clone();
        }
        self.total_cost += other.total_cost;
        if !other.strategy.is_empty() {
            self.strategy = other.strategy.clone();
        }
    }
}

impl TestQualityState {
    fn new(query: String) -> Self {
        Self {
            query,
            response: None,
            tool_called: false,
            tool_results: None,
            quality_score: None,
            retry_count: 0,
            max_retries: 3,
            validation_issues: Vec::new(),
            models_tried: Vec::new(),
            current_model: String::new(),
            total_cost: 0.0,
            strategy: String::new(),
        }
    }
}

// ============================================================================
// Mock Nodes for Testing
// ============================================================================

fn mock_agent_success(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        state.response = Some("High-quality response based on documentation.".to_string());
        state.tool_called = true;
        state.tool_results = Some("Relevant documentation found.".to_string());
        state.current_model = "gpt-4o-mini".to_string();
        state.models_tried.push("gpt-4o-mini".to_string());
        state.total_cost += 0.0005;
        Ok(state)
    })
}

fn mock_agent_failure_then_success(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        if state.retry_count == 0 {
            // First attempt: fail
            state.response = Some("I couldn't find information about that.".to_string());
        } else {
            // Retry: succeed
            state.response = Some("Based on the documentation, here's the answer.".to_string());
        }
        state.tool_called = true;
        state.tool_results = Some("Documentation found.".to_string());
        state.current_model = "gpt-4o-mini".to_string();
        state.models_tried.push("gpt-4o-mini".to_string());
        state.total_cost += 0.0005;
        Ok(state)
    })
}

fn mock_validate_node(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        if let Some(response) = &state.response {
            if response.contains("couldn't find") {
                state
                    .validation_issues
                    .push("tool_results_ignored".to_string());
            }
        }
        Ok(state)
    })
}

fn mock_quality_gate_node(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        let score = if state.validation_issues.is_empty()
            && state
                .response
                .as_ref()
                .map(|r| r.len() > 30)
                .unwrap_or(false)
        {
            0.97
        } else {
            0.85
        };
        state.quality_score = Some(score);
        Ok(state)
    })
}

fn mock_retry_node(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        state.retry_count += 1;
        state.response = None;
        state.quality_score = None;
        state.validation_issues.clear();
        Ok(state)
    })
}

fn route_after_quality(state: &TestQualityState) -> String {
    let score = state.quality_score.unwrap_or(0.0);

    if (score >= 0.95 && state.validation_issues.is_empty())
        || state.retry_count >= state.max_retries
    {
        "end".to_string()
    } else {
        "retry".to_string()
    }
}

// ============================================================================
// Test 1: Basic Functionality - Happy Path
// ============================================================================

#[tokio::test]
async fn test_basic_quality_agent_success() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    graph.add_node_from_fn("agent", mock_agent_success);
    graph.add_node_from_fn("validate", mock_validate_node);
    graph.add_node_from_fn("quality_gate", mock_quality_gate_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "validate");
    graph.add_edge("validate", "quality_gate");
    graph.add_edge("quality_gate", END);

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("What is Rust?".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert_eq!(final_state.retry_count, 0, "Should succeed on first try");
    assert!(final_state.quality_score.is_some(), "Quality should be scored");
    assert!(
        final_state.quality_score.unwrap_or_default() >= 0.95,
        "Quality should be high"
    );
    assert!(
        final_state.validation_issues.is_empty(),
        "No validation issues"
    );
    assert!(
        final_state.response.is_some(),
        "Response should be generated"
    );
    assert!(final_state.tool_called, "Tool should be called");

    Ok(())
}

// ============================================================================
// Test 2: Retry Behavior - Quality Below Threshold
// ============================================================================

#[tokio::test]
async fn test_retry_on_low_quality() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    graph.add_node_from_fn("agent", mock_agent_failure_then_success);
    graph.add_node_from_fn("validate", mock_validate_node);
    graph.add_node_from_fn("quality_gate", mock_quality_gate_node);
    graph.add_node_from_fn("retry", mock_retry_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "validate");
    graph.add_edge("validate", "quality_gate");

    let mut routes = HashMap::new();
    routes.insert("end".to_string(), END.to_string());
    routes.insert("retry".to_string(), "retry".to_string());
    graph.add_conditional_edges("quality_gate", route_after_quality, routes);

    // THE CYCLE: retry → agent
    graph.add_edge("retry", "agent");

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Explain tokio async spawning".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert_eq!(final_state.retry_count, 1, "Should retry once");
    assert!(final_state.quality_score.is_some(), "Quality should be scored");
    assert!(
        final_state.quality_score.unwrap_or_default() >= 0.95,
        "Quality should improve after retry"
    );
    assert!(
        final_state.validation_issues.is_empty(),
        "Validation issues should be resolved"
    );
    assert!(
        final_state.response.is_some(),
        "Response should be generated"
    );

    Ok(())
}

// ============================================================================
// Test 3: Max Retries - Graceful Degradation
// ============================================================================

fn mock_agent_always_fail(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        // Short response to fail quality check
        state.response = Some("No info.".to_string());
        state.tool_called = true;
        state.tool_results = Some("Some results.".to_string());
        Ok(state)
    })
}

#[tokio::test]
async fn test_max_retries_reached() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    graph.add_node_from_fn("agent", mock_agent_always_fail);
    graph.add_node_from_fn("validate", mock_validate_node);
    graph.add_node_from_fn("quality_gate", mock_quality_gate_node);
    graph.add_node_from_fn("retry", mock_retry_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "validate");
    graph.add_edge("validate", "quality_gate");

    let mut routes = HashMap::new();
    routes.insert("end".to_string(), END.to_string());
    routes.insert("retry".to_string(), "retry".to_string());
    graph.add_conditional_edges("quality_gate", route_after_quality, routes);

    graph.add_edge("retry", "agent");

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Complex query".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert_eq!(final_state.retry_count, 3, "Should reach max retries");
    assert!(final_state.quality_score.is_some(), "Quality should be scored");
    assert!(
        final_state.quality_score.unwrap_or_default() < 0.95,
        "Quality still low after max retries"
    );
    // Note: validation_issues may be empty after final retry as they get cleared,
    // but quality score should still be low

    Ok(())
}

// ============================================================================
// Test 4: Validation - Tool Ignorance Detection
// ============================================================================

#[tokio::test]
async fn test_validation_detects_tool_ignorance() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    fn mock_agent_ignore_tools(
        mut state: TestQualityState,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>,
    > {
        Box::pin(async move {
            state.response =
                Some("I couldn't find any documentation about that topic.".to_string());
            state.tool_called = true;
            state.tool_results = Some("Comprehensive documentation about the topic...".to_string());
            Ok(state)
        })
    }

    graph.add_node_from_fn("agent", mock_agent_ignore_tools);
    graph.add_node_from_fn("validate", mock_validate_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "validate");
    graph.add_edge("validate", END);

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("What is tokio?".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert!(
        !final_state.validation_issues.is_empty(),
        "Should detect validation issues"
    );
    assert!(
        final_state
            .validation_issues
            .contains(&"tool_results_ignored".to_string()),
        "Should specifically detect tool_results_ignored"
    );
    assert!(final_state.tool_called, "Tool was called");
    assert!(final_state.tool_results.is_some(), "Tool results exist");

    Ok(())
}

// ============================================================================
// Test 5: Model Cascade - Fast to Premium
// ============================================================================

fn mock_fast_model_node(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        state.response = Some("Basic answer from fast model.".to_string());
        state.current_model = "gpt-4o-mini".to_string();
        state.models_tried.push("gpt-4o-mini".to_string());
        state.total_cost += 0.0005;
        Ok(state)
    })
}

fn mock_premium_model_node(
    mut state: TestQualityState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>>
{
    Box::pin(async move {
        state.response =
            Some("Comprehensive answer from premium model with citations.".to_string());
        state.current_model = "gpt-4".to_string();
        state.models_tried.push("gpt-4".to_string());
        state.total_cost += 0.030;
        Ok(state)
    })
}

#[tokio::test]
async fn test_model_cascade() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    graph.add_node_from_fn("fast_model", mock_fast_model_node);
    graph.add_node_from_fn("judge_fast", mock_quality_gate_node);
    graph.add_node_from_fn("premium_model", mock_premium_model_node);

    graph.set_entry_point("fast_model");
    graph.add_edge("fast_model", "judge_fast");

    fn route_cascade(state: &TestQualityState) -> String {
        let score = state.quality_score.unwrap_or(0.0);
        if score >= 0.95 {
            END.to_string()
        } else {
            "premium_model".to_string()
        }
    }

    let mut routes = HashMap::new();
    routes.insert(END.to_string(), END.to_string());
    routes.insert("premium_model".to_string(), "premium_model".to_string());
    graph.add_conditional_edges("judge_fast", route_cascade, routes);

    graph.add_edge("premium_model", END);

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Complex technical query".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert_eq!(final_state.models_tried.len(), 2, "Should try both models");
    assert!(
        final_state
            .models_tried
            .contains(&"gpt-4o-mini".to_string()),
        "Should try fast model first"
    );
    assert!(
        final_state.models_tried.contains(&"gpt-4".to_string()),
        "Should cascade to premium model"
    );
    assert_eq!(
        final_state.current_model, "gpt-4",
        "Should end with premium model"
    );
    assert!(
        final_state.total_cost > 0.001,
        "Should accumulate costs from both models"
    );

    Ok(())
}

// ============================================================================
// Test 6: Cost Optimization - Fast Model Sufficient
// ============================================================================

#[tokio::test]
async fn test_cost_optimization_fast_sufficient() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    fn mock_fast_high_quality(
        mut state: TestQualityState,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>,
    > {
        Box::pin(async move {
            state.response =
                Some("High-quality answer from fast model with comprehensive details.".to_string());
            state.current_model = "gpt-4o-mini".to_string();
            state.models_tried.push("gpt-4o-mini".to_string());
            state.total_cost += 0.0005;
            Ok(state)
        })
    }

    graph.add_node_from_fn("fast_model", mock_fast_high_quality);
    graph.add_node_from_fn("judge", mock_quality_gate_node);

    graph.set_entry_point("fast_model");
    graph.add_edge("fast_model", "judge");
    graph.add_edge("judge", END);

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Simple query".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert_eq!(
        final_state.models_tried.len(),
        1,
        "Should only use fast model"
    );
    assert_eq!(
        final_state.current_model, "gpt-4o-mini",
        "Should be fast model"
    );
    assert!(final_state.total_cost < 0.001, "Should have minimal cost");
    assert!(final_state.quality_score.is_some(), "Quality should be scored");
    assert!(
        final_state.quality_score.unwrap_or_default() >= 0.95,
        "Quality should still be high"
    );

    Ok(())
}

// ============================================================================
// Test 7: Tool Context Management
// ============================================================================

#[tokio::test]
async fn test_tool_context_injection() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    fn mock_inject_context(
        mut state: TestQualityState,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>,
    > {
        Box::pin(async move {
            if state.tool_results.is_some() {
                // Simulate context injection by marking it
                state.response = state.response.map(|r| format!("{} [with_tool_context]", r));
            }
            Ok(state)
        })
    }

    graph.add_node_from_fn("agent", mock_agent_success);
    graph.add_node_from_fn("inject_context", mock_inject_context);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "inject_context");
    graph.add_edge("inject_context", END);

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Test query".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert!(
        final_state.tool_results.is_some(),
        "Tool results should exist"
    );
    assert!(
        final_state
            .response
            .as_ref()
            .is_some_and(|r| r.contains("[with_tool_context]")),
        "Context should be injected into response"
    );

    Ok(())
}

// ============================================================================
// Test 8: Comprehensive Integration Test
// ============================================================================

#[tokio::test]
async fn test_full_quality_pipeline() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    graph.add_node_from_fn("agent", mock_agent_failure_then_success);
    graph.add_node_from_fn("validate", mock_validate_node);
    graph.add_node_from_fn("quality_gate", mock_quality_gate_node);
    graph.add_node_from_fn("retry", mock_retry_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "validate");
    graph.add_edge("validate", "quality_gate");

    let mut routes = HashMap::new();
    routes.insert("end".to_string(), END.to_string());
    routes.insert("retry".to_string(), "retry".to_string());
    graph.add_conditional_edges("quality_gate", route_after_quality, routes);

    graph.add_edge("retry", "agent");

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Complex technical question".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Comprehensive assertions
    assert!(final_state.response.is_some(), "Response generated");
    assert!(final_state.quality_score.is_some(), "Quality scored");
    assert!(
        final_state.quality_score.unwrap_or_default() >= 0.95,
        "High quality achieved"
    );
    assert!(
        final_state.validation_issues.is_empty(),
        "No validation issues"
    );
    assert!(final_state.retry_count > 0, "Retries occurred");
    assert!(final_state.retry_count <= 3, "Within retry limit");
    assert!(final_state.tool_called, "Tools were called");

    Ok(())
}

// ============================================================================
// Test 9: Performance Metrics
// ============================================================================

#[tokio::test]
async fn test_performance_metrics() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    graph.add_node_from_fn("agent", mock_agent_success);
    graph.add_node_from_fn("quality_gate", mock_quality_gate_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "quality_gate");
    graph.add_edge("quality_gate", END);

    let app = graph.compile()?;

    let test_queries = vec![
        "What is Rust?",
        "Explain async/await",
        "How does tokio work?",
    ];

    let mut total_cost = 0.0;
    let mut total_quality = 0.0;
    let mut success_count = 0;

    for query in test_queries {
        let initial_state = TestQualityState::new(query.to_string());
        let result = app.invoke(initial_state).await?;
        let final_state = result.final_state;

        total_cost += final_state.total_cost;
        total_quality += final_state.quality_score.unwrap_or(0.0);
        if final_state.quality_score.unwrap_or(0.0) >= 0.95 {
            success_count += 1;
        }
    }

    let avg_cost = total_cost / 3.0;
    let avg_quality = total_quality / 3.0;
    let success_rate = (success_count as f64 / 3.0) * 100.0;

    // Assertions
    assert!(avg_cost < 0.01, "Average cost should be low");
    assert!(avg_quality >= 0.95, "Average quality should be high");
    #[allow(clippy::float_cmp)]
    {
        assert_eq!(success_rate, 100.0, "Success rate should be 100%");
    }

    Ok(())
}

// ============================================================================
// Test 10: Edge Case - Empty Response
// ============================================================================

#[tokio::test]
async fn test_edge_case_empty_response() -> dashflow::Result<()> {
    let mut graph = StateGraph::<TestQualityState>::new();

    fn mock_empty_response(
        mut state: TestQualityState,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = dashflow::Result<TestQualityState>> + Send>,
    > {
        Box::pin(async move {
            state.response = Some("".to_string());
            Ok(state)
        })
    }

    graph.add_node_from_fn("agent", mock_empty_response);
    graph.add_node_from_fn("quality_gate", mock_quality_gate_node);

    graph.set_entry_point("agent");
    graph.add_edge("agent", "quality_gate");
    graph.add_edge("quality_gate", END);

    let app = graph.compile()?;

    let initial_state = TestQualityState::new("Test".to_string());
    let result = app.invoke(initial_state).await?;
    let final_state = result.final_state;

    // Assertions
    assert!(final_state.quality_score.is_some(), "Quality should be scored");
    assert!(
        final_state.quality_score.unwrap_or_default() < 0.95,
        "Empty response should have low quality"
    );

    Ok(())
}
