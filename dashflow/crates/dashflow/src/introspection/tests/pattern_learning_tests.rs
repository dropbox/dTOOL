// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Pattern learning tests - extracted from tests.rs for better organization.

use super::*;

#[test]
fn test_pattern_type_display() {
    assert_eq!(format!("{}", PatternType::Success), "success");
    assert_eq!(format!("{}", PatternType::Failure), "failure");
    assert_eq!(format!("{}", PatternType::Slow), "slow");
    assert_eq!(format!("{}", PatternType::Efficient), "efficient");
    assert_eq!(
        format!("{}", PatternType::HighTokenUsage),
        "high_token_usage"
    );
    assert_eq!(format!("{}", PatternType::LowTokenUsage), "low_token_usage");
    assert_eq!(format!("{}", PatternType::Repeated), "repeated");
    assert_eq!(format!("{}", PatternType::Sequential), "sequential");
    assert_eq!(format!("{}", PatternType::ErrorRecovery), "error_recovery");
    assert_eq!(format!("{}", PatternType::Timeout), "timeout");
    assert_eq!(format!("{}", PatternType::Idle), "idle");
    assert_eq!(format!("{}", PatternType::Burst), "burst");
}

#[test]
fn test_pattern_type_default() {
    assert_eq!(PatternType::default(), PatternType::Success);
}

#[test]
fn test_pattern_condition_duration_gt() {
    let cond = PatternCondition::duration_gt(1000);
    assert_eq!(cond.field, "duration_ms");
    assert_eq!(cond.operator, PatternOperator::GreaterThan);

    let fast_exec = NodeExecution::new("test", 500);
    let slow_exec = NodeExecution::new("test", 1500);

    assert!(!cond.matches(&fast_exec));
    assert!(cond.matches(&slow_exec));
}

#[test]
fn test_pattern_condition_duration_lt() {
    let cond = PatternCondition::duration_lt(1000);

    let fast_exec = NodeExecution::new("test", 500);
    let slow_exec = NodeExecution::new("test", 1500);

    assert!(cond.matches(&fast_exec));
    assert!(!cond.matches(&slow_exec));
}

#[test]
fn test_pattern_condition_tokens_gt() {
    let cond = PatternCondition::tokens_gt(500);

    let low_token_exec = NodeExecution::new("test", 100).with_tokens(100);
    let high_token_exec = NodeExecution::new("test", 100).with_tokens(1000);

    assert!(!cond.matches(&low_token_exec));
    assert!(cond.matches(&high_token_exec));
}

#[test]
fn test_pattern_condition_tokens_lt() {
    let cond = PatternCondition::tokens_lt(500);

    let low_token_exec = NodeExecution::new("test", 100).with_tokens(100);
    let high_token_exec = NodeExecution::new("test", 100).with_tokens(1000);

    assert!(cond.matches(&low_token_exec));
    assert!(!cond.matches(&high_token_exec));
}

#[test]
fn test_pattern_condition_is_success() {
    let cond = PatternCondition::is_success();

    let success_exec = NodeExecution::new("test", 100);
    let failure_exec = NodeExecution::new("test", 100).with_error("Failed");

    assert!(cond.matches(&success_exec));
    assert!(!cond.matches(&failure_exec));
}

#[test]
fn test_pattern_condition_is_failure() {
    let cond = PatternCondition::is_failure();

    let success_exec = NodeExecution::new("test", 100);
    let failure_exec = NodeExecution::new("test", 100).with_error("Failed");

    assert!(!cond.matches(&success_exec));
    assert!(cond.matches(&failure_exec));
}

#[test]
fn test_pattern_condition_node_equals() {
    let cond = PatternCondition::node_equals("target_node");

    let matching_exec = NodeExecution::new("target_node", 100);
    let non_matching_exec = NodeExecution::new("other_node", 100);

    assert!(cond.matches(&matching_exec));
    assert!(!cond.matches(&non_matching_exec));
}

#[test]
fn test_pattern_operator_compare_i64() {
    assert!(PatternOperator::Equals.compare_i64(5, 5));
    assert!(!PatternOperator::Equals.compare_i64(5, 6));

    assert!(PatternOperator::NotEquals.compare_i64(5, 6));
    assert!(!PatternOperator::NotEquals.compare_i64(5, 5));

    assert!(PatternOperator::GreaterThan.compare_i64(10, 5));
    assert!(!PatternOperator::GreaterThan.compare_i64(5, 10));

    assert!(PatternOperator::GreaterThanOrEqual.compare_i64(10, 10));
    assert!(PatternOperator::GreaterThanOrEqual.compare_i64(10, 5));

    assert!(PatternOperator::LessThan.compare_i64(5, 10));
    assert!(!PatternOperator::LessThan.compare_i64(10, 5));

    assert!(PatternOperator::LessThanOrEqual.compare_i64(5, 5));
    assert!(PatternOperator::LessThanOrEqual.compare_i64(5, 10));
}

#[test]
fn test_pattern_operator_display() {
    assert_eq!(format!("{}", PatternOperator::Equals), "==");
    assert_eq!(format!("{}", PatternOperator::NotEquals), "!=");
    assert_eq!(format!("{}", PatternOperator::GreaterThan), ">");
    assert_eq!(format!("{}", PatternOperator::GreaterThanOrEqual), ">=");
    assert_eq!(format!("{}", PatternOperator::LessThan), "<");
    assert_eq!(format!("{}", PatternOperator::LessThanOrEqual), "<=");
    assert_eq!(format!("{}", PatternOperator::Contains), "contains");
    assert_eq!(format!("{}", PatternOperator::Between), "between");
}

#[test]
#[allow(clippy::approx_constant)] // 3.14 is test data, not PI
fn test_pattern_value_display() {
    assert_eq!(format!("{}", PatternValue::Integer(42)), "42");
    assert_eq!(format!("{}", PatternValue::Float(3.14)), "3.14");
    assert_eq!(format!("{}", PatternValue::Boolean(true)), "true");
    assert_eq!(
        format!("{}", PatternValue::String("test".to_string())),
        "\"test\""
    );
    assert_eq!(format!("{}", PatternValue::IntegerRange(1, 10)), "[1, 10]");
    assert_eq!(
        format!("{}", PatternValue::FloatRange(1.0, 10.0)),
        "[1.00, 10.00]"
    );
}

#[test]
fn test_pattern_new() {
    let pattern = Pattern::new("test_pattern", PatternType::Failure);
    assert_eq!(pattern.id, "test_pattern");
    assert_eq!(pattern.pattern_type, PatternType::Failure);
    assert_eq!(pattern.frequency, 1);
    assert_eq!(pattern.confidence, 0.5);
    assert!(pattern.conditions.is_empty());
    assert!(pattern.affected_nodes.is_empty());
}

#[test]
fn test_pattern_builder() {
    let pattern = Pattern::builder()
        .id("built_pattern")
        .pattern_type(PatternType::Slow)
        .condition(PatternCondition::duration_gt(5000))
        .frequency(5)
        .affected_node("slow_node")
        .confidence(0.8)
        .description("A slow pattern")
        .evidence("evidence_1")
        .evidence("evidence_2")
        .first_seen(0)
        .last_seen(10)
        .build()
        .unwrap();

    assert_eq!(pattern.id, "built_pattern");
    assert_eq!(pattern.pattern_type, PatternType::Slow);
    assert_eq!(pattern.conditions.len(), 1);
    assert_eq!(pattern.frequency, 5);
    assert_eq!(pattern.affected_nodes, vec!["slow_node"]);
    assert_eq!(pattern.confidence, 0.8);
    assert_eq!(pattern.description, "A slow pattern");
    assert_eq!(pattern.evidence.len(), 2);
    assert_eq!(pattern.first_seen, 0);
    assert_eq!(pattern.last_seen, 10);
}

#[test]
fn test_pattern_builder_missing_id() {
    let result = Pattern::builder()
        .pattern_type(PatternType::Success)
        .description("Missing ID")
        .build();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "id is required");
}

#[test]
fn test_pattern_builder_missing_description() {
    let result = Pattern::builder()
        .id("test")
        .pattern_type(PatternType::Success)
        .build();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "description is required");
}

#[test]
fn test_pattern_matches() {
    let pattern = Pattern::new("test", PatternType::Slow)
        .with_condition(PatternCondition::node_equals("slow_node"))
        .with_condition(PatternCondition::duration_gt(1000));

    let matching_exec = NodeExecution::new("slow_node", 2000);
    let fast_exec = NodeExecution::new("slow_node", 500);
    let wrong_node = NodeExecution::new("fast_node", 2000);

    assert!(pattern.matches(&matching_exec));
    assert!(!pattern.matches(&fast_exec));
    assert!(!pattern.matches(&wrong_node));
}

#[test]
fn test_pattern_matches_trace() {
    let pattern =
        Pattern::new("test", PatternType::Slow).with_condition(PatternCondition::duration_gt(5000));

    let trace_with_slow = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("fast", 100),
            NodeExecution::new("slow", 10000),
        ],
        total_duration_ms: 10100,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let trace_all_fast = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("fast1", 100),
            NodeExecution::new("fast2", 200),
        ],
        total_duration_ms: 300,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    assert!(pattern.matches_trace(&trace_with_slow));
    assert!(!pattern.matches_trace(&trace_all_fast));
}

#[test]
fn test_pattern_count_matches() {
    let pattern =
        Pattern::new("test", PatternType::Success).with_condition(PatternCondition::is_success());

    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("node1", 100),
            NodeExecution::new("node2", 100).with_error("Failed"),
            NodeExecution::new("node3", 100),
        ],
        total_duration_ms: 300,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    assert_eq!(pattern.count_matches(&trace), 2);
}

#[test]
fn test_pattern_is_negative() {
    assert!(Pattern::new("test", PatternType::Failure).is_negative());
    assert!(Pattern::new("test", PatternType::Slow).is_negative());
    assert!(Pattern::new("test", PatternType::HighTokenUsage).is_negative());
    assert!(Pattern::new("test", PatternType::Timeout).is_negative());

    assert!(!Pattern::new("test", PatternType::Success).is_negative());
    assert!(!Pattern::new("test", PatternType::Efficient).is_negative());
}

#[test]
fn test_pattern_is_positive() {
    assert!(Pattern::new("test", PatternType::Success).is_positive());
    assert!(Pattern::new("test", PatternType::Efficient).is_positive());
    assert!(Pattern::new("test", PatternType::LowTokenUsage).is_positive());
    assert!(Pattern::new("test", PatternType::ErrorRecovery).is_positive());

    assert!(!Pattern::new("test", PatternType::Failure).is_positive());
    assert!(!Pattern::new("test", PatternType::Slow).is_positive());
}

#[test]
fn test_pattern_summary() {
    let pattern = Pattern::new("test_id", PatternType::Failure)
        .with_frequency(5)
        .with_confidence(0.8)
        .with_description("Test description");

    let summary = pattern.summary();
    assert!(summary.contains("[failure]"));
    assert!(summary.contains("test_id"));
    assert!(summary.contains("freq: 5"));
    assert!(summary.contains("80%"));
    assert!(summary.contains("Test description"));
}

#[test]
fn test_pattern_to_json() {
    let pattern =
        Pattern::new("json_test", PatternType::Success).with_description("JSON test pattern");
    let json = pattern.to_json().unwrap();

    assert!(json.contains("json_test"));
    assert!(json.contains("Success"));

    let parsed = Pattern::from_json(&json).unwrap();
    assert_eq!(parsed.id, pattern.id);
    assert_eq!(parsed.pattern_type, pattern.pattern_type);
}

#[test]
fn test_pattern_analysis_new() {
    let analysis = PatternAnalysis::new();
    assert!(!analysis.has_patterns());
    assert_eq!(analysis.pattern_count(), 0);
    assert_eq!(analysis.executions_analyzed, 0);
}

#[test]
fn test_pattern_analysis_by_type() {
    let mut analysis = PatternAnalysis::new();
    analysis
        .patterns
        .push(Pattern::new("s1", PatternType::Success).with_description("Success 1"));
    analysis
        .patterns
        .push(Pattern::new("s2", PatternType::Success).with_description("Success 2"));
    analysis
        .patterns
        .push(Pattern::new("f1", PatternType::Failure).with_description("Failure 1"));

    let success_patterns = analysis.by_type(&PatternType::Success);
    let failure_patterns = analysis.by_type(&PatternType::Failure);

    assert_eq!(success_patterns.len(), 2);
    assert_eq!(failure_patterns.len(), 1);
}

#[test]
fn test_pattern_analysis_negative_positive() {
    let mut analysis = PatternAnalysis::new();
    analysis
        .patterns
        .push(Pattern::new("s1", PatternType::Success).with_description("Success"));
    analysis
        .patterns
        .push(Pattern::new("e1", PatternType::Efficient).with_description("Efficient"));
    analysis
        .patterns
        .push(Pattern::new("f1", PatternType::Failure).with_description("Failure"));
    analysis
        .patterns
        .push(Pattern::new("sl1", PatternType::Slow).with_description("Slow"));

    assert_eq!(analysis.positive_patterns().len(), 2);
    assert_eq!(analysis.negative_patterns().len(), 2);
}

#[test]
fn test_pattern_analysis_for_node() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("p1", PatternType::Success)
            .with_affected_node("node_a")
            .with_description("Pattern 1"),
    );
    analysis.patterns.push(
        Pattern::new("p2", PatternType::Failure)
            .with_affected_node("node_b")
            .with_description("Pattern 2"),
    );
    analysis.patterns.push(
        Pattern::new("p3", PatternType::Sequential)
            .with_affected_node("node_a")
            .with_affected_node("node_b")
            .with_description("Pattern 3"),
    );

    let node_a_patterns = analysis.for_node("node_a");
    let node_b_patterns = analysis.for_node("node_b");

    assert_eq!(node_a_patterns.len(), 2); // p1 and p3
    assert_eq!(node_b_patterns.len(), 2); // p2 and p3
}

#[test]
fn test_pattern_analysis_most_frequent() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("p1", PatternType::Success)
            .with_frequency(10)
            .with_description("Most frequent"),
    );
    analysis.patterns.push(
        Pattern::new("p2", PatternType::Failure)
            .with_frequency(5)
            .with_description("Medium"),
    );
    analysis.patterns.push(
        Pattern::new("p3", PatternType::Slow)
            .with_frequency(1)
            .with_description("Least frequent"),
    );

    let most_freq = analysis.most_frequent(2);
    assert_eq!(most_freq.len(), 2);
    assert_eq!(most_freq[0].frequency, 10);
    assert_eq!(most_freq[1].frequency, 5);
}

#[test]
fn test_pattern_analysis_highest_confidence() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("p1", PatternType::Success)
            .with_confidence(0.9)
            .with_description("High confidence"),
    );
    analysis.patterns.push(
        Pattern::new("p2", PatternType::Failure)
            .with_confidence(0.5)
            .with_description("Medium confidence"),
    );
    analysis.patterns.push(
        Pattern::new("p3", PatternType::Slow)
            .with_confidence(0.3)
            .with_description("Low confidence"),
    );

    let highest = analysis.highest_confidence(2);
    assert_eq!(highest.len(), 2);
    assert_eq!(highest[0].confidence, 0.9);
    assert_eq!(highest[1].confidence, 0.5);
}

#[test]
fn test_pattern_analysis_to_json() {
    let mut analysis = PatternAnalysis::new();
    analysis
        .patterns
        .push(Pattern::new("p1", PatternType::Success).with_description("Test"));
    analysis.executions_analyzed = 10;
    analysis.patterns_learned = 1;
    analysis.summary = "Test summary".to_string();

    let json = analysis.to_json().unwrap();
    let parsed = PatternAnalysis::from_json(&json).unwrap();

    assert_eq!(parsed.patterns.len(), 1);
    assert_eq!(parsed.executions_analyzed, 10);
}

#[test]
fn test_pattern_thresholds_default() {
    let thresholds = PatternThresholds::default();
    assert_eq!(thresholds.slow_duration_ms, 5000);
    assert_eq!(thresholds.efficient_duration_ms, 100);
    assert_eq!(thresholds.high_token_threshold, 2000);
    assert_eq!(thresholds.low_token_threshold, 100);
    assert_eq!(thresholds.min_frequency, 2);
    assert_eq!(thresholds.min_confidence, 0.3);
    assert_eq!(thresholds.timeout_threshold_ms, 30000);
}

#[test]
fn test_pattern_thresholds_builder() {
    let thresholds = PatternThresholds::new()
        .with_slow_duration(10000)
        .with_efficient_duration(50)
        .with_high_token_threshold(5000)
        .with_low_token_threshold(50)
        .with_min_frequency(3)
        .with_min_confidence(0.5)
        .with_timeout_threshold(60000);

    assert_eq!(thresholds.slow_duration_ms, 10000);
    assert_eq!(thresholds.efficient_duration_ms, 50);
    assert_eq!(thresholds.high_token_threshold, 5000);
    assert_eq!(thresholds.low_token_threshold, 50);
    assert_eq!(thresholds.min_frequency, 3);
    assert_eq!(thresholds.min_confidence, 0.5);
    assert_eq!(thresholds.timeout_threshold_ms, 60000);
}

#[test]
fn test_learn_patterns_empty_trace() {
    let trace = ExecutionTrace::default();
    let analysis = trace.learn_patterns();

    assert!(!analysis.has_patterns());
    assert_eq!(analysis.executions_analyzed, 0);
    assert!(analysis.summary.contains("No patterns"));
}

#[test]
fn test_learn_patterns_success_pattern() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("reliable_node", 100),
            NodeExecution::new("reliable_node", 110),
            NodeExecution::new("reliable_node", 105),
        ],
        total_duration_ms: 315,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();
    let success_patterns = analysis.by_type(&PatternType::Success);

    assert!(!success_patterns.is_empty());
    assert!(success_patterns
        .iter()
        .any(|p| p.affected_nodes.contains(&"reliable_node".to_string())));
}

#[test]
fn test_learn_patterns_failure_pattern() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("flaky_node", 100).with_error("Error 1"),
            NodeExecution::new("flaky_node", 100).with_error("Error 2"),
            NodeExecution::new("flaky_node", 100).with_error("Error 3"),
        ],
        total_duration_ms: 300,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();
    let failure_patterns = analysis.by_type(&PatternType::Failure);

    assert!(!failure_patterns.is_empty());
    assert!(failure_patterns
        .iter()
        .any(|p| p.affected_nodes.contains(&"flaky_node".to_string())));
}

#[test]
fn test_learn_patterns_slow_pattern() {
    let thresholds = PatternThresholds::new().with_slow_duration(1000);

    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("slow_node", 5000),
            NodeExecution::new("slow_node", 6000),
        ],
        total_duration_ms: 11000,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns_with_thresholds(&thresholds);
    let slow_patterns = analysis.by_type(&PatternType::Slow);

    assert!(!slow_patterns.is_empty());
}

#[test]
fn test_learn_patterns_efficient_pattern() {
    let thresholds = PatternThresholds::new().with_efficient_duration(200);

    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("fast_node", 50),
            NodeExecution::new("fast_node", 60),
            NodeExecution::new("fast_node", 55),
        ],
        total_duration_ms: 165,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns_with_thresholds(&thresholds);
    let efficient_patterns = analysis.by_type(&PatternType::Efficient);

    assert!(!efficient_patterns.is_empty());
}

#[test]
fn test_learn_patterns_high_token_pattern() {
    let thresholds = PatternThresholds::new().with_high_token_threshold(1000);

    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("heavy_node", 100).with_tokens(5000),
            NodeExecution::new("heavy_node", 100).with_tokens(6000),
        ],
        total_duration_ms: 200,
        total_tokens: 11000,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns_with_thresholds(&thresholds);
    let high_token_patterns = analysis.by_type(&PatternType::HighTokenUsage);

    assert!(!high_token_patterns.is_empty());
}

#[test]
fn test_learn_patterns_low_token_pattern() {
    let thresholds = PatternThresholds::new().with_low_token_threshold(100);

    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("light_node", 100).with_tokens(50),
            NodeExecution::new("light_node", 100).with_tokens(60),
        ],
        total_duration_ms: 200,
        total_tokens: 110,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns_with_thresholds(&thresholds);
    let low_token_patterns = analysis.by_type(&PatternType::LowTokenUsage);

    assert!(!low_token_patterns.is_empty());
}

#[test]
fn test_learn_patterns_repeated_pattern() {
    // Create a trace where one node is executed many more times than average
    // avg_count = 10 executions / 3 unique nodes = 3.33
    // loop_node has 7 executions, which is > 3.33 * 2 = 6.66 and >= 5
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("other_node_1", 50),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("other_node_2", 50),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("loop_node", 100),
            NodeExecution::new("other_node_1", 50),
        ],
        total_duration_ms: 800,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();
    let repeated_patterns = analysis.by_type(&PatternType::Repeated);

    assert!(!repeated_patterns.is_empty());
    assert!(repeated_patterns
        .iter()
        .any(|p| p.affected_nodes.contains(&"loop_node".to_string())));
}

#[test]
fn test_learn_patterns_sequential_pattern() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("step_a", 100),
            NodeExecution::new("step_b", 100),
            NodeExecution::new("step_a", 100),
            NodeExecution::new("step_b", 100),
            NodeExecution::new("step_a", 100),
            NodeExecution::new("step_b", 100),
        ],
        total_duration_ms: 600,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();
    let sequential_patterns = analysis.by_type(&PatternType::Sequential);

    assert!(!sequential_patterns.is_empty());
    // Should detect step_a -> step_b pattern
    assert!(sequential_patterns.iter().any(|p| {
        p.affected_nodes.contains(&"step_a".to_string())
            && p.affected_nodes.contains(&"step_b".to_string())
    }));
}

#[test]
fn test_learn_patterns_error_recovery_pattern() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("retry_node", 100).with_error("Error"),
            NodeExecution::new("retry_node", 100), // Success after failure
            NodeExecution::new("retry_node", 100).with_error("Error"),
            NodeExecution::new("retry_node", 100), // Success after failure
        ],
        total_duration_ms: 400,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();
    let recovery_patterns = analysis.by_type(&PatternType::ErrorRecovery);

    assert!(!recovery_patterns.is_empty());
}

#[test]
fn test_learn_patterns_timeout_pattern() {
    let thresholds = PatternThresholds::new().with_timeout_threshold(10000);

    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("timeout_node", 30000),
            NodeExecution::new("timeout_node", 35000),
        ],
        total_duration_ms: 65000,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns_with_thresholds(&thresholds);
    let timeout_patterns = analysis.by_type(&PatternType::Timeout);

    assert!(!timeout_patterns.is_empty());
}

#[test]
fn test_learn_patterns_sorted_by_confidence() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            // Create mixed patterns
            NodeExecution::new("reliable", 50),
            NodeExecution::new("reliable", 55),
            NodeExecution::new("reliable", 60),
            NodeExecution::new("flaky", 100).with_error("Error"),
            NodeExecution::new("flaky", 100).with_error("Error"),
        ],
        total_duration_ms: 365,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();

    // Patterns should be sorted by confidence (highest first)
    if analysis.patterns.len() >= 2 {
        for i in 0..analysis.patterns.len() - 1 {
            assert!(
                analysis.patterns[i].confidence >= analysis.patterns[i + 1].confidence,
                "Patterns not sorted by confidence"
            );
        }
    }
}

#[test]
fn test_learn_patterns_summary_generation() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("node", 100),
            NodeExecution::new("node", 100),
        ],
        total_duration_ms: 200,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns();

    assert!(!analysis.summary.is_empty());
    if analysis.has_patterns() {
        assert!(analysis.summary.contains("patterns"));
    }
}

#[test]
fn test_learn_patterns_with_custom_thresholds() {
    // With very low thresholds, even fast executions should be flagged as slow
    let strict_thresholds = PatternThresholds::new()
        .with_slow_duration(10) // 10ms is slow
        .with_efficient_duration(5) // Only 5ms is efficient
        .with_min_frequency(1)
        .with_min_confidence(0.1);

    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 100)],
        total_duration_ms: 100,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.learn_patterns_with_thresholds(&strict_thresholds);
    let slow_patterns = analysis.by_type(&PatternType::Slow);

    // With strict thresholds, 100ms should be flagged as slow
    assert!(!slow_patterns.is_empty());
}

#[test]
fn test_pattern_analysis_match_trace() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("slow_pattern", PatternType::Slow)
            .with_condition(PatternCondition::duration_gt(5000))
            .with_description("Slow pattern"),
    );

    let slow_trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 10000)],
        total_duration_ms: 10000,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let fast_trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 100)],
        total_duration_ms: 100,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    let matches_slow = analysis.match_trace(&slow_trace);
    let matches_fast = analysis.match_trace(&fast_trace);

    assert_eq!(matches_slow.len(), 1);
    assert_eq!(matches_fast.len(), 0);
}
