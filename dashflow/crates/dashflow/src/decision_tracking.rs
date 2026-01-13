// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Decision Tracking for Self-Aware Agents (Observability Phase 4)
//!
//! Provides a convenient helper for agents to emit `DecisionMade` and `OutcomeObserved`
//! events, enabling the Learning Corpus to analyze agent decision patterns over time.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────┐     ┌─────────────────────┐     ┌─────────────────┐
//! │  DecisionTracker │────▶│   EventCallback<S>  │────▶│  WALEventCallback │
//! │   (agent helper) │     │  (graph execution)  │     │   (persistence)   │
//! └──────────────────┘     └─────────────────────┘     └─────────────────┘
//!         │                                                     │
//!         ▼                                                     ▼
//! ┌──────────────────┐                                ┌─────────────────┐
//! │  record_decision │                                │ ~/.dashflow/wal/│
//! │  record_outcome  │                                └─────────────────┘
//! └──────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::decision_tracking::DecisionTracker;
//! use dashflow::event::DecisionAlternative;
//!
//! // Create tracker with a callback
//! let tracker = DecisionTracker::new(callback, "router_agent".to_string());
//!
//! // Record a decision
//! let decision_id = tracker.record_decision(
//!     "tool_selection",
//!     "search_tool",
//!     vec![
//!         DecisionAlternative {
//!             option: "code_tool".to_string(),
//!             reason: Some("not relevant to query".to_string()),
//!             score: Some(0.3),
//!             was_fully_evaluated: true,
//!         },
//!     ],
//!     Some("User query is about finding information".to_string()),
//! );
//!
//! // ... execute the decision ...
//!
//! // Record the outcome
//! tracker.record_outcome(&decision_id, true, Some(0.95));
//! ```
//!
//! # Integration with Agents
//!
//! Agents can create a DecisionTracker during initialization and use it throughout
//! their execution to track strategic decisions and their outcomes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use crate::event::{DecisionAlternative, EventCallback, GraphEvent};
use crate::state::GraphState;

/// Helper for agents to track decisions and outcomes.
///
/// Provides a convenient API for emitting `DecisionMade` and `OutcomeObserved` events
/// through an `EventCallback`. This enables the Learning Corpus to analyze agent
/// decision patterns and correlate decisions with their outcomes.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::decision_tracking::DecisionTracker;
/// use dashflow::event::DecisionAlternative;
/// use std::sync::Arc;
///
/// let tracker = DecisionTracker::new(callback, "my_agent".to_string());
///
/// // Record a routing decision
/// let decision_id = tracker.record_decision(
///     "routing",
///     "fast_path",
///     vec![DecisionAlternative {
///         option: "slow_path".to_string(),
///         reason: Some("latency requirements".to_string()),
///         score: Some(0.4),
///         was_fully_evaluated: true,
///     }],
///     Some("Low latency required for this request".to_string()),
/// );
///
/// // Later, record the outcome
/// tracker.record_outcome(&decision_id, true, Some(0.92));
/// ```
pub struct DecisionTracker<S: GraphState> {
    callback: Arc<dyn EventCallback<S>>,
    /// Name of the agent/component making decisions
    decision_maker: String,
}

impl<S: GraphState> DecisionTracker<S> {
    /// Create a new DecisionTracker.
    ///
    /// # Arguments
    ///
    /// * `callback` - The event callback to emit events through
    /// * `decision_maker` - Name of the agent or component making decisions
    pub fn new(callback: Arc<dyn EventCallback<S>>, decision_maker: String) -> Self {
        Self {
            callback,
            decision_maker,
        }
    }

    /// Record a decision made by the agent.
    ///
    /// Emits a `DecisionMade` event and returns a unique decision ID that can be
    /// used to correlate with a later `OutcomeObserved` event.
    ///
    /// # Arguments
    ///
    /// * `decision_type` - Category of decision (e.g., "routing", "tool_selection", "retry_strategy")
    /// * `chosen_option` - The option that was chosen
    /// * `alternatives` - Alternatives that were considered but not chosen
    /// * `reasoning` - Human-readable explanation for the decision
    ///
    /// # Returns
    ///
    /// A unique decision ID (UUID) for correlation with `record_outcome()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let decision_id = tracker.record_decision(
    ///     "tool_selection",
    ///     "web_search",
    ///     vec![],
    ///     Some("Query requires external information".to_string()),
    /// );
    /// ```
    pub fn record_decision(
        &self,
        decision_type: &str,
        chosen_option: &str,
        alternatives: Vec<DecisionAlternative>,
        reasoning: Option<String>,
    ) -> String {
        self.record_decision_with_context(
            decision_type,
            chosen_option,
            alternatives,
            reasoning,
            None,
            HashMap::new(),
        )
    }

    /// Record a decision with confidence score and context.
    ///
    /// Extended version of `record_decision()` that includes additional metadata.
    ///
    /// # Arguments
    ///
    /// * `decision_type` - Category of decision
    /// * `chosen_option` - The option that was chosen
    /// * `alternatives` - Alternatives that were considered
    /// * `reasoning` - Human-readable explanation
    /// * `confidence` - Confidence score (0.0 to 1.0) if available
    /// * `context` - Additional key-value context that influenced the decision
    ///
    /// # Returns
    ///
    /// A unique decision ID (UUID) for correlation.
    pub fn record_decision_with_context(
        &self,
        decision_type: &str,
        chosen_option: &str,
        alternatives: Vec<DecisionAlternative>,
        reasoning: Option<String>,
        confidence: Option<f64>,
        context: HashMap<String, String>,
    ) -> String {
        let decision_id = uuid::Uuid::new_v4().to_string();

        let event = GraphEvent::DecisionMade {
            timestamp: SystemTime::now(),
            decision_id: decision_id.clone(),
            decision_maker: self.decision_maker.clone(),
            decision_type: decision_type.to_string(),
            chosen_option: chosen_option.to_string(),
            alternatives_considered: alternatives,
            confidence,
            reasoning,
            context,
        };

        self.callback.on_event(&event);

        decision_id
    }

    /// Record the outcome of a previous decision.
    ///
    /// Emits an `OutcomeObserved` event correlated with a previous `DecisionMade` event.
    ///
    /// # Arguments
    ///
    /// * `decision_id` - The decision ID returned by `record_decision()`
    /// * `success` - Whether the decision led to a successful outcome
    /// * `score` - Quantitative score if applicable (e.g., quality metric)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Decision was made earlier
    /// let decision_id = tracker.record_decision("routing", "path_a", vec![], None);
    ///
    /// // ... execute the path ...
    ///
    /// // Record success
    /// tracker.record_outcome(&decision_id, true, Some(0.95));
    /// ```
    pub fn record_outcome(&self, decision_id: &str, success: bool, score: Option<f64>) {
        self.record_outcome_with_details(decision_id, success, score, None, None, HashMap::new());
    }

    /// Record an outcome with detailed information.
    ///
    /// Extended version of `record_outcome()` with additional metadata.
    ///
    /// # Arguments
    ///
    /// * `decision_id` - The decision ID to correlate with
    /// * `success` - Whether the outcome was successful
    /// * `score` - Quantitative score if applicable
    /// * `outcome_description` - Human-readable description of what happened
    /// * `latency_ms` - Time elapsed since the decision was made
    /// * `metrics` - Additional quantitative metrics
    pub fn record_outcome_with_details(
        &self,
        decision_id: &str,
        success: bool,
        score: Option<f64>,
        outcome_description: Option<String>,
        latency_ms: Option<u64>,
        metrics: HashMap<String, f64>,
    ) {
        let event = GraphEvent::OutcomeObserved {
            timestamp: SystemTime::now(),
            decision_id: decision_id.to_string(),
            success,
            score,
            outcome_description,
            latency_ms,
            metrics,
        };

        self.callback.on_event(&event);
    }

    /// Get the decision maker name.
    pub fn decision_maker(&self) -> &str {
        &self.decision_maker
    }

    /// Create a child tracker with a different decision maker name.
    ///
    /// Useful when sub-components of an agent need to track their own decisions
    /// while sharing the same callback.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let main_tracker = DecisionTracker::new(callback.clone(), "main_agent".to_string());
    /// let sub_tracker = main_tracker.with_decision_maker("sub_component".to_string());
    /// ```
    #[must_use]
    pub fn with_decision_maker(&self, decision_maker: String) -> Self {
        Self {
            callback: Arc::clone(&self.callback),
            decision_maker,
        }
    }
}

/// Builder for creating DecisionAlternative instances.
///
/// Provides a fluent API for constructing alternatives when recording decisions.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::decision_tracking::AlternativeBuilder;
///
/// let alternatives = vec![
///     AlternativeBuilder::new("option_b")
///         .reason("Lower confidence score")
///         .score(0.6)
///         .fully_evaluated()
///         .build(),
///     AlternativeBuilder::new("option_c")
///         .reason("Filtered by constraints")
///         .not_fully_evaluated()
///         .build(),
/// ];
/// ```
pub struct AlternativeBuilder {
    option: String,
    reason: Option<String>,
    score: Option<f64>,
    was_fully_evaluated: bool,
}

impl AlternativeBuilder {
    /// Create a new alternative builder.
    pub fn new(option: impl Into<String>) -> Self {
        Self {
            option: option.into(),
            reason: None,
            score: None,
            was_fully_evaluated: true,
        }
    }

    /// Set the reason this alternative was not chosen.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the score assigned to this alternative.
    pub fn score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    /// Mark this alternative as fully evaluated.
    pub fn fully_evaluated(mut self) -> Self {
        self.was_fully_evaluated = true;
        self
    }

    /// Mark this alternative as not fully evaluated (filtered early).
    pub fn not_fully_evaluated(mut self) -> Self {
        self.was_fully_evaluated = false;
        self
    }

    /// Build the DecisionAlternative.
    pub fn build(self) -> DecisionAlternative {
        DecisionAlternative {
            option: self.option,
            reason: self.reason,
            score: self.score,
            was_fully_evaluated: self.was_fully_evaluated,
        }
    }
}

// FIX-014: Re-export context-based decision API for nodes
// These functions can be called from within node execution without needing callback access
pub use crate::executor::{
    record_decision as record_decision_from_node,
    record_decision_with_context as record_decision_with_context_from_node,
    record_outcome as record_outcome_from_node,
    record_outcome_with_details as record_outcome_with_details_from_node,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::PrintCallback;
    use crate::state::AgentState;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Test callback that records events for verification.
    struct RecordingCallback {
        events: Mutex<Vec<String>>,
        event_count: AtomicUsize,
    }

    impl RecordingCallback {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
                event_count: AtomicUsize::new(0),
            }
        }

        fn event_count(&self) -> usize {
            self.event_count.load(Ordering::SeqCst)
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl<S: GraphState> EventCallback<S> for RecordingCallback {
        fn on_event(&self, event: &GraphEvent<S>) {
            let event_type = match event {
                GraphEvent::DecisionMade { decision_type, .. } => {
                    format!("DecisionMade:{}", decision_type)
                }
                GraphEvent::OutcomeObserved { success, .. } => {
                    format!("OutcomeObserved:{}", success)
                }
                _ => "Other".to_string(),
            };

            self.events.lock().unwrap().push(event_type);
            self.event_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_decision_tracker_creation() {
        let callback: Arc<dyn EventCallback<AgentState>> = Arc::new(PrintCallback);
        let tracker = DecisionTracker::new(callback, "test_agent".to_string());
        assert_eq!(tracker.decision_maker(), "test_agent");
    }

    #[test]
    fn test_record_decision_returns_uuid() {
        let callback: Arc<dyn EventCallback<AgentState>> = Arc::new(RecordingCallback::new());
        let tracker = DecisionTracker::new(callback, "test_agent".to_string());

        let decision_id = tracker.record_decision("routing", "path_a", vec![], None);

        // UUID should be valid format (36 chars with hyphens)
        assert_eq!(decision_id.len(), 36);
        assert!(decision_id.contains('-'));
    }

    #[test]
    fn test_record_decision_emits_event() {
        let callback = Arc::new(RecordingCallback::new());
        let callback_clone: Arc<dyn EventCallback<AgentState>> = callback.clone();
        let tracker = DecisionTracker::new(callback_clone, "test_agent".to_string());

        let _decision_id = tracker.record_decision(
            "tool_selection",
            "search_tool",
            vec![AlternativeBuilder::new("code_tool")
                .reason("not relevant")
                .score(0.3)
                .build()],
            Some("User needs information".to_string()),
        );

        assert_eq!(callback.event_count(), 1);
        let events = callback.events();
        assert_eq!(events[0], "DecisionMade:tool_selection");
    }

    #[test]
    fn test_record_outcome_emits_event() {
        let callback = Arc::new(RecordingCallback::new());
        let callback_clone: Arc<dyn EventCallback<AgentState>> = callback.clone();
        let tracker = DecisionTracker::new(callback_clone, "test_agent".to_string());

        let decision_id = tracker.record_decision("routing", "path_a", vec![], None);
        tracker.record_outcome(&decision_id, true, Some(0.95));

        assert_eq!(callback.event_count(), 2);
        let events = callback.events();
        assert_eq!(events[0], "DecisionMade:routing");
        assert_eq!(events[1], "OutcomeObserved:true");
    }

    #[test]
    fn test_record_outcome_failure() {
        let callback = Arc::new(RecordingCallback::new());
        let callback_clone: Arc<dyn EventCallback<AgentState>> = callback.clone();
        let tracker = DecisionTracker::new(callback_clone, "test_agent".to_string());

        let decision_id = tracker.record_decision("strategy", "aggressive", vec![], None);
        tracker.record_outcome(&decision_id, false, Some(0.2));

        let events = callback.events();
        assert_eq!(events[1], "OutcomeObserved:false");
    }

    #[test]
    fn test_record_decision_with_context() {
        let callback = Arc::new(RecordingCallback::new());
        let callback_clone: Arc<dyn EventCallback<AgentState>> = callback.clone();
        let tracker = DecisionTracker::new(callback_clone, "test_agent".to_string());

        let mut context = HashMap::new();
        context.insert("priority".to_string(), "high".to_string());
        context.insert("user_tier".to_string(), "premium".to_string());

        let decision_id = tracker.record_decision_with_context(
            "routing",
            "fast_path",
            vec![],
            Some("High priority request".to_string()),
            Some(0.95),
            context,
        );

        assert!(!decision_id.is_empty());
        assert_eq!(callback.event_count(), 1);
    }

    #[test]
    fn test_record_outcome_with_details() {
        let callback = Arc::new(RecordingCallback::new());
        let callback_clone: Arc<dyn EventCallback<AgentState>> = callback.clone();
        let tracker = DecisionTracker::new(callback_clone, "test_agent".to_string());

        let decision_id = tracker.record_decision("routing", "path_a", vec![], None);

        let mut metrics = HashMap::new();
        metrics.insert("response_time_ms".to_string(), 150.0);
        metrics.insert("tokens_used".to_string(), 500.0);

        tracker.record_outcome_with_details(
            &decision_id,
            true,
            Some(0.92),
            Some("Request completed successfully".to_string()),
            Some(150),
            metrics,
        );

        assert_eq!(callback.event_count(), 2);
    }

    #[test]
    fn test_with_decision_maker() {
        let callback: Arc<dyn EventCallback<AgentState>> = Arc::new(RecordingCallback::new());
        let main_tracker = DecisionTracker::new(callback, "main_agent".to_string());
        let sub_tracker = main_tracker.with_decision_maker("sub_component".to_string());

        assert_eq!(main_tracker.decision_maker(), "main_agent");
        assert_eq!(sub_tracker.decision_maker(), "sub_component");
    }

    #[test]
    fn test_alternative_builder() {
        let alt = AlternativeBuilder::new("option_b")
            .reason("Lower confidence")
            .score(0.6)
            .fully_evaluated()
            .build();

        assert_eq!(alt.option, "option_b");
        assert_eq!(alt.reason, Some("Lower confidence".to_string()));
        assert_eq!(alt.score, Some(0.6));
        assert!(alt.was_fully_evaluated);
    }

    #[test]
    fn test_alternative_builder_not_fully_evaluated() {
        let alt = AlternativeBuilder::new("option_c")
            .reason("Filtered by constraint")
            .not_fully_evaluated()
            .build();

        assert_eq!(alt.option, "option_c");
        assert!(!alt.was_fully_evaluated);
    }

    #[test]
    fn test_multiple_decisions_unique_ids() {
        let callback: Arc<dyn EventCallback<AgentState>> = Arc::new(RecordingCallback::new());
        let tracker = DecisionTracker::new(callback, "test_agent".to_string());

        let id1 = tracker.record_decision("routing", "path_a", vec![], None);
        let id2 = tracker.record_decision("routing", "path_a", vec![], None);
        let id3 = tracker.record_decision("routing", "path_b", vec![], None);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }
}
