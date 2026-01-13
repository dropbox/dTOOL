//! Optimizer Composition Example (BetterTogether)
//!
//! This example demonstrates the concept of BetterTogether, a meta-optimizer that
//! composes multiple optimizers into pipelines.
//!
//! BetterTogether enables:
//! 1. Sequential optimization (Optimizer A → B → C)
//! 2. Parallel optimization (run all, pick best)
//! 3. Ensemble optimization (combine results)
//!
//! This is a simplified conceptual example demonstrating optimization pipeline concepts.
//!
//! Run with: cargo run --package dashflow --example optimizer_composition

use dashflow::optimize::BootstrapFewShot;

fn main() -> dashflow::Result<()> {
    println!("=== BetterTogether Meta-Optimizer - Concept Demo ===\n");

    // 1. Create individual optimizers
    println!("🔧 Creating Individual Optimizers:");
    let bootstrap = BootstrapFewShot::new().with_max_demos(3);
    println!("   ✓ BootstrapFewShot (max_demos=3)");

    // In production, you might add:
    // let mipro = MIPROv2::new(metric.clone());
    // let copro = COPRO::new(metric.clone());
    println!("   (In production: MIPROv2, COPRO, GRPO, etc.)\n");

    // 2. Create BetterTogether with sequential strategy
    println!("⚡ Creating BetterTogether Pipeline:");
    println!("   ```rust");
    println!("   let mut pipeline = BetterTogether::new()");
    println!("       .add_optimizer(Box::new(BootstrapFewShot::new()))");
    println!("       .add_optimizer(Box::new(MIPROv2::new(metric)))");
    println!("       .add_optimizer(Box::new(COPRO::new(metric)));");
    println!("   ```");
    println!("   ✓ BetterTogether created");
    println!("   Strategy: Sequential (default)");
    println!("   Pipeline: BootstrapFewShot → MIPROv2 → COPRO\n");

    // Silence unused variable warning
    let _ = bootstrap;

    // 3. Demonstrate composition strategies
    println!("=== Composition Strategies ===\n");

    println!("1. Sequential Strategy");
    println!("   → Run optimizers one after another");
    println!("   → Each optimizer builds on previous results");
    println!("   → Example: Bootstrap → Hyperparameter tuning → Fine-tuning");
    println!("   → Best for: Multi-stage optimization\n");

    println!("2. Parallel Strategy");
    println!("   → Run all optimizers simultaneously");
    println!("   → Pick best result based on metric");
    println!("   → Example: Try BootstrapFewShot vs MIPROv2, keep winner");
    println!("   → Best for: Exploring different strategies");
    println!(
        "   → See: dashflow::optimize::optimizers::ensemble::Ensemble::builder().with_size(k)\n"
    );

    println!("3. Ensemble Strategy (IMPLEMENTED)");
    println!("   → Run all optimizers");
    println!("   → Combine results (voting, averaging)");
    println!("   → Example: Merge few-shot examples from multiple optimizers");
    println!("   → Best for: Robustness");
    println!("   → See: dashflow::optimize::optimizers::ensemble::Ensemble::builder().with_reduce_fn()\n");

    // 4. Example pipeline scenarios
    println!("=== Example Pipelines ===\n");

    println!("Pipeline 1: Quick Optimization");
    println!("   BootstrapFewShot → BootstrapOptuna");
    println!("   1. Bootstrap generates few-shot examples");
    println!("   2. Optuna tunes hyperparameters");
    println!("   Result: Fast, effective optimization\n");

    println!("Pipeline 2: Maximum Quality");
    println!("   BootstrapFewShot → MIPROv2 → COPRO");
    println!("   1. Bootstrap: Initial few-shot examples");
    println!("   2. MIPROv2: Optimize both demos and instructions");
    println!("   3. COPRO: Fine-tune instructions with LLM meta-prompting");
    println!("   Result: Highest quality, longer runtime\n");

    println!("Pipeline 3: Cost Optimization");
    println!("   BootstrapFewShot → BootstrapFinetune → GRPO");
    println!("   1. Bootstrap: Collect successful examples");
    println!("   2. BootstrapFinetune: Export fine-tuning dataset");
    println!("   3. GRPO: RL-based fine-tuning");
    println!("   Result: Optimized model weights, not just prompts\n");

    // 5. Key benefits
    println!("=== Key Benefits ===\n");
    println!("✓ Experiment with different optimization strategies");
    println!("✓ Combine complementary optimizers");
    println!("✓ Multi-stage optimization for maximum quality");
    println!("✓ Meta-optimization: find best optimization pipeline");
    println!("✓ Flexible: add custom optimizers to pipeline\n");

    // 6. Production usage
    println!("=== Production Usage ===\n");
    println!("```rust");
    println!("let mut pipeline = BetterTogether::new(CompositionStrategy::Sequential);");
    println!("pipeline.add_stage(Box::new(BootstrapFewShot::new()));");
    println!("pipeline.add_stage(Box::new(MIPROv2::new(metric)));");
    println!("pipeline.add_stage(Box::new(COPRO::new(metric)));");
    println!();
    println!("// Optimize graph through entire pipeline");
    println!("let optimized = pipeline.optimize(graph, trainset, metric).await?;");
    println!("```\n");

    println!("See integration tests for full examples:");
    println!("  tests/optimizer_integration_tests.rs\n");

    Ok(())
}
