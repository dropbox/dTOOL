// INNOVATION 12: Active Learning from Production
//
// Problem: Static prompts/rules don't improve from real usage
// Solution: Collect production data → Train classifier → Deploy updates → Continuous improvement
//
// Architecture:
//   Production → Collect (query, tool_used, quality) → Train classifier →
//   → Deploy updated model → Better tool decisions → Collect more data → Repeat
//
// Key insight: System learns from its own successes and failures!

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// TRAINING DATA
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrainingExample {
    query: String,
    features: HashMap<String, f64>,
    tool_used: bool,       // Ground truth: Did we use tools?
    quality_score: f64,    // Ground truth: Was it good?
    should_use_tool: bool, // Label: Should we have used tools?
}

impl TrainingExample {
    fn from_production(query: String, tool_used: bool, quality: f64) -> Self {
        // Extract features
        let mut features = HashMap::new();

        let query_lower = query.to_lowercase();

        // Feature: Contains documentation-related keywords
        let has_doc_keywords = query_lower.contains("documentation")
            || query_lower.contains("docs")
            || query_lower.contains("api")
            || query_lower.contains("guide");
        features.insert(
            "has_doc_keywords".to_string(),
            if has_doc_keywords { 1.0 } else { 0.0 },
        );

        // Feature: Contains version/technical terms
        let has_version = query.chars().any(|c| c.is_numeric());
        features.insert(
            "has_version".to_string(),
            if has_version { 1.0 } else { 0.0 },
        );

        // Feature: Question type
        let is_factual = query_lower.contains("what is") || query_lower.contains("who is");
        let is_howto = query_lower.contains("how to") || query_lower.contains("how do");
        features.insert("is_factual".to_string(), if is_factual { 1.0 } else { 0.0 });
        features.insert("is_howto".to_string(), if is_howto { 1.0 } else { 0.0 });

        // Feature: Query length
        let query_length = query.len() as f64;
        features.insert("query_length".to_string(), query_length);

        // Label: Should we have used tools?
        // If we used tools and quality was high → positive example
        // If we didn't use tools and quality was low → positive example (should have used tools!)
        let should_use_tool = (tool_used && quality > 0.85) || (!tool_used && quality < 0.70);

        Self {
            query,
            features,
            tool_used,
            quality_score: quality,
            should_use_tool,
        }
    }
}

// ============================================================================
// CLASSIFIER
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolUsageClassifier {
    // Simple logistic regression weights (in production: use real ML library)
    weights: HashMap<String, f64>,
    bias: f64,
    version: usize,
    training_size: usize,
}

impl ToolUsageClassifier {
    fn new() -> Self {
        // Initial heuristic weights
        let mut weights = HashMap::new();
        weights.insert("has_doc_keywords".to_string(), 0.5);
        weights.insert("has_version".to_string(), 0.3);
        weights.insert("is_factual".to_string(), -0.2); // Factual questions often don't need search
        weights.insert("is_howto".to_string(), 0.4);
        weights.insert("query_length".to_string(), 0.001);

        Self {
            weights,
            bias: 0.0,
            version: 0,
            training_size: 0,
        }
    }

    fn predict(&self, features: &HashMap<String, f64>) -> (bool, f64) {
        // Calculate score: sum(weight_i * feature_i) + bias
        let mut score = self.bias;

        for (key, value) in features {
            if let Some(weight) = self.weights.get(key) {
                score += weight * value;
            }
        }

        // Sigmoid: 1 / (1 + e^(-score))
        let probability = 1.0 / (1.0 + (-score).exp());

        // Threshold at 0.5
        let should_use_tools = probability >= 0.5;

        (should_use_tools, probability)
    }

    fn train(&mut self, examples: &[TrainingExample]) {
        println!(
            "\n[TRAIN] Training classifier on {} examples...",
            examples.len()
        );

        // Simple gradient descent update (in production: use proper ML library)
        let learning_rate = 0.01;

        for example in examples {
            let (_predicted, probability) = self.predict(&example.features);
            let label = if example.should_use_tool { 1.0 } else { 0.0 };
            let error = label - probability;

            // Update weights: weight += learning_rate * error * feature
            for (key, value) in &example.features {
                let current_weight = self.weights.get(key).unwrap_or(&0.0);
                self.weights
                    .insert(key.clone(), current_weight + learning_rate * error * value);
            }

            // Update bias
            self.bias += learning_rate * error;
        }

        self.version += 1;
        self.training_size = examples.len();

        println!("[TRAIN] Updated to version {}", self.version);
        println!("[TRAIN] New weights:");
        for (key, value) in &self.weights {
            println!("  - {}: {:.3}", key, value);
        }
        println!("  - bias: {:.3}", self.bias);
    }

    fn evaluate(&self, examples: &[TrainingExample]) -> (f64, usize, usize) {
        let mut correct = 0;
        let mut total = 0;

        for example in examples {
            let (predicted, _) = self.predict(&example.features);
            if predicted == example.should_use_tool {
                correct += 1;
            }
            total += 1;
        }

        let accuracy = correct as f64 / total as f64;
        (accuracy, correct, total)
    }
}

// ============================================================================
// ACTIVE LEARNING STATE
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ActiveLearningState {
    // Production data (simulated)
    production_queries: Vec<(String, bool, f64)>, // (query, tool_used, quality)

    // Training data
    training_examples: Vec<TrainingExample>,

    // Classifier
    classifier_version: usize,
    classifier_accuracy: f64,

    // Current query being processed
    current_query: String,
    predicted_should_use_tools: bool,
    prediction_confidence: f64,
}

impl MergeableState for ActiveLearningState {
    fn merge(&mut self, other: &Self) {
        self.production_queries
            .extend(other.production_queries.clone());
        self.training_examples
            .extend(other.training_examples.clone());
        self.classifier_version = self.classifier_version.max(other.classifier_version);
        self.classifier_accuracy = self.classifier_accuracy.max(other.classifier_accuracy);
        if !other.current_query.is_empty() {
            if self.current_query.is_empty() {
                self.current_query = other.current_query.clone();
            } else {
                self.current_query.push('\n');
                self.current_query.push_str(&other.current_query);
            }
        }
        self.predicted_should_use_tools =
            self.predicted_should_use_tools || other.predicted_should_use_tools;
        self.prediction_confidence = self.prediction_confidence.max(other.prediction_confidence);
    }
}

// ============================================================================
// NODES
// ============================================================================

fn collect_production_data_node(
    mut state: ActiveLearningState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ActiveLearningState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[COLLECT] Simulating production data collection...");

        // Simulate production queries (in production: from Kafka/database)
        state.production_queries = vec![
            // (query, tool_used, quality_score)
            // Positive examples: Tool used AND high quality
            (
                "What is the DashFlow API documentation?".to_string(),
                true,
                0.95,
            ),
            ("How do I use the v2.0 agent?".to_string(), true, 0.90),
            ("Show me the configuration guide".to_string(), true, 0.88),
            // Negative examples: No tool, low quality (should have used tool!)
            (
                "Tell me about the advanced features".to_string(),
                false,
                0.50,
            ),
            ("Explain the new API changes".to_string(), false, 0.45),
            // Negative examples: No tool, high quality (didn't need tool)
            ("What is 2+2?".to_string(), false, 0.95),
            ("Hello, how are you?".to_string(), false, 0.90),
            // Mixed examples
            ("What is the capital of France?".to_string(), false, 0.95),
            ("How to configure DashFlow docs?".to_string(), true, 0.92),
            ("Describe quantum computing basics".to_string(), false, 0.60),
        ];

        println!(
            "[COLLECT] Collected {} production queries",
            state.production_queries.len()
        );

        Ok(state)
    })
}

fn create_training_data_node(
    mut state: ActiveLearningState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ActiveLearningState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[LABEL] Creating labeled training data...");

        state.training_examples = state
            .production_queries
            .iter()
            .map(|(query, tool_used, quality)| {
                TrainingExample::from_production(query.clone(), *tool_used, *quality)
            })
            .collect();

        println!(
            "[LABEL] Created {} training examples",
            state.training_examples.len()
        );

        // Show labeling logic
        let positive_examples = state
            .training_examples
            .iter()
            .filter(|e| e.should_use_tool)
            .count();
        let negative_examples = state.training_examples.len() - positive_examples;

        println!("[LABEL] Labels:");
        println!("  - Should use tools: {}", positive_examples);
        println!("  - Should NOT use tools: {}", negative_examples);

        Ok(state)
    })
}

fn train_classifier_node(
    mut state: ActiveLearningState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ActiveLearningState>> + Send>,
> {
    Box::pin(async move {
        println!("\n[TRAIN] Training tool usage classifier...");

        let mut classifier = ToolUsageClassifier::new();

        println!("[TRAIN] Initial classifier (heuristic weights):");
        let (before_acc, before_correct, before_total) =
            classifier.evaluate(&state.training_examples);
        println!(
            "  - Accuracy: {:.1}% ({}/{})",
            before_acc * 100.0,
            before_correct,
            before_total
        );

        // Train on production data
        classifier.train(&state.training_examples);

        println!("\n[TRAIN] Trained classifier (learned weights):");
        let (after_acc, after_correct, after_total) = classifier.evaluate(&state.training_examples);
        println!(
            "  - Accuracy: {:.1}% ({}/{})",
            after_acc * 100.0,
            after_correct,
            after_total
        );

        state.classifier_version = classifier.version;
        state.classifier_accuracy = after_acc;

        if after_acc > before_acc {
            println!(
                "\n[TRAIN] ✓ Improvement: {:.1}% → {:.1}%",
                before_acc * 100.0,
                after_acc * 100.0
            );
        } else {
            println!("\n[TRAIN] No improvement (may need more data or better features)");
        }

        Ok(state)
    })
}

fn deploy_classifier_node(
    state: ActiveLearningState,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = dashflow::Result<ActiveLearningState>> + Send>,
> {
    Box::pin(async move {
        println!(
            "\n[DEPLOY] Deploying classifier version {}...",
            state.classifier_version
        );
        println!(
            "[DEPLOY] Model accuracy: {:.1}%",
            state.classifier_accuracy * 100.0
        );
        println!(
            "[DEPLOY] Training size: {} examples",
            state.training_examples.len()
        );
        println!("[DEPLOY] ✓ Deployed to production");
        println!("[DEPLOY] Future queries will use this model");

        Ok(state)
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sep = "=".repeat(80);
    println!("{}", sep);
    println!("INNOVATION 12: Active Learning from Production");
    println!("{}", sep);

    println!("\n📋 Problem:");
    println!("   - Static prompts/rules don't improve over time");
    println!("   - Manual rule updates don't scale");
    println!("   - System doesn't learn from real usage patterns");

    println!("\n💡 Solution:");
    println!("   - Collect production data (queries + outcomes)");
    println!("   - Train classifier on real patterns");
    println!("   - Deploy updated model automatically");
    println!("   - Continuous improvement cycle");

    println!("\n🏗️ Architecture:");
    println!("   Production → Collect data → Label → Train → Deploy → Production");
    println!("   (queries)    (Kafka/DB)     (auto)  (ML)    (v++)    (improved)");
    println!("        ↑                                                    ↓");
    println!("        └──────────── continuous improvement loop ──────────┘");

    println!("\n{}", sep);
    println!("ACTIVE LEARNING CYCLE");
    println!("{}", sep);

    // Build graph
    let mut graph = StateGraph::<ActiveLearningState>::new();

    graph.add_node_from_fn("collect", collect_production_data_node);
    graph.add_node_from_fn("label", create_training_data_node);
    graph.add_node_from_fn("train", train_classifier_node);
    graph.add_node_from_fn("deploy", deploy_classifier_node);

    graph.set_entry_point("collect");
    graph.add_edge("collect", "label");
    graph.add_edge("label", "train");
    graph.add_edge("train", "deploy");
    graph.add_edge("deploy", END);

    let app = graph.compile()?;

    // Run active learning cycle
    let initial_state = ActiveLearningState::default();
    let execution_result = app.invoke(initial_state).await?;
    let result = execution_result.final_state;

    println!("\n{}", sep);
    println!("TRAINING DATA EXAMPLES");
    println!("{}", sep);

    // Show some training examples
    println!("\n📝 Sample labeled examples:");
    for (i, example) in result.training_examples.iter().take(5).enumerate() {
        println!("\nExample {}:", i + 1);
        println!("  Query: \"{}\"", example.query);
        println!("  Tool used: {}", example.tool_used);
        println!("  Quality: {:.2}", example.quality_score);
        println!("  Label: Should use tools = {}", example.should_use_tool);

        let reasoning = if example.should_use_tool {
            if example.tool_used && example.quality_score > 0.85 {
                "(Tool used AND high quality → Good example)"
            } else {
                "(No tool AND low quality → Should have used tool!)"
            }
        } else {
            "(No tool needed OR tool didn't help)"
        };
        println!("  Reasoning: {}", reasoning);
    }

    println!("\n{}", sep);
    println!("CONTINUOUS IMPROVEMENT");
    println!("{}", sep);

    println!("\n📈 How it works in production:");
    println!("\n1. Data Collection (Real-time):");
    println!("   - Every query → Kafka event");
    println!("   - Track: query text, tool_used (bool), quality_score (0-1)");
    println!("   - Store: Database (PostgreSQL, S3)");

    println!("\n2. Labeling (Automatic):");
    println!("   - Rule: tool_used=true AND quality>0.85 → label=true (use tools)");
    println!("   - Rule: tool_used=false AND quality<0.70 → label=true (should have!)");
    println!("   - Collect 1000s of labeled examples per week");

    println!("\n3. Training (Weekly):");
    println!("   - Extract features from queries");
    println!("   - Train classifier (scikit-learn, PyTorch, etc.)");
    println!("   - Validate on held-out test set");
    println!("   - Compare to previous version");

    println!("\n4. Deployment (Automated):");
    println!("   - If new model accuracy > old + 1%:");
    println!("     → Package model (ONNX, pickle)");
    println!("     → Deploy to inference service");
    println!("     → Update version in config");
    println!("     → Monitor A/B test metrics");

    println!("\n5. Monitoring (Continuous):");
    println!("   - Track prediction accuracy");
    println!("   - Alert if accuracy drops");
    println!("   - Detect data drift");
    println!("   - Retrain when needed");

    println!("\n{}", sep);
    println!("BENEFITS");
    println!("{}", sep);

    println!("\n💡 Key Benefits:");
    println!("   ✓ Self-improving: Gets better from real usage");
    println!("   ✓ Scalable: Learns patterns, doesn't need manual rules");
    println!("   ✓ Adaptive: Adjusts to changing user patterns");
    println!("   ✓ Data-driven: Learns from 1000s of examples, not gut feeling");

    println!("\n📊 Expected Improvement Trajectory:");
    println!("   - Week 1: 65% accuracy (heuristic baseline)");
    println!("   - Week 4: 75% accuracy (100 examples)");
    println!("   - Week 12: 85% accuracy (1000 examples)");
    println!("   - Week 24: 90% accuracy (5000 examples)");
    println!("   - Week 52: 95% accuracy (20000 examples)");

    println!("\n🎯 Impact on 100% Quality Goal:");
    println!("   - Learns when tools are needed from REAL usage");
    println!("   - Adapts to user patterns (domain-specific vocabulary)");
    println!("   - Continuously improves (never stops learning)");
    println!("   - Combines with ALL other innovations (better routing, confidence, etc.)");

    println!("\n{}", sep);
    println!("EXAMPLE: Weekly Update Cycle");
    println!("{}", sep);

    println!("\n📅 Monday 8am: Collect data from last week");
    println!("   - 5,427 queries processed");
    println!("   - 3,891 with tool calls");
    println!("   - 1,536 without tools");

    println!("\n📅 Monday 9am: Label data");
    println!("   - 2,134 positive examples (should use tools)");
    println!("   - 3,293 negative examples (don't need tools)");

    println!("\n📅 Monday 10am: Train new model");
    println!("   - Old model (v47): 87.3% accuracy");
    println!("   - New model (v48): 89.1% accuracy (+1.8pp)");

    println!("\n📅 Monday 11am: Deploy to production");
    println!("   - A/B test: 10% traffic to v48");
    println!("   - Monitor: Quality scores, latency, errors");

    println!("\n📅 Tuesday 8am: A/B test results");
    println!("   - v47 (old): 90.2% quality average");
    println!("   - v48 (new): 92.4% quality average (+2.2pp) ✓");
    println!("   - Decision: Roll out v48 to 100%");

    println!("\n📅 Next Monday: Repeat cycle");
    println!("   - Collect new data (including v48 decisions)");
    println!("   - Train v49");
    println!("   - Continuous improvement never stops!");

    println!("\n{}", sep);
    println!("\n✅ SUCCESS: Active learning cycle demonstrated!");
    println!("   Version: {}", result.classifier_version);
    println!("   Accuracy: {:.1}%", result.classifier_accuracy * 100.0);
    println!(
        "   Training size: {} examples",
        result.training_examples.len()
    );

    println!("\n{}", sep);

    Ok(())
}
