//! Basic PostgreSQL checkpointing example
//!
//! This example demonstrates basic usage of PostgresCheckpointer for persistent state storage.
//! For a more comprehensive example with DashFlow integration, see:
//! `crates/dashflow/examples/postgres_checkpointing.rs`
//!
//! # Prerequisites
//!
//! Start PostgreSQL with Docker Compose:
//! ```bash
//! docker-compose -f docker-compose.postgres.yml up -d
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example basic_postgres_checkpointing
//! ```
//!
//! # Cleanup
//!
//! ```bash
//! docker-compose -f docker-compose.postgres.yml down -v
//! ```

use dashflow::{Checkpointer, StateGraph};
use dashflow_postgres_checkpointer::PostgresCheckpointer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CustomerServiceState {
    customer_id: String,
    issue: String,
    status: String,
    resolution: Option<String>,
    notes: Vec<String>,
}

impl dashflow::MergeableState for CustomerServiceState {
    fn merge(&mut self, other: &Self) {
        // Merge notes from parallel branches
        self.notes.extend(other.notes.clone());
        // Prefer non-None resolution
        if self.resolution.is_none() && other.resolution.is_some() {
            self.resolution = other.resolution.clone();
        }
    }
}

async fn triage_node(mut state: CustomerServiceState) -> dashflow::Result<CustomerServiceState> {
    println!("🔍 Triaging issue: {}", state.issue);
    state.status = "triaged".to_string();
    state
        .notes
        .push("Issue categorized as: general inquiry".to_string());
    Ok(state)
}

async fn investigate_node(
    mut state: CustomerServiceState,
) -> dashflow::Result<CustomerServiceState> {
    println!("🔎 Investigating issue...");
    state.status = "investigating".to_string();
    state
        .notes
        .push("Checking knowledge base for similar issues".to_string());
    Ok(state)
}

async fn resolve_node(mut state: CustomerServiceState) -> dashflow::Result<CustomerServiceState> {
    println!("✅ Resolving issue...");
    state.status = "resolved".to_string();
    state.resolution = Some("Issue resolved through standard documentation".to_string());
    state
        .notes
        .push("Customer satisfied with resolution".to_string());
    Ok(state)
}

#[tokio::main]
async fn main() -> dashflow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("📊 PostgreSQL Checkpointing Example\n");

    // Connect to PostgreSQL
    let connection_string = std::env::var("POSTGRES_CONNECTION_STRING").unwrap_or_else(|_| {
        "host=localhost port=5432 user=postgres password=postgres dbname=dashflow".to_string()
    });

    println!("🔌 Connecting to PostgreSQL...");
    let checkpointer = PostgresCheckpointer::<CustomerServiceState>::new(&connection_string)
        .await
        ?;
    println!("✅ Connected to PostgreSQL\n");

    // Build the graph
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("triage", |state| Box::pin(triage_node(state)));
    graph.add_node_from_fn("investigate", |state| Box::pin(investigate_node(state)));
    graph.add_node_from_fn("resolve", |state| Box::pin(resolve_node(state)));

    graph.add_edge("triage", "investigate");
    graph.add_edge("investigate", "resolve");
    graph.add_edge("resolve", "__end__");

    graph.set_entry_point("triage");

    // Compile with PostgreSQL checkpointer
    let app = graph.compile()?.with_checkpointer(checkpointer);

    // Example 1: Run a workflow with checkpointing
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Example 1: Run workflow with checkpointing");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let initial_state = CustomerServiceState {
        customer_id: "CUST-12345".to_string(),
        issue: "Unable to login to account".to_string(),
        status: "new".to_string(),
        resolution: None,
        notes: Vec::new(),
    };

    let thread_id = "session-001";
    let execution_result = app
        .with_thread_id(thread_id)
        .invoke(initial_state.clone())
        .await?;
    let result = execution_result.final_state;

    println!("\n📝 Final state:");
    println!("  Customer: {}", result.customer_id);
    println!("  Status: {}", result.status);
    println!(
        "  Resolution: {}",
        result.resolution.unwrap_or_else(|| "None".to_string())
    );
    println!("  Notes:");
    for note in &result.notes {
        println!("    - {}", note);
    }

    // Example 2: List checkpoints for the thread
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Example 2: List checkpoints for thread");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Reconnect to get a separate checkpointer instance for querying
    let checkpointer2 = PostgresCheckpointer::<CustomerServiceState>::new(&connection_string)
        .await?;

    let checkpoints = checkpointer2.list(thread_id).await?;
    println!(
        "📋 Found {} checkpoints for thread '{}':",
        checkpoints.len(),
        thread_id
    );
    for (i, cp) in checkpoints.iter().enumerate() {
        println!("  {}. Checkpoint {} (node: {})", i + 1, cp.id, cp.node);
    }

    // Example 3: Load latest checkpoint and inspect state
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Example 3: Load and inspect latest checkpoint");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let latest = checkpointer2.get_latest(thread_id).await?;
    if let Some(checkpoint) = latest {
        println!("📦 Latest checkpoint:");
        println!("  ID: {}", checkpoint.id);
        println!("  Node: {}", checkpoint.node);
        println!("  Timestamp: {:?}", checkpoint.timestamp);
        println!("  State status: {}", checkpoint.state.status);
    }

    // Example 4: Run another workflow with different thread
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Example 4: Run another workflow");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Create a new workflow for a different customer
    let new_state = CustomerServiceState {
        customer_id: "CUST-67890".to_string(),
        issue: "Billing question".to_string(),
        status: "new".to_string(),
        resolution: None,
        notes: Vec::new(),
    };

    let thread_id_2 = "session-002";
    println!("💾 Running new workflow (thread: {})...", thread_id_2);

    // Build a new graph for the second execution
    let mut graph2 = StateGraph::new();
    graph2.add_node_from_fn("triage", |state| Box::pin(triage_node(state)));
    graph2.add_node_from_fn("investigate", |state| Box::pin(investigate_node(state)));
    graph2.add_node_from_fn("resolve", |state| Box::pin(resolve_node(state)));
    graph2.add_edge("triage", "investigate");
    graph2.add_edge("investigate", "resolve");
    graph2.add_edge("resolve", "__end__");
    graph2.set_entry_point("triage");

    let checkpointer3 = PostgresCheckpointer::<CustomerServiceState>::new(&connection_string)
        .await?;
    let app2 = graph2.compile()?.with_checkpointer(checkpointer3);

    let execution_result2 = app2.with_thread_id(thread_id_2).invoke(new_state).await?;
    let result2 = execution_result2.final_state;
    println!("✅ Completed: Status = {}", result2.status);

    // Example 5: Cleanup - delete old thread checkpoints
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Example 5: Cleanup old checkpoints");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("🗑️  Deleting checkpoints for thread '{}'...", thread_id);
    checkpointer2.delete_thread(thread_id).await?;

    let remaining = checkpointer2.list(thread_id).await?;
    println!("✅ Deleted. Remaining checkpoints: {}", remaining.len());

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✨ Example complete!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("💡 Key features demonstrated:");
    println!("  • Persistent state storage in PostgreSQL");
    println!("  • Automatic checkpoint creation at each node");
    println!("  • Thread-based isolation (multiple workflows)");
    println!("  • Checkpoint listing and inspection");
    println!("  • Crash recovery via checkpoint resumption");
    println!("  • Checkpoint cleanup and management");

    Ok(())
}
