//! MapReduce Template Example
//!
//! This example demonstrates the MapReduce graph template pattern:
//! - Input preparation node sets up data
//! - Multiple mapper nodes execute in parallel
//! - Reducer node aggregates all results
//!
//! Use case: Document analysis pipeline
//! - Input: Large document split into sections
//! - Mappers: Analyze different aspects (sentiment, keywords, entities)
//! - Reducer: Combine into comprehensive report
//!
//! Run: cargo run --example template_mapreduce

use dashflow::templates::GraphTemplate;
use dashflow::MergeableState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DocumentAnalysisState {
    document: String,
    word_count: usize,
    char_count: usize,
    sentence_count: usize,
    sentiment_score: f32,
    top_keywords: Vec<String>,
    analysis_summary: String,
}

impl MergeableState for DocumentAnalysisState {
    fn merge(&mut self, other: &Self) {
        if !other.document.is_empty() {
            if self.document.is_empty() {
                self.document = other.document.clone();
            } else {
                self.document.push('\n');
                self.document.push_str(&other.document);
            }
        }
        self.word_count = self.word_count.max(other.word_count);
        self.char_count = self.char_count.max(other.char_count);
        self.sentence_count = self.sentence_count.max(other.sentence_count);
        self.sentiment_score = self.sentiment_score.max(other.sentiment_score);
        self.top_keywords.extend(other.top_keywords.clone());
        if !other.analysis_summary.is_empty() {
            if self.analysis_summary.is_empty() {
                self.analysis_summary = other.analysis_summary.clone();
            } else {
                self.analysis_summary.push('\n');
                self.analysis_summary.push_str(&other.analysis_summary);
            }
        }
    }
}

impl DocumentAnalysisState {
    fn new(document: impl Into<String>) -> Self {
        Self {
            document: document.into(),
            word_count: 0,
            char_count: 0,
            sentence_count: 0,
            sentiment_score: 0.0,
            top_keywords: Vec::new(),
            analysis_summary: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📄 Document Analysis Pipeline - MapReduce Pattern\n");

    // Build graph using MapReduce template
    let graph = GraphTemplate::map_reduce()
        .with_input_node_fn("prepare_document", |state: DocumentAnalysisState| {
            Box::pin(async move {
                println!("📥 Input: Preparing document for analysis...");
                println!("   Document length: {} characters", state.document.len());
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                Ok(state)
            })
        })
        .with_mapper_fn("mapper_word_count", |mut state: DocumentAnalysisState| {
            Box::pin(async move {
                println!("🔄 Mapper 1: Counting words...");
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

                state.word_count = state.document.split_whitespace().count();
                println!("   Word count: {}", state.word_count);

                Ok(state)
            })
        })
        .with_mapper_fn(
            "mapper_char_sentence",
            |mut state: DocumentAnalysisState| {
                Box::pin(async move {
                    println!("🔄 Mapper 2: Counting characters and sentences...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

                    state.char_count = state.document.len();
                    state.sentence_count = state
                        .document
                        .split(['.', '!', '?'])
                        .filter(|s| !s.trim().is_empty())
                        .count();

                    println!("   Character count: {}", state.char_count);
                    println!("   Sentence count: {}", state.sentence_count);

                    Ok(state)
                })
            },
        )
        .with_mapper_fn("mapper_sentiment", |mut state: DocumentAnalysisState| {
            Box::pin(async move {
                println!("🔄 Mapper 3: Analyzing sentiment...");
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                // Simple sentiment analysis (count positive words)
                let positive_words = ["good", "great", "excellent", "amazing", "wonderful", "best"];
                let negative_words = ["bad", "terrible", "awful", "worst", "poor", "horrible"];

                let doc_lower = state.document.to_lowercase();
                let positive_count = positive_words
                    .iter()
                    .filter(|w| doc_lower.contains(*w))
                    .count();
                let negative_count = negative_words
                    .iter()
                    .filter(|w| doc_lower.contains(*w))
                    .count();

                state.sentiment_score = if positive_count + negative_count > 0 {
                    (positive_count as f32 - negative_count as f32)
                        / (positive_count + negative_count) as f32
                } else {
                    0.0
                };

                println!("   Sentiment score: {:.2}", state.sentiment_score);

                Ok(state)
            })
        })
        .with_mapper_fn("mapper_keywords", |mut state: DocumentAnalysisState| {
            Box::pin(async move {
                println!("🔄 Mapper 4: Extracting keywords...");
                tokio::time::sleep(tokio::time::Duration::from_millis(180)).await;

                // Simple keyword extraction (words > 6 chars, frequency > 1)
                let mut word_freq: HashMap<String, usize> = HashMap::new();
                let stop_words = ["the", "and", "for", "with", "this", "that", "from"];

                for word in state.document.split_whitespace() {
                    let clean_word = word
                        .trim_matches(|c: char| !c.is_alphanumeric())
                        .to_lowercase();
                    if clean_word.len() > 6 && !stop_words.contains(&clean_word.as_str()) {
                        *word_freq.entry(clean_word).or_insert(0) += 1;
                    }
                }

                let mut keywords: Vec<(String, usize)> = word_freq
                    .into_iter()
                    .filter(|(_, count)| *count > 1)
                    .collect();
                keywords.sort_by(|a, b| b.1.cmp(&a.1));

                state.top_keywords = keywords.into_iter().take(5).map(|(word, _)| word).collect();

                println!("   Top keywords: {:?}", state.top_keywords);

                Ok(state)
            })
        })
        .with_reducer_node_fn("aggregate_results", |mut state: DocumentAnalysisState| {
            Box::pin(async move {
                println!("\n🔀 Reducer: Aggregating analysis results...");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // Compile comprehensive summary
                let sentiment_label = if state.sentiment_score > 0.3 {
                    "Positive"
                } else if state.sentiment_score < -0.3 {
                    "Negative"
                } else {
                    "Neutral"
                };

                let avg_word_length = if state.word_count > 0 {
                    state.char_count as f32 / state.word_count as f32
                } else {
                    0.0
                };

                state.analysis_summary = format!(
                    "Document Analysis Complete:\n\
                         - Word Count: {}\n\
                         - Character Count: {}\n\
                         - Sentence Count: {}\n\
                         - Average Word Length: {:.1} chars\n\
                         - Sentiment: {} ({:.2})\n\
                         - Top Keywords: {}",
                    state.word_count,
                    state.char_count,
                    state.sentence_count,
                    avg_word_length,
                    sentiment_label,
                    state.sentiment_score,
                    state.top_keywords.join(", ")
                );

                println!("\n📊 Analysis Summary:");
                println!("{}", state.analysis_summary);

                Ok(state)
            })
        })
        .build()?;

    // Test Case 1: Positive article about technology
    println!("=== Test Case 1: Technology Article ===\n");
    let compiled = graph.compile()?;

    let doc1 = "Artificial intelligence continues to demonstrate amazing capabilities. \
                The technology has excellent potential for solving complex problems. \
                Researchers have developed wonderful algorithms that can process information \
                efficiently. These systems show great promise for the future of computing.";

    let state1 = DocumentAnalysisState::new(doc1);
    let _result1 = compiled.invoke(state1).await?;

    // Test Case 2: Mixed sentiment business text
    println!("\n\n=== Test Case 2: Business Report ===\n");

    let doc2 = "The quarterly earnings report shows terrible performance in several sectors. \
                However, the company has excellent growth potential in emerging markets. \
                Management faces difficult challenges but remains optimistic about recovery. \
                Innovation continues to drive competitive advantages despite poor short-term results.";

    let state2 = DocumentAnalysisState::new(doc2);
    let _result2 = compiled.invoke(state2).await?;

    println!("\n✅ All analyses completed!");

    Ok(())
}
