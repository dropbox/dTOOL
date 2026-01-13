//! PostgreSQL checkpointing example
//!
//! Demonstrates production-ready durable state persistence with PostgreSQL:
//! - Connection and schema initialization
//! - State persistence across process restarts
//! - Resume from checkpoint
//! - List and inspect checkpoints
//! - Checkpoint history and audit trails
//!
//! # Prerequisites
//!
//! 1. Start PostgreSQL with Docker Compose:
//!    ```bash
//!    docker-compose -f docker-compose.postgres.yml up -d
//!    ```
//!
//! 2. Run this example:
//!    ```bash
//!    cargo run --example postgres_checkpointing
//!    ```
//!
//! 3. Stop PostgreSQL when done:
//!    ```bash
//!    docker-compose -f docker-compose.postgres.yml down
//!    ```

use dashflow::{Checkpointer, MergeableState, Result, StateGraph};
use dashflow_postgres_checkpointer::PostgresCheckpointer;
use serde::{Deserialize, Serialize};

/// Order processing state
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OrderState {
    /// Order ID
    order_id: String,
    /// Current processing stage
    stage: String,
    /// Customer name
    customer: String,
    /// Order total in dollars
    total: f64,
    /// Payment confirmed
    payment_confirmed: bool,
    /// Inventory reserved
    inventory_reserved: bool,
    /// Shipped
    shipped: bool,
    /// Processing steps completed
    steps_completed: Vec<String>,
}

impl MergeableState for OrderState {
    fn merge(&mut self, other: &Self) {
        if !other.order_id.is_empty() {
            if self.order_id.is_empty() {
                self.order_id = other.order_id.clone();
            } else {
                self.order_id.push('\n');
                self.order_id.push_str(&other.order_id);
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
        if !other.customer.is_empty() {
            if self.customer.is_empty() {
                self.customer = other.customer.clone();
            } else {
                self.customer.push('\n');
                self.customer.push_str(&other.customer);
            }
        }
        self.total = self.total.max(other.total);
        self.payment_confirmed = self.payment_confirmed || other.payment_confirmed;
        self.inventory_reserved = self.inventory_reserved || other.inventory_reserved;
        self.shipped = self.shipped || other.shipped;
        self.steps_completed.extend(other.steps_completed.clone());
    }
}

/// Validate order
async fn validate_order(state: OrderState) -> Result<OrderState> {
    println!("✓ Validating order {}...", state.order_id);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let mut steps = state.steps_completed;
    steps.push("validated".to_string());

    Ok(OrderState {
        order_id: state.order_id,
        stage: "validated".to_string(),
        customer: state.customer,
        total: state.total,
        payment_confirmed: state.payment_confirmed,
        inventory_reserved: state.inventory_reserved,
        shipped: state.shipped,
        steps_completed: steps,
    })
}

/// Process payment
async fn process_payment(state: OrderState) -> Result<OrderState> {
    println!("💳 Processing payment for order {}...", state.order_id);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let mut steps = state.steps_completed;
    steps.push("payment_processed".to_string());

    Ok(OrderState {
        order_id: state.order_id,
        stage: "payment_confirmed".to_string(),
        customer: state.customer,
        total: state.total,
        payment_confirmed: true,
        inventory_reserved: state.inventory_reserved,
        shipped: state.shipped,
        steps_completed: steps,
    })
}

/// Reserve inventory
async fn reserve_inventory(state: OrderState) -> Result<OrderState> {
    println!("📦 Reserving inventory for order {}...", state.order_id);
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;

    let mut steps = state.steps_completed;
    steps.push("inventory_reserved".to_string());

    Ok(OrderState {
        order_id: state.order_id,
        stage: "inventory_reserved".to_string(),
        customer: state.customer,
        total: state.total,
        payment_confirmed: state.payment_confirmed,
        inventory_reserved: true,
        shipped: state.shipped,
        steps_completed: steps,
    })
}

/// Ship order
async fn ship_order(state: OrderState) -> Result<OrderState> {
    println!("🚚 Shipping order {}...", state.order_id);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let mut steps = state.steps_completed;
    steps.push("shipped".to_string());

    Ok(OrderState {
        order_id: state.order_id,
        stage: "shipped".to_string(),
        customer: state.customer,
        total: state.total,
        payment_confirmed: state.payment_confirmed,
        inventory_reserved: state.inventory_reserved,
        shipped: true,
        steps_completed: steps,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== DashFlow PostgreSQL Checkpointing Example ===\n");

    // Connection string for Docker Compose PostgreSQL
    let connection_string =
        "host=localhost port=5432 user=dashflow password=dashflow_dev dbname=dashflow_test";

    println!("📡 Connecting to PostgreSQL and initializing schema...");
    let checkpointer_for_graph =
        PostgresCheckpointer::new(connection_string)
            .await
            .map_err(|e| {
                dashflow::Error::Generic(format!(
                    "Failed to connect to PostgreSQL: {}. Is PostgreSQL running? \
                 Try: docker-compose -f docker-compose.postgres.yml up -d",
                    e
                ))
            })?;

    // Create a second connection for direct queries (with_checkpointer consumes the checkpointer)
    let checkpointer_for_queries: PostgresCheckpointer<OrderState> =
        PostgresCheckpointer::new(connection_string)
            .await
            .map_err(|e| {
                dashflow::Error::Generic(format!("Failed to create second connection: {}", e))
            })?;

    println!("✓ Connected to PostgreSQL and schema initialized\n");

    // Part 1: Run workflow with checkpointing
    println!("📋 Part 1: Running Order Processing Workflow\n");

    let thread_id = "order-2025-001";

    let mut graph = StateGraph::new();
    graph.add_node_from_fn("validate", |state| Box::pin(validate_order(state)));
    graph.add_node_from_fn("payment", |state| Box::pin(process_payment(state)));
    graph.add_node_from_fn("inventory", |state| Box::pin(reserve_inventory(state)));
    graph.add_node_from_fn("ship", |state| Box::pin(ship_order(state)));
    graph.add_edge("validate", "payment");
    graph.add_edge("payment", "inventory");
    graph.add_edge("inventory", "ship");
    graph.add_edge("ship", "__end__");
    graph.set_entry_point("validate");

    let app = graph
        .compile()?
        .with_checkpointer(checkpointer_for_graph)
        .with_thread_id(thread_id);

    let initial_state = OrderState {
        order_id: "ORD-2025-12345".to_string(),
        stage: "created".to_string(),
        customer: "Alice Johnson".to_string(),
        total: 149.99,
        payment_confirmed: false,
        inventory_reserved: false,
        shipped: false,
        steps_completed: Vec::new(),
    };

    println!("Processing order: {}", initial_state.order_id);
    println!("Customer: {}", initial_state.customer);
    println!("Total: ${:.2}\n", initial_state.total);

    let result = app.invoke(initial_state).await?;

    println!("\n✅ Order processing complete!");
    println!("   Order ID: {}", result.final_state.order_id);
    println!("   Stage: {}", result.final_state.stage);
    println!(
        "   Payment Confirmed: {}",
        result.final_state.payment_confirmed
    );
    println!(
        "   Inventory Reserved: {}",
        result.final_state.inventory_reserved
    );
    println!("   Shipped: {}", result.final_state.shipped);
    println!("   Steps: {:?}", result.final_state.steps_completed);
    println!("\n   💾 Checkpoints saved to PostgreSQL after each node");

    // Part 2: List checkpoints
    println!("\n\n📋 Part 2: Listing Checkpoints\n");

    println!("Retrieving checkpoint history for thread: {}", thread_id);
    let checkpoints = checkpointer_for_queries
        .list(thread_id)
        .await
        .map_err(|e| dashflow::Error::Generic(format!("Failed to list: {}", e)))?;

    println!("Found {} checkpoints:\n", checkpoints.len());
    for (i, checkpoint) in checkpoints.iter().enumerate() {
        println!("  {}. Checkpoint ID: {}", i + 1, checkpoint.id);
        println!("     Node: {}", checkpoint.node);
        println!("     Timestamp: {:?}", checkpoint.timestamp);
        if let Some(parent_id) = &checkpoint.parent_id {
            println!("     Parent ID: {}", parent_id);
        }
        println!();
    }

    // Part 3: Load and inspect a specific checkpoint
    println!("📋 Part 3: Loading Specific Checkpoint\n");

    if let Some(checkpoint_meta) = checkpoints.first() {
        println!("Loading latest checkpoint: {}", checkpoint_meta.id);
        let checkpoint = checkpointer_for_queries
            .load(&checkpoint_meta.id)
            .await
            .map_err(|e| dashflow::Error::Generic(format!("Failed to load: {}", e)))?;

        if let Some(cp) = checkpoint {
            println!("✓ Checkpoint loaded successfully");
            println!("   Thread ID: {}", cp.thread_id);
            println!("   Node: {}", cp.node);
            println!("   Stage: {}", cp.state.stage);
            println!("   Steps completed: {:?}", cp.state.steps_completed);
            println!("\n   ✨ In production, you could:");
            println!("      - Resume from this checkpoint after failures");
            println!("      - Replay execution from any point");
            println!("      - Debug state transitions");
            println!("      - Create audit trails");
        }
    }

    // Part 4: Get latest checkpoint for thread
    println!("\n\n📋 Part 4: Get Latest Checkpoint\n");

    println!("Retrieving latest checkpoint for thread: {}", thread_id);
    let latest = checkpointer_for_queries
        .get_latest(thread_id)
        .await
        .map_err(|e| dashflow::Error::Generic(format!("Failed to get latest: {}", e)))?;

    if let Some(cp) = latest {
        println!("✓ Latest checkpoint found");
        println!("   Checkpoint ID: {}", cp.id);
        println!("   Node: {}", cp.node);
        println!("   Stage: {}", cp.state.stage);
        println!("   Shipped: {}", cp.state.shipped);
    }

    // Part 5: Demonstrate resuming from checkpoint
    println!("\n\n📋 Part 5: Resume from Checkpoint (Simulated)\n");

    println!("In a real scenario, if the process crashed after 'payment' node:");
    println!("1. Load the latest checkpoint");
    println!("2. The graph would resume from the next node ('inventory')");
    println!("3. No need to reprocess payment or validation");
    println!("\nThis enables fault-tolerant workflows with exactly-once semantics.");

    // Part 6: Cleanup demonstration
    println!("\n\n📋 Part 6: Checkpoint Management\n");

    println!("PostgreSQL checkpointer supports:");
    println!("  • delete(checkpoint_id) - Delete specific checkpoint");
    println!("  • delete_thread(thread_id) - Delete all checkpoints for thread");
    println!("  • Automatic parent_id tracking for checkpoint history");
    println!("  • JSONB metadata for custom indexing");
    println!(
        "\nCheckpoints for thread '{}' remain in database for inspection.",
        thread_id
    );
    println!("Use `delete_thread()` to clean up when ready.");

    println!("\n=== Example Complete ===");
    println!("\n💡 Tips:");
    println!("   - Use separate databases for dev/staging/prod");
    println!("   - Configure connection pooling for production");
    println!("   - Add custom metadata for business logic");
    println!("   - Implement retention policies for old checkpoints");

    Ok(())
}
