//! Example demonstrating multi-tier checkpointing with layered caching
//!
//! This example shows how to use MultiTierCheckpointer to implement a two-tier
//! checkpointing strategy:
//! - L1 cache (fast): MemoryCheckpointer (simulating Redis)
//! - L2 storage (durable): FileCheckpointer (simulating S3/DynamoDB)
//!
//! # Write Policies
//!
//! - **WriteThrough**: Write to both L1 and L2 simultaneously (default)
//! - **WriteBehind**: Write to L1 immediately, L2 asynchronously
//! - **WriteAround**: Write only to L2, skip L1 cache
//!
//! # Running
//!
//! ```bash
//! cargo run --example multi_tier_checkpointing
//! ```

use dashflow::{
    Checkpointer, FileCheckpointer, MemoryCheckpointer, MergeableState, MultiTierCheckpointer,
    Result, StateGraph, WritePolicy, END,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DataProcessingState {
    items: Vec<String>,
    processed_count: usize,
    current_stage: String,
}

impl MergeableState for DataProcessingState {
    fn merge(&mut self, other: &Self) {
        self.items.extend(other.items.clone());
        self.processed_count = self.processed_count.max(other.processed_count);
        if !other.current_stage.is_empty() {
            if self.current_stage.is_empty() {
                self.current_stage = other.current_stage.clone();
            } else {
                self.current_stage.push('\n');
                self.current_stage.push_str(&other.current_stage);
            }
        }
    }
}

/// Fetch data node - simulates fetching data from external source
async fn fetch_data(mut state: DataProcessingState) -> Result<DataProcessingState> {
    println!("📥 Fetching data...");
    state.items = vec![
        "item1".to_string(),
        "item2".to_string(),
        "item3".to_string(),
    ];
    state.current_stage = "fetched".to_string();
    println!("   Fetched {} items", state.items.len());
    Ok(state)
}

/// Process data node - simulates data processing
async fn process_data(mut state: DataProcessingState) -> Result<DataProcessingState> {
    println!("⚙️  Processing data...");
    state.processed_count = state.items.len();
    state.current_stage = "processed".to_string();
    println!("   Processed {} items", state.processed_count);
    Ok(state)
}

/// Store results node - simulates storing results
async fn store_results(mut state: DataProcessingState) -> Result<DataProcessingState> {
    println!("💾 Storing results...");
    state.current_stage = "stored".to_string();
    println!("   Results stored successfully");
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Multi-Tier Checkpointing Example ===\n");

    // Create temporary directory for file checkpointer (simulating S3/DynamoDB)
    let temp_dir =
        std::env::temp_dir().join(format!("dashflow_multi_tier_{}", uuid::Uuid::new_v4()));

    // L1 cache (fast): Memory checkpointer (simulating Redis)
    let l1_cache = Arc::new(MemoryCheckpointer::<DataProcessingState>::new());

    // L2 storage (durable): File checkpointer (simulating S3/DynamoDB)
    let l2_storage = Arc::new(FileCheckpointer::<DataProcessingState>::new(&temp_dir)?);

    println!("📋 Testing Write-Through Policy");
    println!("   (Write to both L1 and L2 simultaneously)\n");

    // Create multi-tier checkpointer with write-through policy
    let checkpointer = MultiTierCheckpointer::new(
        Arc::<MemoryCheckpointer<DataProcessingState>>::clone(&l1_cache),
        Arc::<FileCheckpointer<DataProcessingState>>::clone(&l2_storage),
    )
        .with_write_policy(WritePolicy::WriteThrough)
        .with_warm_l1_on_read(true);

    // Build the graph
    let mut graph = StateGraph::<DataProcessingState>::new();

    graph.add_node_from_fn("fetch", |state| Box::pin(fetch_data(state)));
    graph.add_node_from_fn("process", |state| Box::pin(process_data(state)));
    graph.add_node_from_fn("store", |state| Box::pin(store_results(state)));

    graph.add_edge("fetch", "process");
    graph.add_edge("process", "store");
    graph.add_edge("store", END);

    graph.set_entry_point("fetch");

    // Compile with multi-tier checkpointer
    let thread_id = "pipeline_1";
    let app = graph
        .compile()?
        .with_checkpointer(checkpointer)
        .with_thread_id(thread_id.to_string());

    // Execute the workflow
    println!("🚀 Executing workflow...\n");
    let initial_state = DataProcessingState {
        items: vec![],
        processed_count: 0,
        current_stage: "initial".to_string(),
    };

    let result = app.invoke(initial_state).await?;
    println!("\n✅ Workflow completed!");
    println!("   Final stage: {}", result.final_state.current_stage);
    println!(
        "   Processed items: {}\n",
        result.final_state.processed_count
    );

    // Demonstrate L1 cache hits
    println!("📊 Checkpointer Statistics:");
    println!("   L1 cache size: {} checkpoints", l1_cache.len());
    println!("   L2 storage size: {} checkpoints", {
        let metadata = l2_storage.list(thread_id).await?;
        metadata.len()
    });

    // Demonstrate cache warming on L2 read
    println!("\n🧪 Testing Cache Warming:");
    println!("   Clearing L1 cache...");
    l1_cache.delete_thread(thread_id).await?;
    println!("   L1 cache size: {} checkpoints", l1_cache.len());

    println!("   Reading from L2 storage...");
    let latest = l2_storage.get_latest(thread_id).await?;
    if let Some(checkpoint) = latest {
        println!("   Retrieved checkpoint: {}", checkpoint.node);
        println!("   Stage: {}", checkpoint.state.current_stage);

        // In write-through mode with cache warming, the L1 cache would be populated
        // on the next read through MultiTierCheckpointer
    }

    // Demonstrate write-around policy
    println!("\n📋 Testing Write-Around Policy");
    println!("   (Write only to L2, bypass L1 cache)\n");

    let l1_cache_2 = Arc::new(MemoryCheckpointer::<DataProcessingState>::new());
    let l2_storage_2 = Arc::new(FileCheckpointer::<DataProcessingState>::new(
        temp_dir.join("write_around"),
    )?);

    let checkpointer_around = MultiTierCheckpointer::new(
        Arc::<MemoryCheckpointer<DataProcessingState>>::clone(&l1_cache_2),
        Arc::<FileCheckpointer<DataProcessingState>>::clone(&l2_storage_2),
    )
    .with_write_policy(WritePolicy::WriteAround);

    // Build a new graph for the second test
    let mut graph2 = StateGraph::<DataProcessingState>::new();
    graph2.add_node_from_fn("fetch", |state| Box::pin(fetch_data(state)));
    graph2.add_node_from_fn("process", |state| Box::pin(process_data(state)));
    graph2.add_node_from_fn("store", |state| Box::pin(store_results(state)));
    graph2.add_edge("fetch", "process");
    graph2.add_edge("process", "store");
    graph2.add_edge("store", END);
    graph2.set_entry_point("fetch");

    let app_around = graph2
        .compile()?
        .with_checkpointer(checkpointer_around)
        .with_thread_id("pipeline_2".to_string());

    let initial_state_2 = DataProcessingState {
        items: vec![],
        processed_count: 0,
        current_stage: "initial".to_string(),
    };

    app_around.invoke(initial_state_2).await?;

    println!(
        "   L1 cache size: {} checkpoints (should be 0)",
        l1_cache_2.len()
    );
    println!("   L2 storage has data: {}", {
        let metadata = l2_storage_2.list("pipeline_2").await?;
        !metadata.is_empty()
    });

    // Cleanup
    println!("\n🧹 Cleanup...");
    if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
        eprintln!("Warning: Failed to cleanup temp directory: {}", e);
    } else {
        println!("   Temporary files removed\n");
    }

    println!("=== Example Complete ===\n");

    println!("💡 Key Takeaways:");
    println!("   - WriteThrough: Ensures L1 and L2 are always in sync");
    println!("   - WriteBehind: Fast writes, but risk of data loss");
    println!("   - WriteAround: Good for large states that don't fit in L1");
    println!("   - Cache warming: Populate L1 automatically on L2 reads");
    println!("   - Multi-tier: Best of both worlds - fast + durable\n");

    Ok(())
}
