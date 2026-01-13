//! LLM-as-Judge quality evaluation for multi-turn conversation tests

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Quality evaluation scores from LLM judge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    /// Accuracy: 0.0-1.0, is information factually correct?
    pub accuracy: f32,
    /// Relevance: 0.0-1.0, does it address the query?
    pub relevance: f32,
    /// Completeness: 0.0-1.0, covers all important aspects?
    pub completeness: f32,
    /// LLM's reasoning for the scores
    pub reasoning: String,
}

impl QualityScore {
    /// Calculate average quality score
    #[must_use]
    pub fn average(&self) -> f32 {
        (self.accuracy + self.relevance + self.completeness) / 3.0
    }

    /// Check if quality meets threshold (default 0.7)
    #[must_use]
    pub fn meets_threshold(&self, threshold: f32) -> bool {
        self.accuracy >= threshold && self.relevance >= threshold && self.completeness >= threshold
    }
}

/// LLM-as-Judge evaluator for response quality
pub struct QualityJudge {
    judge_model: Arc<dyn ChatModel>,
}

impl QualityJudge {
    /// Create new quality judge with a provider-agnostic model
    ///
    /// Use with `create_llm()` from llm_factory:
    /// ```ignore
    /// let model = create_llm(LLMRequirements::default()).await?;
    /// let judge = QualityJudge::new(model);
    /// ```
    #[must_use]
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { judge_model: model }
    }

    /// Judge response quality for a single conversational turn
    ///
    /// # Arguments
    /// * `query` - User's query
    /// * `response` - AI assistant's response
    /// * `expected_topics` - Topics that should be covered
    /// * `context` - Optional previous conversation context
    /// * `should_use_tools` - Whether the agent should have used tools for this query (defaults to true)
    ///
    /// # Returns
    /// Quality scores on 0.0-1.0 scale for accuracy, relevance, completeness
    pub async fn judge_response(
        &self,
        query: &str,
        response: &str,
        expected_topics: &[&str],
        context: Option<&str>,
    ) -> Result<QualityScore, Box<dyn std::error::Error>> {
        self.judge_response_with_options(query, response, expected_topics, context, true)
            .await
    }

    /// Judge response quality with explicit tool usage expectation
    pub async fn judge_response_with_options(
        &self,
        query: &str,
        response: &str,
        expected_topics: &[&str],
        context: Option<&str>,
        should_use_tools: bool,
    ) -> Result<QualityScore, Box<dyn std::error::Error>> {
        let context_info = context
            .map(|c| format!("\nPrevious Context: {c}\n"))
            .unwrap_or_default();

        let tool_expectation = if should_use_tools {
            "\n**Tool Usage Expectation**: This agent has tools available and MUST use them to answer queries.\n"
        } else {
            ""
        };

        let completeness_rules = if should_use_tools {
            "\n**CRITICAL COMPLETENESS RULES (Tool-based agent)**:\n\
             - If response says \"couldn't find documentation\" or \"no information available\", completeness = 0.0\n\
             - If response doesn't cite or reference retrieved information, completeness ≤ 0.5\n\
             - If response relies primarily on internal knowledge, completeness ≤ 0.7\n\
             - If response properly uses tool results and covers all expected topics, completeness ≥ 0.95\n\
             - Partial coverage of expected topics: completeness proportional to coverage (e.g., 2/3 topics = 0.67)\n\
             - Look for phrases like \"based on the documentation\" or \"according to the search results\" as evidence of tool usage\n"
        } else {
            "\n**COMPLETENESS RULES**:\n\
             - Full coverage of all expected topics: completeness ≥ 0.9\n\
             - Partial coverage: completeness proportional to coverage (e.g., 2/3 topics = 0.67)\n\
             - Missing critical information: completeness ≤ 0.5\n"
        };

        let prompt = format!(
            "You are evaluating an AI assistant's response quality.{tool_expectation}\n\
             User Query: {query}\n{context_info}\
             AI Response: {response}\n\
             Expected Topics: {expected_topics:?}\n\n\
             Evaluate the response on three dimensions (0.0-1.0 scale):\n\n\
             1. **Accuracy** (0.0-1.0): Is the information factually correct?\n\
             2. **Relevance** (0.0-1.0): Does it directly address the user's query?\n\
             3. **Completeness** (0.0-1.0): Does it cover all important aspects?\n\
             {completeness_rules}\n\
             Respond with ONLY valid JSON in this exact format:\n\
             {{\"accuracy\": 0.9, \"relevance\": 0.95, \"completeness\": 0.85, \"reasoning\": \"Brief 1-2 sentence explanation\"}}\n\n\
             Important: Respond ONLY with JSON, no additional text."
        );

        let messages = vec![Message::human(prompt)];

        let judge_response = self
            .judge_model
            .generate(&messages, None, None, None, None)
            .await?;

        // Extract JSON from response (may have markdown formatting)
        let content = judge_response.generations[0].message.content().as_text();
        let json_str = if content.contains("```json") {
            // Extract from markdown code block
            content
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&content)
                .trim()
        } else if content.contains("```") {
            // Extract from generic code block
            content.split("```").nth(1).unwrap_or(&content).trim()
        } else {
            content.trim()
        };

        let score: QualityScore = serde_json::from_str(json_str).map_err(|e| {
            format!("Failed to parse judge response as JSON: {e}. Response was: {json_str}")
        })?;

        Ok(score)
    }

    /// Judge a complete multi-turn conversation
    ///
    /// Evaluates each turn and returns aggregate scores
    pub async fn judge_conversation(
        &self,
        turns: &[(String, String, Vec<String>)], // (query, response, expected_topics)
    ) -> Result<Vec<QualityScore>, Box<dyn std::error::Error>> {
        let mut scores = Vec::new();
        let mut context = String::new();

        for (i, (query, response, topics)) in turns.iter().enumerate() {
            println!("Judging turn {}...", i + 1);

            let topics_refs: Vec<&str> = topics.iter().map(std::string::String::as_str).collect();
            let score = self
                .judge_response(
                    query,
                    response,
                    &topics_refs,
                    if context.is_empty() {
                        None
                    } else {
                        Some(&context)
                    },
                )
                .await?;

            println!(
                "  Turn {} Quality: {:.2} (Acc:{:.2}, Rel:{:.2}, Comp:{:.2})",
                i + 1,
                score.average(),
                score.accuracy,
                score.relevance,
                score.completeness
            );
            println!("  Reasoning: {}", score.reasoning);

            // Update context for next turn
            context = format!("{context}Q: {query}\nA: {response}\n");

            scores.push(score);
        }

        Ok(scores)
    }
}

// Note: Default impl removed since QualityJudge::new() now requires a model parameter.
// Use: let judge = QualityJudge::new(create_llm(LLMRequirements::default()).await?);
