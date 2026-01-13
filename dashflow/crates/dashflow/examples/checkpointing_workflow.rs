//! Checkpointing workflow example
//!
//! Demonstrates how to use checkpointing for:
//! - State persistence across executions
//! - Resume from failures
//! - Audit trails and debugging
//!
//! This example shows a multi-step document processing workflow that
//! can be interrupted and resumed from checkpoints.

use dashflow::{FileCheckpointer, MemoryCheckpointer, MergeableState, Result, StateGraph};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Document processing state
#[derive(Clone, Debug, Serialize, Deserialize)]
struct DocumentState {
    /// Document content
    content: String,
    /// Processing stage
    stage: String,
    /// Number of words
    word_count: usize,
    /// Summary
    summary: Option<String>,
    /// Tags extracted
    tags: Vec<String>,
    /// Quality score
    quality_score: Option<f64>,
}

impl MergeableState for DocumentState {
    fn merge(&mut self, other: &Self) {
        if !other.content.is_empty() {
            if self.content.is_empty() {
                self.content = other.content.clone();
            } else {
                self.content.push('\n');
                self.content.push_str(&other.content);
            }
        }
        if !other.stage.is_empty() {
            if self.stage.is_empty() {
                self.stage = other.stage.clone();
            } else {
                self.stage.push('\n');
                self.stage.push_str(&other.stage);
            }
        }
        self.word_count = self.word_count.max(other.word_count);
        if other.summary.is_some() {
            self.summary = other.summary.clone();
        }
        self.tags.extend(other.tags.clone());
        if other.quality_score.is_some() {
            self.quality_score = other.quality_score;
        }
    }
}

/// Parse document
async fn parse_document(state: DocumentState) -> Result<DocumentState> {
    println!("📄 Parsing document...");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let word_count = state.content.split_whitespace().count();
    Ok(DocumentState {
        content: state.content,
        stage: "parsed".to_string(),
        word_count,
        summary: state.summary,
        tags: state.tags,
        quality_score: state.quality_score,
    })
}

/// Extract summary
async fn extract_summary(state: DocumentState) -> Result<DocumentState> {
    println!("📝 Extracting summary...");
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // Simple summary: first 50 chars
    let summary = state.content.chars().take(50).collect::<String>() + "...";
    Ok(DocumentState {
        content: state.content,
        stage: "summarized".to_string(),
        word_count: state.word_count,
        summary: Some(summary),
        tags: state.tags,
        quality_score: state.quality_score,
    })
}

/// Extract tags
async fn extract_tags(state: DocumentState) -> Result<DocumentState> {
    println!("🏷️  Extracting tags...");
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;

    // Simple tag extraction: words > 5 chars
    let tags: Vec<String> = state
        .content
        .split_whitespace()
        .filter(|w| w.len() > 5)
        .take(5)
        .map(|s| s.to_lowercase())
        .collect();

    Ok(DocumentState {
        content: state.content,
        stage: "tagged".to_string(),
        word_count: state.word_count,
        summary: state.summary,
        tags,
        quality_score: state.quality_score,
    })
}

/// Calculate quality score
async fn calculate_quality(state: DocumentState) -> Result<DocumentState> {
    println!("⭐ Calculating quality score...");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Simple quality: based on word count and tag count
    let quality = (state.word_count as f64 / 100.0).min(1.0) * 0.7
        + (state.tags.len() as f64 / 10.0).min(1.0) * 0.3;

    Ok(DocumentState {
        content: state.content,
        stage: "completed".to_string(),
        word_count: state.word_count,
        summary: state.summary,
        tags: state.tags,
        quality_score: Some(quality),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== DashFlow Checkpointing Example ===\n");

    // Part 1: In-memory checkpointing
    println!("📋 Part 1: In-Memory Checkpointing\n");

    let memory_checkpointer = MemoryCheckpointer::new();
    let thread_id = "doc-process-1";

    let mut graph = StateGraph::new();
    graph.add_node_from_fn("parse", |state| Box::pin(parse_document(state)));
    graph.add_node_from_fn("summarize", |state| Box::pin(extract_summary(state)));
    graph.add_node_from_fn("tag", |state| Box::pin(extract_tags(state)));
    graph.add_node_from_fn("quality", |state| Box::pin(calculate_quality(state)));
    graph.add_edge("parse", "summarize");
    graph.add_edge("summarize", "tag");
    graph.add_edge("tag", "quality");
    graph.add_edge("quality", "__end__");
    graph.set_entry_point("parse");

    let app = graph
        .compile()?
        .with_checkpointer(memory_checkpointer)
        .with_thread_id(thread_id);

    let initial_state = DocumentState {
        content: "Artificial intelligence and machine learning are transforming \
                  modern software development. Large language models enable \
                  sophisticated natural language processing applications."
            .to_string(),
        stage: "initial".to_string(),
        word_count: 0,
        summary: None,
        tags: Vec::new(),
        quality_score: None,
    };

    println!("Running workflow with checkpointing...\n");
    let result = app.invoke(initial_state).await?;

    println!("\n✅ Workflow complete!");
    println!("   Stage: {}", result.final_state.stage);
    println!("   Word count: {}", result.final_state.word_count);
    println!(
        "   Summary: {}",
        result.final_state.summary.as_deref().unwrap_or("<no summary>")
    );
    println!("   Tags: {:?}", result.final_state.tags);
    if let Some(score) = result.final_state.quality_score {
        println!("   Quality: {:.2}", score);
    } else {
        println!("   Quality: <none>");
    }
    println!("\n   💾 Checkpoints saved after each node execution");

    // Part 2: File-based checkpointing with resume
    println!("\n\n📋 Part 2: File-Based Checkpointing\n");

    let temp_dir = std::env::temp_dir().join("dashflow_checkpoints_example");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)
            .map_err(|e| dashflow::Error::Generic(format!("Failed to remove temp dir: {}", e)))?;
    }

    let file_checkpointer = FileCheckpointer::new(&temp_dir)?;
    let thread_id_2 = "doc-process-2";

    let mut graph2 = StateGraph::new();
    graph2.add_node_from_fn("parse", |state| Box::pin(parse_document(state)));
    graph2.add_node_from_fn("summarize", |state| Box::pin(extract_summary(state)));
    graph2.add_node_from_fn("tag", |state| Box::pin(extract_tags(state)));
    graph2.add_node_from_fn("quality", |state| Box::pin(calculate_quality(state)));
    graph2.add_node_from_fn("resume", |state| Box::pin(async move { Ok(state) }));
    graph2.add_edge("parse", "summarize");
    graph2.add_edge("summarize", "tag");
    graph2.add_edge("tag", "quality");
    graph2.add_edge("quality", "resume");
    graph2.add_edge("resume", "__end__");
    graph2.set_entry_point("parse");

    let app2 = graph2
        .compile()?
        .with_checkpointer(file_checkpointer)
        .with_thread_id(thread_id_2);

    let initial_state_2 = DocumentState {
        content: "Checkpointing enables fault-tolerant workflows by persisting \
                  state at each step. This allows resuming execution from \
                  the last successful checkpoint after failures."
            .to_string(),
        stage: "initial".to_string(),
        word_count: 0,
        summary: None,
        tags: Vec::new(),
        quality_score: None,
    };

    println!("Running workflow with file-based checkpointing...\n");
    let _result2 = app2.invoke(initial_state_2).await?;

    println!("\n✅ Workflow complete!");
    println!("   Checkpoints saved to: {:?}", temp_dir);

    // Show checkpoint files
    let checkpoint_files: Vec<PathBuf> = std::fs::read_dir(&temp_dir)
        .map_err(|e| dashflow::Error::Generic(format!("Failed to read dir: {}", e)))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    println!("   Checkpoint files: {}", checkpoint_files.len());
    println!("\n   ✨ In a real application, you could:");
    println!("      - Resume execution from checkpoints after failures");
    println!("      - Inspect state for debugging");
    println!("      - Create audit trails");
    println!("      - Implement retry logic");

    // Cleanup
    println!("\n🧹 Cleaning up checkpoint files...");
    std::fs::remove_dir_all(&temp_dir)
        .map_err(|e| dashflow::Error::Generic(format!("Failed to cleanup temp dir: {}", e)))?;

    println!("\n=== Example Complete ===");
    Ok(())
}
