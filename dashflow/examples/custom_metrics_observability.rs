//! Custom Metrics and Advanced Observability Example
//!
//! This example demonstrates:
//! 1. Custom metrics API usage
//! 2. LLM-specific metrics tracking
//! 3. Integration with OpenTelemetry tracing
//! 4. Structured logging with trace context
//!
//! Run with:
//! ```bash
//! cargo run --example custom_metrics_observability
//! ```

use dashflow::core::observability::{CustomMetricsRegistry, LLMMetrics};
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Custom Metrics & Observability Example\n");
    println!("==========================================\n");

    // Example 1: Custom Counter Metrics
    example_custom_counters()?;

    // Example 2: Custom Gauge Metrics
    example_custom_gauges()?;

    // Example 3: Custom Histogram Metrics
    example_custom_histograms()?;

    // Example 4: LLM Metrics
    example_llm_metrics().await?;

    // Example 5: Export All Metrics
    example_export_metrics()?;

    Ok(())
}

/// Example 1: Custom Counter Metrics
///
/// Counters track monotonically increasing values like:
/// - Total requests
/// - Total errors
/// - Total events
fn example_custom_counters() -> Result<(), Box<dyn std::error::Error>> {
    println!("1️⃣  Custom Counter Metrics");
    println!("─────────────────────────");

    let registry = CustomMetricsRegistry::global();

    // Register a custom counter
    registry.register_counter(
        "user_actions_total",
        "Total number of user actions by type",
        &["action_type", "user_tier"],
    )?;

    // Record some events
    println!("   Recording user actions...");
    registry.increment_counter(
        "user_actions_total",
        &[("action_type", "login"), ("user_tier", "premium")],
    )?;
    registry.increment_counter(
        "user_actions_total",
        &[("action_type", "login"), ("user_tier", "free")],
    )?;
    registry.increment_counter(
        "user_actions_total",
        &[("action_type", "query"), ("user_tier", "premium")],
    )?;

    // Add custom amount to counter
    registry.add_counter(
        "user_actions_total",
        &[("action_type", "query"), ("user_tier", "free")],
        5.0,  // 5 queries
    )?;

    println!("   ✅ Recorded 4 unique metric combinations\n");

    Ok(())
}

/// Example 2: Custom Gauge Metrics
///
/// Gauges track values that can go up and down like:
/// - Active connections
/// - Queue size
/// - Memory usage
fn example_custom_gauges() -> Result<(), Box<dyn std::error::Error>> {
    println!("2️⃣  Custom Gauge Metrics");
    println!("────────────────────────");

    let registry = CustomMetricsRegistry::global();

    // Register a custom gauge
    registry.register_gauge(
        "active_sessions",
        "Number of active user sessions by type",
        &["session_type"],
    )?;

    println!("   Simulating session lifecycle...");

    // Start sessions
    registry.set_gauge("active_sessions", &[("session_type", "chat")], 5.0)?;
    registry.set_gauge("active_sessions", &[("session_type", "api")], 3.0)?;
    println!("   📊 Current sessions: 5 chat, 3 API");

    // New session connects
    registry.increment_gauge("active_sessions", &[("session_type", "chat")])?;
    println!("   ➕ New chat session connected (6 total)");

    // Session disconnects
    registry.decrement_gauge("active_sessions", &[("session_type", "api")])?;
    println!("   ➖ API session disconnected (2 remaining)\n");

    Ok(())
}

/// Example 3: Custom Histogram Metrics
///
/// Histograms track distributions of values like:
/// - Request duration
/// - Response size
/// - Token counts
fn example_custom_histograms() -> Result<(), Box<dyn std::error::Error>> {
    println!("3️⃣  Custom Histogram Metrics");
    println!("───────────────────────────");

    let registry = CustomMetricsRegistry::global();

    // Register a custom histogram with specific buckets
    registry.register_histogram(
        "response_size_bytes",
        "Distribution of response sizes in bytes",
        &["endpoint"],
        Some(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0]),
    )?;

    println!("   Recording response sizes...");

    // Observe various response sizes
    let responses = vec![
        (150, "/api/chat"),
        (450, "/api/chat"),
        (1200, "/api/search"),
        (8500, "/api/search"),
        (250, "/api/chat"),
    ];

    for (size, endpoint) in responses {
        registry.observe_histogram(
            "response_size_bytes",
            &[("endpoint", endpoint)],
            size as f64,
        )?;
        println!("   📦 {}: {} bytes", endpoint, size);
    }

    println!("   ✅ Recorded 5 response size observations\n");

    Ok(())
}

/// Example 4: LLM-Specific Metrics
///
/// Track LLM API calls, token usage, errors, and cache hits
async fn example_llm_metrics() -> Result<(), Box<dyn std::error::Error>> {
    println!("4️⃣  LLM-Specific Metrics");
    println!("────────────────────────");

    // Initialize standard LLM metrics
    let llm_metrics = LLMMetrics::init()?;
    println!("   📊 Initialized standard LLM metrics");

    // Simulate LLM API calls
    println!("\n   Simulating LLM API calls...\n");

    // Successful call 1: OpenAI GPT-4
    {
        let start = Instant::now();
        llm_metrics.start_request("openai", "gpt-4")?;

        // Simulate API call delay
        sleep(Duration::from_millis(1500)).await;

        let duration = start.elapsed().as_secs_f64();
        llm_metrics.record_call("openai", "gpt-4", duration, 150, 75)?;
        llm_metrics.end_request("openai", "gpt-4")?;

        println!(
            "   ✅ OpenAI GPT-4: {} tokens (150 prompt + 75 completion) in {:.2}s",
            150 + 75,
            duration
        );
    }

    // Successful call 2: Anthropic Claude
    {
        let start = Instant::now();
        llm_metrics.start_request("anthropic", "claude-3-5-sonnet")?;

        sleep(Duration::from_millis(1200)).await;

        let duration = start.elapsed().as_secs_f64();
        llm_metrics.record_call("anthropic", "claude-3-5-sonnet", duration, 200, 100)?;
        llm_metrics.end_request("anthropic", "claude-3-5-sonnet")?;

        println!(
            "   ✅ Anthropic Claude: {} tokens (200 prompt + 100 completion) in {:.2}s",
            200 + 100,
            duration
        );
    }

    // Cache hit
    {
        llm_metrics.record_cache_hit("openai", "gpt-4")?;
        println!("   💾 Cache hit for OpenAI GPT-4 (saved API call)");
    }

    // Error scenario
    {
        llm_metrics.record_error("openai", "gpt-4", "rate_limit_exceeded")?;
        println!("   ❌ OpenAI GPT-4: Rate limit exceeded");
    }

    println!("\n   📈 LLM Metrics Summary:");
    println!("      • 2 successful calls (GPT-4, Claude)");
    println!("      • 1 cache hit");
    println!("      • 1 error (rate limit)");
    println!("      • Total tokens: {} (450 prompt + 175 completion)\n", 450 + 175);

    Ok(())
}

/// Example 5: Export All Metrics
///
/// Export metrics in Prometheus text format
fn example_export_metrics() -> Result<(), Box<dyn std::error::Error>> {
    println!("5️⃣  Export Metrics");
    println!("───────────────────");

    let registry = CustomMetricsRegistry::global();

    // Get all metrics in Prometheus format
    let metrics_output = registry.get_metrics()?;

    println!("   📤 Exported metrics in Prometheus format:\n");
    println!("   {}", "─".repeat(70));

    // Display a subset of metrics (full output would be very long)
    let lines: Vec<&str> = metrics_output.lines().collect();
    let sample_size = 30.min(lines.len());

    for line in &lines[0..sample_size] {
        println!("   {}", line);
    }

    if lines.len() > sample_size {
        println!("   ... ({} more lines)", lines.len() - sample_size);
    }

    println!("   {}", "─".repeat(70));
    println!("\n   ✅ Total metrics lines: {}\n", lines.len());

    // Summary
    let help_count = lines.iter().filter(|l| l.starts_with("# HELP")).count();
    let type_count = lines.iter().filter(|l| l.starts_with("# TYPE")).count();
    let data_count = lines.len() - help_count - type_count;

    println!("   📊 Metrics Breakdown:");
    println!("      • {} metric definitions (HELP)", help_count);
    println!("      • {} metric types (TYPE)", type_count);
    println!("      • {} data points\n", data_count);

    println!("   💡 These metrics can be scraped by Prometheus at /metrics endpoint\n");

    Ok(())
}

// Helper function: Simulate realistic API call timing
async fn simulate_api_call(duration_ms: u64) {
    sleep(Duration::from_millis(duration_ms)).await;
}
