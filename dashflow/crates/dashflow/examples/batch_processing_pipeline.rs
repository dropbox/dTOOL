//! Batch Processing Pipeline Example
//!
//! This example demonstrates a production-quality batch processing system using DashFlow:
//! - Batch input handling with multiple items
//! - Parallel processing of batch items
//! - Checkpointing for resume capability
//! - Error handling with retry logic
//! - Results aggregation across all items
//!
//! Architecture:
//! - Load Batch → Validate Items → Process in Parallel → Aggregate Results → Complete
//! - Checkpoint after each stage for resume capability
//! - Retry failed items up to 3 times
//! - Track success/failure metrics
//!
//! Run: cargo run --example batch_processing_pipeline

use dashflow::{FileCheckpointer, MergeableState, Result, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BatchItem {
    id: String,
    data: String,
    status: String, // "pending", "processing", "success", "failed"
    retry_count: u32,
    error_message: Option<String>,
    processed_result: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct BatchProcessingState {
    batch_id: String,
    items: Vec<BatchItem>,
    current_stage: String,
    total_items: usize,
    processed_items: usize,
    successful_items: usize,
    failed_items: usize,
    aggregated_results: Vec<String>,
    processing_errors: Vec<String>,
    max_retries: u32,
}

impl MergeableState for BatchProcessingState {
    fn merge(&mut self, other: &Self) {
        if !other.batch_id.is_empty() {
            if self.batch_id.is_empty() {
                self.batch_id = other.batch_id.clone();
            } else {
                self.batch_id.push('\n');
                self.batch_id.push_str(&other.batch_id);
            }
        }
        self.items.extend(other.items.clone());
        if !other.current_stage.is_empty() {
            if self.current_stage.is_empty() {
                self.current_stage = other.current_stage.clone();
            } else {
                self.current_stage.push('\n');
                self.current_stage.push_str(&other.current_stage);
            }
        }
        self.total_items = self.total_items.max(other.total_items);
        self.processed_items = self.processed_items.max(other.processed_items);
        self.successful_items = self.successful_items.max(other.successful_items);
        self.failed_items = self.failed_items.max(other.failed_items);
        self.aggregated_results
            .extend(other.aggregated_results.clone());
        self.processing_errors
            .extend(other.processing_errors.clone());
        self.max_retries = self.max_retries.max(other.max_retries);
    }
}

impl BatchProcessingState {
    fn new(batch_id: impl Into<String>, items: Vec<BatchItem>) -> Self {
        let total = items.len();
        Self {
            batch_id: batch_id.into(),
            items,
            current_stage: "initialized".to_string(),
            total_items: total,
            processed_items: 0,
            successful_items: 0,
            failed_items: 0,
            aggregated_results: Vec::new(),
            processing_errors: Vec::new(),
            max_retries: 3,
        }
    }
}

fn build_batch_processing_graph() -> StateGraph<BatchProcessingState> {
    let mut graph = StateGraph::new();

    // Node 1: Load and validate batch
    graph.add_node_from_fn("load_batch", |mut state: BatchProcessingState| {
        Box::pin(async move {
            println!("\n📦 Load Batch: Processing batch {}", state.batch_id);
            println!("   Total items: {}", state.total_items);
            state.current_stage = "loaded".to_string();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            Ok(state)
        })
    });

    // Node 2: Validate items
    graph.add_node_from_fn("validate_items", |mut state: BatchProcessingState| {
        Box::pin(async move {
            println!("\n✓ Validate: Checking {} items", state.items.len());
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

            let mut valid_count = 0;
            for item in &mut state.items {
                // Simulate validation - items with "invalid" in data fail
                if item.data.contains("invalid") {
                    item.status = "failed".to_string();
                    item.error_message = Some("Invalid data format".to_string());
                    state.failed_items += 1;
                    state
                        .processing_errors
                        .push(format!("Item {}: Invalid data format", item.id));
                } else {
                    item.status = "pending".to_string();
                    valid_count += 1;
                }
            }

            println!(
                "   Valid: {}, Invalid: {}",
                valid_count,
                state.items.len() - valid_count
            );
            state.current_stage = "validated".to_string();
            Ok(state)
        })
    });

    // Node 3: Process items in parallel (simulated)
    graph.add_node_from_fn("process_items", |mut state: BatchProcessingState| {
        Box::pin(async move {
            println!("\n⚙️  Process: Starting parallel processing");
            state.current_stage = "processing".to_string();

            // In a real implementation, you would spawn parallel tasks here
            // For this example, we'll simulate parallel processing sequentially

            for item in &mut state.items {
                if item.status == "pending"
                    || (item.status == "failed" && item.retry_count < state.max_retries)
                {
                    item.status = "processing".to_string();

                    // Simulate processing time
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                    // Simulate processing - items with "error" in data fail randomly
                    if item.data.contains("error") && item.retry_count == 0 {
                        // First attempt fails
                        item.status = "failed".to_string();
                        item.retry_count += 1;
                        item.error_message = Some("Processing error - will retry".to_string());
                        println!(
                            "   ❌ Item {} failed (attempt {}/{})",
                            item.id, item.retry_count, state.max_retries
                        );
                    } else {
                        // Process successfully
                        item.status = "success".to_string();
                        item.processed_result = Some(format!(
                            "Processed: {} (length: {})",
                            item.data,
                            item.data.len()
                        ));
                        state.processed_items += 1;
                        state.successful_items += 1;
                        println!("   ✅ Item {} processed successfully", item.id);
                    }
                }
            }

            println!(
                "   Processed: {}/{} items",
                state.processed_items, state.total_items
            );
            Ok(state)
        })
    });

    // Node 4: Retry failed items
    graph.add_node_from_fn("retry_failed", |mut state: BatchProcessingState| {
        Box::pin(async move {
            let failed_retryable: Vec<String> = state
                .items
                .iter()
                .filter(|item| item.status == "failed" && item.retry_count < state.max_retries)
                .map(|item| item.id.clone())
                .collect();

            if !failed_retryable.is_empty() {
                println!(
                    "\n🔄 Retry: Processing {} failed items",
                    failed_retryable.len()
                );

                for item in &mut state.items {
                    if item.status == "failed" && item.retry_count < state.max_retries {
                        item.retry_count += 1;
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                        // Retry succeeds (in real system, would re-run processing logic)
                        item.status = "success".to_string();
                        item.error_message = None;
                        item.processed_result = Some(format!(
                            "Processed: {} (retry {}, length: {})",
                            item.data,
                            item.retry_count,
                            item.data.len()
                        ));
                        state.processed_items += 1;
                        state.successful_items += 1;
                        println!(
                            "   ✅ Item {} succeeded on retry {}",
                            item.id, item.retry_count
                        );
                    }
                }
            } else {
                println!("\n🔄 Retry: No items need retry");
            }

            state.current_stage = "retried".to_string();
            Ok(state)
        })
    });

    // Node 5: Aggregate results
    graph.add_node_from_fn("aggregate_results", |mut state: BatchProcessingState| {
        Box::pin(async move {
            println!("\n📊 Aggregate: Collecting results");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Collect successful results
            for item in &state.items {
                if item.status == "success" {
                    if let Some(ref result) = item.processed_result {
                        state.aggregated_results.push(result.clone());
                    }
                } else if item.status == "failed" {
                    state.failed_items += 1;
                    if let Some(ref error) = item.error_message {
                        state
                            .processing_errors
                            .push(format!("Item {}: {}", item.id, error));
                    }
                }
            }

            println!("   Total results: {}", state.aggregated_results.len());
            state.current_stage = "aggregated".to_string();
            Ok(state)
        })
    });

    // Node 6: Complete and report
    graph.add_node_from_fn("complete", |mut state: BatchProcessingState| {
        Box::pin(async move {
            println!("\n✨ Complete: Batch {} finished", state.batch_id);
            println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("   Total items:      {}", state.total_items);
            println!("   Successful:       {}", state.successful_items);
            println!("   Failed:           {}", state.failed_items);
            println!(
                "   Success rate:     {:.1}%",
                (state.successful_items as f64 / state.total_items as f64) * 100.0
            );

            if !state.processing_errors.is_empty() {
                println!("\n   Errors:");
                for error in &state.processing_errors {
                    println!("   - {}", error);
                }
            }

            state.current_stage = "completed".to_string();
            Ok(state)
        })
    });

    // Build the pipeline
    graph.set_entry_point("load_batch");
    graph.add_edge("load_batch", "validate_items");
    graph.add_edge("validate_items", "process_items");
    graph.add_edge("process_items", "retry_failed");
    graph.add_edge("retry_failed", "aggregate_results");
    graph.add_edge("aggregate_results", "complete");
    graph.add_edge("complete", END);

    graph
}

async fn run_batch_scenario(
    scenario_name: &str,
    batch_id: &str,
    items: Vec<BatchItem>,
    use_checkpointing: bool,
) -> Result<()> {
    println!("\n═══════════════════════════════════════════════════════");
    println!(" SCENARIO: {}", scenario_name);
    println!("═══════════════════════════════════════════════════════\n");

    let state = BatchProcessingState::new(batch_id, items);

    if use_checkpointing {
        // With checkpointing
        let checkpoint_dir = PathBuf::from("target/batch_checkpoints");
        std::fs::create_dir_all(&checkpoint_dir).map_err(|e| {
            dashflow::Error::Generic(format!("Failed to create checkpoint dir: {}", e))
        })?;
        let checkpointer = FileCheckpointer::new(&checkpoint_dir)?;

        let graph = build_batch_processing_graph();
        let runnable = graph
            .compile()?
            .with_checkpointer(checkpointer)
            .with_thread_id(batch_id);

        println!("📌 Checkpointing enabled (can resume from failures)");
        let result = runnable.invoke(state).await?;
        let _final_state = result.final_state;
    } else {
        // Without checkpointing
        let graph = build_batch_processing_graph();
        let runnable = graph.compile()?;
        let result = runnable.invoke(state).await?;
        let _final_state = result.final_state;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Batch Processing Pipeline Example");
    println!("==================================\n");
    println!("This example demonstrates:");
    println!("- Batch input handling");
    println!("- Parallel processing simulation");
    println!("- Checkpointing for resume");
    println!("- Error handling with retry");
    println!("- Results aggregation\n");

    // Scenario 1: Simple batch (all succeed)
    let items1 = vec![
        BatchItem {
            id: "item-001".to_string(),
            data: "Process customer order #1234".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-002".to_string(),
            data: "Process customer order #1235".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-003".to_string(),
            data: "Process customer order #1236".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
    ];

    run_batch_scenario("Simple Batch (All Succeed)", "batch-001", items1, false).await?;

    // Scenario 2: Batch with validation failures
    let items2 = vec![
        BatchItem {
            id: "item-004".to_string(),
            data: "Process order #1237".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-005".to_string(),
            data: "invalid data format".to_string(), // Will fail validation
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-006".to_string(),
            data: "Process order #1238".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
    ];

    run_batch_scenario("Batch with Validation Failures", "batch-002", items2, false).await?;

    // Scenario 3: Batch with processing errors and retry
    let items3 = vec![
        BatchItem {
            id: "item-007".to_string(),
            data: "Process order #1239".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-008".to_string(),
            data: "Process order with error #1240".to_string(), // Will fail first attempt
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-009".to_string(),
            data: "Process order #1241".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
        BatchItem {
            id: "item-010".to_string(),
            data: "Process order with error #1242".to_string(), // Will fail first attempt
            status: "pending".to_string(),
            retry_count: 0,
            error_message: None,
            processed_result: None,
        },
    ];

    run_batch_scenario(
        "Batch with Retry Logic (With Checkpointing)",
        "batch-003",
        items3,
        true,
    )
    .await?;

    println!("\n═══════════════════════════════════════════════════════");
    println!(" All scenarios completed successfully!");
    println!("═══════════════════════════════════════════════════════\n");

    Ok(())
}
