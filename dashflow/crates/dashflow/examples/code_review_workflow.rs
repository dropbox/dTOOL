//! Code Review Workflow with Parallel Execution
//!
//! This example demonstrates a code review workflow where multiple quality checks
//! execute in parallel, followed by report generation and pass/fail routing.
//!
//! Workflow:
//! 1. Parse code (syntax validation)
//! 2. Run checks in parallel:
//!    - Linter (style issues)
//!    - Security check (vulnerabilities)
//!    - Style check (naming conventions)
//! 3. Generate report
//! 4. Conditional routing: pass → approved, fail → rejected
//!
//! This demonstrates:
//! - Parallel execution for independent checks
//! - Realistic code quality analysis (simulated)
//! - Conditional routing based on aggregate results

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CodeReviewState {
    code: String,
    syntax_valid: bool,
    #[serde(skip)]
    linter_issues: Arc<Mutex<Vec<String>>>,
    #[serde(skip)]
    security_issues: Arc<Mutex<Vec<String>>>,
    #[serde(skip)]
    style_issues: Arc<Mutex<Vec<String>>>,
    report: String,
    approved: bool,
}

impl MergeableState for CodeReviewState {
    fn merge(&mut self, other: &Self) {
        if !other.code.is_empty() {
            if self.code.is_empty() {
                self.code = other.code.clone();
            } else {
                self.code.push('\n');
                self.code.push_str(&other.code);
            }
        }
        self.syntax_valid = self.syntax_valid || other.syntax_valid;
        self.linter_issues = Arc::clone(&other.linter_issues);
        self.security_issues = Arc::clone(&other.security_issues);
        self.style_issues = Arc::clone(&other.style_issues);
        if !other.report.is_empty() {
            if self.report.is_empty() {
                self.report = other.report.clone();
            } else {
                self.report.push('\n');
                self.report.push_str(&other.report);
            }
        }
        self.approved = self.approved || other.approved;
    }
}

impl CodeReviewState {
    fn new(code: String) -> Self {
        Self {
            code,
            syntax_valid: false,
            linter_issues: Arc::new(Mutex::new(Vec::new())),
            security_issues: Arc::new(Mutex::new(Vec::new())),
            style_issues: Arc::new(Mutex::new(Vec::new())),
            report: String::new(),
            approved: false,
        }
    }

    /// Calculate total issue count across all checks
    fn total_issues(&self) -> usize {
        let linter = match self.linter_issues.lock() {
            Ok(issues) => issues.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        };
        let security = match self.security_issues.lock() {
            Ok(issues) => issues.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        };
        let style = match self.style_issues.lock() {
            Ok(issues) => issues.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        };
        linter + security + style
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the graph
    let mut graph: StateGraph<CodeReviewState> = StateGraph::new();

    // Parse code - validates syntax
    graph.add_node_from_fn("parse", |mut state| {
        Box::pin(async move {
            println!("📝 Parsing code...");
            // Simulate syntax check
            state.syntax_valid = !state.code.contains("syntax_error");
            if state.syntax_valid {
                println!("✅ Syntax valid");
            } else {
                println!("❌ Syntax error detected");
            }
            Ok(state)
        })
    });

    // Linter - checks code quality issues
    graph.add_node_from_fn("linter", |state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            println!("🔍 Running linter...");

            let mut issues = match state.linter_issues.lock() {
                Ok(issues) => issues,
                Err(poisoned) => poisoned.into_inner(),
            };

            // Simulate linter checks
            if state.code.contains("var ") {
                issues.push("Use 'let' or 'const' instead of 'var'".to_string());
            }
            if state.code.contains("== ") {
                issues.push("Use '===' for strict equality".to_string());
            }
            if state.code.len() > 500 {
                issues.push("Function too long (>500 chars)".to_string());
            }

            let count = issues.len();
            drop(issues); // Release lock
            println!("  Linter issues: {}", count);
            Ok(state)
        })
    });

    // Security check - identifies vulnerabilities
    graph.add_node_from_fn("security", |state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
            println!("🔒 Running security check...");

            let mut issues = match state.security_issues.lock() {
                Ok(issues) => issues,
                Err(poisoned) => poisoned.into_inner(),
            };

            // Simulate security checks
            if state.code.contains("eval(") {
                issues.push("Avoid eval() - code injection risk".to_string());
            }
            if state.code.contains("innerHTML") {
                issues.push("Avoid innerHTML - XSS vulnerability".to_string());
            }
            if state.code.contains("SELECT * FROM") {
                issues.push("SQL injection risk - use parameterized queries".to_string());
            }

            let count = issues.len();
            drop(issues); // Release lock
            println!("  Security issues: {}", count);
            Ok(state)
        })
    });

    // Style check - naming conventions and formatting
    graph.add_node_from_fn("style", |state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
            println!("✨ Running style check...");

            let mut issues = match state.style_issues.lock() {
                Ok(issues) => issues,
                Err(poisoned) => poisoned.into_inner(),
            };

            // Simulate style checks
            if state.code.contains("function_name") {
                issues.push("Use camelCase for function names".to_string());
            }
            if !state.code.contains("//") && !state.code.contains("/*") {
                issues.push("Add code comments".to_string());
            }
            if state.code.lines().count() > 50 {
                issues.push("Function exceeds 50 lines - consider refactoring".to_string());
            }

            let count = issues.len();
            drop(issues); // Release lock
            println!("  Style issues: {}", count);
            Ok(state)
        })
    });

    // Generate report - aggregates all findings
    graph.add_node_from_fn("generate_report", |mut state| {
        Box::pin(async move {
            println!("\n📊 Generating report...");

            let mut report = String::new();
            report.push_str("CODE REVIEW REPORT\n");
            report.push_str("==================\n\n");

            // Syntax section
            report.push_str(&format!(
                "Syntax: {}\n\n",
                if state.syntax_valid {
                    "✅ Valid"
                } else {
                    "❌ Invalid"
                }
            ));

            // Linter section
            {
                let linter_issues = match state.linter_issues.lock() {
                    Ok(issues) => issues,
                    Err(poisoned) => poisoned.into_inner(),
                };
                report.push_str(&format!("Linter Issues ({})\n", linter_issues.len()));
                for issue in linter_issues.iter() {
                    report.push_str(&format!("  - {}\n", issue));
                }
            }
            report.push('\n');

            // Security section
            {
                let security_issues = match state.security_issues.lock() {
                    Ok(issues) => issues,
                    Err(poisoned) => poisoned.into_inner(),
                };
                report.push_str(&format!("Security Issues ({})\n", security_issues.len()));
                for issue in security_issues.iter() {
                    report.push_str(&format!("  - {}\n", issue));
                }
            }
            report.push('\n');

            // Style section
            {
                let style_issues = match state.style_issues.lock() {
                    Ok(issues) => issues,
                    Err(poisoned) => poisoned.into_inner(),
                };
                report.push_str(&format!("Style Issues ({})\n", style_issues.len()));
                for issue in style_issues.iter() {
                    report.push_str(&format!("  - {}\n", issue));
                }
            }
            report.push('\n');

            // Summary
            let total = state.total_issues();
            report.push_str(&format!("Total Issues: {}\n", total));
            report.push_str(&format!(
                "Status: {}\n",
                if state.syntax_valid && total == 0 {
                    "APPROVED ✅"
                } else if state.syntax_valid && total <= 3 {
                    "APPROVED WITH WARNINGS ⚠️"
                } else {
                    "REJECTED ❌"
                }
            ));

            state.report = report;
            println!("\n{}", state.report);

            Ok(state)
        })
    });

    // Approved node - code passes review
    graph.add_node_from_fn("approved", |mut state| {
        Box::pin(async move {
            state.approved = true;
            println!("✅ Code review: APPROVED");
            println!("   Ready for merge!");
            Ok(state)
        })
    });

    // Rejected node - code fails review
    graph.add_node_from_fn("rejected", |mut state| {
        Box::pin(async move {
            state.approved = false;
            println!("❌ Code review: REJECTED");
            println!("   Please address issues and resubmit.");
            Ok(state)
        })
    });

    // Build the graph
    graph.set_entry_point("parse");

    // Parallel execution: linter, security, and style checks run concurrently
    // All three checks start from parse and execute in parallel
    graph.add_parallel_edges(
        "parse",
        vec![
            "linter".to_string(),
            "security".to_string(),
            "style".to_string(),
        ],
    );

    // All checks converge to report generation
    // Note: Due to parallel execution semantics, we only need one edge
    // The last parallel node to complete will trigger generate_report
    graph.add_edge("style", "generate_report");

    // Conditional routing based on total issues
    let mut routes = HashMap::new();
    routes.insert("approved".to_string(), "approved".to_string());
    routes.insert("rejected".to_string(), "rejected".to_string());

    graph.add_conditional_edges(
        "generate_report",
        |state: &CodeReviewState| {
            if state.syntax_valid && state.total_issues() <= 3 {
                "approved".to_string()
            } else {
                "rejected".to_string()
            }
        },
        routes,
    );

    graph.add_edge("approved", END);
    graph.add_edge("rejected", END);

    // Compile the graph
    let app = graph.compile()?;

    // Example 1: Clean code (should pass)
    println!("🚀 Scenario 1: Clean Code\n");
    println!("{}\n", "=".repeat(60));

    let clean_code = r#"
// User authentication function
function authenticateUser(username, password) {
    const hashedPassword = hashPassword(password);
    return database.query(
        "SELECT * FROM users WHERE username = ? AND password = ?",
        [username, hashedPassword]
    );
}
"#
    .to_string();

    let state1 = CodeReviewState::new(clean_code);
    let result1 = app.invoke(state1).await?;
    println!(
        "\n📈 Result: {}",
        if result1.final_state.approved {
            "APPROVED ✅"
        } else {
            "REJECTED ❌"
        }
    );

    // Example 2: Code with issues (should fail)
    println!("\n\n🚀 Scenario 2: Problematic Code\n");
    println!("{}\n", "=".repeat(60));

    let problematic_code = r#"
var user_input = getUserInput();
if (user_input == "admin") {
    document.getElementById("content").innerHTML = user_input;
    eval(user_input);
    database.query("SELECT * FROM users WHERE name = '" + user_input + "'");
}
"#
    .to_string();

    let state2 = CodeReviewState::new(problematic_code);
    let result2 = app.invoke(state2).await?;
    println!(
        "\n📈 Result: {}",
        if result2.final_state.approved {
            "APPROVED ✅"
        } else {
            "REJECTED ❌"
        }
    );

    Ok(())
}
