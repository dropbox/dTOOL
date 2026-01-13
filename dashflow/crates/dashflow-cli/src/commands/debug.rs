// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Interactive debugger command - Step-through graph execution with web UI

use crate::output::{print_info, print_success, print_warning};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Interactive debugger for DashFlow graphs
#[derive(Args)]
pub struct DebugArgs {
    #[command(subcommand)]
    pub command: DebugCommand,
}

#[derive(Subcommand)]
pub enum DebugCommand {
    /// Start interactive debugging server with web UI
    Serve(ServeArgs),

    /// Inspect a debug session file
    Inspect(InspectArgs),
}

#[derive(Args)]
pub struct ServeArgs {
    /// Port to serve on
    #[arg(short, long, default_value = "8766")]
    pub port: u16,

    /// Open browser automatically
    #[arg(long, default_value = "true")]
    pub open: bool,

    /// Allow connections from any IP (not just localhost)
    #[arg(long, default_value = "false")]
    pub public: bool,
}

#[derive(Args)]
pub struct InspectArgs {
    /// Path to debug session JSON file
    #[arg(required = true)]
    pub file: PathBuf,
}

/// Debug session state shared across requests
#[derive(Clone, Default)]
struct DebugSession {
    /// Unique session ID
    id: String,
    /// Graph definition (Mermaid or JSON)
    graph: String,
    /// List of nodes in the graph
    nodes: Vec<String>,
    /// Current execution position (node name)
    current_node: Option<String>,
    /// Nodes that have been executed
    executed_nodes: Vec<String>,
    /// Breakpoints (node names)
    breakpoints: HashSet<String>,
    /// State at each execution step (node -> state JSON)
    state_history: HashMap<String, String>,
    /// Current state (JSON)
    current_state: String,
    /// Is execution paused?
    paused: bool,
    /// Execution complete?
    completed: bool,
    /// Error message if any
    error: Option<String>,
}

/// Run the debug command
pub async fn run(args: DebugArgs) -> Result<()> {
    match args.command {
        DebugCommand::Serve(serve_args) => run_serve(serve_args).await,
        DebugCommand::Inspect(inspect_args) => run_inspect(inspect_args).await,
    }
}

async fn run_serve(args: ServeArgs) -> Result<()> {
    let bind_addr = if args.public {
        format!("0.0.0.0:{}", args.port)
    } else {
        format!("127.0.0.1:{}", args.port)
    };

    print_info(&format!(
        "Starting interactive debugger on http://{}",
        bind_addr
    ));
    print_info("Features:");
    print_info("  - Upload graph definitions (Mermaid/JSON)");
    print_info("  - Set breakpoints on nodes");
    print_info("  - Step-through execution");
    print_info("  - Inspect state at each step");
    print_info("  - Visual execution path");

    let listener =
        TcpListener::bind(&bind_addr).context("Failed to bind to address. Is the port in use?")?;

    // Create shared debug session
    let session = Arc::new(Mutex::new(DebugSession::default()));

    if args.open {
        let url = format!("http://127.0.0.1:{}", args.port);
        if let Err(e) = webbrowser::open(&url) {
            print_warning(&format!("Could not open browser: {e}"));
            print_info(&format!("Please open {} manually", url));
        }
    }

    print_success(&format!("Debugger running at http://{}", bind_addr));
    println!("Press Ctrl+C to stop\n");

    // Handle HTTP requests
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let session_clone = Arc::clone(&session);

                // Read request
                let mut reader = BufReader::new(&stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }

                // Parse method and path
                let parts: Vec<&str> = request_line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }

                let method = parts[0];
                let path = parts[1];

                // Read headers to get content length
                let mut content_length = 0;
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).is_err() || line == "\r\n" || line == "\n" {
                        break;
                    }
                    if line.to_lowercase().starts_with("content-length:") {
                        if let Some(len_str) = line.split(':').nth(1) {
                            content_length = len_str.trim().parse().unwrap_or(0);
                        }
                    }
                }

                // Read body if present
                let mut body = vec![0u8; content_length];
                if content_length > 0 {
                    if let Err(e) = reader.read_exact(&mut body) {
                        print_warning(&format!("Failed to read request body: {e}"));
                        body.clear(); // Use empty body on read failure
                    }
                }
                let body_str = String::from_utf8_lossy(&body).to_string();

                // Route request
                let response = match (method, path) {
                    ("GET", "/") => serve_main_html(),
                    ("GET", "/api/session") => get_session_json(&session_clone),
                    ("POST", "/api/graph") => set_graph(&session_clone, &body_str),
                    ("POST", "/api/breakpoint") => toggle_breakpoint(&session_clone, &body_str),
                    ("POST", "/api/step") => step_execution(&session_clone),
                    ("POST", "/api/continue") => continue_execution(&session_clone),
                    ("POST", "/api/reset") => reset_session(&session_clone),
                    ("POST", "/api/state") => set_state(&session_clone, &body_str),
                    _ => not_found_response(),
                };

                if let Err(e) = stream.write_all(response.as_bytes()) {
                    print_warning(&format!("Failed to send response: {e}"));
                }
            }
            Err(e) => {
                print_warning(&format!("Connection error: {e}"));
            }
        }
    }

    Ok(())
}

async fn run_inspect(args: InspectArgs) -> Result<()> {
    let content = tokio::fs::read_to_string(&args.file)
        .await
        .with_context(|| format!("Failed to read file: {:?}", args.file))?;

    let session: serde_json::Value =
        serde_json::from_str(&content).context("Invalid JSON in debug session file")?;

    println!("Debug Session Inspection");
    println!("========================\n");

    if let Some(id) = session.get("id").and_then(|v| v.as_str()) {
        println!("Session ID: {}", id);
    }

    if let Some(nodes) = session.get("nodes").and_then(|v| v.as_array()) {
        println!("\nNodes ({}):", nodes.len());
        for node in nodes {
            if let Some(name) = node.as_str() {
                println!("  - {}", name);
            }
        }
    }

    if let Some(executed) = session.get("executed_nodes").and_then(|v| v.as_array()) {
        println!("\nExecution Path ({} steps):", executed.len());
        for (i, node) in executed.iter().enumerate() {
            if let Some(name) = node.as_str() {
                println!("  {}. {}", i + 1, name);
            }
        }
    }

    if let Some(breakpoints) = session.get("breakpoints").and_then(|v| v.as_array()) {
        if !breakpoints.is_empty() {
            println!("\nBreakpoints:");
            for bp in breakpoints {
                if let Some(name) = bp.as_str() {
                    println!("  - {}", name);
                }
            }
        }
    }

    if let Some(state) = session.get("current_state") {
        println!("\nCurrent State:");
        println!(
            "{}",
            serde_json::to_string_pretty(state).unwrap_or_else(|_| state.to_string())
        );
    }

    if let Some(error) = session.get("error").and_then(|v| v.as_str()) {
        println!("\nError: {}", error);
    }

    Ok(())
}

// API handlers

fn get_session_json(session: &Arc<Mutex<DebugSession>>) -> String {
    let session = session.lock().expect("debug session mutex poisoned");
    let json = serde_json::json!({
        "id": session.id,
        "graph": session.graph,
        "nodes": session.nodes,
        "current_node": session.current_node,
        "executed_nodes": session.executed_nodes,
        "breakpoints": session.breakpoints.iter().collect::<Vec<_>>(),
        "state_history": session.state_history,
        "current_state": session.current_state,
        "paused": session.paused,
        "completed": session.completed,
        "error": session.error,
    });

    http_json_response(&json.to_string())
}

fn set_graph(session: &Arc<Mutex<DebugSession>>, body: &str) -> String {
    let mut session = session.lock().expect("debug session mutex poisoned");

    // Parse the request body
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(graph) = data.get("graph").and_then(|v| v.as_str()) {
            session.graph = graph.to_string();
            session.id = uuid::Uuid::new_v4().to_string();

            // Extract nodes from the graph
            session.nodes = extract_nodes_from_graph(graph);

            // Reset execution state
            session.current_node = None;
            session.executed_nodes.clear();
            session.state_history.clear();
            session.paused = false;
            session.completed = false;
            session.error = None;

            // Set initial state if provided
            if let Some(state) = data.get("initial_state") {
                session.current_state = state.to_string();
            } else {
                session.current_state = "{}".to_string();
            }

            return http_json_response(r#"{"success": true}"#);
        }
    }

    http_json_response(r#"{"success": false, "error": "Invalid request body"}"#)
}

fn toggle_breakpoint(session: &Arc<Mutex<DebugSession>>, body: &str) -> String {
    let mut session = session.lock().expect("debug session mutex poisoned");

    if let Ok(data) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(node) = data.get("node").and_then(|v| v.as_str()) {
            if session.breakpoints.contains(node) {
                session.breakpoints.remove(node);
            } else {
                session.breakpoints.insert(node.to_string());
            }
            return http_json_response(r#"{"success": true}"#);
        }
    }

    http_json_response(r#"{"success": false, "error": "Invalid request body"}"#)
}

fn step_execution(session: &Arc<Mutex<DebugSession>>) -> String {
    let mut session = session.lock().expect("debug session mutex poisoned");

    if session.completed {
        return http_json_response(r#"{"success": false, "error": "Execution already completed"}"#);
    }

    if session.nodes.is_empty() {
        return http_json_response(r#"{"success": false, "error": "No graph loaded"}"#);
    }

    // Simulate stepping to the next node
    let next_index = session.executed_nodes.len();

    if next_index >= session.nodes.len() {
        session.completed = true;
        session.current_node = None;
        return http_json_response(r#"{"success": true, "completed": true}"#);
    }

    let next_node = session.nodes[next_index].clone();

    // Save current state for this node
    let current_state_clone = session.current_state.clone();
    session
        .state_history
        .insert(next_node.clone(), current_state_clone);

    // Update execution state
    session.current_node = Some(next_node.clone());
    session.executed_nodes.push(next_node.clone());

    // Update state (simulated - in real use this would run the actual node)
    if let Ok(mut state) = serde_json::from_str::<serde_json::Value>(&session.current_state) {
        if let Some(obj) = state.as_object_mut() {
            obj.insert(
                "last_node".to_string(),
                serde_json::Value::String(next_node.clone()),
            );
            obj.insert(
                "step_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(session.executed_nodes.len())),
            );
            session.current_state = state.to_string();
        }
    }

    // Check if we hit a breakpoint
    if session.breakpoints.contains(&next_node) {
        session.paused = true;
    }

    http_json_response(r#"{"success": true}"#)
}

fn continue_execution(session: &Arc<Mutex<DebugSession>>) -> String {
    let mut session = session.lock().expect("debug session mutex poisoned");

    if session.completed {
        return http_json_response(r#"{"success": false, "error": "Execution already completed"}"#);
    }

    session.paused = false;

    // Continue until breakpoint or end
    loop {
        let next_index = session.executed_nodes.len();

        if next_index >= session.nodes.len() {
            session.completed = true;
            session.current_node = None;
            break;
        }

        let next_node = session.nodes[next_index].clone();

        // Save and execute
        let current_state_clone = session.current_state.clone();
        session
            .state_history
            .insert(next_node.clone(), current_state_clone);
        session.current_node = Some(next_node.clone());
        session.executed_nodes.push(next_node.clone());

        // Update state
        if let Ok(mut state) = serde_json::from_str::<serde_json::Value>(&session.current_state) {
            if let Some(obj) = state.as_object_mut() {
                obj.insert(
                    "last_node".to_string(),
                    serde_json::Value::String(next_node.clone()),
                );
                obj.insert(
                    "step_count".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(
                        session.executed_nodes.len(),
                    )),
                );
                session.current_state = state.to_string();
            }
        }

        // Check for breakpoint
        if session.breakpoints.contains(&next_node) {
            session.paused = true;
            break;
        }
    }

    http_json_response(r#"{"success": true}"#)
}

fn reset_session(session: &Arc<Mutex<DebugSession>>) -> String {
    let mut session = session.lock().expect("debug session mutex poisoned");

    // Keep graph and breakpoints, reset execution
    session.current_node = None;
    session.executed_nodes.clear();
    session.state_history.clear();
    session.current_state = "{}".to_string();
    session.paused = false;
    session.completed = false;
    session.error = None;

    http_json_response(r#"{"success": true}"#)
}

fn set_state(session: &Arc<Mutex<DebugSession>>, body: &str) -> String {
    let mut session = session.lock().expect("debug session mutex poisoned");

    if let Ok(data) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(state) = data.get("state") {
            session.current_state = state.to_string();
            return http_json_response(r#"{"success": true}"#);
        }
    }

    http_json_response(r#"{"success": false, "error": "Invalid request body"}"#)
}

// Helper functions

fn extract_nodes_from_graph(graph: &str) -> Vec<String> {
    let mut nodes = Vec::new();
    let mut seen = HashSet::new();

    for line in graph.lines() {
        let line = line.trim();

        // Skip flowchart/graph declarations and empty lines
        if line.is_empty()
            || line.starts_with("flowchart")
            || line.starts_with("graph")
            || line.starts_with("classDef")
            || line.starts_with("class ")
            || line.starts_with("%%")
        {
            continue;
        }

        // Extract node names from edges (A --> B, A -->|label| B, etc.)
        // Also handles node definitions like A[Label] or A([Label])
        let node_pattern = |s: &str| -> Option<String> {
            // Remove brackets and labels
            let name = s
                .split('[')
                .next()?
                .split('(')
                .next()?
                .split('{')
                .next()?
                .trim();

            if name.is_empty()
                || name == "END"
                || name == "End"
                || name == "__end__"
                || name.to_lowercase() == "end"
            {
                return None;
            }

            Some(name.to_string())
        };

        // Split by edge markers
        let parts: Vec<&str> = line
            .split("-->")
            .flat_map(|s| s.split("==>"))
            .flat_map(|s| s.split("-.->"))
            .collect();

        for part in parts {
            // Handle labels like |condition|
            let clean = part.split('|').next().unwrap_or(part).trim();
            if let Some(name) = node_pattern(clean) {
                if !seen.contains(&name) {
                    seen.insert(name.clone());
                    nodes.push(name);
                }
            }
        }
    }

    nodes
}

fn http_json_response(json: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Content-Length: {}\r\n\r\n{}",
        json.len(),
        json
    )
}

fn not_found_response() -> String {
    let body = r#"{"error": "Not found"}"#;
    format!(
        "HTTP/1.1 404 Not Found\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn serve_main_html() -> String {
    let html = generate_debugger_html();
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\r\n{}",
        html.len(),
        html
    )
}

fn generate_debugger_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DashFlow Interactive Debugger</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        :root {
            --bg-primary: #1a1a2e;
            --bg-secondary: #16213e;
            --bg-tertiary: #0f0f1a;
            --text-primary: #e4e4e7;
            --text-secondary: #a1a1aa;
            --border-color: #3f3f46;
            --accent-color: #818cf8;
            --accent-hover: #6366f1;
            --success-color: #22c55e;
            --warning-color: #f59e0b;
            --error-color: #ef4444;
            --breakpoint-color: #ef4444;
            --current-color: #22c55e;
            --executed-color: #818cf8;
        }

        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            min-height: 100vh;
        }

        .header {
            background: linear-gradient(135deg, var(--accent-color), var(--accent-hover));
            color: white;
            padding: 15px 25px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .header h1 {
            font-size: 22px;
            font-weight: 600;
        }

        .header-controls {
            display: flex;
            gap: 10px;
        }

        .btn {
            padding: 8px 16px;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-size: 13px;
            font-weight: 500;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            gap: 6px;
        }

        .btn-primary {
            background: rgba(255,255,255,0.2);
            color: white;
        }

        .btn-primary:hover {
            background: rgba(255,255,255,0.3);
        }

        .btn-success {
            background: var(--success-color);
            color: white;
        }

        .btn-success:hover {
            opacity: 0.9;
        }

        .btn-warning {
            background: var(--warning-color);
            color: white;
        }

        .btn-secondary {
            background: var(--bg-secondary);
            color: var(--text-primary);
            border: 1px solid var(--border-color);
        }

        .btn-secondary:hover {
            background: var(--bg-tertiary);
        }

        .btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }

        .main-container {
            display: grid;
            grid-template-columns: 350px 1fr 350px;
            height: calc(100vh - 60px);
        }

        .panel {
            background: var(--bg-secondary);
            border-right: 1px solid var(--border-color);
            padding: 20px;
            overflow-y: auto;
        }

        .panel:last-child {
            border-right: none;
            border-left: 1px solid var(--border-color);
        }

        .panel h2 {
            font-size: 14px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            color: var(--text-secondary);
            margin-bottom: 15px;
        }

        .graph-panel {
            background: var(--bg-tertiary);
            display: flex;
            flex-direction: column;
            padding: 0;
        }

        .graph-toolbar {
            background: var(--bg-secondary);
            padding: 10px 15px;
            display: flex;
            gap: 10px;
            align-items: center;
            border-bottom: 1px solid var(--border-color);
        }

        .graph-container {
            flex: 1;
            overflow: auto;
            padding: 20px;
            display: flex;
            justify-content: center;
            align-items: flex-start;
        }

        .mermaid-wrapper {
            background: var(--bg-secondary);
            border-radius: 8px;
            padding: 20px;
            min-width: 300px;
        }

        textarea {
            width: 100%;
            padding: 12px;
            border: 1px solid var(--border-color);
            border-radius: 6px;
            background: var(--bg-tertiary);
            color: var(--text-primary);
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 12px;
            resize: vertical;
            min-height: 200px;
        }

        textarea:focus {
            outline: none;
            border-color: var(--accent-color);
        }

        .node-list {
            margin-top: 15px;
        }

        .node-item {
            display: flex;
            align-items: center;
            gap: 10px;
            padding: 10px 12px;
            background: var(--bg-tertiary);
            border-radius: 6px;
            margin-bottom: 8px;
            cursor: pointer;
            transition: all 0.2s;
        }

        .node-item:hover {
            background: var(--bg-primary);
        }

        .node-item.current {
            border-left: 3px solid var(--current-color);
            background: rgba(34, 197, 94, 0.1);
        }

        .node-item.executed {
            border-left: 3px solid var(--executed-color);
        }

        .node-item.breakpoint {
            border-right: 3px solid var(--breakpoint-color);
        }

        .node-name {
            flex: 1;
            font-size: 13px;
        }

        .node-status {
            font-size: 11px;
            padding: 2px 8px;
            border-radius: 10px;
            background: var(--bg-secondary);
        }

        .node-status.current {
            background: var(--current-color);
            color: white;
        }

        .node-status.executed {
            background: var(--executed-color);
            color: white;
        }

        .breakpoint-toggle {
            width: 16px;
            height: 16px;
            border-radius: 50%;
            border: 2px solid var(--border-color);
            cursor: pointer;
        }

        .breakpoint-toggle.active {
            background: var(--breakpoint-color);
            border-color: var(--breakpoint-color);
        }

        .state-panel .state-viewer {
            background: var(--bg-tertiary);
            border-radius: 6px;
            padding: 15px;
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 12px;
            white-space: pre-wrap;
            word-break: break-all;
            max-height: 300px;
            overflow-y: auto;
        }

        .execution-info {
            margin-top: 20px;
        }

        .info-row {
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid var(--border-color);
            font-size: 13px;
        }

        .info-label {
            color: var(--text-secondary);
        }

        .info-value {
            font-weight: 500;
        }

        .info-value.paused {
            color: var(--warning-color);
        }

        .info-value.completed {
            color: var(--success-color);
        }

        .info-value.running {
            color: var(--accent-color);
        }

        .step-history {
            margin-top: 20px;
        }

        .step-item {
            display: flex;
            align-items: center;
            gap: 10px;
            padding: 8px 10px;
            background: var(--bg-tertiary);
            border-radius: 4px;
            margin-bottom: 4px;
            font-size: 12px;
        }

        .step-number {
            width: 24px;
            height: 24px;
            background: var(--executed-color);
            color: white;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 11px;
            font-weight: 600;
        }

        .empty-state {
            text-align: center;
            padding: 40px 20px;
            color: var(--text-secondary);
        }

        .empty-state-icon {
            font-size: 48px;
            margin-bottom: 15px;
            opacity: 0.5;
        }

        .status-badge {
            display: inline-flex;
            align-items: center;
            gap: 5px;
            padding: 4px 10px;
            border-radius: 12px;
            font-size: 12px;
            font-weight: 500;
        }

        .status-badge.paused {
            background: rgba(245, 158, 11, 0.2);
            color: var(--warning-color);
        }

        .status-badge.running {
            background: rgba(129, 140, 248, 0.2);
            color: var(--accent-color);
        }

        .status-badge.completed {
            background: rgba(34, 197, 94, 0.2);
            color: var(--success-color);
        }

        @media (max-width: 1200px) {
            .main-container {
                grid-template-columns: 1fr;
            }
            .panel {
                border: none;
                border-bottom: 1px solid var(--border-color);
            }
        }
    </style>
</head>
<body>
    <header class="header">
        <h1>DashFlow Interactive Debugger</h1>
        <div class="header-controls">
            <span id="status-badge" class="status-badge">Ready</span>
        </div>
    </header>

    <div class="main-container">
        <!-- Left Panel: Graph Input & Nodes -->
        <div class="panel">
            <h2>Graph Definition</h2>
            <textarea id="graph-input" placeholder="Paste your Mermaid graph here...

flowchart TD
    Start([Start]) --> research[Research]
    research --> analyze[Analyze]
    analyze --> write[Write Report]
    write --> End([End])"></textarea>
            <button class="btn btn-primary" style="margin-top: 10px; width: 100%;" onclick="loadGraph()">
                Load Graph
            </button>

            <h2 style="margin-top: 25px;">Nodes</h2>
            <div id="node-list" class="node-list">
                <div class="empty-state">
                    <div class="empty-state-icon">📊</div>
                    <p>Load a graph to see nodes</p>
                </div>
            </div>
        </div>

        <!-- Center: Graph Visualization -->
        <div class="graph-panel">
            <div class="graph-toolbar">
                <button class="btn btn-success" id="step-btn" onclick="stepExecution()" disabled>
                    ▶ Step
                </button>
                <button class="btn btn-warning" id="continue-btn" onclick="continueExecution()" disabled>
                    ⏩ Continue
                </button>
                <button class="btn btn-secondary" id="reset-btn" onclick="resetSession()" disabled>
                    ↺ Reset
                </button>
                <div style="flex: 1;"></div>
                <button class="btn btn-secondary" onclick="exportSession()">
                    📥 Export Session
                </button>
            </div>
            <div class="graph-container">
                <div class="mermaid-wrapper" id="mermaid-wrapper">
                    <div class="empty-state">
                        <div class="empty-state-icon">🔍</div>
                        <p>Load a graph to visualize</p>
                    </div>
                </div>
            </div>
        </div>

        <!-- Right Panel: State & Execution -->
        <div class="panel state-panel">
            <h2>Current State</h2>
            <div class="state-viewer" id="state-viewer">
                {
                  "message": "Load a graph and start stepping"
                }
            </div>

            <div class="execution-info">
                <h2>Execution Info</h2>
                <div class="info-row">
                    <span class="info-label">Status</span>
                    <span class="info-value" id="exec-status">Ready</span>
                </div>
                <div class="info-row">
                    <span class="info-label">Current Node</span>
                    <span class="info-value" id="exec-node">-</span>
                </div>
                <div class="info-row">
                    <span class="info-label">Steps Executed</span>
                    <span class="info-value" id="exec-steps">0</span>
                </div>
                <div class="info-row">
                    <span class="info-label">Breakpoints</span>
                    <span class="info-value" id="exec-breakpoints">0</span>
                </div>
            </div>

            <div class="step-history">
                <h2>Execution Path</h2>
                <div id="step-list">
                    <div class="empty-state" style="padding: 20px;">
                        <p>No steps executed yet</p>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <script>
        let session = {
            id: '',
            graph: '',
            nodes: [],
            current_node: null,
            executed_nodes: [],
            breakpoints: [],
            state_history: {},
            current_state: '{}',
            paused: false,
            completed: false,
            error: null
        };

        // Initialize Mermaid
        mermaid.initialize({
            startOnLoad: false,
            theme: 'dark',
            flowchart: {
                useMaxWidth: true,
                htmlLabels: true,
                curve: 'basis'
            }
        });

        async function loadGraph() {
            const graphInput = document.getElementById('graph-input').value.trim();
            if (!graphInput) {
                alert('Please enter a graph definition');
                return;
            }

            try {
                const response = await fetch('/api/graph', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ graph: graphInput, initial_state: {} })
                });

                if (response.ok) {
                    await refreshSession();
                    renderGraph();
                }
            } catch (e) {
                console.error('Failed to load graph:', e);
            }
        }

        async function refreshSession() {
            try {
                const response = await fetch('/api/session');
                session = await response.json();
                updateUI();
            } catch (e) {
                console.error('Failed to refresh session:', e);
            }
        }

        async function stepExecution() {
            try {
                await fetch('/api/step', { method: 'POST' });
                await refreshSession();
                renderGraph();
            } catch (e) {
                console.error('Failed to step:', e);
            }
        }

        async function continueExecution() {
            try {
                await fetch('/api/continue', { method: 'POST' });
                await refreshSession();
                renderGraph();
            } catch (e) {
                console.error('Failed to continue:', e);
            }
        }

        async function resetSession() {
            try {
                await fetch('/api/reset', { method: 'POST' });
                await refreshSession();
                renderGraph();
            } catch (e) {
                console.error('Failed to reset:', e);
            }
        }

        async function toggleBreakpoint(node) {
            try {
                await fetch('/api/breakpoint', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ node })
                });
                await refreshSession();
            } catch (e) {
                console.error('Failed to toggle breakpoint:', e);
            }
        }

        function renderGraph() {
            const wrapper = document.getElementById('mermaid-wrapper');

            if (!session.graph) {
                wrapper.innerHTML = `
                    <div class="empty-state">
                        <div class="empty-state-icon">🔍</div>
                        <p>Load a graph to visualize</p>
                    </div>
                `;
                return;
            }

            // Add styling to graph based on execution state
            let styledGraph = session.graph;

            // Add style definitions
            let styles = `
    classDef executed fill:#4f46e5,stroke:#818cf8,color:white
    classDef current fill:#16a34a,stroke:#22c55e,color:white
    classDef breakpoint stroke:#ef4444,stroke-width:3px
`;

            // Apply classes
            const executedNodes = session.executed_nodes.filter(n => n !== session.current_node);
            if (executedNodes.length > 0) {
                styles += `    class ${executedNodes.join(',')} executed\n`;
            }
            if (session.current_node) {
                styles += `    class ${session.current_node} current\n`;
            }
            if (session.breakpoints.length > 0) {
                styles += `    class ${session.breakpoints.join(',')} breakpoint\n`;
            }

            styledGraph = styledGraph + '\n' + styles;

            wrapper.innerHTML = `<pre class="mermaid">${styledGraph}</pre>`;
            mermaid.init(undefined, wrapper.querySelector('.mermaid'));
        }

        function updateUI() {
            // Update node list
            const nodeList = document.getElementById('node-list');
            if (session.nodes.length === 0) {
                nodeList.innerHTML = `
                    <div class="empty-state">
                        <div class="empty-state-icon">📊</div>
                        <p>Load a graph to see nodes</p>
                    </div>
                `;
            } else {
                nodeList.innerHTML = session.nodes.map(node => {
                    const isExecuted = session.executed_nodes.includes(node);
                    const isCurrent = session.current_node === node;
                    const isBreakpoint = session.breakpoints.includes(node);

                    let classes = ['node-item'];
                    if (isCurrent) classes.push('current');
                    else if (isExecuted) classes.push('executed');
                    if (isBreakpoint) classes.push('breakpoint');

                    let status = '';
                    if (isCurrent) status = '<span class="node-status current">Current</span>';
                    else if (isExecuted) status = '<span class="node-status executed">Done</span>';

                    return `
                        <div class="${classes.join(' ')}" onclick="toggleBreakpoint('${node}')">
                            <div class="breakpoint-toggle ${isBreakpoint ? 'active' : ''}" title="Toggle breakpoint"></div>
                            <span class="node-name">${node}</span>
                            ${status}
                        </div>
                    `;
                }).join('');
            }

            // Update state viewer
            const stateViewer = document.getElementById('state-viewer');
            try {
                const state = JSON.parse(session.current_state);
                stateViewer.textContent = JSON.stringify(state, null, 2);
            } catch {
                stateViewer.textContent = session.current_state;
            }

            // Update execution info
            let statusClass = 'running';
            let statusText = 'Ready';
            if (session.completed) {
                statusClass = 'completed';
                statusText = 'Completed';
            } else if (session.paused) {
                statusClass = 'paused';
                statusText = 'Paused (Breakpoint)';
            } else if (session.executed_nodes.length > 0) {
                statusText = 'Running';
            }

            document.getElementById('exec-status').className = `info-value ${statusClass}`;
            document.getElementById('exec-status').textContent = statusText;
            document.getElementById('exec-node').textContent = session.current_node || '-';
            document.getElementById('exec-steps').textContent = session.executed_nodes.length;
            document.getElementById('exec-breakpoints').textContent = session.breakpoints.length;

            // Update status badge
            const badge = document.getElementById('status-badge');
            badge.className = `status-badge ${statusClass}`;
            badge.textContent = statusText;

            // Update step history
            const stepList = document.getElementById('step-list');
            if (session.executed_nodes.length === 0) {
                stepList.innerHTML = `
                    <div class="empty-state" style="padding: 20px;">
                        <p>No steps executed yet</p>
                    </div>
                `;
            } else {
                stepList.innerHTML = session.executed_nodes.map((node, i) => `
                    <div class="step-item">
                        <div class="step-number">${i + 1}</div>
                        <span>${node}</span>
                    </div>
                `).join('');
            }

            // Update button states
            const hasGraph = session.nodes.length > 0;
            const canStep = hasGraph && !session.completed;

            document.getElementById('step-btn').disabled = !canStep;
            document.getElementById('continue-btn').disabled = !canStep;
            document.getElementById('reset-btn').disabled = !hasGraph;
        }

        function exportSession() {
            const data = JSON.stringify(session, null, 2);
            const blob = new Blob([data], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `debug-session-${Date.now()}.json`;
            a.click();
            URL.revokeObjectURL(url);
        }

        // Initial load
        refreshSession();
    </script>
</body>
</html>"##.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_nodes_simple() {
        let graph = r#"
flowchart TD
    Start --> A
    A --> B
    B --> End
"#;
        let nodes = extract_nodes_from_graph(graph);
        assert_eq!(nodes, vec!["Start", "A", "B"]);
    }

    #[test]
    fn test_extract_nodes_with_labels() {
        let graph = r#"
flowchart TD
    Start([Start]) --> research[Research Node]
    research --> analyze[Analyze Data]
    analyze --> End([End])
"#;
        let nodes = extract_nodes_from_graph(graph);
        assert!(nodes.contains(&"Start".to_string()));
        assert!(nodes.contains(&"research".to_string()));
        assert!(nodes.contains(&"analyze".to_string()));
    }

    #[test]
    fn test_extract_nodes_with_conditions() {
        let graph = r#"
flowchart TD
    check --> |yes| approve
    check --> |no| reject
    approve --> done
    reject --> done
"#;
        let nodes = extract_nodes_from_graph(graph);
        assert!(nodes.contains(&"check".to_string()));
        assert!(nodes.contains(&"approve".to_string()));
        assert!(nodes.contains(&"reject".to_string()));
        assert!(nodes.contains(&"done".to_string()));
    }

    #[test]
    fn test_debug_session_default() {
        let session = DebugSession::default();
        assert!(session.id.is_empty());
        assert!(session.nodes.is_empty());
        assert!(!session.paused);
        assert!(!session.completed);
    }

    #[test]
    fn test_http_json_response() {
        let response = http_json_response(r#"{"test": true}"#);
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("application/json"));
        assert!(response.contains(r#"{"test": true}"#));
    }
}
