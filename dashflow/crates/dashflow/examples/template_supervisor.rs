//! Supervisor Template Example
//!
//! This example demonstrates the Supervisor graph template pattern:
//! - A supervisor node coordinates multiple worker agents
//! - Workers execute specialized tasks
//! - Supervisor decides which worker to call next based on state
//! - Pattern continues until supervisor routes to END
//!
//! Use case: Customer support system with specialized agents
//! - Supervisor analyzes customer query and routes to appropriate specialist
//! - Specialists handle their domain (billing, technical, sales)
//! - Supervisor reviews and decides if more work is needed
//!
//! Run: cargo run --example template_supervisor

use dashflow::templates::GraphTemplate;
use dashflow::MergeableState;
use dashflow::END;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CustomerSupportState {
    customer_query: String,
    query_type: String,
    next_action: String,
    interactions: Vec<String>,
    billing_info: String,
    technical_info: String,
    sales_info: String,
    resolution: String,
}

impl MergeableState for CustomerSupportState {
    fn merge(&mut self, other: &Self) {
        if !other.customer_query.is_empty() {
            if self.customer_query.is_empty() {
                self.customer_query = other.customer_query.clone();
            } else {
                self.customer_query.push('\n');
                self.customer_query.push_str(&other.customer_query);
            }
        }
        if !other.query_type.is_empty() {
            if self.query_type.is_empty() {
                self.query_type = other.query_type.clone();
            } else {
                self.query_type.push('\n');
                self.query_type.push_str(&other.query_type);
            }
        }
        if !other.next_action.is_empty() {
            if self.next_action.is_empty() {
                self.next_action = other.next_action.clone();
            } else {
                self.next_action.push('\n');
                self.next_action.push_str(&other.next_action);
            }
        }
        self.interactions.extend(other.interactions.clone());
        if !other.billing_info.is_empty() {
            if self.billing_info.is_empty() {
                self.billing_info = other.billing_info.clone();
            } else {
                self.billing_info.push('\n');
                self.billing_info.push_str(&other.billing_info);
            }
        }
        if !other.technical_info.is_empty() {
            if self.technical_info.is_empty() {
                self.technical_info = other.technical_info.clone();
            } else {
                self.technical_info.push('\n');
                self.technical_info.push_str(&other.technical_info);
            }
        }
        if !other.sales_info.is_empty() {
            if self.sales_info.is_empty() {
                self.sales_info = other.sales_info.clone();
            } else {
                self.sales_info.push('\n');
                self.sales_info.push_str(&other.sales_info);
            }
        }
        if !other.resolution.is_empty() {
            if self.resolution.is_empty() {
                self.resolution = other.resolution.clone();
            } else {
                self.resolution.push('\n');
                self.resolution.push_str(&other.resolution);
            }
        }
    }
}

impl CustomerSupportState {
    fn new(query: impl Into<String>) -> Self {
        Self {
            customer_query: query.into(),
            query_type: String::new(),
            next_action: String::new(),
            interactions: Vec::new(),
            billing_info: String::new(),
            technical_info: String::new(),
            sales_info: String::new(),
            resolution: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Customer Support System - Supervisor Pattern\n");

    // Build graph using Supervisor template
    let graph = GraphTemplate::supervisor()
        .with_supervisor_node_fn("supervisor", |mut state: CustomerSupportState| {
            Box::pin(async move {
                state
                    .interactions
                    .push("🎯 Supervisor: Analyzing request...".to_string());
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                // Simulate supervisor decision making
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                // Initial routing based on query
                if state.query_type.is_empty() {
                    // Classify the query
                    if state.customer_query.to_lowercase().contains("bill")
                        || state.customer_query.to_lowercase().contains("charge")
                    {
                        state.query_type = "billing".to_string();
                        state.next_action = "billing_agent".to_string();
                    } else if state.customer_query.to_lowercase().contains("error")
                        || state.customer_query.to_lowercase().contains("bug")
                    {
                        state.query_type = "technical".to_string();
                        state.next_action = "technical_agent".to_string();
                    } else if state.customer_query.to_lowercase().contains("upgrade")
                        || state.customer_query.to_lowercase().contains("feature")
                    {
                        state.query_type = "sales".to_string();
                        state.next_action = "sales_agent".to_string();
                    } else {
                        state.query_type = "general".to_string();
                        state.next_action = "technical_agent".to_string(); // Default
                    }
                } else {
                    // After worker execution, check if we need more info
                    let needs_billing =
                        state.query_type == "billing" && state.billing_info.is_empty();
                    let needs_technical =
                        state.query_type == "technical" && state.technical_info.is_empty();
                    let needs_sales = state.query_type == "sales" && state.sales_info.is_empty();

                    if needs_billing {
                        state.next_action = "billing_agent".to_string();
                    } else if needs_technical {
                        state.next_action = "technical_agent".to_string();
                    } else if needs_sales {
                        state.next_action = "sales_agent".to_string();
                    } else {
                        // All necessary information gathered
                        state.resolution = format!(
                            "Query resolved. Type: {}. {} interactions completed.",
                            state.query_type,
                            state.interactions.len()
                        );
                        state.next_action = END.to_string();
                    }
                }

                let action_str = if state.next_action == END {
                    "✅ COMPLETE".to_string()
                } else {
                    format!("→ {}", state.next_action)
                };
                state
                    .interactions
                    .push(format!("🎯 Supervisor: {}", action_str));
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                Ok(state)
            })
        })
        .with_worker_fn("billing_agent", |mut state: CustomerSupportState| {
            Box::pin(async move {
                state
                    .interactions
                    .push("💰 Billing Agent: Checking account...".to_string());
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

                state.billing_info = format!(
                    "Account status: Active. Last charge: $29.99 on 2025-10-15. Query: '{}'",
                    state.customer_query
                );

                state
                    .interactions
                    .push(format!("💰 Billing Agent: {}", state.billing_info));
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                Ok(state)
            })
        })
        .with_worker_fn("technical_agent", |mut state: CustomerSupportState| {
            Box::pin(async move {
                state
                    .interactions
                    .push("🔧 Technical Agent: Investigating issue...".to_string());
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                state.technical_info = format!(
                    "System check complete. No errors found. Query: '{}' - Logs reviewed.",
                    state.customer_query
                );

                state
                    .interactions
                    .push(format!("🔧 Technical Agent: {}", state.technical_info));
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                Ok(state)
            })
        })
        .with_worker_fn("sales_agent", |mut state: CustomerSupportState| {
            Box::pin(async move {
                state
                    .interactions
                    .push("🎁 Sales Agent: Checking available options...".to_string());
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;

                state.sales_info = format!(
                    "Premium plan available: $49.99/month. Includes {} features. Query: '{}'",
                    "advanced", state.customer_query
                );

                state
                    .interactions
                    .push(format!("🎁 Sales Agent: {}", state.sales_info));
                if let Some(last) = state.interactions.last() {
                    println!("{last}");
                }

                Ok(state)
            })
        })
        .with_router(|state| state.next_action.clone())
        .build()?;

    // Test Case 1: Billing query
    println!("\n=== Test Case 1: Billing Query ===\n");
    let compiled = graph.compile()?;
    let state1 = CustomerSupportState::new("Why was I charged twice on my bill?");
    let result1 = compiled.invoke(state1).await?;

    println!("\n📊 Result:");
    println!("  Resolution: {}", result1.final_state.resolution);
    println!(
        "  Total Interactions: {}",
        result1.final_state.interactions.len()
    );

    // Test Case 2: Technical query
    println!("\n\n=== Test Case 2: Technical Query ===\n");
    let state2 = CustomerSupportState::new("I'm getting an error when I try to login");
    let result2 = compiled.invoke(state2).await?;

    println!("\n📊 Result:");
    println!("  Resolution: {}", result2.final_state.resolution);
    println!(
        "  Total Interactions: {}",
        result2.final_state.interactions.len()
    );

    // Test Case 3: Sales query
    println!("\n\n=== Test Case 3: Sales Query ===\n");
    let state3 = CustomerSupportState::new("I want to upgrade to get more features");
    let result3 = compiled.invoke(state3).await?;

    println!("\n📊 Result:");
    println!("  Resolution: {}", result3.final_state.resolution);
    println!(
        "  Total Interactions: {}",
        result3.final_state.interactions.len()
    );

    println!("\n✅ All test cases completed!");

    Ok(())
}
