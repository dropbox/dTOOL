//! GRPO Multihop QA Optimization Example
//!
//! This example demonstrates the concept of GRPO (Group Relative Policy Optimization)
//! for optimizing multi-hop question answering through reinforcement learning.
//!
//! GRPO is a cutting-edge RL optimizer that:
//! 1. Collects execution traces via DashStream
//! 2. Computes reward signals from a metric function
//! 3. Generates multiple rollouts per example
//! 4. Normalizes rewards within groups (reduces variance)
//! 5. Fine-tunes model weights using policy gradients
//!
//! This is a simplified conceptual example. For production use, GRPO requires:
//! - A compiled DashFlow with DashStream integration
//! - Access to model fine-tuning API (OpenAI, local models, etc.)
//! - Kafka for trace collection
//!
//! Run with: cargo run --package dashflow --example grpo_multihop_qa

use dashflow::optimize::optimizers::GRPOConfig;

fn main() -> dashflow::Result<()> {
    println!("=== GRPO (Group Relative Policy Optimization) - Concept Demo ===\n");

    // 1. Configure GRPO optimizer
    println!("⚙️  Configuring GRPO Optimizer:");
    let config = GRPOConfig::new()
        .with_num_train_steps(10) // Number of RL training iterations
        .with_num_examples_per_step(2) // Examples per training batch
        .with_num_rollouts_per_step(4) // Generate 4 completions per example
        .with_kafka_brokers("localhost:9092")
        .with_kafka_topic("dashstream-events")
        .with_failure_score(0.0)
        .with_format_failure_score(-1.0)
        .with_reinforce_config(dashflow::core::language_models::ReinforceConfig::default());
    println!("   Training steps: {}", config.num_train_steps);
    println!("   Examples per step: {}", config.num_examples_per_step);
    println!("   Rollouts per step: {}", config.num_rollouts_per_step);
    println!("   Kafka: {}", config.kafka_brokers);
    println!();

    // 2. Define reward metric
    println!("📊 Defining Reward Metric:");
    println!("   ```rust");
    println!("   let metric = Arc::new(|example, prediction, trace| {{");
    println!("       // Compare prediction to expected output");
    println!("       // Return reward score (-1.0 to 1.0)");
    println!("       if prediction.get(\"answer\") == example.get(\"answer\") {{");
    println!("           Ok(1.0)  // Correct answer");
    println!("       }} else {{");
    println!("           Ok(0.0)  // Incorrect");
    println!("       }}");
    println!("   }});");
    println!("   ```");
    println!("   Higher rewards = better predictions (should be reinforced)");
    println!("   Lower rewards = worse predictions (should be discouraged)");
    println!();

    // 3. Create GRPO optimizer
    println!("🔧 Creating GRPO Optimizer:");
    println!("   ```rust");
    println!("   let grpo = GRPO::new(metric, config);");
    println!("   ```");
    println!("   ✅ GRPO optimizer created");
    println!();

    // 4. Demonstrate concept
    println!("=== How GRPO Works ===\n");

    println!("1. Trace Collection (via DashStream)");
    println!("   → Graph executes with training examples");
    println!("   → DashStream logs all events to Kafka");
    println!("   → TraceCollector consumes events after execution\n");

    println!("2. Rollout Generation");
    println!("   → For each example, generate multiple completions");
    println!("   → Example: 'What is the capital of France?'");
    println!("     Rollout 1: 'Paris' (reward: 1.0)");
    println!("     Rollout 2: 'Lyon' (reward: 0.0)");
    println!("     Rollout 3: 'Paris' (reward: 1.0)");
    println!("     Rollout 4: 'Marseille' (reward: 0.0)\n");

    println!("3. Group Relative Normalization");
    println!("   → Normalize rewards within each group (rollouts for same example)");
    println!("   → Reduces variance, improves training stability");
    println!("   → Mean = 0, Variance = 1 per group\n");

    println!("4. Policy Gradient Optimization");
    println!("   → Create weighted training examples");
    println!("   → High-reward examples get positive weight");
    println!("   → Low-reward examples get negative weight\n");

    println!("5. Model Fine-Tuning");
    println!("   → Submit examples to ChatModel::reinforce() API");
    println!("   → OpenAI: Uses fine-tuning API");
    println!("   → Local models: Uses training service");
    println!("   → Wait for training job completion\n");

    println!("6. Return Updated Model");
    println!("   → Model weights updated based on rewards");
    println!("   → Reinforced to produce high-reward outputs");
    println!("   → Discouraged from low-reward outputs\n");

    println!("=== Key Benefits ===\n");
    println!("✓ Group-relative normalization reduces variance");
    println!("✓ Integrates with existing DashStream infrastructure");
    println!("✓ Supports both OpenAI and local models");
    println!("✓ Fine-tunes model weights, not just prompts");
    println!("✓ Best-in-class for complex reasoning tasks\n");

    println!("=== Production Usage ===\n");
    println!("For production use:");
    println!("1. Build a DashFlow with DashStream integration");
    println!("2. Compile the graph: let app = graph.compile()?;");
    println!("3. Run GRPO optimization: grpo.optimize(app, trainset).await?;");
    println!("4. Deploy optimized model\n");

    println!("See integration tests for full examples:");
    println!("  tests/optimizer_integration_tests.rs\n");

    Ok(())
}
