//! Customer Service Router Example
//!
//! This example demonstrates a multi-agent customer service system using DashFlow:
//! - Intent classifier routes customer queries to appropriate specialist
//! - Specialist agents handle domain-specific queries (billing, tech support, sales)
//! - Escalation logic routes complex cases to human agents
//! - Human-in-the-loop pattern for quality control
//!
//! Architecture:
//! - Intake → Intent Classifier → [Billing | Tech Support | Sales]
//! - Each specialist can escalate to human agent if needed
//! - Quality check determines if human review is required
//!
//! Run: cargo run --example customer_service_router

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct CustomerServiceState {
    customer_query: String,
    customer_id: String,
    intent: String,        // "billing", "tech_support", "sales", "unknown"
    urgency_level: String, // "low", "medium", "high", "critical"
    specialist_response: String,
    requires_human: bool,
    human_review_notes: String,
    resolution: String,
    satisfaction_score: Option<f32>,
    next_action: String,
    escalation_reason: String,
}

impl MergeableState for CustomerServiceState {
    fn merge(&mut self, other: &Self) {
        if !other.customer_query.is_empty() {
            if self.customer_query.is_empty() {
                self.customer_query = other.customer_query.clone();
            } else {
                self.customer_query.push('\n');
                self.customer_query.push_str(&other.customer_query);
            }
        }
        if !other.customer_id.is_empty() {
            if self.customer_id.is_empty() {
                self.customer_id = other.customer_id.clone();
            } else {
                self.customer_id.push('\n');
                self.customer_id.push_str(&other.customer_id);
            }
        }
        if !other.intent.is_empty() {
            if self.intent.is_empty() {
                self.intent = other.intent.clone();
            } else {
                self.intent.push('\n');
                self.intent.push_str(&other.intent);
            }
        }
        if !other.urgency_level.is_empty() {
            if self.urgency_level.is_empty() {
                self.urgency_level = other.urgency_level.clone();
            } else {
                self.urgency_level.push('\n');
                self.urgency_level.push_str(&other.urgency_level);
            }
        }
        if !other.specialist_response.is_empty() {
            if self.specialist_response.is_empty() {
                self.specialist_response = other.specialist_response.clone();
            } else {
                self.specialist_response.push('\n');
                self.specialist_response
                    .push_str(&other.specialist_response);
            }
        }
        self.requires_human = self.requires_human || other.requires_human;
        if !other.human_review_notes.is_empty() {
            if self.human_review_notes.is_empty() {
                self.human_review_notes = other.human_review_notes.clone();
            } else {
                self.human_review_notes.push('\n');
                self.human_review_notes.push_str(&other.human_review_notes);
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
        if other.satisfaction_score.is_some() {
            self.satisfaction_score = other.satisfaction_score;
        }
        if !other.next_action.is_empty() {
            if self.next_action.is_empty() {
                self.next_action = other.next_action.clone();
            } else {
                self.next_action.push('\n');
                self.next_action.push_str(&other.next_action);
            }
        }
        if !other.escalation_reason.is_empty() {
            if self.escalation_reason.is_empty() {
                self.escalation_reason = other.escalation_reason.clone();
            } else {
                self.escalation_reason.push('\n');
                self.escalation_reason.push_str(&other.escalation_reason);
            }
        }
    }
}

impl CustomerServiceState {
    fn new(customer_id: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            customer_query: query.into(),
            customer_id: customer_id.into(),
            intent: String::new(),
            urgency_level: "medium".to_string(),
            specialist_response: String::new(),
            requires_human: false,
            human_review_notes: String::new(),
            resolution: String::new(),
            satisfaction_score: None,
            next_action: String::new(),
            escalation_reason: String::new(),
        }
    }
}

fn build_customer_service_graph() -> StateGraph<CustomerServiceState> {
    let mut graph = StateGraph::new();

    // Node 1: Intake - Initial customer contact processing
    graph.add_node_from_fn("intake", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!(
                "\n📞 Intake: Processing request from customer {}",
                state.customer_id
            );
            println!("   Query: \"{}\"", state.customer_query);
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Analyze urgency based on keywords
            let query_lower = state.customer_query.to_lowercase();
            state.urgency_level = if query_lower.contains("urgent")
                || query_lower.contains("critical")
                || query_lower.contains("emergency")
            {
                "critical".to_string()
            } else if query_lower.contains("asap") || query_lower.contains("important") {
                "high".to_string()
            } else if query_lower.contains("when you can") || query_lower.contains("no rush") {
                "low".to_string()
            } else {
                "medium".to_string()
            };

            println!("   Urgency: {}", state.urgency_level);
            Ok(state)
        })
    });

    // Node 2: Intent Classifier - Routes to appropriate specialist
    graph.add_node_from_fn("intent_classifier", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("🎯 Intent Classifier: Analyzing customer query...");
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

            // Classify intent based on keywords
            let query_lower = state.customer_query.to_lowercase();
            state.intent = if query_lower.contains("bill")
                || query_lower.contains("charge")
                || query_lower.contains("payment")
                || query_lower.contains("refund")
                || query_lower.contains("invoice")
            {
                "billing".to_string()
            } else if query_lower.contains("error")
                || query_lower.contains("bug")
                || query_lower.contains("not working")
                || query_lower.contains("broken")
                || query_lower.contains("crash")
                || query_lower.contains("technical")
            {
                "tech_support".to_string()
            } else if query_lower.contains("upgrade")
                || query_lower.contains("feature")
                || query_lower.contains("pricing")
                || query_lower.contains("plan")
                || query_lower.contains("buy")
                || query_lower.contains("purchase")
            {
                "sales".to_string()
            } else {
                "unknown".to_string()
            };

            println!(
                "🎯 Intent Classifier: Routing to {} department",
                state.intent
            );
            Ok(state)
        })
    });

    // Node 3: Billing Specialist
    graph.add_node_from_fn("billing_specialist", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("💰 Billing Specialist: Processing billing inquiry...");
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            let query_lower = state.customer_query.to_lowercase();

            // Check if this requires human escalation
            if query_lower.contains("refund") || query_lower.contains("dispute") {
                state.requires_human = true;
                state.escalation_reason = "Refund/dispute requires manager approval".to_string();
                state.specialist_response = format!(
                    "I've reviewed your billing inquiry regarding your account ({}). \
                    This appears to be a {} request, which requires manager review. \
                    I'm escalating this to our billing manager who will contact you within 2 hours.",
                    state.customer_id,
                    if query_lower.contains("refund") {
                        "refund"
                    } else {
                        "dispute"
                    }
                );
                println!("💰 Billing Specialist: ⚠️  Escalating to human (refund/dispute)");
            } else {
                state.requires_human = false;
                state.specialist_response = format!(
                    "I've reviewed your billing inquiry for account {}. \
                    Your current balance is $0.00, and your next bill date is in 15 days. \
                    I can see your recent payment was processed successfully. \
                    Is there anything specific about your billing I can help clarify?",
                    state.customer_id
                );
                state.satisfaction_score = Some(0.85);
                println!("💰 Billing Specialist: ✓ Inquiry resolved");
            }

            Ok(state)
        })
    });

    // Node 4: Tech Support Specialist
    graph.add_node_from_fn(
        "tech_support_specialist",
        |mut state: CustomerServiceState| {
            Box::pin(async move {
                println!("🔧 Tech Support: Diagnosing technical issue...");
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let query_lower = state.customer_query.to_lowercase();

                // Check if this is a critical issue
                if state.urgency_level == "critical"
                    || query_lower.contains("crash")
                    || query_lower.contains("data loss")
                {
                    state.requires_human = true;
                    state.escalation_reason =
                        "Critical technical issue requires senior engineer".to_string();
                    state.specialist_response = format!(
                        "I understand you're experiencing a critical issue: '{}'. \
                    I'm immediately escalating this to our senior technical team. \
                    An engineer will reach out within 30 minutes. \
                    Your ticket number is TECH-{}-URGENT.",
                        state.customer_query, state.customer_id
                    );
                    println!("🔧 Tech Support: ⚠️  Escalating to senior engineer (critical issue)");
                } else {
                    state.requires_human = false;
                    state.specialist_response = format!(
                        "I've analyzed your technical issue: '{}'. \
                    Based on our diagnostics, this is typically resolved by: \
                    1. Clearing your browser cache\n\
                    2. Signing out and back in\n\
                    3. Ensuring you're using the latest version\n\
                    \n\
                    Please try these steps and let me know if the issue persists. \
                    If it does, I'll escalate to our engineering team. Ticket: TECH-{}",
                        state.customer_query, state.customer_id
                    );
                    state.satisfaction_score = Some(0.78);
                    println!("🔧 Tech Support: ✓ Troubleshooting steps provided");
                }

                Ok(state)
            })
        },
    );

    // Node 5: Sales Specialist
    graph.add_node_from_fn("sales_specialist", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("📈 Sales Specialist: Handling sales inquiry...");
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            let query_lower = state.customer_query.to_lowercase();

            // Enterprise inquiries go to account executive
            if query_lower.contains("enterprise")
                || query_lower.contains("100+ users")
                || query_lower.contains("custom")
            {
                state.requires_human = true;
                state.escalation_reason =
                    "Enterprise inquiry requires account executive".to_string();
                state.specialist_response = format!(
                    "Thank you for your interest in our {} solution! \
                    I'm connecting you with one of our enterprise account executives \
                    who can discuss custom pricing, SLAs, and dedicated support. \
                    They'll reach out within 4 business hours. Reference: SALES-{}",
                    if query_lower.contains("enterprise") {
                        "enterprise"
                    } else {
                        "custom"
                    },
                    state.customer_id
                );
                println!("📈 Sales Specialist: ⚠️  Routing to account executive (enterprise)");
            } else {
                state.requires_human = false;
                state.specialist_response = format!(
                    "I'd be happy to help with your {} inquiry! \
                    \n\
                    Our current plans are:\n\
                    • Basic: $10/month (up to 5 users)\n\
                    • Professional: $25/month (up to 25 users)\n\
                    • Business: $50/month (up to 100 users)\n\
                    \n\
                    All plans include 24/7 support and a 14-day free trial. \
                    Would you like me to start your trial? Customer: {}",
                    state.customer_query, state.customer_id
                );
                state.satisfaction_score = Some(0.82);
                println!("📈 Sales Specialist: ✓ Pricing information provided");
            }

            Ok(state)
        })
    });

    // Node 6: Unknown Handler - Handles queries that can't be classified
    graph.add_node_from_fn("unknown_handler", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("❓ Unknown Handler: Unable to classify query");
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            state.requires_human = true;
            state.escalation_reason = "Unable to classify query intent".to_string();
            state.specialist_response = format!(
                "Thank you for contacting us. Your query: '{}' \
                I want to make sure you get the best help possible. \
                I'm connecting you with a customer service representative \
                who can better assist you. Reference: GENERAL-{}",
                state.customer_query, state.customer_id
            );

            println!("❓ Unknown Handler: ⚠️  Routing to general support");
            Ok(state)
        })
    });

    // Node 7: Human Review - Simulates human agent review
    graph.add_node_from_fn("human_review", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("👤 Human Agent: Reviewing escalated case...");
            println!("   Reason: {}", state.escalation_reason);
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            // Simulate human agent review and resolution
            state.human_review_notes =
                "Human agent reviewed case. Original response from specialist was appropriate. \
                Additional context provided to customer. \
                Resolution time: 15 minutes. Customer verified satisfied."
                    .to_string();

            state.resolution = format!(
                "{}\n\n[Human Agent Follow-up]\n\
                A member of our team has reviewed your case personally. \
                We've processed your request and confirmed everything is resolved. \
                If you have any other questions, please don't hesitate to reach out.",
                state.specialist_response
            );

            state.satisfaction_score = Some(0.95);
            state.next_action = "complete".to_string();

            println!("👤 Human Agent: ✓ Case resolved with customer satisfaction");
            Ok(state)
        })
    });

    // Node 8: Quality Check - Determines if human review is needed
    graph.add_node_from_fn("quality_check", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("🔍 Quality Check: Evaluating resolution...");
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            if state.requires_human {
                state.next_action = "escalate".to_string();
                println!("🔍 Quality Check: → Escalating to human review");
            } else {
                state.resolution = state.specialist_response.clone();
                state.next_action = "complete".to_string();
                println!(
                    "🔍 Quality Check: ✓ Auto-resolved (satisfaction: {:.0}%)",
                    state.satisfaction_score.unwrap_or(0.0) * 100.0
                );
            }

            Ok(state)
        })
    });

    // Build the workflow graph
    graph.set_entry_point("intake");

    // Intake → Intent Classifier
    graph.add_edge("intake", "intent_classifier");

    // Intent Classifier routes to appropriate specialist
    graph.add_conditional_edges(
        "intent_classifier",
        |state: &CustomerServiceState| state.intent.clone(),
        [
            ("billing".to_string(), "billing_specialist".to_string()),
            (
                "tech_support".to_string(),
                "tech_support_specialist".to_string(),
            ),
            ("sales".to_string(), "sales_specialist".to_string()),
            ("unknown".to_string(), "unknown_handler".to_string()),
        ]
        .into_iter()
        .collect(),
    );

    // All specialists and unknown handler route to quality check
    graph.add_edge("billing_specialist", "quality_check");
    graph.add_edge("tech_support_specialist", "quality_check");
    graph.add_edge("sales_specialist", "quality_check");
    graph.add_edge("unknown_handler", "quality_check");

    // Quality check routes to human review or completion
    graph.add_conditional_edges(
        "quality_check",
        |state: &CustomerServiceState| state.next_action.clone(),
        [
            ("escalate".to_string(), "human_review".to_string()),
            ("complete".to_string(), END.to_string()),
        ]
        .into_iter()
        .collect(),
    );

    // Human review completes the workflow
    graph.add_edge("human_review", END);

    graph
}

#[tokio::main]
async fn main() -> dashflow::Result<()> {
    println!("\n═══════════════════════════════════════════════════════════");
    println!("         CUSTOMER SERVICE ROUTER SYSTEM");
    println!("═══════════════════════════════════════════════════════════\n");
    println!("This example demonstrates:");
    println!("  • Intent classification and routing");
    println!("  • Multi-specialist agent coordination");
    println!("  • Escalation logic for complex cases");
    println!("  • Human-in-the-loop pattern for quality control\n");

    let graph = build_customer_service_graph();
    let app = graph.compile()?;

    // Scenario 1: Simple billing query (auto-resolved)
    println!("═══════════════════════════════════════════════════════════");
    println!("SCENARIO 1: Billing Query (Auto-Resolved)");
    println!("═══════════════════════════════════════════════════════════");

    let state1 = CustomerServiceState::new("CUST-10234", "What's my current bill amount?");
    let result1 = app.invoke(state1).await?;

    println!("\n📊 RESOLUTION SUMMARY:");
    println!("  • Intent: {}", result1.state().intent);
    println!("  • Path: {}", result1.execution_path().join(" → "));
    println!(
        "  • Human review: {}",
        if result1.state().requires_human {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  • Satisfaction: {:.0}%",
        result1.state().satisfaction_score.unwrap_or(0.0) * 100.0
    );
    println!("\n📝 RESOLUTION:\n{}\n", result1.state().resolution);

    // Scenario 2: Refund request (requires human escalation)
    println!("═══════════════════════════════════════════════════════════");
    println!("SCENARIO 2: Refund Request (Human Escalation)");
    println!("═══════════════════════════════════════════════════════════");

    let graph2 = build_customer_service_graph();
    let app2 = graph2.compile()?;

    let state2 = CustomerServiceState::new("CUST-10235", "I need a refund for last month's charge");
    let result2 = app2.invoke(state2).await?;

    println!("\n📊 RESOLUTION SUMMARY:");
    println!("  • Intent: {}", result2.state().intent);
    println!("  • Path: {}", result2.execution_path().join(" → "));
    println!(
        "  • Human review: {}",
        if result2.state().requires_human {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  • Escalation reason: {}",
        result2.state().escalation_reason
    );
    println!(
        "  • Satisfaction: {:.0}%",
        result2.state().satisfaction_score.unwrap_or(0.0) * 100.0
    );
    println!("\n📝 RESOLUTION:\n{}\n", result2.state().resolution);

    // Scenario 3: Critical technical issue (urgent escalation)
    println!("═══════════════════════════════════════════════════════════");
    println!("SCENARIO 3: Critical Technical Issue (Urgent Escalation)");
    println!("═══════════════════════════════════════════════════════════");

    let graph3 = build_customer_service_graph();
    let app3 = graph3.compile()?;

    let state3 =
        CustomerServiceState::new("CUST-10236", "URGENT: Application crash causing data loss!");
    let result3 = app3.invoke(state3).await?;

    println!("\n📊 RESOLUTION SUMMARY:");
    println!("  • Intent: {}", result3.state().intent);
    println!("  • Urgency: {}", result3.state().urgency_level);
    println!("  • Path: {}", result3.execution_path().join(" → "));
    println!(
        "  • Human review: {}",
        if result3.state().requires_human {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  • Escalation reason: {}",
        result3.state().escalation_reason
    );
    println!(
        "  • Satisfaction: {:.0}%",
        result3.state().satisfaction_score.unwrap_or(0.0) * 100.0
    );
    println!("\n📝 RESOLUTION:\n{}\n", result3.state().resolution);

    // Scenario 4: Enterprise sales inquiry (account executive routing)
    println!("═══════════════════════════════════════════════════════════");
    println!("SCENARIO 4: Enterprise Sales Inquiry");
    println!("═══════════════════════════════════════════════════════════");

    let graph4 = build_customer_service_graph();
    let app4 = graph4.compile()?;

    let state4 =
        CustomerServiceState::new("CUST-10237", "Interested in enterprise plan for 500+ users");
    let result4 = app4.invoke(state4).await?;

    println!("\n📊 RESOLUTION SUMMARY:");
    println!("  • Intent: {}", result4.state().intent);
    println!("  • Path: {}", result4.execution_path().join(" → "));
    println!(
        "  • Human review: {}",
        if result4.state().requires_human {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  • Escalation reason: {}",
        result4.state().escalation_reason
    );
    println!(
        "  • Satisfaction: {:.0}%",
        result4.state().satisfaction_score.unwrap_or(0.0) * 100.0
    );
    println!("\n📝 RESOLUTION:\n{}\n", result4.state().resolution);

    println!("═══════════════════════════════════════════════════════════");
    println!("             CUSTOMER SERVICE COMPLETE");
    println!("═══════════════════════════════════════════════════════════\n");
    println!("Key Features Demonstrated:");
    println!("  ✓ Intent classification routing");
    println!("  ✓ Multiple specialist agents (billing, tech, sales)");
    println!("  ✓ Urgency detection and prioritization");
    println!("  ✓ Conditional escalation logic");
    println!("  ✓ Human-in-the-loop for complex cases");
    println!("  ✓ Quality check and satisfaction tracking\n");

    Ok(())
}
