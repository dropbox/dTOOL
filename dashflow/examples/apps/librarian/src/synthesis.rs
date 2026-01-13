//! Answer synthesis for Librarian queries
//!
//! This module provides the ability to synthesize natural language answers
//! from retrieved search results using DashFlow's LLM abstractions.

use crate::SearchResult;
use anyhow::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::HumanMessage;
use std::sync::Arc;

const EMPTY_RESULTS_MESSAGE: &str = "No relevant passages found to answer your question.";

/// Synthesizes natural language answers from search results.
///
/// Uses a chat model to generate concise, accurate answers based on
/// retrieved passages from classic literature.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_openai::ChatOpenAI;
/// use librarian::synthesis::AnswerSynthesizer;
///
/// let model = Arc::new(ChatOpenAI::new().with_model("gpt-4o-mini"));
/// let synthesizer = AnswerSynthesizer::new(model);
///
/// let answer = synthesizer.synthesize("What drives Ahab?", &results).await?;
/// println!("{}", answer);
/// ```
pub struct AnswerSynthesizer {
    model: Arc<dyn ChatModel>,
}

impl AnswerSynthesizer {
    /// Create a new answer synthesizer with the given chat model.
    ///
    /// # Arguments
    ///
    /// * `model` - A chat model implementing the `ChatModel` trait
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { model }
    }

    /// Synthesize a natural language answer from search results.
    ///
    /// # Arguments
    ///
    /// * `query` - The original search query
    /// * `results` - Search results to use as context
    ///
    /// # Returns
    ///
    /// A synthesized answer string based on the retrieved passages.
    pub async fn synthesize(&self, query: &str, results: &[SearchResult]) -> Result<String> {
        if results.is_empty() {
            return Ok(EMPTY_RESULTS_MESSAGE.to_string());
        }

        let context = format_context(results);

        let prompt = format!(
            r#"Based on the following excerpts from classic literature, answer this question: {}

PASSAGES:
{}

Instructions:
- Provide a concise, accurate answer based only on the provided passages
- Cite specific books and authors when making claims
- If the passages don't fully answer the question, acknowledge what is and isn't covered
- Keep your answer focused and readable"#,
            query, context
        );

        let messages = vec![HumanMessage::new(prompt.as_str())];
        let result = dashflow::generate(Arc::clone(&self.model), &messages).await?;

        // Extract the response text
        let response = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_else(|| "Failed to generate response.".to_string());

        Ok(response)
    }
}

fn format_context(results: &[SearchResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] From \"{}\" by {}:\n{}", i + 1, r.title, r.author, r.content))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_results_message_constant_is_stable() {
        assert_eq!(
            EMPTY_RESULTS_MESSAGE,
            "No relevant passages found to answer your question."
        );
    }

    #[test]
    fn format_context_includes_citations() {
        let results = vec![SearchResult {
            content: "Call me Ishmael.".to_string(),
            title: "Moby-Dick".to_string(),
            author: "Herman Melville".to_string(),
            book_id: "2701".to_string(),
            chunk_index: 0,
            score: 0.9,
        }];

        let ctx = format_context(&results);
        assert!(ctx.contains("From \"Moby-Dick\" by Herman Melville"));
        assert!(ctx.contains("Call me Ishmael."));
    }
}
