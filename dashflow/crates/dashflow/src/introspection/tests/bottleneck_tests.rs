//! Bottleneck detection tests for the introspection module.

use super::*;

#[test]
fn test_bottleneck_metric_display() {
    assert_eq!(BottleneckMetric::Latency.to_string(), "latency");
    assert_eq!(BottleneckMetric::TokenUsage.to_string(), "token_usage");
    assert_eq!(BottleneckMetric::ErrorRate.to_string(), "error_rate");
    assert_eq!(
        BottleneckMetric::HighFrequency.to_string(),
        "high_frequency"
    );
    assert_eq!(BottleneckMetric::HighVariance.to_string(), "high_variance");
}

#[test]
fn test_bottleneck_severity_display() {
    assert_eq!(BottleneckSeverity::Minor.to_string(), "minor");
    assert_eq!(BottleneckSeverity::Moderate.to_string(), "moderate");
    assert_eq!(BottleneckSeverity::Severe.to_string(), "severe");
    assert_eq!(BottleneckSeverity::Critical.to_string(), "critical");
}

#[test]
fn test_bottleneck_severity_ordering() {
    assert!(BottleneckSeverity::Minor < BottleneckSeverity::Moderate);
    assert!(BottleneckSeverity::Moderate < BottleneckSeverity::Severe);
    assert!(BottleneckSeverity::Severe < BottleneckSeverity::Critical);
}

#[test]
fn test_bottleneck_severity_default() {
    let severity: BottleneckSeverity = Default::default();
    assert_eq!(severity, BottleneckSeverity::Minor);
}

#[test]
fn test_bottleneck_new() {
    let bottleneck = Bottleneck::new(
        "test_node",
        BottleneckMetric::Latency,
        75.0,
        50.0,
        BottleneckSeverity::Severe,
        "High latency",
        "Optimize",
    );

    assert_eq!(bottleneck.node, "test_node");
    assert_eq!(bottleneck.metric, BottleneckMetric::Latency);
    assert_eq!(bottleneck.value, 75.0);
    assert_eq!(bottleneck.threshold, 50.0);
    assert_eq!(bottleneck.severity, BottleneckSeverity::Severe);
    assert_eq!(bottleneck.description, "High latency");
    assert_eq!(bottleneck.suggestion, "Optimize");
    assert!(bottleneck.percentage_of_total.is_none());
}

#[test]
fn test_bottleneck_with_percentage() {
    let bottleneck = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        75.0,
        50.0,
        BottleneckSeverity::Severe,
        "desc",
        "sug",
    )
    .with_percentage(75.0);

    assert_eq!(bottleneck.percentage_of_total, Some(75.0));
}

#[test]
fn test_bottleneck_is_critical() {
    let critical = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "desc",
        "sug",
    );
    let severe = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        75.0,
        70.0,
        BottleneckSeverity::Severe,
        "desc",
        "sug",
    );

    assert!(critical.is_critical());
    assert!(!severe.is_critical());
}

#[test]
fn test_bottleneck_is_severe_or_critical() {
    let critical = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "desc",
        "sug",
    );
    let severe = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        75.0,
        70.0,
        BottleneckSeverity::Severe,
        "desc",
        "sug",
    );
    let moderate = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        55.0,
        50.0,
        BottleneckSeverity::Moderate,
        "desc",
        "sug",
    );
    let minor = Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    );

    assert!(critical.is_severe_or_critical());
    assert!(severe.is_severe_or_critical());
    assert!(!moderate.is_severe_or_critical());
    assert!(!minor.is_severe_or_critical());
}

#[test]
fn test_bottleneck_summary() {
    let bottleneck = Bottleneck::new(
        "slow_node",
        BottleneckMetric::Latency,
        75.5,
        50.0,
        BottleneckSeverity::Severe,
        "Too slow",
        "Optimize",
    );

    let summary = bottleneck.summary();
    assert!(summary.contains("severe"));
    assert!(summary.contains("latency"));
    assert!(summary.contains("slow_node"));
    assert!(summary.contains("75.50"));
    assert!(summary.contains("50.00"));
}

#[test]
fn test_bottleneck_json_roundtrip() {
    let bottleneck = Bottleneck::new(
        "node",
        BottleneckMetric::TokenUsage,
        80.0,
        60.0,
        BottleneckSeverity::Severe,
        "High token usage",
        "Reduce context",
    )
    .with_percentage(80.0);

    let json = bottleneck.to_json().unwrap();
    let parsed = Bottleneck::from_json(&json).unwrap();

    assert_eq!(parsed.node, bottleneck.node);
    assert_eq!(parsed.metric, bottleneck.metric);
    assert_eq!(parsed.value, bottleneck.value);
    assert_eq!(parsed.threshold, bottleneck.threshold);
    assert_eq!(parsed.severity, bottleneck.severity);
    assert_eq!(parsed.percentage_of_total, bottleneck.percentage_of_total);
}

#[test]
fn test_bottleneck_builder() {
    let bottleneck = Bottleneck::builder()
        .node("builder_node")
        .metric(BottleneckMetric::ErrorRate)
        .value(25.0)
        .threshold(15.0)
        .severity(BottleneckSeverity::Moderate)
        .description("High error rate")
        .suggestion("Add retries")
        .percentage_of_total(25.0)
        .build()
        .unwrap();

    assert_eq!(bottleneck.node, "builder_node");
    assert_eq!(bottleneck.metric, BottleneckMetric::ErrorRate);
    assert_eq!(bottleneck.value, 25.0);
    assert_eq!(bottleneck.threshold, 15.0);
    assert_eq!(bottleneck.severity, BottleneckSeverity::Moderate);
    assert_eq!(bottleneck.description, "High error rate");
    assert_eq!(bottleneck.suggestion, "Add retries");
    assert_eq!(bottleneck.percentage_of_total, Some(25.0));
}

#[test]
fn test_bottleneck_builder_missing_node() {
    let result = Bottleneck::builder()
        .metric(BottleneckMetric::Latency)
        .description("desc")
        .suggestion("sug")
        .build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "node is required");
}

#[test]
fn test_bottleneck_builder_missing_metric() {
    let result = Bottleneck::builder()
        .node("node")
        .description("desc")
        .suggestion("sug")
        .build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "metric is required");
}

#[test]
fn test_bottleneck_builder_missing_description() {
    let result = Bottleneck::builder()
        .node("node")
        .metric(BottleneckMetric::Latency)
        .suggestion("sug")
        .build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "description is required");
}

#[test]
fn test_bottleneck_builder_missing_suggestion() {
    let result = Bottleneck::builder()
        .node("node")
        .metric(BottleneckMetric::Latency)
        .description("desc")
        .build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "suggestion is required");
}

#[test]
fn test_bottleneck_thresholds_default() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.latency_percentage_minor, 30.0);
    assert_eq!(thresholds.latency_percentage_moderate, 50.0);
    assert_eq!(thresholds.latency_percentage_severe, 70.0);
    assert_eq!(thresholds.latency_percentage_critical, 85.0);

    assert_eq!(thresholds.latency_absolute_minor_ms, 5000);
    assert_eq!(thresholds.latency_absolute_critical_ms, 60000);

    assert_eq!(thresholds.error_rate_minor, 0.05);
    assert_eq!(thresholds.error_rate_critical, 0.50);

    assert_eq!(thresholds.frequency_minor, 10);
    assert_eq!(thresholds.frequency_critical, 100);
}

#[test]
fn test_bottleneck_thresholds_strict() {
    let thresholds = BottleneckThresholds::strict();

    assert!(
        thresholds.latency_percentage_minor
            < BottleneckThresholds::default().latency_percentage_minor
    );
    assert!(thresholds.error_rate_minor < BottleneckThresholds::default().error_rate_minor);
    assert!(thresholds.frequency_minor < BottleneckThresholds::default().frequency_minor);
}

#[test]
fn test_bottleneck_thresholds_lenient() {
    let thresholds = BottleneckThresholds::lenient();

    assert!(
        thresholds.latency_percentage_minor
            > BottleneckThresholds::default().latency_percentage_minor
    );
    assert!(thresholds.error_rate_minor > BottleneckThresholds::default().error_rate_minor);
    assert!(thresholds.frequency_minor > BottleneckThresholds::default().frequency_minor);
}

#[test]
fn test_bottleneck_thresholds_latency_percentage_severity() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.latency_percentage_severity(20.0), None);
    assert_eq!(
        thresholds.latency_percentage_severity(30.0),
        Some(BottleneckSeverity::Minor)
    );
    assert_eq!(
        thresholds.latency_percentage_severity(50.0),
        Some(BottleneckSeverity::Moderate)
    );
    assert_eq!(
        thresholds.latency_percentage_severity(70.0),
        Some(BottleneckSeverity::Severe)
    );
    assert_eq!(
        thresholds.latency_percentage_severity(85.0),
        Some(BottleneckSeverity::Critical)
    );
    assert_eq!(
        thresholds.latency_percentage_severity(95.0),
        Some(BottleneckSeverity::Critical)
    );
}

#[test]
fn test_bottleneck_thresholds_latency_absolute_severity() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.latency_absolute_severity(1000), None);
    assert_eq!(
        thresholds.latency_absolute_severity(5000),
        Some(BottleneckSeverity::Minor)
    );
    assert_eq!(
        thresholds.latency_absolute_severity(15000),
        Some(BottleneckSeverity::Moderate)
    );
    assert_eq!(
        thresholds.latency_absolute_severity(30000),
        Some(BottleneckSeverity::Severe)
    );
    assert_eq!(
        thresholds.latency_absolute_severity(60000),
        Some(BottleneckSeverity::Critical)
    );
}

#[test]
fn test_bottleneck_thresholds_token_percentage_severity() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.token_percentage_severity(30.0), None);
    assert_eq!(
        thresholds.token_percentage_severity(40.0),
        Some(BottleneckSeverity::Minor)
    );
    assert_eq!(
        thresholds.token_percentage_severity(60.0),
        Some(BottleneckSeverity::Moderate)
    );
    assert_eq!(
        thresholds.token_percentage_severity(80.0),
        Some(BottleneckSeverity::Severe)
    );
    assert_eq!(
        thresholds.token_percentage_severity(90.0),
        Some(BottleneckSeverity::Critical)
    );
}

#[test]
fn test_bottleneck_thresholds_error_rate_severity() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.error_rate_severity(0.02), None);
    assert_eq!(
        thresholds.error_rate_severity(0.05),
        Some(BottleneckSeverity::Minor)
    );
    assert_eq!(
        thresholds.error_rate_severity(0.15),
        Some(BottleneckSeverity::Moderate)
    );
    assert_eq!(
        thresholds.error_rate_severity(0.30),
        Some(BottleneckSeverity::Severe)
    );
    assert_eq!(
        thresholds.error_rate_severity(0.50),
        Some(BottleneckSeverity::Critical)
    );
}

#[test]
fn test_bottleneck_thresholds_frequency_severity() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.frequency_severity(5), None);
    assert_eq!(
        thresholds.frequency_severity(10),
        Some(BottleneckSeverity::Minor)
    );
    assert_eq!(
        thresholds.frequency_severity(25),
        Some(BottleneckSeverity::Moderate)
    );
    assert_eq!(
        thresholds.frequency_severity(50),
        Some(BottleneckSeverity::Severe)
    );
    assert_eq!(
        thresholds.frequency_severity(100),
        Some(BottleneckSeverity::Critical)
    );
}

#[test]
fn test_bottleneck_thresholds_variance_severity() {
    let thresholds = BottleneckThresholds::default();

    assert_eq!(thresholds.variance_severity(0.3), None);
    assert_eq!(
        thresholds.variance_severity(0.5),
        Some(BottleneckSeverity::Minor)
    );
    assert_eq!(
        thresholds.variance_severity(1.0),
        Some(BottleneckSeverity::Moderate)
    );
    assert_eq!(
        thresholds.variance_severity(1.5),
        Some(BottleneckSeverity::Severe)
    );
    assert_eq!(
        thresholds.variance_severity(2.0),
        Some(BottleneckSeverity::Critical)
    );
}

#[test]
fn test_bottleneck_thresholds_json_roundtrip() {
    let thresholds = BottleneckThresholds::strict();
    let json = thresholds.to_json().unwrap();
    let parsed = BottleneckThresholds::from_json(&json).unwrap();

    assert_eq!(
        parsed.latency_percentage_minor,
        thresholds.latency_percentage_minor
    );
    assert_eq!(parsed.error_rate_critical, thresholds.error_rate_critical);
}

#[test]
fn test_bottleneck_analysis_new() {
    let analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    assert!(!analysis.has_bottlenecks());
    assert_eq!(analysis.bottleneck_count(), 0);
    assert!(!analysis.has_critical());
    assert_eq!(analysis.nodes_analyzed, 0);
}

#[test]
fn test_bottleneck_analysis_has_critical() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "node1",
        BottleneckMetric::Latency,
        50.0,
        30.0,
        BottleneckSeverity::Moderate,
        "desc",
        "sug",
    ));
    assert!(!analysis.has_critical());

    analysis.bottlenecks.push(Bottleneck::new(
        "node2",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "desc",
        "sug",
    ));
    assert!(analysis.has_critical());
}

#[test]
fn test_bottleneck_analysis_by_severity() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "node1",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "node2",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "desc",
        "sug",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "node3",
        BottleneckMetric::TokenUsage,
        45.0,
        40.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));

    assert_eq!(analysis.by_severity(BottleneckSeverity::Minor).len(), 2);
    assert_eq!(analysis.by_severity(BottleneckSeverity::Critical).len(), 1);
    assert_eq!(analysis.by_severity(BottleneckSeverity::Moderate).len(), 0);
}

#[test]
fn test_bottleneck_analysis_by_metric() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "node1",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "node2",
        BottleneckMetric::Latency,
        55.0,
        50.0,
        BottleneckSeverity::Moderate,
        "desc",
        "sug",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "node3",
        BottleneckMetric::TokenUsage,
        45.0,
        40.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));

    assert_eq!(analysis.by_metric(&BottleneckMetric::Latency).len(), 2);
    assert_eq!(analysis.by_metric(&BottleneckMetric::TokenUsage).len(), 1);
    assert_eq!(analysis.by_metric(&BottleneckMetric::ErrorRate).len(), 0);
}

#[test]
fn test_bottleneck_analysis_for_node() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "problematic_node",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "problematic_node",
        BottleneckMetric::TokenUsage,
        45.0,
        40.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "other_node",
        BottleneckMetric::Latency,
        55.0,
        50.0,
        BottleneckSeverity::Moderate,
        "desc",
        "sug",
    ));

    assert_eq!(analysis.for_node("problematic_node").len(), 2);
    assert_eq!(analysis.for_node("other_node").len(), 1);
    assert_eq!(analysis.for_node("unknown").len(), 0);
}

#[test]
fn test_bottleneck_analysis_most_severe() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
    assert!(analysis.most_severe().is_none());

    analysis.bottlenecks.push(Bottleneck::new(
        "node1",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));
    assert_eq!(
        analysis.most_severe().unwrap().severity,
        BottleneckSeverity::Minor
    );

    analysis.bottlenecks.push(Bottleneck::new(
        "node2",
        BottleneckMetric::ErrorRate,
        60.0,
        50.0,
        BottleneckSeverity::Critical,
        "desc",
        "sug",
    ));
    assert_eq!(
        analysis.most_severe().unwrap().severity,
        BottleneckSeverity::Critical
    );
}

#[test]
fn test_bottleneck_analysis_count_by_severity() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "n1",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n2",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n3",
        BottleneckMetric::Latency,
        55.0,
        50.0,
        BottleneckSeverity::Moderate,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n4",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "d",
        "s",
    ));

    let counts = analysis.count_by_severity();
    assert_eq!(counts.get("minor"), Some(&2));
    assert_eq!(counts.get("moderate"), Some(&1));
    assert_eq!(counts.get("critical"), Some(&1));
    assert!(counts.get("severe").is_none());
}

#[test]
fn test_bottleneck_analysis_count_by_metric() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "n1",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n2",
        BottleneckMetric::Latency,
        55.0,
        50.0,
        BottleneckSeverity::Moderate,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n3",
        BottleneckMetric::TokenUsage,
        45.0,
        40.0,
        BottleneckSeverity::Minor,
        "d",
        "s",
    ));

    let counts = analysis.count_by_metric();
    assert_eq!(counts.get("latency"), Some(&2));
    assert_eq!(counts.get("token_usage"), Some(&1));
}

#[test]
fn test_bottleneck_analysis_generate_summary_no_bottlenecks() {
    let analysis = BottleneckAnalysis {
        bottlenecks: Vec::new(),
        nodes_analyzed: 5,
        total_duration_ms: 1000,
        total_tokens: 5000,
        thresholds: BottleneckThresholds::default(),
        summary: String::new(),
    };

    let summary = analysis.generate_summary();
    assert!(summary.contains("No bottlenecks detected"));
    assert!(summary.contains("5 nodes"));
    assert!(summary.contains("1000 ms"));
    assert!(summary.contains("5000 tokens"));
}

#[test]
fn test_bottleneck_analysis_generate_summary_with_critical() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
    analysis.nodes_analyzed = 3;
    analysis.total_duration_ms = 5000;
    analysis.total_tokens = 10000;

    analysis.bottlenecks.push(Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "desc",
        "sug",
    ));

    let summary = analysis.generate_summary();
    assert!(summary.contains("CRITICAL"));
    assert!(summary.contains("immediate attention"));
}

#[test]
fn test_bottleneck_analysis_json_roundtrip() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
    analysis.nodes_analyzed = 3;
    analysis.total_duration_ms = 5000;
    analysis.total_tokens = 10000;
    analysis.bottlenecks.push(Bottleneck::new(
        "node",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "desc",
        "sug",
    ));
    analysis.summary = "test".to_string();

    let json = analysis.to_json().unwrap();
    let parsed = BottleneckAnalysis::from_json(&json).unwrap();

    assert_eq!(parsed.nodes_analyzed, 3);
    assert_eq!(parsed.total_duration_ms, 5000);
    assert_eq!(parsed.bottleneck_count(), 1);
}

// Tests for ExecutionTrace.detect_bottlenecks()

fn create_node_execution(
    node: &str,
    duration_ms: u64,
    tokens: u64,
    success: bool,
    index: usize,
) -> NodeExecution {
    NodeExecution {
        node: node.to_string(),
        duration_ms,
        tokens_used: tokens,
        state_before: None,
        state_after: None,
        tools_called: Vec::new(),
        success,
        error_message: if success {
            None
        } else {
            Some("Error".to_string())
        },
        index,
        started_at: None,
        metadata: HashMap::new(),
    }
}

#[test]
fn test_detect_bottlenecks_no_issues() {
    // Use 4 nodes with 25% each - under the 30% minor threshold
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("node_a", 100, 100, true, 0),
            create_node_execution("node_b", 100, 100, true, 1),
            create_node_execution("node_c", 100, 100, true, 2),
            create_node_execution("node_d", 100, 100, true, 3),
        ],
        total_duration_ms: 400,
        total_tokens: 400,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    assert!(!analysis.has_bottlenecks());
    assert_eq!(analysis.nodes_analyzed, 4);
}

#[test]
fn test_detect_bottlenecks_latency_percentage() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("slow_node", 800, 100, true, 0),
            create_node_execution("fast_node", 200, 100, true, 1),
        ],
        total_duration_ms: 1000,
        total_tokens: 200,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    assert!(analysis.has_bottlenecks());

    let latency_bottlenecks = analysis.by_metric(&BottleneckMetric::Latency);
    assert!(!latency_bottlenecks.is_empty());

    let slow_bottleneck = latency_bottlenecks
        .iter()
        .find(|b| b.node == "slow_node")
        .unwrap();
    assert_eq!(slow_bottleneck.severity, BottleneckSeverity::Severe); // 80% >= 70%
}

#[test]
fn test_detect_bottlenecks_latency_critical() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("dominant_node", 900, 100, true, 0),
            create_node_execution("other_node", 100, 100, true, 1),
        ],
        total_duration_ms: 1000,
        total_tokens: 200,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    let node_bottlenecks = analysis.for_node("dominant_node");
    let bottleneck = node_bottlenecks
        .iter()
        .find(|b| b.metric == BottleneckMetric::Latency)
        .unwrap();

    assert_eq!(bottleneck.severity, BottleneckSeverity::Critical); // 90% >= 85%
    assert!(bottleneck.is_critical());
}

#[test]
fn test_detect_bottlenecks_token_usage() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("token_heavy", 100, 8500, true, 0),
            create_node_execution("token_light", 100, 1500, true, 1),
        ],
        total_duration_ms: 200,
        total_tokens: 10000,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    let token_bottlenecks = analysis.by_metric(&BottleneckMetric::TokenUsage);
    assert!(!token_bottlenecks.is_empty());

    let bottleneck = token_bottlenecks
        .iter()
        .find(|b| b.node == "token_heavy")
        .unwrap();
    assert_eq!(bottleneck.severity, BottleneckSeverity::Severe); // 85% >= 80%
}

#[test]
fn test_detect_bottlenecks_error_rate() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("failing_node", 100, 100, false, 0),
            create_node_execution("failing_node", 100, 100, false, 1),
            create_node_execution("failing_node", 100, 100, true, 2),
            create_node_execution("failing_node", 100, 100, true, 3),
            create_node_execution("good_node", 100, 100, true, 4),
        ],
        total_duration_ms: 500,
        total_tokens: 500,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    let error_bottlenecks = analysis.by_metric(&BottleneckMetric::ErrorRate);

    // failing_node has 50% error rate (2/4)
    let bottleneck = error_bottlenecks
        .iter()
        .find(|b| b.node == "failing_node")
        .unwrap();
    assert_eq!(bottleneck.severity, BottleneckSeverity::Critical); // 50% >= 50%
}

#[test]
fn test_detect_bottlenecks_high_frequency() {
    let mut nodes = Vec::new();
    for i in 0..30 {
        nodes.push(create_node_execution("looping_node", 10, 10, true, i));
    }
    nodes.push(create_node_execution("other_node", 10, 10, true, 30));

    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: nodes,
        total_duration_ms: 310,
        total_tokens: 310,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    let freq_bottlenecks = analysis.by_metric(&BottleneckMetric::HighFrequency);

    let bottleneck = freq_bottlenecks
        .iter()
        .find(|b| b.node == "looping_node")
        .unwrap();
    assert_eq!(bottleneck.severity, BottleneckSeverity::Moderate); // 30 >= 25
}

#[test]
fn test_detect_bottlenecks_high_variance() {
    // Create executions with high variance: [100, 1000] has CV > 0.5
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("variable_node", 100, 100, true, 0),
            create_node_execution("variable_node", 1000, 100, true, 1),
            create_node_execution("stable_node", 500, 100, true, 2),
            create_node_execution("stable_node", 500, 100, true, 3),
        ],
        total_duration_ms: 2100,
        total_tokens: 400,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    let variance_bottlenecks = analysis.by_metric(&BottleneckMetric::HighVariance);

    // variable_node: mean=550, std_dev=450, CV=0.818 which is >= 0.5 (minor threshold)
    let bottleneck = variance_bottlenecks
        .iter()
        .find(|b| b.node == "variable_node");
    assert!(bottleneck.is_some());
}

#[test]
fn test_detect_bottlenecks_with_custom_thresholds() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("slow_node", 400, 100, true, 0),
            create_node_execution("fast_node", 600, 100, true, 1),
        ],
        total_duration_ms: 1000,
        total_tokens: 200,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    // With default thresholds, 60% latency = moderate
    let default_analysis = trace.detect_bottlenecks();
    let fast_node_default = default_analysis
        .for_node("fast_node")
        .iter()
        .find(|b| b.metric == BottleneckMetric::Latency)
        .map(|b| b.severity);
    assert_eq!(fast_node_default, Some(BottleneckSeverity::Moderate));

    // With strict thresholds, 60% latency = severe (threshold is 50%)
    let strict = BottleneckThresholds::strict();
    let strict_analysis = trace.detect_bottlenecks_with_thresholds(&strict);
    let fast_node_strict = strict_analysis
        .for_node("fast_node")
        .iter()
        .find(|b| b.metric == BottleneckMetric::Latency)
        .map(|b| b.severity);
    assert_eq!(fast_node_strict, Some(BottleneckSeverity::Severe));
}

#[test]
fn test_detect_bottlenecks_sorted_by_severity() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("minor_node", 350, 100, true, 0), // 35% latency = minor
            create_node_execution("critical_node", 550, 100, false, 1), // 55% latency = moderate, but error = critical
            create_node_execution("other", 100, 100, true, 2),
        ],
        total_duration_ms: 1000,
        total_tokens: 300,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();

    // First bottleneck should be most severe
    if let Some(first) = analysis.bottlenecks.first() {
        // The error rate bottleneck should be critical (100% failure)
        assert!(first.severity >= BottleneckSeverity::Severe);
    }
}

#[test]
fn test_detect_bottlenecks_empty_trace() {
    let trace = ExecutionTrace::new();
    let analysis = trace.detect_bottlenecks();

    assert!(!analysis.has_bottlenecks());
    assert_eq!(analysis.nodes_analyzed, 0);
    assert_eq!(analysis.total_duration_ms, 0);
    assert_eq!(analysis.total_tokens, 0);
}

#[test]
fn test_detect_bottlenecks_suggestions_present() {
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("slow_node", 900, 9000, true, 0),
            create_node_execution("other", 100, 1000, true, 1),
        ],
        total_duration_ms: 1000,
        total_tokens: 10000,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();

    for bottleneck in &analysis.bottlenecks {
        assert!(!bottleneck.suggestion.is_empty());
        assert!(!bottleneck.description.is_empty());
    }
}

#[test]
fn test_detect_bottlenecks_absolute_latency() {
    // Test absolute latency threshold (5 seconds = 5000ms for minor)
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        execution_id: None,
        parent_execution_id: None,
        root_execution_id: None,
        depth: Some(0),
        nodes_executed: vec![
            create_node_execution("very_slow", 65000, 100, true, 0), // 65 seconds
        ],
        total_duration_ms: 65000,
        total_tokens: 100,
        errors: Vec::new(),
        completed: true,
        started_at: None,
        ended_at: None,
        final_state: None,
        metadata: HashMap::new(),
        execution_metrics: None,
        performance_metrics: None,
    };

    let analysis = trace.detect_bottlenecks();
    let latency_issues = analysis.by_metric(&BottleneckMetric::Latency);

    // Should have at least one latency bottleneck
    assert!(!latency_issues.is_empty());
}

#[test]
fn test_bottleneck_threshold_getters() {
    let thresholds = BottleneckThresholds::default();

    // Test threshold getters return expected values
    assert_eq!(
        thresholds.latency_percentage_threshold(BottleneckSeverity::Minor),
        30.0
    );
    assert_eq!(
        thresholds.latency_percentage_threshold(BottleneckSeverity::Critical),
        85.0
    );

    assert_eq!(
        thresholds.latency_absolute_threshold(BottleneckSeverity::Minor),
        5000
    );
    assert_eq!(
        thresholds.latency_absolute_threshold(BottleneckSeverity::Critical),
        60000
    );

    assert_eq!(
        thresholds.token_percentage_threshold(BottleneckSeverity::Minor),
        40.0
    );
    assert_eq!(
        thresholds.token_percentage_threshold(BottleneckSeverity::Critical),
        90.0
    );

    assert_eq!(
        thresholds.error_rate_threshold(BottleneckSeverity::Minor),
        0.05
    );
    assert_eq!(
        thresholds.error_rate_threshold(BottleneckSeverity::Critical),
        0.50
    );

    assert_eq!(
        thresholds.frequency_threshold(BottleneckSeverity::Minor),
        10
    );
    assert_eq!(
        thresholds.frequency_threshold(BottleneckSeverity::Critical),
        100
    );

    assert_eq!(
        thresholds.variance_threshold(BottleneckSeverity::Minor),
        0.5
    );
    assert_eq!(
        thresholds.variance_threshold(BottleneckSeverity::Critical),
        2.0
    );
}

#[test]
fn test_bottleneck_critical_bottlenecks() {
    let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

    analysis.bottlenecks.push(Bottleneck::new(
        "n1",
        BottleneckMetric::Latency,
        35.0,
        30.0,
        BottleneckSeverity::Minor,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n2",
        BottleneckMetric::Latency,
        90.0,
        85.0,
        BottleneckSeverity::Critical,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n3",
        BottleneckMetric::ErrorRate,
        60.0,
        50.0,
        BottleneckSeverity::Critical,
        "d",
        "s",
    ));
    analysis.bottlenecks.push(Bottleneck::new(
        "n4",
        BottleneckMetric::Latency,
        75.0,
        70.0,
        BottleneckSeverity::Severe,
        "d",
        "s",
    ));

    let critical = analysis.critical_bottlenecks();
    assert_eq!(critical.len(), 2);
    for b in &critical {
        assert!(b.is_critical());
    }
}
