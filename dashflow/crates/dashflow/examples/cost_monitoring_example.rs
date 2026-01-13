//! Cost Monitoring and Budget Enforcement Example
//!
//! Demonstrates the cost monitoring features:
//! - Track LLM API costs in real-time
//! - Set daily/monthly budget limits
//! - Alert when approaching budget limits
//! - Hard vs soft budget enforcement
//! - Prometheus metrics export
//!
//! Note: This example uses `dashflow_observability::cost` which is the canonical
//! cost tracking module. The `dashflow::optimize::cost_monitoring` module is deprecated.

use dashflow_observability::cost::{AlertLevel, BudgetConfig, BudgetEnforcer, CostTracker};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cost Monitoring Example ===\n");

    // Example 1: Basic Cost Monitoring
    println!("Example 1: Basic Cost Monitoring");
    println!("---------------------------------");
    basic_monitoring()?;

    // Example 2: Budget Alerts (Soft Limits)
    println!("\n\nExample 2: Budget Alerts (Soft Limits)");
    println!("----------------------------------------");
    soft_limit_alerts()?;

    // Example 3: Budget Enforcement (Hard Limits)
    println!("\n\nExample 3: Budget Enforcement (Hard Limits)");
    println!("---------------------------------------------");
    hard_limit_enforcement()?;

    // Example 4: Prometheus Metrics Export
    println!("\n\nExample 4: Prometheus Metrics Export");
    println!("--------------------------------------");
    prometheus_export()?;

    // Example 5: Multi-Model Cost Tracking
    println!("\n\nExample 5: Multi-Model Cost Tracking");
    println!("--------------------------------------");
    multi_model_tracking()?;

    Ok(())
}

fn basic_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    let tracker = CostTracker::with_defaults();

    // Simulate API usage
    let cost1 = tracker.record_usage("gpt-4o-mini", 1000, 500)?;
    println!("Request 1 (gpt-4o-mini, 1000 in, 500 out): ${:.4}", cost1);

    let cost2 = tracker.record_usage("gpt-4o", 2000, 1000)?;
    println!("Request 2 (gpt-4o, 2000 in, 1000 out): ${:.4}", cost2);

    let cost3 = tracker.record_usage("claude-3-haiku-20240307", 1500, 750)?;
    println!(
        "Request 3 (claude-3-haiku, 1500 in, 750 out): ${:.4}",
        cost3
    );

    // Get cost report
    let report = tracker.report();
    println!("\nCost Report:");
    println!("  Total spent: ${:.4}", report.total_cost);
    println!("  Total requests: {}", report.total_calls);
    println!(
        "  Average cost/request: ${:.4}",
        report.average_cost_per_call()
    );
    println!("\n  Breakdown by model:");
    for (model, cost) in &report.cost_by_model {
        println!("    {}: ${:.4}", model, cost);
    }

    Ok(())
}

fn soft_limit_alerts() -> Result<(), Box<dyn std::error::Error>> {
    let config = BudgetConfig::with_daily_limit(0.02)
        .warning_threshold(0.7)
        .critical_threshold(0.9)
        .enforce_hard_limit(false); // Soft limit - don't block requests

    let enforcer = BudgetEnforcer::new(CostTracker::with_defaults(), config);

    // First request: ~$0.00135 (67.5% of $0.02 budget)
    println!("Request 1 (3000 in, 1500 out)...");
    enforcer.record_and_check("gpt-4o-mini", 3000, 1500)?;
    match enforcer.alert_level() {
        None => println!("  Status: OK"),
        Some(AlertLevel::Warning) => println!("  Status: WARNING - Approaching budget limit"),
        Some(AlertLevel::Critical) => println!("  Status: CRITICAL - Budget limit exceeded"),
    }

    // Second request: ~$0.0027 total (135% of budget)
    println!("\nRequest 2 (3000 in, 1500 out)...");
    enforcer.record_and_check("gpt-4o-mini", 3000, 1500)?;
    match enforcer.alert_level() {
        None => println!("  Status: OK"),
        Some(AlertLevel::Warning) => println!("  Status: WARNING - Approaching budget limit"),
        Some(AlertLevel::Critical) => println!("  Status: CRITICAL - Budget limit exceeded"),
    }

    let report = enforcer.tracker().report();
    println!("\nFinal Report:");
    println!("  Spent today: ${:.4}", report.spent_today);
    println!("  Daily limit: ${:.4}", report.daily_limit.unwrap());
    println!("  Usage: {:.1}%", report.daily_usage_percent.unwrap_or(0.0));
    println!("\nNote: Soft limit allows requests to continue even when budget exceeded.");

    Ok(())
}

fn hard_limit_enforcement() -> Result<(), Box<dyn std::error::Error>> {
    let config = BudgetConfig::with_daily_limit(0.002)
        .warning_threshold(0.3) // Set warning lower than critical
        .critical_threshold(0.5)
        .enforce_hard_limit(true); // Hard limit - block requests

    let enforcer = BudgetEnforcer::new(CostTracker::with_defaults(), config);

    // First request: ~$0.00135 (67.5% of $0.002 budget)
    println!("Request 1 (3000 in, 1500 out)...");
    match enforcer.record_and_check("gpt-4o-mini", 3000, 1500) {
        Ok(cost) => println!("  Success: ${:.4}", cost),
        Err(e) => println!("  Blocked: {}", e),
    }

    let report = enforcer.tracker().report();
    println!(
        "  Current usage: {:.1}%",
        report.daily_usage_percent.unwrap_or(0.0)
    );

    // Second request: Would exceed critical threshold (50%)
    println!("\nRequest 2 (3000 in, 1500 out)...");
    match enforcer.record_and_check("gpt-4o-mini", 3000, 1500) {
        Ok(cost) => println!("  Success: ${:.4}", cost),
        Err(e) => println!("  Blocked: {}", e),
    }

    println!(
        "\nNote: Hard limit blocks requests when budget exceeded, protecting against overruns."
    );

    Ok(())
}

fn prometheus_export() -> Result<(), Box<dyn std::error::Error>> {
    let tracker = CostTracker::with_defaults();

    // Simulate some usage
    tracker.record_usage("gpt-4o-mini", 5000, 2500)?;
    tracker.record_usage("gpt-4o", 3000, 1500)?;
    tracker.record_usage("claude-3-haiku-20240307", 2000, 1000)?;

    // Export metrics in Prometheus format
    let metrics = tracker.export_prometheus();
    println!("{}", metrics);

    println!("Note: These metrics can be scraped by Prometheus for monitoring dashboards.");

    Ok(())
}

fn multi_model_tracking() -> Result<(), Box<dyn std::error::Error>> {
    let tracker = CostTracker::with_defaults().with_daily_budget(1.0);

    // Simulate different model usage
    let models = vec![
        ("gpt-4o-mini", 10000, 5000),
        ("gpt-4o", 5000, 2500),
        ("gpt-3.5-turbo", 15000, 7500),
        ("claude-3-5-sonnet-20241022", 8000, 4000),
        ("claude-3-haiku-20240307", 20000, 10000),
        ("gemini-1.5-flash", 12000, 6000),
    ];

    for (model, input_tokens, output_tokens) in models {
        let cost = tracker.record_usage(model, input_tokens, output_tokens)?;
        println!("{}: ${:.4}", model, cost);
    }

    let report = tracker.report();
    println!("\nTotal Cost: ${:.4}", report.total_cost);
    println!("Budget Remaining: ${:.4}", 1.0 - report.spent_today);
    println!(
        "Budget Usage: {:.1}%",
        report.daily_usage_percent.unwrap_or(0.0)
    );

    println!("\nMost Expensive Models:");
    let mut by_model: Vec<_> = report.cost_by_model.iter().collect();
    by_model.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (i, (model, cost)) in by_model.iter().take(3).enumerate() {
        println!("  {}. {}: ${:.4}", i + 1, model, cost);
    }

    Ok(())
}
