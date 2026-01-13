// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Configuration recommendations tests - extracted from tests.rs for better organization.

use super::*;

#[test]
fn test_reconfiguration_type_display() {
    assert_eq!(ReconfigurationType::AddCache.to_string(), "add_cache");
    assert_eq!(ReconfigurationType::Parallelize.to_string(), "parallelize");
    assert_eq!(ReconfigurationType::SwapModel.to_string(), "swap_model");
    assert_eq!(
        ReconfigurationType::AdjustTimeout.to_string(),
        "adjust_timeout"
    );
    assert_eq!(ReconfigurationType::AddRetry.to_string(), "add_retry");
    assert_eq!(ReconfigurationType::SkipNode.to_string(), "skip_node");
    assert_eq!(ReconfigurationType::MergeNodes.to_string(), "merge_nodes");
    assert_eq!(ReconfigurationType::SplitNode.to_string(), "split_node");
    assert_eq!(ReconfigurationType::AddBatching.to_string(), "add_batching");
    assert_eq!(
        ReconfigurationType::AddRateLimiting.to_string(),
        "add_rate_limiting"
    );
    assert_eq!(
        ReconfigurationType::ChangeRouting.to_string(),
        "change_routing"
    );
    assert_eq!(
        ReconfigurationType::Custom("test".to_string()).to_string(),
        "custom:test"
    );
}

#[test]
fn test_reconfiguration_type_default() {
    let default: ReconfigurationType = Default::default();
    assert_eq!(default, ReconfigurationType::AddCache);
}

#[test]
fn test_reconfiguration_priority_display() {
    assert_eq!(ReconfigurationPriority::Low.to_string(), "low");
    assert_eq!(ReconfigurationPriority::Medium.to_string(), "medium");
    assert_eq!(ReconfigurationPriority::High.to_string(), "high");
    assert_eq!(ReconfigurationPriority::Critical.to_string(), "critical");
}

#[test]
fn test_reconfiguration_priority_ordering() {
    assert!(ReconfigurationPriority::Low < ReconfigurationPriority::Medium);
    assert!(ReconfigurationPriority::Medium < ReconfigurationPriority::High);
    assert!(ReconfigurationPriority::High < ReconfigurationPriority::Critical);
}

#[test]
fn test_graph_reconfiguration_new() {
    let reconfig = GraphReconfiguration::new(
        "test_id",
        ReconfigurationType::AddCache,
        vec!["node1".to_string()],
        "Test description",
    );

    assert_eq!(reconfig.id, "test_id");
    assert_eq!(reconfig.reconfiguration_type, ReconfigurationType::AddCache);
    assert_eq!(reconfig.target_nodes, vec!["node1"]);
    assert_eq!(reconfig.description, "Test description");
    assert_eq!(reconfig.priority, ReconfigurationPriority::Medium);
    assert_eq!(reconfig.effort, 3);
    assert!((reconfig.confidence - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_graph_reconfiguration_builder() {
    let reconfig = GraphReconfiguration::builder()
        .id("reconfig_1")
        .reconfiguration_type(ReconfigurationType::Parallelize)
        .target_node("node_a")
        .target_node("node_b")
        .description("Parallelize nodes")
        .expected_improvement("30% faster")
        .implementation("Use parallel edges")
        .priority(ReconfigurationPriority::High)
        .effort(2)
        .confidence(0.8)
        .triggering_pattern("pattern_1")
        .evidence("Evidence line 1")
        .estimated_impact(30.0)
        .prerequisite("prereq_1")
        .conflict("conflict_1")
        .build()
        .unwrap();

    assert_eq!(reconfig.id, "reconfig_1");
    assert_eq!(
        reconfig.reconfiguration_type,
        ReconfigurationType::Parallelize
    );
    assert_eq!(reconfig.target_nodes, vec!["node_a", "node_b"]);
    assert_eq!(reconfig.description, "Parallelize nodes");
    assert_eq!(reconfig.expected_improvement, "30% faster");
    assert_eq!(reconfig.implementation, "Use parallel edges");
    assert_eq!(reconfig.priority, ReconfigurationPriority::High);
    assert_eq!(reconfig.effort, 2);
    assert!((reconfig.confidence - 0.8).abs() < f64::EPSILON);
    assert_eq!(reconfig.triggering_patterns, vec!["pattern_1"]);
    assert_eq!(reconfig.evidence, vec!["Evidence line 1"]);
    assert_eq!(reconfig.estimated_impact_pct, Some(30.0));
    assert_eq!(reconfig.prerequisites, vec!["prereq_1"]);
    assert_eq!(reconfig.conflicts, vec!["conflict_1"]);
}

#[test]
fn test_graph_reconfiguration_builder_missing_id() {
    let result = GraphReconfiguration::builder().description("Test").build();
    assert!(result.is_err());
}

#[test]
fn test_graph_reconfiguration_builder_missing_description() {
    let result = GraphReconfiguration::builder().id("test").build();
    assert!(result.is_err());
}

#[test]
fn test_graph_reconfiguration_is_high_priority() {
    let low = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_priority(ReconfigurationPriority::Low);
    let medium = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_priority(ReconfigurationPriority::Medium);
    let high = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_priority(ReconfigurationPriority::High);
    let critical = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_priority(ReconfigurationPriority::Critical);

    assert!(!low.is_high_priority());
    assert!(!medium.is_high_priority());
    assert!(high.is_high_priority());
    assert!(critical.is_high_priority());
}

#[test]
fn test_graph_reconfiguration_is_low_effort() {
    let effort_1 = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_effort(1);
    let effort_2 = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_effort(2);
    let effort_3 = GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
        .with_effort(3);

    assert!(effort_1.is_low_effort());
    assert!(effort_2.is_low_effort());
    assert!(!effort_3.is_low_effort());
}

#[test]
fn test_graph_reconfiguration_is_quick_win() {
    let quick_win =
        GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::High)
            .with_effort(2);
    let not_quick_1 =
        GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::Low)
            .with_effort(2);
    let not_quick_2 =
        GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::High)
            .with_effort(4);

    assert!(quick_win.is_quick_win());
    assert!(!not_quick_1.is_quick_win());
    assert!(!not_quick_2.is_quick_win());
}

#[test]
fn test_graph_reconfiguration_quick_win_score() {
    let high_score =
        GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::Critical)
            .with_effort(1)
            .with_confidence(1.0);
    let low_score =
        GraphReconfiguration::new("test", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::Low)
            .with_effort(5)
            .with_confidence(0.1);

    assert!(high_score.quick_win_score() > low_score.quick_win_score());
}

#[test]
fn test_graph_reconfiguration_summary() {
    let reconfig = GraphReconfiguration::new(
        "test",
        ReconfigurationType::AddCache,
        vec!["node1".to_string(), "node2".to_string()],
        "Add caching",
    )
    .with_expected_improvement("50% faster")
    .with_priority(ReconfigurationPriority::High);

    let summary = reconfig.summary();
    assert!(summary.contains("high"));
    assert!(summary.contains("add_cache"));
    assert!(summary.contains("node1, node2"));
    assert!(summary.contains("Add caching"));
    assert!(summary.contains("50% faster"));
}

#[test]
fn test_graph_reconfiguration_to_optimization_suggestion() {
    let reconfig = GraphReconfiguration::new(
        "test",
        ReconfigurationType::AddCache,
        vec!["node1".to_string()],
        "Add caching",
    )
    .with_expected_improvement("50% faster")
    .with_implementation("Add cache node")
    .with_priority(ReconfigurationPriority::High)
    .with_effort(2)
    .with_confidence(0.9)
    .with_evidence("Evidence 1");

    let suggestion = reconfig.to_optimization_suggestion();
    assert_eq!(suggestion.category, OptimizationCategory::Caching);
    assert_eq!(suggestion.target_nodes, vec!["node1"]);
    assert_eq!(suggestion.description, "Add caching");
    assert_eq!(suggestion.expected_improvement, "50% faster");
    assert_eq!(suggestion.implementation, "Add cache node");
    assert_eq!(suggestion.priority, OptimizationPriority::High);
    assert_eq!(suggestion.effort, 2);
    assert!((suggestion.confidence - 0.9).abs() < f64::EPSILON);
    assert_eq!(suggestion.evidence, vec!["Evidence 1"]);
}

#[test]
fn test_graph_reconfiguration_to_json() {
    let reconfig = GraphReconfiguration::new(
        "test",
        ReconfigurationType::AddCache,
        vec!["node1".to_string()],
        "Add caching",
    );

    let json = reconfig.to_json().unwrap();
    assert!(json.contains("test"));
    assert!(json.contains("AddCache"));
    assert!(json.contains("node1"));

    let parsed = GraphReconfiguration::from_json(&json).unwrap();
    assert_eq!(parsed.id, reconfig.id);
}

#[test]
fn test_configuration_recommendations_new() {
    let recommendations = ConfigurationRecommendations::new();
    assert!(recommendations.recommendations.is_empty());
    assert_eq!(recommendations.patterns_analyzed, 0);
    assert_eq!(recommendations.recommendations_count, 0);
    assert!(recommendations.summary.is_empty());
}

#[test]
fn test_configuration_recommendations_has_recommendations() {
    let mut recommendations = ConfigurationRecommendations::new();
    assert!(!recommendations.has_recommendations());

    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "test",
            ReconfigurationType::AddCache,
            vec![],
            "Test",
        ));
    assert!(recommendations.has_recommendations());
}

#[test]
fn test_configuration_recommendations_high_priority() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations.recommendations.push(
        GraphReconfiguration::new("low", ReconfigurationType::AddCache, vec![], "Low")
            .with_priority(ReconfigurationPriority::Low),
    );
    recommendations.recommendations.push(
        GraphReconfiguration::new("high", ReconfigurationType::AddCache, vec![], "High")
            .with_priority(ReconfigurationPriority::High),
    );
    recommendations.recommendations.push(
        GraphReconfiguration::new(
            "critical",
            ReconfigurationType::AddCache,
            vec![],
            "Critical",
        )
        .with_priority(ReconfigurationPriority::Critical),
    );

    let high_priority = recommendations.high_priority();
    assert_eq!(high_priority.len(), 2);
}

#[test]
fn test_configuration_recommendations_quick_wins() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations.recommendations.push(
        GraphReconfiguration::new("quick", ReconfigurationType::AddCache, vec![], "Quick")
            .with_priority(ReconfigurationPriority::High)
            .with_effort(1),
    );
    recommendations.recommendations.push(
        GraphReconfiguration::new("slow", ReconfigurationType::AddCache, vec![], "Slow")
            .with_priority(ReconfigurationPriority::High)
            .with_effort(5),
    );

    let quick_wins = recommendations.quick_wins();
    assert_eq!(quick_wins.len(), 1);
    assert_eq!(quick_wins[0].id, "quick");
}

#[test]
fn test_configuration_recommendations_by_type() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "cache1",
            ReconfigurationType::AddCache,
            vec![],
            "Cache 1",
        ));
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "cache2",
            ReconfigurationType::AddCache,
            vec![],
            "Cache 2",
        ));
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "parallel",
            ReconfigurationType::Parallelize,
            vec![],
            "Parallel",
        ));

    let caches = recommendations.by_type(&ReconfigurationType::AddCache);
    assert_eq!(caches.len(), 2);

    let parallels = recommendations.by_type(&ReconfigurationType::Parallelize);
    assert_eq!(parallels.len(), 1);
}

#[test]
fn test_configuration_recommendations_for_node() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "r1",
            ReconfigurationType::AddCache,
            vec!["node1".to_string()],
            "R1",
        ));
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "r2",
            ReconfigurationType::AddCache,
            vec!["node2".to_string()],
            "R2",
        ));
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "r3",
            ReconfigurationType::AddCache,
            vec!["node1".to_string(), "node2".to_string()],
            "R3",
        ));

    let node1_recs = recommendations.for_node("node1");
    assert_eq!(node1_recs.len(), 2);

    let node2_recs = recommendations.for_node("node2");
    assert_eq!(node2_recs.len(), 2);
}

#[test]
fn test_configuration_recommendations_sorted_by_quick_win_score() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations.recommendations.push(
        GraphReconfiguration::new("low", ReconfigurationType::AddCache, vec![], "Low")
            .with_priority(ReconfigurationPriority::Low)
            .with_effort(5)
            .with_confidence(0.1),
    );
    recommendations.recommendations.push(
        GraphReconfiguration::new("high", ReconfigurationType::AddCache, vec![], "High")
            .with_priority(ReconfigurationPriority::Critical)
            .with_effort(1)
            .with_confidence(1.0),
    );

    let sorted = recommendations.sorted_by_quick_win_score();
    assert_eq!(sorted[0].id, "high");
    assert_eq!(sorted[1].id, "low");
}

#[test]
fn test_configuration_recommendations_sorted_by_priority() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations.recommendations.push(
        GraphReconfiguration::new("low", ReconfigurationType::AddCache, vec![], "Low")
            .with_priority(ReconfigurationPriority::Low),
    );
    recommendations.recommendations.push(
        GraphReconfiguration::new("high", ReconfigurationType::AddCache, vec![], "High")
            .with_priority(ReconfigurationPriority::High),
    );

    let sorted = recommendations.sorted_by_priority();
    assert_eq!(sorted[0].id, "high");
    assert_eq!(sorted[1].id, "low");
}

#[test]
fn test_configuration_recommendations_sorted_by_impact() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations.recommendations.push(
        GraphReconfiguration::new("low", ReconfigurationType::AddCache, vec![], "Low")
            .with_estimated_impact(10.0),
    );
    recommendations.recommendations.push(
        GraphReconfiguration::new("high", ReconfigurationType::AddCache, vec![], "High")
            .with_estimated_impact(50.0),
    );

    let sorted = recommendations.sorted_by_impact();
    assert_eq!(sorted[0].id, "high");
    assert_eq!(sorted[1].id, "low");
}

#[test]
fn test_configuration_recommendations_to_optimization_suggestions() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "r1",
            ReconfigurationType::AddCache,
            vec!["n1".to_string()],
            "R1",
        ));
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "r2",
            ReconfigurationType::Parallelize,
            vec!["n2".to_string()],
            "R2",
        ));

    let suggestions = recommendations.to_optimization_suggestions();
    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0].category, OptimizationCategory::Caching);
    assert_eq!(
        suggestions[1].category,
        OptimizationCategory::Parallelization
    );
}

#[test]
fn test_configuration_recommendations_to_json() {
    let mut recommendations = ConfigurationRecommendations::new();
    recommendations
        .recommendations
        .push(GraphReconfiguration::new(
            "r1",
            ReconfigurationType::AddCache,
            vec![],
            "R1",
        ));
    recommendations.patterns_analyzed = 5;
    recommendations.recommendations_count = 1;
    recommendations.summary = "Test summary".to_string();

    let json = recommendations.to_json().unwrap();
    assert!(json.contains("r1"));
    assert!(json.contains("patterns_analyzed"));

    let parsed = ConfigurationRecommendations::from_json(&json).unwrap();
    assert_eq!(parsed.recommendations.len(), 1);
}

#[test]
fn test_recommendation_config_default() {
    let config = RecommendationConfig::default();
    assert!((config.min_confidence - 0.3).abs() < f64::EPSILON);
    assert_eq!(config.min_pattern_frequency, 2);
    assert!(config.include_cache);
    assert!(config.include_parallelization);
    assert!(config.include_model_swap);
    assert!(config.include_timeout);
    assert!(config.include_retry);
    assert!(config.include_batching);
}

#[test]
fn test_recommendation_config_builder() {
    let config = RecommendationConfig::new()
        .with_min_confidence(0.5)
        .with_min_pattern_frequency(3)
        .with_cache(false)
        .with_parallelization(false)
        .with_model_swap(false)
        .with_timeout(false)
        .with_retry(false)
        .with_batching(false);

    assert!((config.min_confidence - 0.5).abs() < f64::EPSILON);
    assert_eq!(config.min_pattern_frequency, 3);
    assert!(!config.include_cache);
    assert!(!config.include_parallelization);
    assert!(!config.include_model_swap);
    assert!(!config.include_timeout);
    assert!(!config.include_retry);
    assert!(!config.include_batching);
}

#[test]
fn test_recommend_configurations_empty_patterns() {
    let analysis = PatternAnalysis::new();
    let recommendations = analysis.recommend_configurations();

    assert!(!recommendations.has_recommendations());
    assert_eq!(recommendations.patterns_analyzed, 0);
    assert!(recommendations.summary.contains("No patterns"));
}

#[test]
fn test_recommend_configurations_caching_from_repeated() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("repeated_node1", PatternType::Repeated)
            .with_affected_node("node1")
            .with_frequency(10)
            .with_confidence(0.8)
            .with_description("Repeated pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    assert!(recommendations.has_recommendations());
    let caching = recommendations.by_type(&ReconfigurationType::AddCache);
    assert!(!caching.is_empty());
    assert!(caching
        .iter()
        .any(|r| r.target_nodes.contains(&"node1".to_string())));
}

#[test]
fn test_recommend_configurations_caching_from_slow_repeated() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("slow_node1", PatternType::Slow)
            .with_affected_node("node1")
            .with_frequency(5)
            .with_confidence(0.7)
            .with_description("Slow pattern"),
    );
    analysis.patterns.push(
        Pattern::new("repeated_node1", PatternType::Repeated)
            .with_affected_node("node1")
            .with_frequency(5)
            .with_confidence(0.7)
            .with_description("Repeated pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    let caching = recommendations.by_type(&ReconfigurationType::AddCache);
    // Should have both regular caching and slow+repeated caching
    assert!(caching.len() >= 2);
}

#[test]
fn test_recommend_configurations_timeout_from_timeout_pattern() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("timeout_node1", PatternType::Timeout)
            .with_affected_node("node1")
            .with_frequency(5)
            .with_confidence(0.8)
            .with_description("Timeout pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    let timeout_recs = recommendations.by_type(&ReconfigurationType::AdjustTimeout);
    assert!(!timeout_recs.is_empty());
}

#[test]
fn test_recommend_configurations_retry_from_failure() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("failure_node1", PatternType::Failure)
            .with_affected_node("node1")
            .with_frequency(5)
            .with_confidence(0.6)
            .with_description("Failure pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    let retry_recs = recommendations.by_type(&ReconfigurationType::AddRetry);
    assert!(!retry_recs.is_empty());
    // Should suggest adding retry since there's no error recovery
    assert!(retry_recs
        .iter()
        .any(|r| r.target_nodes.contains(&"node1".to_string())));
}

#[test]
fn test_recommend_configurations_model_swap_from_high_tokens() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("high_tokens_node1", PatternType::HighTokenUsage)
            .with_affected_node("node1")
            .with_frequency(5)
            .with_confidence(0.7)
            .with_description("High token usage"),
    );
    analysis.patterns.push(
        Pattern::new("success_node1", PatternType::Success)
            .with_affected_node("node1")
            .with_frequency(5)
            .with_confidence(0.9)
            .with_description("Success pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    let swap_recs = recommendations.by_type(&ReconfigurationType::SwapModel);
    assert!(!swap_recs.is_empty());
}

#[test]
fn test_recommend_configurations_with_custom_config() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("repeated_node1", PatternType::Repeated)
            .with_affected_node("node1")
            .with_frequency(10)
            .with_confidence(0.8)
            .with_description("Repeated pattern"),
    );

    // Disable caching recommendations
    let config = RecommendationConfig::new().with_cache(false);
    let recommendations = analysis.recommend_configurations_with_config(&config);

    let caching = recommendations.by_type(&ReconfigurationType::AddCache);
    assert!(caching.is_empty());
}

#[test]
fn test_recommend_configurations_min_confidence_filter() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("repeated_node1", PatternType::Repeated)
            .with_affected_node("node1")
            .with_frequency(10)
            .with_confidence(0.2) // Low confidence
            .with_description("Repeated pattern"),
    );

    let config = RecommendationConfig::new().with_min_confidence(0.5);
    let recommendations = analysis.recommend_configurations_with_config(&config);

    // Should filter out low confidence patterns
    let caching = recommendations.by_type(&ReconfigurationType::AddCache);
    assert!(caching.is_empty());
}

#[test]
fn test_recommend_configurations_sorted_by_quick_win_score() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("repeated_node1", PatternType::Repeated)
            .with_affected_node("node1")
            .with_frequency(15) // High frequency -> High priority
            .with_confidence(0.9)
            .with_description("Repeated pattern"),
    );
    analysis.patterns.push(
        Pattern::new("timeout_node2", PatternType::Timeout)
            .with_affected_node("node2")
            .with_frequency(3)
            .with_confidence(0.5)
            .with_description("Timeout pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    // Results should be sorted by quick win score
    let sorted = recommendations.sorted_by_quick_win_score();
    if sorted.len() >= 2 {
        assert!(sorted[0].quick_win_score() >= sorted[1].quick_win_score());
    }
}

#[test]
fn test_recommend_configurations_summary_generation() {
    let mut analysis = PatternAnalysis::new();
    analysis.patterns.push(
        Pattern::new("repeated_node1", PatternType::Repeated)
            .with_affected_node("node1")
            .with_frequency(10)
            .with_confidence(0.8)
            .with_description("Repeated pattern"),
    );
    analysis.patterns.push(
        Pattern::new("failure_node2", PatternType::Failure)
            .with_affected_node("node2")
            .with_frequency(5)
            .with_confidence(0.6)
            .with_description("Failure pattern"),
    );

    let recommendations = analysis.recommend_configurations();

    assert!(!recommendations.summary.is_empty());
    assert!(recommendations.summary.contains("Generated"));
    assert!(recommendations.summary.contains("recommendations"));
}

#[test]
fn test_full_pipeline_learn_patterns_to_recommendations() {
    // Create a trace with patterns that will generate recommendations
    let trace = ExecutionTrace {
        thread_id: Some("test".to_string()),
        nodes_executed: vec![
            // Repeated successful node - should suggest caching
            // NodeExecution::new() defaults to success=true
            NodeExecution::new("fast_node", 50),
            NodeExecution::new("fast_node", 55),
            NodeExecution::new("fast_node", 60),
            NodeExecution::new("fast_node", 52),
            NodeExecution::new("fast_node", 58),
            NodeExecution::new("fast_node", 53),
            NodeExecution::new("fast_node", 57),
            // Failing node - should suggest retry (with_error sets success=false)
            NodeExecution::new("flaky_node", 100).with_error("Error 1"),
            NodeExecution::new("flaky_node", 110).with_error("Error 2"),
            NodeExecution::new("flaky_node", 105).with_error("Error 3"),
        ],
        total_duration_ms: 700,
        total_tokens: 0,
        completed: true,
        ..Default::default()
    };

    // Learn patterns
    let patterns = trace.learn_patterns_with_thresholds(
        &PatternThresholds::new()
            .with_min_frequency(2)
            .with_min_confidence(0.3),
    );

    assert!(patterns.has_patterns());

    // Generate recommendations
    let recommendations = patterns.recommend_configurations();

    // Should have some recommendations
    assert!(recommendations.has_recommendations());

    // Convert to optimization suggestions for compatibility
    let suggestions = recommendations.to_optimization_suggestions();
    assert!(!suggestions.is_empty());
}

#[test]
fn test_reconfiguration_type_to_optimization_category_mapping() {
    // Test all mappings
    let mappings = vec![
        (ReconfigurationType::AddCache, OptimizationCategory::Caching),
        (
            ReconfigurationType::Parallelize,
            OptimizationCategory::Parallelization,
        ),
        (
            ReconfigurationType::SwapModel,
            OptimizationCategory::ModelChoice,
        ),
        (
            ReconfigurationType::AdjustTimeout,
            OptimizationCategory::Stabilization,
        ),
        (
            ReconfigurationType::AddRetry,
            OptimizationCategory::ErrorHandling,
        ),
        (
            ReconfigurationType::SkipNode,
            OptimizationCategory::Performance,
        ),
        (
            ReconfigurationType::MergeNodes,
            OptimizationCategory::Performance,
        ),
        (
            ReconfigurationType::SplitNode,
            OptimizationCategory::Performance,
        ),
        (
            ReconfigurationType::AddBatching,
            OptimizationCategory::Performance,
        ),
        (
            ReconfigurationType::AddRateLimiting,
            OptimizationCategory::Stabilization,
        ),
        (
            ReconfigurationType::ChangeRouting,
            OptimizationCategory::Performance,
        ),
    ];

    for (reconfig_type, expected_category) in mappings {
        let reconfig = GraphReconfiguration::new("test", reconfig_type, vec![], "test");
        let suggestion = reconfig.to_optimization_suggestion();
        assert_eq!(suggestion.category, expected_category);
    }
}
