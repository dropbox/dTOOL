//! Financial Analysis Agent Example
//!
//! This example demonstrates a multi-agent financial analysis system using DashFlow:
//! - Supervisor agent coordinates the analysis workflow
//! - Data gathering agent collects market data and company metrics
//! - Analysis agent performs financial modeling and risk assessment
//! - Report writing agent produces investment recommendations
//! - Conditional routing based on confidence levels
//!
//! Architecture:
//! - Supervisor → Data Gatherer → Analyst → Writer → Quality Check
//! - Quality check routes to either revision (low confidence) or approval (high confidence)
//!
//! Run: cargo run --example financial_analysis_agent

use dashflow::error::Result;
use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct FinancialState {
    ticker: String,
    analysis_type: String, // "quick" or "comprehensive"
    market_data: HashMap<String, f64>,
    company_metrics: HashMap<String, f64>,
    risk_assessment: String,
    valuation: f64,
    confidence_score: f64,
    recommendation: String,
    report: String,
    next_action: String,
    revision_count: u32,
}

impl MergeableState for FinancialState {
    fn merge(&mut self, other: &Self) {
        if !other.ticker.is_empty() {
            if self.ticker.is_empty() {
                self.ticker = other.ticker.clone();
            } else {
                self.ticker.push('\n');
                self.ticker.push_str(&other.ticker);
            }
        }
        if !other.analysis_type.is_empty() {
            if self.analysis_type.is_empty() {
                self.analysis_type = other.analysis_type.clone();
            } else {
                self.analysis_type.push('\n');
                self.analysis_type.push_str(&other.analysis_type);
            }
        }
        self.market_data.extend(other.market_data.clone());
        self.company_metrics.extend(other.company_metrics.clone());
        if !other.risk_assessment.is_empty() {
            if self.risk_assessment.is_empty() {
                self.risk_assessment = other.risk_assessment.clone();
            } else {
                self.risk_assessment.push('\n');
                self.risk_assessment.push_str(&other.risk_assessment);
            }
        }
        self.valuation = self.valuation.max(other.valuation);
        self.confidence_score = self.confidence_score.max(other.confidence_score);
        if !other.recommendation.is_empty() {
            if self.recommendation.is_empty() {
                self.recommendation = other.recommendation.clone();
            } else {
                self.recommendation.push('\n');
                self.recommendation.push_str(&other.recommendation);
            }
        }
        if !other.report.is_empty() {
            if self.report.is_empty() {
                self.report = other.report.clone();
            } else {
                self.report.push('\n');
                self.report.push_str(&other.report);
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
        self.revision_count = self.revision_count.max(other.revision_count);
    }
}

impl FinancialState {
    fn new(ticker: impl Into<String>, analysis_type: impl Into<String>) -> Self {
        Self {
            ticker: ticker.into(),
            analysis_type: analysis_type.into(),
            market_data: HashMap::new(),
            company_metrics: HashMap::new(),
            risk_assessment: String::new(),
            valuation: 0.0,
            confidence_score: 0.0,
            recommendation: String::new(),
            report: String::new(),
            next_action: String::new(),
            revision_count: 0,
        }
    }
}

fn build_financial_analysis_graph() -> StateGraph<FinancialState> {
    let mut graph = StateGraph::new();

    // Node 1: Supervisor - Orchestrates the analysis workflow
    graph.add_node_from_fn("supervisor", |state: FinancialState| {
        Box::pin(async move {
            println!(
                "\n📋 Supervisor: Initiating {} analysis for {}",
                state.analysis_type, state.ticker
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            println!(
                "📋 Supervisor: Workflow configured - {}",
                if state.analysis_type == "comprehensive" {
                    "deep dive with extended metrics"
                } else {
                    "quick assessment"
                }
            );

            Ok(state)
        })
    });

    // Node 2: Data Gatherer - Collects market data and company metrics
    graph.add_node_from_fn("data_gatherer", |mut state: FinancialState| {
        Box::pin(async move {
            println!("💾 Data Gatherer: Collecting data for {}...", state.ticker);
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            // Simulate gathering market data
            state.market_data.insert("price".to_string(), 150.75);
            state.market_data.insert("52w_high".to_string(), 180.00);
            state.market_data.insert("52w_low".to_string(), 120.00);
            state.market_data.insert("volume".to_string(), 2_500_000.0);
            state
                .market_data
                .insert("market_cap".to_string(), 500_000_000_000.0);

            // Simulate gathering company metrics
            state.company_metrics.insert("pe_ratio".to_string(), 25.5);
            state.company_metrics.insert("eps".to_string(), 5.91);
            state
                .company_metrics
                .insert("revenue_growth".to_string(), 0.15);
            state
                .company_metrics
                .insert("debt_to_equity".to_string(), 0.45);
            state.company_metrics.insert("roe".to_string(), 0.22);

            if state.analysis_type == "comprehensive" {
                state
                    .company_metrics
                    .insert("free_cash_flow".to_string(), 10_000_000_000.0);
                state
                    .company_metrics
                    .insert("operating_margin".to_string(), 0.28);
                state
                    .company_metrics
                    .insert("dividend_yield".to_string(), 0.015);
            }

            println!(
                "💾 Data Gatherer: Collected {} market data points, {} company metrics",
                state.market_data.len(),
                state.company_metrics.len()
            );

            Ok(state)
        })
    });

    // Node 3: Analyst - Performs financial analysis and risk assessment
    graph.add_node_from_fn("analyst", |mut state: FinancialState| {
        Box::pin(async move {
            println!("📊 Analyst: Performing financial analysis...");
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            // Calculate valuation (simplified DCF model)
            let eps = state.company_metrics.get("eps").unwrap_or(&0.0);
            let growth = state.company_metrics.get("revenue_growth").unwrap_or(&0.0);
            let pe_ratio = state.company_metrics.get("pe_ratio").unwrap_or(&0.0);

            // Simple valuation: EPS * (1 + growth)^5 * PE ratio
            state.valuation = eps * (1.0 + growth).powi(5) * pe_ratio;

            // Risk assessment
            let debt_to_equity = state.company_metrics.get("debt_to_equity").unwrap_or(&0.0);
            let roe = state.company_metrics.get("roe").unwrap_or(&0.0);

            let risk_level = if *debt_to_equity > 1.0 {
                "HIGH"
            } else if *debt_to_equity > 0.5 {
                "MODERATE"
            } else {
                "LOW"
            };

            state.risk_assessment = format!(
                "Risk Level: {}\n\
                 - Debt-to-Equity: {:.2}\n\
                 - Return on Equity: {:.1}%\n\
                 - Financial Health: {}",
                risk_level,
                debt_to_equity,
                roe * 100.0,
                if *roe > 0.15 && *debt_to_equity < 0.6 {
                    "Strong"
                } else if *roe > 0.10 {
                    "Adequate"
                } else {
                    "Concerning"
                }
            );

            // Calculate confidence based on data completeness and metrics quality
            let base_confidence = if state.analysis_type == "comprehensive" {
                0.85
            } else {
                0.70
            };

            let metric_quality: f64 = if *roe > 0.15 && *debt_to_equity < 0.6 {
                0.1
            } else {
                -0.05
            };
            state.confidence_score = (base_confidence + metric_quality).clamp(0.0, 1.0);

            // Generate recommendation
            let current_price = state.market_data.get("price").unwrap_or(&0.0);
            let upside = (state.valuation - current_price) / current_price;

            state.recommendation = if upside > 0.20 && risk_level == "LOW" {
                "STRONG BUY"
            } else if upside > 0.10 {
                "BUY"
            } else if upside > -0.10 {
                "HOLD"
            } else {
                "SELL"
            }
            .to_string();

            println!(
                "📊 Analyst: Analysis complete - {} recommendation (confidence: {:.0}%)",
                state.recommendation,
                state.confidence_score * 100.0
            );

            Ok(state)
        })
    });

    // Node 4: Report Writer - Creates investment report
    graph.add_node_from_fn("writer", |mut state: FinancialState| {
        Box::pin(async move {
            state.revision_count += 1;
            println!(
                "✍️  Writer: Generating report (revision {})...",
                state.revision_count
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            let current_price = state.market_data.get("price").unwrap_or(&0.0);
            let upside = (state.valuation - current_price) / current_price * 100.0;

            state.report = format!(
                "═══════════════════════════════════════════════════════════\n\
                 INVESTMENT ANALYSIS REPORT: {}\n\
                 ═══════════════════════════════════════════════════════════\n\
                 \n\
                 RECOMMENDATION: {} (Confidence: {:.0}%)\n\
                 \n\
                 VALUATION SUMMARY:\n\
                 • Current Price:     ${:.2}\n\
                 • Target Valuation:  ${:.2}\n\
                 • Upside Potential:  {:.1}%\n\
                 \n\
                 KEY METRICS:\n\
                 • P/E Ratio:         {:.1}x\n\
                 • EPS:               ${:.2}\n\
                 • Revenue Growth:    {:.1}%\n\
                 • ROE:               {:.1}%\n\
                 \n\
                 RISK ASSESSMENT:\n\
                 {}\n\
                 \n\
                 ANALYSIS TYPE: {} (Revision: {})\n\
                 ═══════════════════════════════════════════════════════════\n",
                state.ticker,
                state.recommendation,
                state.confidence_score * 100.0,
                current_price,
                state.valuation,
                upside,
                state.company_metrics.get("pe_ratio").unwrap_or(&0.0),
                state.company_metrics.get("eps").unwrap_or(&0.0),
                state.company_metrics.get("revenue_growth").unwrap_or(&0.0) * 100.0,
                state.company_metrics.get("roe").unwrap_or(&0.0) * 100.0,
                state.risk_assessment,
                state.analysis_type.to_uppercase(),
                state.revision_count
            );

            println!("✍️  Writer: Report generated");

            Ok(state)
        })
    });

    // Node 5: Quality Check - Reviews confidence and determines next action
    graph.add_node_from_fn("quality_check", |mut state: FinancialState| {
        Box::pin(async move {
            println!("🔍 Quality Check: Reviewing analysis quality...");
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Determine if revision is needed based on confidence
            let needs_revision = state.confidence_score < 0.75 && state.revision_count < 2;

            if needs_revision {
                state.next_action = "revise".to_string();
                // Upgrade to comprehensive analysis for revision
                state.analysis_type = "comprehensive".to_string();
                println!(
                    "🔍 Quality Check: Low confidence ({:.0}%), requesting comprehensive revision",
                    state.confidence_score * 100.0
                );
            } else {
                state.next_action = "approve".to_string();
                println!("🔍 Quality Check: ✓ Analysis approved");
            }

            Ok(state)
        })
    });

    // Build the workflow graph
    graph.set_entry_point("supervisor");

    // Linear workflow with conditional loop
    graph.add_edge("supervisor", "data_gatherer");
    graph.add_edge("data_gatherer", "analyst");
    graph.add_edge("analyst", "writer");
    graph.add_edge("writer", "quality_check");

    // Conditional routing: revise (back to data gathering) or approve (end)
    let mut routes = HashMap::new();
    routes.insert("revise".to_string(), "data_gatherer".to_string());
    routes.insert("approve".to_string(), END.to_string());

    graph.add_conditional_edges(
        "quality_check",
        |state: &FinancialState| state.next_action.clone(),
        routes,
    );

    graph
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n════════════════════════════════════════════════════════════");
    println!("          FINANCIAL ANALYSIS AGENT SYSTEM");
    println!("════════════════════════════════════════════════════════════\n");
    println!("This example demonstrates:");
    println!("  • Multi-agent coordination (supervisor pattern)");
    println!("  • Sequential workflow with conditional routing");
    println!("  • Quality-based revision loops");
    println!("  • Financial analysis with risk assessment\n");

    let graph = build_financial_analysis_graph();
    let app = graph.compile()?;

    // Scenario 1: Quick analysis (may trigger revision due to lower confidence)
    println!("════════════════════════════════════════════════════════════");
    println!("SCENARIO 1: Quick Analysis");
    println!("════════════════════════════════════════════════════════════");

    let initial_state = FinancialState::new("ACME", "quick");
    let result = app.invoke(initial_state).await?;

    println!("\n📊 EXECUTION SUMMARY:");
    println!("  • Total steps:     {}", result.execution_path().len());
    println!(
        "  • Execution path:  {}",
        result.execution_path().join(" → ")
    );
    println!("  • Revisions:       {}", result.state().revision_count);
    println!(
        "  • Final confidence: {:.0}%",
        result.state().confidence_score * 100.0
    );

    println!("\n📄 FINAL REPORT:\n");
    println!("{}", result.state().report);

    // Scenario 2: Comprehensive analysis (high confidence, no revision needed)
    println!("\n════════════════════════════════════════════════════════════");
    println!("SCENARIO 2: Comprehensive Analysis");
    println!("════════════════════════════════════════════════════════════");

    let graph2 = build_financial_analysis_graph();
    let app2 = graph2.compile()?;

    let initial_state2 = FinancialState::new("TECH", "comprehensive");
    let result2 = app2.invoke(initial_state2).await?;

    println!("\n📊 EXECUTION SUMMARY:");
    println!("  • Total steps:     {}", result2.execution_path().len());
    println!(
        "  • Execution path:  {}",
        result2.execution_path().join(" → ")
    );
    println!("  • Revisions:       {}", result2.state().revision_count);
    println!(
        "  • Final confidence: {:.0}%",
        result2.state().confidence_score * 100.0
    );

    println!("\n📄 FINAL REPORT:\n");
    println!("{}", result2.state().report);

    println!("\n════════════════════════════════════════════════════════════");
    println!("               ANALYSIS COMPLETE");
    println!("════════════════════════════════════════════════════════════\n");
    println!("Key Features Demonstrated:");
    println!("  ✓ Supervisor coordination pattern");
    println!("  ✓ Multi-stage analysis workflow");
    println!("  ✓ Conditional routing based on confidence");
    println!("  ✓ Quality-driven revision loops");
    println!("  ✓ Financial modeling and risk assessment\n");

    Ok(())
}
