//! LeetCode Integration Test for Codex DashFlow
//!
//! This test demonstrates DashFlow's observability capabilities by running
//! Codex DashFlow on a LeetCode problem and capturing comprehensive telemetry.
//!
//! ## Test Tasks (WORKER_DIRECTIVE.md)
//!
//! - LEET-001: Create LeetCode Two Sum test harness
//! - LEET-002: Wire telemetry capture during execution
//! - LEET-003: Save streaming telemetry snapshots to ~/Desktop
//! - LEET-004: Read graph state transitions during execution
//! - LEET-005: Write observability report proving visibility
//!
//! ## Running the Test
//!
//! ```bash
//! # Set API key (required)
//! source .env && export OPENAI_API_KEY
//!
//! # Run the test
//! cargo test -p codex-dashflow --test leetcode_integration -- --ignored --nocapture
//!
//! # Check output
//! ls ~/Desktop/codex_leetcode_*.txt
//! ls ~/Desktop/codex_leetcode_*.html
//! node scripts/capture_codex_leetcode_screenshots.js
//! ls ~/Desktop/codex_leetcode_*.png
//! ls reports/codex_leetcode_observability_*.md
//! ```

// Allow common test patterns
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr
)]

use chrono::{DateTime, Utc};
use codex_dashflow::create_coding_agent;
use common::{create_llm, LLMRequirements};
use dashflow::prebuilt::AgentState;
use dashflow::{CollectingCallback, EdgeType, GraphEvent};
use dashflow::core::messages::Message;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

// =============================================================================
// Test Constants
// =============================================================================

/// The LeetCode Two Sum problem prompt
const LEETCODE_TWO_SUM_PROMPT: &str = r#"
Solve the following LeetCode problem in Rust:

**Problem: Two Sum**

Given an array of integers nums and an integer target, return indices of the two numbers such that they add up to target.

You may assume that each input would have exactly one solution, and you may not use the same element twice.

You can return the answer in any order.

**Example 1:**
Input: nums = [2,7,11,15], target = 9
Output: [0,1]
Explanation: Because nums[0] + nums[1] == 9, we return [0, 1].

**Example 2:**
Input: nums = [3,2,4], target = 6
Output: [1,2]

**Example 3:**
Input: nums = [3,3], target = 6
Output: [0,1]

**Constraints:**
- 2 <= nums.length <= 10^4
- -10^9 <= nums[i] <= 10^9
- -10^9 <= target <= 2 * 10^9
- Only one valid answer exists.

Please implement an efficient O(n) solution using a HashMap. Write the solution as a Rust function with tests.
"#;

// =============================================================================
// Helper Functions
// =============================================================================

/// Check if LLM is available
fn llm_available() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("ANTHROPIC_API_KEY").is_ok()
        || std::env::var("AWS_ACCESS_KEY_ID").is_ok()
}

/// Get the Desktop path for saving snapshots
fn get_desktop_path() -> PathBuf {
    dirs::desktop_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Get the reports directory path
fn get_reports_path() -> PathBuf {
    // Navigate to workspace root from test crate
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    // examples/apps/codex-dashflow -> workspace root
    let workspace_root = manifest_dir
        .ancestors()
        .nth(3)
        .unwrap_or(&manifest_dir);

    workspace_root.join("reports")
}

/// Format SystemTime as human-readable string
fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.format("%H:%M:%S%.3f").to_string()
}

/// Format Duration as human-readable string
fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{}.{:03}s", duration.as_secs(), duration.subsec_millis())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}...[truncated]")
}

/// Capture a snapshot as both `.txt` and `.html` (for Playwright PNG capture).
fn capture_snapshot_pair(stem: &str, title: &str, content: &str) -> std::io::Result<(PathBuf, PathBuf)> {
    let desktop = get_desktop_path();
    let txt_path = desktop.join(format!("{stem}.txt"));
    let html_path = desktop.join(format!("{stem}.html"));

    {
        let mut file = std::fs::File::create(&txt_path)?;
        file.write_all(content.as_bytes())?;
    }

    let html = format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n\
         <title>{}</title>\n\
         <style>\n\
           :root {{ color-scheme: dark; }}\n\
           body {{ margin: 0; padding: 24px; background: #0b0f14; color: #dce1e7; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace; }}\n\
           h1 {{ margin: 0 0 16px 0; font-size: 18px; font-weight: 600; }}\n\
           pre {{ margin: 0; padding: 16px; background: #0f1620; border: 1px solid #1f2a37; border-radius: 10px; white-space: pre-wrap; word-break: break-word; font-size: 12px; line-height: 1.45; }}\n\
         </style>\n\
         </head>\n\
         <body>\n\
         <h1>{}</h1>\n\
         <pre>{}</pre>\n\
         </body>\n\
         </html>\n",
        escape_html(title),
        escape_html(title),
        escape_html(content)
    );

    {
        let mut file = std::fs::File::create(&html_path)?;
        file.write_all(html.as_bytes())?;
    }

    println!("  Saved snapshot: {}", txt_path.display());
    println!("  Saved snapshot: {}", html_path.display());
    Ok((txt_path, html_path))
}

// =============================================================================
// Event Analysis Functions
// =============================================================================

/// Collected telemetry data from a graph execution
#[derive(Default)]
struct TelemetryData {
    /// All captured events
    events: Vec<String>,
    /// Node execution timeline
    node_timeline: Vec<NodeExecution>,
    /// Edge traversals
    edge_traversals: Vec<EdgeTraversal>,
    /// State change summaries
    state_changes: Vec<StateChange>,
    /// Total execution duration
    total_duration: Option<Duration>,
    /// Execution path
    execution_path: Vec<String>,
    /// Snapshot file paths
    snapshots: Vec<PathBuf>,
}

#[derive(Debug)]
struct NodeExecution {
    node: String,
    start_time: String,
    end_time: Option<String>,
    duration: Option<Duration>,
    error: Option<String>,
}

#[derive(Debug)]
struct EdgeTraversal {
    from: String,
    to: Vec<String>,
    edge_type: String,
    timestamp: String,
}

#[derive(Debug)]
struct StateChange {
    node: String,
    summary: String,
    timestamp: String,
}

/// Analyze collected events and extract telemetry data
fn analyze_events(events: &[GraphEvent<AgentState>]) -> TelemetryData {
    let mut data = TelemetryData::default();
    let mut node_starts: std::collections::HashMap<String, (SystemTime, String)> =
        std::collections::HashMap::new();

    for event in events {
        match event {
            GraphEvent::GraphStart { timestamp, .. } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!("[{}] GraphStart", time_str));
            }
            GraphEvent::GraphEnd {
                timestamp,
                duration,
                execution_path,
                ..
            } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!(
                    "[{}] GraphEnd (duration: {})",
                    time_str,
                    format_duration(*duration)
                ));
                data.total_duration = Some(*duration);
                data.execution_path = execution_path.clone();
            }
            GraphEvent::NodeStart {
                timestamp, node, ..
            } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!("[{}] NodeStart: {}", time_str, node));
                node_starts.insert(node.clone(), (*timestamp, time_str.clone()));
                data.node_timeline.push(NodeExecution {
                    node: node.clone(),
                    start_time: time_str,
                    end_time: None,
                    duration: None,
                    error: None,
                });
            }
            GraphEvent::NodeEnd {
                timestamp,
                node,
                duration,
                ..
            } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!(
                    "[{}] NodeEnd: {} ({})",
                    time_str,
                    node,
                    format_duration(*duration)
                ));

                // Update the corresponding node execution
                if let Some(exec) = data.node_timeline.iter_mut().rev().find(|e| e.node == *node) {
                    exec.end_time = Some(time_str);
                    exec.duration = Some(*duration);
                }
            }
            GraphEvent::NodeError {
                timestamp,
                node,
                error,
                ..
            } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!("[{}] NodeError: {} - {}", time_str, node, error));

                if let Some(exec) = data.node_timeline.iter_mut().rev().find(|e| e.node == *node) {
                    exec.error = Some(error.clone());
                }
            }
            GraphEvent::EdgeTraversal {
                timestamp,
                from,
                to,
                edge_type,
                ..
            } => {
                let time_str = format_time(*timestamp);
                let edge_desc = match edge_type {
                    EdgeType::Simple => "simple".to_string(),
                    EdgeType::Conditional { condition_result } => {
                        format!("conditional[{}]", condition_result)
                    }
                    EdgeType::Parallel => "parallel".to_string(),
                };
                data.events.push(format!(
                    "[{}] Edge: {} -> {} ({})",
                    time_str,
                    from,
                    to.join(", "),
                    edge_desc
                ));
                data.edge_traversals.push(EdgeTraversal {
                    from: from.clone(),
                    to: to.clone(),
                    edge_type: edge_desc,
                    timestamp: time_str,
                });
            }
            GraphEvent::StateChanged {
                timestamp,
                node,
                summary,
                ..
            } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!(
                    "[{}] StateChanged: {} - {}",
                    time_str, node, summary
                ));
                data.state_changes.push(StateChange {
                    node: node.clone(),
                    summary: summary.clone(),
                    timestamp: time_str,
                });
            }
            GraphEvent::DecisionMade {
                timestamp,
                decision_type,
                chosen_option,
                ..
            } => {
                let time_str = format_time(*timestamp);
                data.events.push(format!(
                    "[{}] Decision: {} -> {}",
                    time_str, decision_type, chosen_option
                ));
            }
            GraphEvent::OutcomeObserved {
                timestamp,
                success,
                ..
            } => {
                let time_str = format_time(*timestamp);
                let status = if *success { "SUCCESS" } else { "FAILURE" };
                data.events.push(format!("[{}] Outcome: {}", time_str, status));
            }
            _ => {
                // Handle other event types
                data.events.push(format!("[?] Other event: {:?}", std::mem::discriminant(event)));
            }
        }
    }

    data
}

/// Generate the observability report markdown
fn generate_report(
    telemetry: &TelemetryData,
    final_state: &AgentState,
    start_time: DateTime<Utc>,
) -> String {
    let mut report = String::new();

    // Header
    report.push_str("# Codex DashFlow LeetCode Integration Test Report\n\n");
    report.push_str(&format!("**Date:** {}\n", start_time.format("%Y-%m-%d %H:%M:%S UTC")));
    report.push_str("**Problem:** LeetCode Two Sum\n");
    if let Some(duration) = telemetry.total_duration {
        report.push_str(&format!("**Duration:** {}\n", format_duration(duration)));
    }
    report.push_str("\n---\n\n");

    // Screenshots section
    report.push_str("## Snapshots Captured\n\n");
    report.push_str("| Snapshot | Path | Description |\n");
    report.push_str("|----------|------|-------------|\n");
    for path in &telemetry.snapshots {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let desc = match path.extension().and_then(|e| e.to_str()) {
            Some("txt") => "Text snapshot",
            Some("html") => "HTML (renderable; use Playwright for PNG)",
            Some("png") => "PNG screenshot",
            _ => "Snapshot",
        };
        report.push_str(&format!("| {} | {} | {} |\n", filename, path.display(), desc));
    }
    report.push('\n');

    report.push_str("To generate PNG screenshots from the `.html` snapshots:\n\n");
    report.push_str("```bash\n");
    report.push_str("node scripts/capture_codex_leetcode_screenshots.js\n");
    report.push_str("```\n\n");

    // Graph State Transitions
    report.push_str("## Graph State Transitions\n\n");
    report.push_str("### Execution Timeline\n\n");
    report.push_str("| Time | Event | Node | Details |\n");
    report.push_str("|------|-------|------|----------|\n");
    for event in &telemetry.events {
        // Parse the event string to extract components
        if let Some(idx) = event.find(']') {
            let time = &event[1..idx];
            let rest = &event[idx + 2..];
            if let Some(colon_idx) = rest.find(':') {
                let event_type = &rest[..colon_idx];
                let details = &rest[colon_idx + 2..];
                report.push_str(&format!("| {} | {} | - | {} |\n", time, event_type, details));
            } else {
                report.push_str(&format!("| {} | {} | - | - |\n", time, rest));
            }
        }
    }
    report.push('\n');

    // Execution Path
    report.push_str("### Execution Path\n\n");
    if !telemetry.execution_path.is_empty() {
        report.push_str(&format!("```\n{}\n```\n\n", telemetry.execution_path.join(" -> ")));
    } else {
        report.push_str("_No execution path captured_\n\n");
    }

    // Node Executions
    report.push_str("### Node Executions\n\n");
    report.push_str("| Node | Start | End | Duration | Status |\n");
    report.push_str("|------|-------|-----|----------|--------|\n");
    for exec in &telemetry.node_timeline {
        let end = exec.end_time.as_deref().unwrap_or("-");
        let duration = exec
            .duration
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string());
        let status = if exec.error.is_some() { "ERROR" } else { "OK" };
        report.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            exec.node, exec.start_time, end, duration, status
        ));
    }
    report.push('\n');

    // Edge Traversals
    report.push_str("### Edge Traversals\n\n");
    report.push_str("| Time | From | To | Type |\n");
    report.push_str("|------|------|----|------|\n");
    for edge in &telemetry.edge_traversals {
        report.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            edge.timestamp,
            edge.from,
            edge.to.join(", "),
            edge.edge_type
        ));
    }
    report.push('\n');

    // State Changes
    if !telemetry.state_changes.is_empty() {
        report.push_str("### State Changes\n\n");
        report.push_str("| Time | Node | Summary |\n");
        report.push_str("|------|------|--------|\n");
        for change in &telemetry.state_changes {
            report.push_str(&format!(
                "| {} | {} | {} |\n",
                change.timestamp, change.node, change.summary
            ));
        }
        report.push('\n');
    }

    // Final State Summary
    report.push_str("## Final State\n\n");
    report.push_str(&format!("**Message count:** {}\n\n", final_state.messages.len()));

    report.push_str("### Conversation\n\n");
    for (i, msg) in final_state.messages.iter().enumerate() {
        let role = msg.message_type();
        let content = msg.as_text();
        // Truncate long content for readability
        let display_content = if content.len() > 500 {
            format!("{}...[truncated]", &content[..500])
        } else {
            content.to_string()
        };
        report.push_str(&format!(
            "**[{}] {}:**\n```\n{}\n```\n\n",
            i, role, display_content
        ));
    }

    // Findings
    report.push_str("## Findings\n\n");
    report.push_str(&format!(
        "1. **Telemetry Visibility:** {} - {} events captured\n",
        if telemetry.events.len() > 5 { "PASS" } else { "FAIL" },
        telemetry.events.len()
    ));
    report.push_str(&format!(
        "2. **State Transitions:** {} - {} node executions tracked\n",
        if !telemetry.node_timeline.is_empty() { "PASS" } else { "FAIL" },
        telemetry.node_timeline.len()
    ));
    report.push_str(&format!(
        "3. **Edge Routing:** {} - {} edge traversals captured\n",
        if !telemetry.edge_traversals.is_empty() { "PASS" } else { "FAIL" },
        telemetry.edge_traversals.len()
    ));
    report.push_str(&format!(
        "4. **Conversation Tracking:** {} - {} messages in final state\n",
        if final_state.messages.len() >= 2 { "PASS" } else { "FAIL" },
        final_state.messages.len()
    ));

    // Conclusion
    report.push_str("\n## Conclusion\n\n");
    let all_pass = telemetry.events.len() > 5
        && !telemetry.node_timeline.is_empty()
        && !telemetry.edge_traversals.is_empty()
        && final_state.messages.len() >= 2;

    if all_pass {
        report.push_str("**SUCCESS:** DashFlow observability system provides comprehensive visibility into agent execution.\n\n");
        report.push_str("The test demonstrates:\n");
        report.push_str("- Graph events are captured for all node executions\n");
        report.push_str("- Edge traversals (including conditional routing) are logged\n");
        report.push_str("- State changes are tracked throughout execution\n");
        report.push_str("- Complete conversation history is preserved\n");
    } else {
        report.push_str("**PARTIAL:** Some observability features may need investigation.\n");
    }

    report.push_str("\n---\n\n");
    report.push_str("Generated with [Claude Code](https://claude.com/claude-code)\n");

    report
}

// =============================================================================
// Integration Test
// =============================================================================

/// Integration test: Codex DashFlow solves a LeetCode problem
///
/// This test:
/// 1. Presents the Two Sum problem to Codex DashFlow
/// 2. Captures all streaming telemetry events
/// 3. Saves snapshots to ~/Desktop
/// 4. Reads and reports on graph state transitions
/// 5. Writes observability report to reports/
#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored --nocapture"]
async fn test_codex_solves_leetcode_two_sum() {
    println!("\n=== LeetCode Integration Test: Two Sum ===\n");

    // LEET-001: Verify LLM availability
    if !llm_available() {
        eprintln!("Skipping test: No LLM API key available");
        eprintln!("Set OPENAI_API_KEY or ANTHROPIC_API_KEY to run this test");
        return;
    }

    let start_time: DateTime<Utc> = Utc::now();
    println!("Start time: {}", start_time.format("%Y-%m-%d %H:%M:%S UTC"));

    // Create LLM
    println!("\n[1/5] Creating LLM...");
    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    println!("  LLM created successfully");

    // LEET-001: Create the agent
    println!("\n[2/5] Creating Codex DashFlow agent...");
    let agent = create_coding_agent(model, None).expect("Should create agent");
    println!("  Agent created successfully");

    // LEET-002: Wire telemetry capture
    println!("\n[3/5] Wiring telemetry capture...");
    let callback = CollectingCallback::new();
    let callback_for_analysis = callback.shared_clone();
    let agent_with_telemetry = agent.with_callback(callback);
    println!("  CollectingCallback attached");

    // LEET-003: Capture graph start snapshot
    let mut telemetry_data = TelemetryData::default();
    let initial_snapshot = format!(
        "=== Codex DashFlow LeetCode Test ===\n\
         Start: {}\n\
         Problem: Two Sum\n\n\
         === Initial State ===\n\
         Agent: Ready\n\
         Telemetry: CollectingCallback attached\n\
         Events captured: 0\n",
        start_time.format("%Y-%m-%d %H:%M:%S UTC")
    );
    if let Ok((txt, html)) = capture_snapshot_pair(
        "codex_leetcode_graph_start",
        "Codex LeetCode: Graph Start",
        &initial_snapshot,
    ) {
        telemetry_data.snapshots.push(txt);
        telemetry_data.snapshots.push(html);
    }

    // Prepare initial state with the LeetCode problem
    let initial_state = AgentState::new(Message::human(LEETCODE_TWO_SUM_PROMPT));
    println!("  Initial state prepared with Two Sum problem");

    // LEET-004: Execute the agent and capture graph state transitions
    println!("\n[4/5] Running agent on Two Sum problem...");
    println!("  (This may take 30-60 seconds depending on LLM response time)");

    let result = agent_with_telemetry.invoke(initial_state).await;

    // Capture events immediately after execution
    let events = callback_for_analysis.events();
    println!("  Execution complete. Captured {} events.", events.len());

    // Analyze the collected events
    let mut analysis = analyze_events(&events);
    analysis.snapshots = telemetry_data.snapshots;

    // Handle result
    let final_state = match result {
        Ok(result) => {
            println!("  Agent completed successfully");
            println!("  Execution path: {}", result.execution_path().join(" -> "));
            println!("  Messages in final state: {}", result.final_state.messages.len());
            result.final_state
        }
        Err(e) => {
            eprintln!("  Agent execution failed: {}", e);
            // Create minimal state for report
            AgentState::new(Message::human(format!("ERROR: {}", e)))
        }
    };

    let final_messages: Vec<String> = final_state
        .messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let content = m.as_text();
            format!("[{i}] {}: {}", m.message_type(), truncate(&content, 200))
        })
        .collect();

    let mut message_type_counts: BTreeMap<String, usize> = BTreeMap::new();
    for msg in &final_state.messages {
        *message_type_counts
            .entry(msg.message_type().to_string())
            .or_insert(0) += 1;
    }
    let message_types_summary = if message_type_counts.is_empty() {
        "_none_".to_string()
    } else {
        message_type_counts
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    // LEET-003: Capture agent thinking snapshot (events + timeline)
    let timeline_lines = analysis
        .node_timeline
        .iter()
        .map(|n| {
            let duration = n
                .duration
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string());
            let status = if let Some(err) = &n.error {
                format!("ERROR: {err}")
            } else {
                "OK".to_string()
            };
            format!("{}: {} ({})", n.node, status, duration)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let state_change_lines = if analysis.state_changes.is_empty() {
        "_none_".to_string()
    } else {
        analysis
            .state_changes
            .iter()
            .map(|c| format!("[{}] {} - {}", c.timestamp, c.node, c.summary))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let thinking_snapshot = format!(
        "=== Codex DashFlow LeetCode Test ===\n\
         Phase: Agent Thinking (post-run analysis)\n\
         Captured: {}\n\n\
         === Event Log ===\n\
         {}\n\n\
         === Execution Path ===\n\
         {}\n\n\
         === Node Timeline ===\n\
         {}\n\n\
         === State Changes ===\n\
         {}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        analysis.events.join("\n"),
        analysis.execution_path.join(" -> "),
        timeline_lines,
        state_change_lines
    );
    if let Ok((txt, html)) = capture_snapshot_pair(
        "codex_leetcode_agent_thinking",
        "Codex LeetCode: Agent Thinking",
        &thinking_snapshot,
    ) {
        analysis.snapshots.push(txt);
        analysis.snapshots.push(html);
    }

    // LEET-003: Capture tool call snapshot (best-effort)
    let tool_node_hits = analysis.node_timeline.iter().any(|n| {
        let node = n.node.to_lowercase();
        node.contains("tool")
    });
    let tool_call_snapshot = format!(
        "=== Codex DashFlow LeetCode Test ===\n\
         Phase: Tool Calls (best-effort)\n\
         Captured: {}\n\n\
         Tools node observed: {}\n\
         Message types: {}\n\n\
         === Notes ===\n\
         - This test surfaces tool usage via graph nodes and message types.\n\
         - If the model answers directly, tool usage will be absent.\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        tool_node_hits,
        message_types_summary
    );
    if let Ok((txt, html)) = capture_snapshot_pair(
        "codex_leetcode_tool_calls",
        "Codex LeetCode: Tool Calls",
        &tool_call_snapshot,
    ) {
        analysis.snapshots.push(txt);
        analysis.snapshots.push(html);
    }

    // LEET-003: Capture streaming snapshot (conversation transcript)
    let transcript = final_state
        .messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let text = m.as_text();
            let content = truncate(&text, 4000);
            format!("[{i}] {}:\n{content}\n", m.message_type())
        })
        .collect::<Vec<_>>()
        .join("\n");
    let streaming_snapshot = format!(
        "=== Codex DashFlow LeetCode Test ===\n\
         Phase: Streaming (transcript snapshot)\n\
         Captured: {}\n\n\
         === Notes ===\n\
         - Token-by-token streaming is not captured in GraphEvents.\n\
         - This snapshot records the final transcript emitted by the model.\n\n\
         === Transcript ===\n\
         {}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        transcript
    );
    if let Ok((txt, html)) = capture_snapshot_pair(
        "codex_leetcode_streaming",
        "Codex LeetCode: Streaming",
        &streaming_snapshot,
    ) {
        analysis.snapshots.push(txt);
        analysis.snapshots.push(html);
    }

    // LEET-003: Capture final state snapshot
    let final_snapshot = format!(
        "=== Codex DashFlow LeetCode Test ===\n\
         Completed: {}\n\
         Duration: {}\n\n\
         === Final State ===\n\
         Message count: {}\n\n\
         === Messages ===\n\
         {}\n\n\
         === Telemetry Summary ===\n\
         Total events: {}\n\
         Node executions: {}\n\
         Edge traversals: {}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        analysis
            .total_duration
            .map(format_duration)
            .unwrap_or_else(|| "unknown".to_string()),
        final_state.messages.len(),
        final_messages.join("\n"),
        analysis.events.len(),
        analysis.node_timeline.len(),
        analysis.edge_traversals.len()
    );
    if let Ok((txt, html)) = capture_snapshot_pair(
        "codex_leetcode_final_state",
        "Codex LeetCode: Final State",
        &final_snapshot,
    ) {
        analysis.snapshots.push(txt);
        analysis.snapshots.push(html);
    }

    // LEET-005: Generate and save observability report
    println!("\n[5/5] Generating observability report...");
    let report = generate_report(&analysis, &final_state, start_time);

    // Save report
    let reports_dir = get_reports_path();
    std::fs::create_dir_all(&reports_dir).ok();

    let report_filename = format!(
        "codex_leetcode_observability_{}.md",
        start_time.format("%Y-%m-%d-%H-%M")
    );
    let report_path = reports_dir.join(&report_filename);
    std::fs::write(&report_path, &report).expect("Failed to write report");
    println!("  Report saved: {}", report_path.display());

    // Print summary
    println!("\n=== Test Summary ===");
    println!("Events captured: {}", analysis.events.len());
    println!("Node executions: {}", analysis.node_timeline.len());
    println!("Edge traversals: {}", analysis.edge_traversals.len());
    println!("Final messages: {}", final_state.messages.len());
    println!("\nSnapshots saved to: {}", get_desktop_path().display());
    println!("Report saved to: {}", report_path.display());

    // Assertions
    assert!(
        analysis.events.len() >= 2,
        "Should capture at least GraphStart and GraphEnd events"
    );
    assert!(
        !analysis.node_timeline.is_empty(),
        "Should capture at least one node execution"
    );
    assert!(
        final_state.messages.len() >= 2,
        "Should have at least human query and AI response"
    );

    println!("\n=== Test Complete ===\n");
}

/// Minimal test to verify telemetry infrastructure without LLM
#[tokio::test]
async fn test_telemetry_infrastructure() {
    use dashflow::{StateGraph, END};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestState {
        value: i32,
    }

    impl dashflow::MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = other.value;
        }
    }

    // Build a simple test graph
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("increment", |mut state: TestState| {
        Box::pin(async move {
            state.value += 1;
            Ok(state)
        })
    });

    graph.set_entry_point("increment");
    graph.add_edge("increment", END);

    // Compile with callback
    let compiled = graph.compile().expect("Should compile");
    let callback = CollectingCallback::new();
    let callback_clone = callback.shared_clone();
    let app = compiled.with_callback(callback);

    // Execute
    let initial = TestState { value: 0 };
    let result = app.invoke(initial).await.expect("Should execute");

    // Verify
    assert_eq!(result.final_state.value, 1);

    let events = callback_clone.events();
    assert!(!events.is_empty(), "Should capture events");

    // Check for expected event types
    let has_graph_start = events.iter().any(|e| matches!(e, GraphEvent::GraphStart { .. }));
    let has_node_start = events.iter().any(|e| matches!(e, GraphEvent::NodeStart { node, .. } if node == "increment"));
    let has_node_end = events.iter().any(|e| matches!(e, GraphEvent::NodeEnd { node, .. } if node == "increment"));
    let has_graph_end = events.iter().any(|e| matches!(e, GraphEvent::GraphEnd { .. }));

    assert!(has_graph_start, "Should have GraphStart event");
    assert!(has_node_start, "Should have NodeStart event for 'increment'");
    assert!(has_node_end, "Should have NodeEnd event for 'increment'");
    assert!(has_graph_end, "Should have GraphEnd event");

    println!("Telemetry infrastructure test passed. Captured {} events.", events.len());
}
