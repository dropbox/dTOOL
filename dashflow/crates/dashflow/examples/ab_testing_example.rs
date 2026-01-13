/// A/B Testing Example - Compare Optimizer Variants
///
/// This example demonstrates how to use the A/B testing framework to compare
/// two different optimization strategies for a sentiment classifier.
use dashflow::optimize::ab_testing::ABTest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== A/B Testing Example: Optimizer Comparison ===\n");

    // Create A/B test comparing baseline vs optimized classifier
    let mut ab_test = ABTest::new("sentiment_classifier_v2")
        .with_minimum_sample_size(40) // Allow for hash-based traffic split variance
        .with_significance_level(0.05);

    // Add two variants: baseline (50%) and optimized (50%)
    ab_test.add_variant("baseline", 0.5);
    ab_test.add_variant("optimized", 0.5);

    println!("Configured A/B test with 2 variants:");
    println!("  - baseline: 50% traffic");
    println!("  - optimized: 50% traffic\n");

    // Simulate 100 user requests with different results for each variant
    println!("Simulating 100 user requests...\n");

    for i in 0..100 {
        let user_id = format!("user_{}", i);

        // Deterministically assign variant based on user_id hash
        let variant = ab_test.assign_variant(&user_id).to_string();

        // Simulate different accuracy for baseline vs optimized
        // Baseline: mean ~= 0.75, Optimized: mean ~= 0.85
        let accuracy = match variant.as_str() {
            "baseline" => {
                // Baseline performance: 0.70-0.80 accuracy
                0.75 + ((i % 10) as f64 - 5.0) * 0.01
            }
            "optimized" => {
                // Optimized performance: 0.80-0.90 accuracy
                0.85 + ((i % 10) as f64 - 5.0) * 0.01
            }
            _ => unreachable!(),
        };

        ab_test.record_result(&variant, accuracy)?;
    }

    println!("Total observations: {}", ab_test.total_observations());
    println!(
        "Minimum sample size reached: {}\n",
        ab_test.has_minimum_samples()
    );

    // Analyze results
    println!("Analyzing results...\n");
    let report = ab_test.analyze()?;

    // Print summary to console
    println!("{}", report.summary());

    // Save reports
    report.save_markdown("ab_test_results.md")?;
    report.save_html("ab_test_results.html")?;

    println!("\nReports saved:");
    println!("  - ab_test_results.md (Markdown)");
    println!("  - ab_test_results.html (HTML)");

    Ok(())
}
