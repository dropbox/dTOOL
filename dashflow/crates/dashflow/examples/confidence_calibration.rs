// INNOVATION 15: Confidence Calibration
//
// Problem: LLM doesn't know when it will fail until after generating response
// Solution: PREDICT confidence BEFORE generation, route accordingly
//
// Architecture:
//   query → predict_confidence → route → [fast/careful/search-first] → validate → calibrate
//           (learn from history)
//
// Key insight: Learn to predict failure before it happens!

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConfidenceState {
    // Input
    query: String,

    // Confidence prediction (BEFORE generation)
    predicted_confidence: f64,                 // 0.0-1.0
    prediction_features: HashMap<String, f64>, // Features used for prediction

    // Response generation
    response: String,
    strategy: String, // "fast", "careful", "search-first"

    // Actual quality (AFTER generation)
    actual_quality: f64,

    // Calibration
    prediction_error: f64,       // abs(predicted - actual)
    calibration_adjustment: f64, // How to adjust future predictions

    // History tracking (simulated)
    total_predictions: usize,
    correct_predictions: usize, // Within 0.1 of actual
}

impl MergeableState for ConfidenceState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        self.predicted_confidence = self.predicted_confidence.max(other.predicted_confidence);
        self.prediction_features
            .extend(other.prediction_features.clone());
        if !other.response.is_empty() {
            if self.response.is_empty() {
                self.response = other.response.clone();
            } else {
                self.response.push('\n');
                self.response.push_str(&other.response);
            }
        }
        if !other.strategy.is_empty() {
            if self.strategy.is_empty() {
                self.strategy = other.strategy.clone();
            } else {
                self.strategy.push('\n');
                self.strategy.push_str(&other.strategy);
            }
        }
        self.actual_quality = self.actual_quality.max(other.actual_quality);
        self.prediction_error = self.prediction_error.max(other.prediction_error);
        self.calibration_adjustment = self
            .calibration_adjustment
            .max(other.calibration_adjustment);
        self.total_predictions = self.total_predictions.max(other.total_predictions);
        self.correct_predictions = self.correct_predictions.max(other.correct_predictions);
    }
}

// ============================================================================
// CONFIDENCE PREDICTION
// ============================================================================

fn extract_features(query: &str) -> HashMap<String, f64> {
    let mut features = HashMap::new();

    // Feature 1: Query length (longer = more complex)
    let query_length = query.len() as f64;
    features.insert("query_length".to_string(), query_length);

    // Feature 2: Question words (how/why = harder than what/when)
    let has_how = query.to_lowercase().contains("how");
    let has_why = query.to_lowercase().contains("why");
    let has_what = query.to_lowercase().contains("what");
    let complexity_score = if has_how || has_why {
        1.0
    } else if has_what {
        0.5
    } else {
        0.0
    };
    features.insert("complexity".to_string(), complexity_score);

    // Feature 3: Specificity (proper nouns, version numbers)
    let has_version = query.chars().any(|c| c.is_numeric());
    let has_capitals = query.chars().filter(|c| c.is_uppercase()).count() > 1;
    let specificity = if has_version && has_capitals {
        1.0
    } else if has_version || has_capitals {
        0.5
    } else {
        0.0
    };
    features.insert("specificity".to_string(), specificity);

    // Feature 4: Ambiguity indicators
    let has_or = query.contains(" or ");
    let has_maybe = query.to_lowercase().contains("maybe") || query.contains("might");
    let ambiguity = if has_or || has_maybe { 1.0 } else { 0.0 };
    features.insert("ambiguity".to_string(), ambiguity);

    features
}

fn predict_confidence(features: &HashMap<String, f64>) -> f64 {
    // Simple heuristic model (in production: trained classifier)
    let query_length = features.get("query_length").unwrap_or(&0.0);
    let complexity = features.get("complexity").unwrap_or(&0.0);
    let specificity = features.get("specificity").unwrap_or(&0.0);
    let ambiguity = features.get("ambiguity").unwrap_or(&0.0);

    // Base confidence: 0.7
    let mut confidence = 0.70;

    // Adjust for query length (very short or very long = harder)
    if *query_length < 20.0 || *query_length > 200.0 {
        confidence -= 0.15;
    }

    // Complex questions = lower confidence
    confidence -= complexity * 0.20;

    // Specific queries = higher confidence (easier to answer)
    confidence += specificity * 0.15;

    // Ambiguous queries = lower confidence
    confidence -= ambiguity * 0.25;

    // Clamp to [0.0, 1.0]
    confidence.clamp(0.0, 1.0)
}

// ============================================================================
// NODES
// ============================================================================

fn predict_confidence_node(
    mut state: ConfidenceState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceState>> + Send>>
{
    Box::pin(async move {
        println!("\n[PREDICT] Analyzing query to predict confidence...");

        // Extract features
        state.prediction_features = extract_features(&state.query);

        println!("[PREDICT] Features:");
        for (key, value) in &state.prediction_features {
            println!("  - {}: {:.2}", key, value);
        }

        // Predict confidence
        state.predicted_confidence = predict_confidence(&state.prediction_features);

        println!(
            "[PREDICT] Predicted confidence: {:.2}",
            state.predicted_confidence
        );

        Ok(state)
    })
}

fn route_by_confidence(state: &ConfidenceState) -> String {
    println!(
        "\n[ROUTE] Routing based on predicted confidence {:.2}...",
        state.predicted_confidence
    );

    if state.predicted_confidence >= 0.80 {
        println!("[ROUTE] High confidence → FAST strategy (gpt-4o-mini, direct)");
        "fast_strategy".to_string()
    } else if state.predicted_confidence >= 0.60 {
        println!("[ROUTE] Medium confidence → CAREFUL strategy (gpt-4, detailed prompt)");
        "careful_strategy".to_string()
    } else {
        println!("[ROUTE] Low confidence → SEARCH-FIRST strategy (retrieve, then answer)");
        "search_first_strategy".to_string()
    }
}

fn fast_strategy_node(
    mut state: ConfidenceState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceState>> + Send>>
{
    Box::pin(async move {
        println!("\n[FAST STRATEGY] Using gpt-4o-mini, direct answer...");
        state.strategy = "fast".to_string();

        // Simulate fast response (may be low quality if confidence was wrong)
        if state.query.to_lowercase().contains("capital of france") {
            state.response = "The capital of France is Paris.".to_string();
            state.actual_quality = 0.95;
        } else {
            state.response = "I'm not sure about that.".to_string();
            state.actual_quality = 0.40;
        }

        println!("[FAST STRATEGY] Response generated");

        Ok(state)
    })
}

fn careful_strategy_node(
    mut state: ConfidenceState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceState>> + Send>>
{
    Box::pin(async move {
        println!("\n[CAREFUL STRATEGY] Using gpt-4, detailed prompt...");
        state.strategy = "careful".to_string();

        // Simulate careful response (better quality, slower)
        state.response = format!(
            "Let me carefully answer your question: {}. [Detailed response with reasoning]",
            state.query
        );
        state.actual_quality = 0.75;

        println!("[CAREFUL STRATEGY] Response generated with extra care");

        Ok(state)
    })
}

fn search_first_strategy_node(
    mut state: ConfidenceState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceState>> + Send>>
{
    Box::pin(async move {
        println!("\n[SEARCH-FIRST STRATEGY] Retrieving documents before answering...");
        state.strategy = "search-first".to_string();

        // Simulate search + answer (highest quality, slowest)
        state.response = format!(
            "Based on the documentation search:\n\
             [Retrieved information about: {}]\n\
             [Comprehensive answer based on sources]",
            state.query
        );
        state.actual_quality = 0.90;

        println!("[SEARCH-FIRST STRATEGY] Response generated with retrieved context");

        Ok(state)
    })
}

fn validate_quality_node(
    state: ConfidenceState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceState>> + Send>>
{
    Box::pin(async move {
        println!("\n[VALIDATE] Measuring actual quality...");

        // actual_quality already set by strategy nodes (simulated judge)
        println!("[VALIDATE] Actual quality: {:.2}", state.actual_quality);

        Ok(state)
    })
}

fn calibrate_node(
    mut state: ConfidenceState,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = dashflow::Result<ConfidenceState>> + Send>>
{
    Box::pin(async move {
        println!("\n[CALIBRATE] Updating confidence model...");

        // Calculate prediction error
        state.prediction_error = (state.predicted_confidence - state.actual_quality).abs();

        println!(
            "[CALIBRATE] Predicted: {:.2}, Actual: {:.2}, Error: {:.2}",
            state.predicted_confidence, state.actual_quality, state.prediction_error
        );

        // Track accuracy
        state.total_predictions += 1;
        if state.prediction_error <= 0.10 {
            state.correct_predictions += 1;
            println!("[CALIBRATE] ✓ Prediction accurate (within 0.10)");
        } else {
            println!("[CALIBRATE] ✗ Prediction inaccurate");
        }

        // Calculate calibration adjustment
        // If we over-predicted confidence: reduce future predictions
        // If we under-predicted: increase future predictions
        state.calibration_adjustment = if state.predicted_confidence > state.actual_quality {
            -0.05 // We were too confident
        } else {
            0.05 // We were too pessimistic
        };

        println!(
            "[CALIBRATE] Adjustment: {:+.2} (apply to similar queries)",
            state.calibration_adjustment
        );
        println!(
            "[CALIBRATE] Accuracy: {}/{} = {:.1}%",
            state.correct_predictions,
            state.total_predictions,
            (state.correct_predictions as f64 / state.total_predictions as f64) * 100.0
        );

        Ok(state)
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sep = "=".repeat(80);
    println!("{}", sep);
    println!("INNOVATION 15: Confidence Calibration");
    println!("{}", sep);

    println!("\n📋 Problem:");
    println!("   - LLM doesn't know it will fail until AFTER generation");
    println!("   - Wastes time generating low-quality responses");
    println!("   - Can't pre-emptively choose better strategy");

    println!("\n💡 Solution:");
    println!("   - PREDICT confidence BEFORE generation");
    println!("   - Route to appropriate strategy based on prediction");
    println!("   - LEARN from actual quality to improve predictions");

    println!("\n🏗️ Architecture:");
    println!("   query → predict → route → [fast/careful/search] → validate → calibrate");
    println!("                              (choose strategy)");

    println!("\n{}", sep);
    println!("TEST SCENARIOS");
    println!("{}", sep);

    // Build graph
    let mut graph = StateGraph::<ConfidenceState>::new();

    graph.add_node_from_fn("predict", predict_confidence_node);
    graph.add_node_from_fn("fast_strategy", fast_strategy_node);
    graph.add_node_from_fn("careful_strategy", careful_strategy_node);
    graph.add_node_from_fn("search_first_strategy", search_first_strategy_node);
    graph.add_node_from_fn("validate", validate_quality_node);
    graph.add_node_from_fn("calibrate", calibrate_node);

    graph.set_entry_point("predict");

    // Route based on confidence
    let mut route_map = HashMap::new();
    route_map.insert("fast_strategy".to_string(), "fast_strategy".to_string());
    route_map.insert(
        "careful_strategy".to_string(),
        "careful_strategy".to_string(),
    );
    route_map.insert(
        "search_first_strategy".to_string(),
        "search_first_strategy".to_string(),
    );

    graph.add_conditional_edges("predict", route_by_confidence, route_map);

    // All strategies → validate → calibrate → END
    graph.add_edge("fast_strategy", "validate");
    graph.add_edge("careful_strategy", "validate");
    graph.add_edge("search_first_strategy", "validate");
    graph.add_edge("validate", "calibrate");
    graph.add_edge("calibrate", END);

    let app = graph.compile()?;

    // Test scenarios
    let test_queries = [
        (
            "What is the capital of France?",
            "Simple factual query - should predict high confidence",
        ),
        (
            "How does quantum entanglement work?",
            "Complex 'how' question - should predict low confidence",
        ),
        (
            "Tell me about the DashFlow v2.0 API",
            "Specific query with version - should predict medium confidence",
        ),
        (
            "Maybe explain algorithms or something?",
            "Ambiguous query - should predict very low confidence",
        ),
    ];

    let mut cumulative_state = ConfidenceState {
        total_predictions: 0,
        correct_predictions: 0,
        ..Default::default()
    };

    for (i, (query, description)) in test_queries.iter().enumerate() {
        println!("\n{}", sep);
        println!("SCENARIO {}: {}", i + 1, description);
        println!("{}", sep);
        println!("Query: \"{}\"", query);

        let initial_state = ConfidenceState {
            query: query.to_string(),
            total_predictions: cumulative_state.total_predictions,
            correct_predictions: cumulative_state.correct_predictions,
            ..Default::default()
        };

        let execution_result = app.invoke(initial_state).await?;
        let result = execution_result.final_state;

        cumulative_state.total_predictions = result.total_predictions;
        cumulative_state.correct_predictions = result.correct_predictions;

        println!("\n📊 Result:");
        println!("   - Strategy: {}", result.strategy);
        println!(
            "   - Response: {}",
            result.response.chars().take(80).collect::<String>()
        );
        println!(
            "   - Predicted confidence: {:.2}",
            result.predicted_confidence
        );
        println!("   - Actual quality: {:.2}", result.actual_quality);
        println!("   - Error: {:.2}", result.prediction_error);

        if result.prediction_error <= 0.10 {
            println!("   - ✅ Accurate prediction");
        } else {
            println!("   - ❌ Inaccurate prediction");
        }
    }

    println!("\n{}", sep);
    println!("OVERALL CALIBRATION");
    println!("{}", sep);

    let accuracy = (cumulative_state.correct_predictions as f64
        / cumulative_state.total_predictions as f64)
        * 100.0;
    println!(
        "\n📈 Prediction Accuracy: {}/{} = {:.1}%",
        cumulative_state.correct_predictions, cumulative_state.total_predictions, accuracy
    );

    println!("\n💡 Key Benefits:");
    println!("   ✓ Pre-emptive optimization: Choose strategy BEFORE generation");
    println!("   ✓ Resource efficiency: Use fast strategy when safe");
    println!("   ✓ Quality guarantee: Use careful/search for uncertain queries");
    println!("   ✓ Self-improving: Learn from mistakes to predict better");

    println!("\n🎯 Impact on 100% Quality Goal:");
    println!("   - Avoid low-quality fast responses on hard queries");
    println!("   - Optimize cost: Don't over-engineer easy queries");
    println!("   - Continuous improvement: Model gets better over time");
    println!("   - Combines with other innovations (retry, judge, etc.)");

    println!("\n{}", sep);

    Ok(())
}
