// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph visualization command - Interactive web UI for viewing DashFlow graphs

use crate::output::{print_info, print_success, print_warning};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;

/// Visualize DashFlow graphs with interactive web UI
///
/// NOTE: This command is deprecated in favor of `dashflow timeline view/export`.
/// The visualize command will continue to work but new users should use timeline.
#[derive(Args)]
pub struct VisualizeArgs {
    #[command(subcommand)]
    pub command: VisualizeCommand,
}

#[derive(Subcommand)]
pub enum VisualizeCommand {
    /// View a Mermaid diagram file in an interactive web UI
    View(ViewArgs),

    /// Export an HTML visualization file (standalone, no server needed)
    Export(ExportArgs),

    /// Start interactive visualization server
    Serve(ServeArgs),
}

/// Arguments for `dashflow visualize view` (also used by `dashflow timeline view`)
#[derive(Args)]
pub struct ViewArgs {
    /// Path to Mermaid (.mmd) or JSON graph file
    #[arg(required = true)]
    pub input: PathBuf,

    /// Open browser automatically
    #[arg(long, default_value = "true")]
    pub open: bool,

    /// Port to serve on
    #[arg(short, long, default_value = "8765")]
    pub port: u16,
}

/// Arguments for `dashflow visualize export` (also used by `dashflow timeline export`)
#[derive(Args)]
pub struct ExportArgs {
    /// Path to Mermaid (.mmd) or JSON graph file
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output HTML file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Include dark mode support
    #[arg(long, default_value = "true")]
    pub dark_mode: bool,
}

#[derive(Args)]
pub struct ServeArgs {
    /// Port to serve on
    #[arg(short, long, default_value = "8765")]
    pub port: u16,

    /// Allow connections from any IP (not just localhost)
    #[arg(long, default_value = "false")]
    pub public: bool,
}

/// Run the visualize command
pub async fn run(args: VisualizeArgs) -> Result<()> {
    match args.command {
        VisualizeCommand::View(view_args) => run_view(view_args).await,
        VisualizeCommand::Export(export_args) => run_export(export_args).await,
        VisualizeCommand::Serve(serve_args) => run_serve(serve_args).await,
    }
}

async fn run_view(args: ViewArgs) -> Result<()> {
    print_info(&format!("Loading graph from {:?}...", args.input));

    let content = fs::read_to_string(&args.input)
        .with_context(|| format!("Failed to read file: {:?}", args.input))?;

    let mermaid_diagram = if args.input.extension().is_some_and(|e| e == "json") {
        // Parse JSON and extract mermaid or convert
        extract_mermaid_from_json(&content)?
    } else {
        content
    };

    let html = generate_visualization_html(&mermaid_diagram, true);

    // Find available port
    let port = find_available_port(args.port)?;
    let addr = format!("127.0.0.1:{port}");

    print_info(&format!("Starting visualization server on http://{addr}"));

    // Simple HTTP server
    let listener = TcpListener::bind(&addr)?;

    if args.open {
        if let Err(e) = webbrowser::open(&format!("http://{addr}")) {
            print_warning(&format!("Could not open browser: {e}"));
            print_info(&format!("Please open http://{addr} manually"));
        }
    }

    print_success(&format!("Server running at http://{addr}"));
    println!("Press Ctrl+C to stop\n");

    // Handle requests
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                    html.len(),
                    html
                );
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

async fn run_export(args: ExportArgs) -> Result<()> {
    print_info(&format!("Loading graph from {:?}...", args.input));

    let content = fs::read_to_string(&args.input)
        .with_context(|| format!("Failed to read file: {:?}", args.input))?;

    let mermaid_diagram = if args.input.extension().is_some_and(|e| e == "json") {
        extract_mermaid_from_json(&content)?
    } else {
        content
    };

    let html = generate_visualization_html(&mermaid_diagram, args.dark_mode);

    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension("html");
        path
    });

    fs::write(&output_path, html)
        .with_context(|| format!("Failed to write file: {:?}", output_path))?;

    print_success(&format!("Exported visualization to {:?}", output_path));
    println!("Open this file in a browser to view the interactive graph.");

    Ok(())
}

async fn run_serve(args: ServeArgs) -> Result<()> {
    let bind_addr = if args.public {
        format!("0.0.0.0:{}", args.port)
    } else {
        format!("127.0.0.1:{}", args.port)
    };

    print_info(&format!("Starting interactive graph server on {bind_addr}"));
    print_info("Upload a .mmd file or paste Mermaid syntax to visualize");

    let listener = TcpListener::bind(&bind_addr)?;

    let html = generate_upload_server_html();

    print_success(&format!("Server running at http://{bind_addr}"));
    println!("Press Ctrl+C to stop\n");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                    html.len(),
                    html
                );
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

fn find_available_port(preferred: u16) -> Result<u16> {
    if TcpListener::bind(format!("127.0.0.1:{preferred}")).is_ok() {
        return Ok(preferred);
    }

    // Try a few alternative ports
    for port in [8766, 8767, 8768, 8769, 8770] {
        if TcpListener::bind(format!("127.0.0.1:{port}")).is_ok() {
            return Ok(port);
        }
    }

    anyhow::bail!("Could not find available port");
}

fn extract_mermaid_from_json(json_content: &str) -> Result<String> {
    // Try to parse as JSON and look for mermaid field
    let value: serde_json::Value =
        serde_json::from_str(json_content).context("Invalid JSON file")?;

    // Look for a "mermaid" or "diagram" field
    if let Some(diagram) = value.get("mermaid").or(value.get("diagram")) {
        if let Some(s) = diagram.as_str() {
            return Ok(s.to_string());
        }
    }

    // Try to extract graph structure and convert to mermaid
    if let Some(nodes) = value.get("nodes") {
        if let Some(edges) = value.get("edges") {
            return convert_json_graph_to_mermaid(nodes, edges);
        }
    }

    anyhow::bail!("Could not extract Mermaid diagram from JSON. Expected 'mermaid', 'diagram', or 'nodes'/'edges' fields.")
}

fn convert_json_graph_to_mermaid(
    nodes: &serde_json::Value,
    edges: &serde_json::Value,
) -> Result<String> {
    let mut diagram = String::from("flowchart TD\n");

    // Add nodes
    if let Some(nodes_array) = nodes.as_array() {
        for node in nodes_array {
            if let Some(name) = node.get("name").or(node.get("id")).and_then(|v| v.as_str()) {
                let label = node.get("label").and_then(|v| v.as_str()).unwrap_or(name);
                diagram.push_str(&format!("    {name}[{label}]\n"));
            }
        }
    }

    // Add edges
    if let Some(edges_array) = edges.as_array() {
        for edge in edges_array {
            if let (Some(from), Some(to)) = (
                edge.get("from")
                    .or(edge.get("source"))
                    .and_then(|v| v.as_str()),
                edge.get("to")
                    .or(edge.get("target"))
                    .and_then(|v| v.as_str()),
            ) {
                if let Some(label) = edge.get("label").and_then(|v| v.as_str()) {
                    diagram.push_str(&format!("    {from} -->|{label}| {to}\n"));
                } else {
                    diagram.push_str(&format!("    {from} --> {to}\n"));
                }
            }
        }
    }

    Ok(diagram)
}

/// HTML escape helper to prevent XSS in HTML context
/// (M-467: Proper HTML entity encoding)
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Escape content for embedding in a JavaScript template literal inside a script tag
/// (M-467: Prevents script tag injection + template literal escapes)
fn js_template_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
        // M-467: Prevent premature script tag closure - escape </script in any case variation
        .replace("</script", "<\\/script")
        .replace("</SCRIPT", "<\\/SCRIPT")
        .replace("</Script", "<\\/Script")
}

/// Generate the main visualization HTML with embedded Mermaid diagram
fn generate_visualization_html(mermaid_diagram: &str, dark_mode: bool) -> String {
    // M-467: Escape for JS template literal context (inside script tag)
    let escaped_diagram = js_template_escape(mermaid_diagram);
    // M-467: Escape for HTML context (inside <pre> tag)
    let html_escaped_diagram = html_escape(mermaid_diagram);

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DashFlow Graph Visualizer</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        :root {{
            --bg-primary: #ffffff;
            --bg-secondary: #f8f9fa;
            --text-primary: #212529;
            --text-secondary: #6c757d;
            --border-color: #dee2e6;
            --accent-color: #667eea;
            --accent-hover: #5a67d8;
            --success-color: #28a745;
            --warning-color: #ffc107;
            --danger-color: #dc3545;
        }}

        .dark {{
            --bg-primary: #1a1a2e;
            --bg-secondary: #16213e;
            --text-primary: #e4e4e7;
            --text-secondary: #a1a1aa;
            --border-color: #3f3f46;
            --accent-color: #818cf8;
            --accent-hover: #6366f1;
        }}

        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            min-height: 100vh;
        }}

        .header {{
            background: linear-gradient(135deg, var(--accent-color), var(--accent-hover));
            color: white;
            padding: 20px 30px;
            display: flex;
            justify-content: space-between;
            align-items: center;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}

        .header h1 {{
            font-size: 24px;
            font-weight: 600;
        }}

        .header-controls {{
            display: flex;
            gap: 15px;
            align-items: center;
        }}

        .btn {{
            padding: 8px 16px;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-size: 14px;
            font-weight: 500;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            gap: 6px;
        }}

        .btn-primary {{
            background: rgba(255,255,255,0.2);
            color: white;
            border: 1px solid rgba(255,255,255,0.3);
        }}

        .btn-primary:hover {{
            background: rgba(255,255,255,0.3);
        }}

        .toolbar {{
            background: var(--bg-secondary);
            padding: 15px 30px;
            display: flex;
            gap: 20px;
            align-items: center;
            border-bottom: 1px solid var(--border-color);
        }}

        .zoom-controls {{
            display: flex;
            align-items: center;
            gap: 10px;
        }}

        .zoom-controls button {{
            width: 32px;
            height: 32px;
            border: 1px solid var(--border-color);
            background: var(--bg-primary);
            border-radius: 6px;
            cursor: pointer;
            font-size: 18px;
            color: var(--text-primary);
        }}

        .zoom-controls button:hover {{
            background: var(--accent-color);
            color: white;
            border-color: var(--accent-color);
        }}

        .zoom-level {{
            font-size: 14px;
            color: var(--text-secondary);
            min-width: 50px;
            text-align: center;
        }}

        .main-container {{
            display: flex;
            height: calc(100vh - 130px);
        }}

        .graph-panel {{
            flex: 1;
            overflow: auto;
            padding: 30px;
            display: flex;
            justify-content: center;
            align-items: flex-start;
        }}

        .mermaid-container {{
            background: var(--bg-secondary);
            border-radius: 12px;
            padding: 30px;
            box-shadow: 0 4px 20px rgba(0,0,0,0.1);
            transform-origin: top center;
            transition: transform 0.2s;
        }}

        .sidebar {{
            width: 350px;
            background: var(--bg-secondary);
            border-left: 1px solid var(--border-color);
            padding: 20px;
            overflow-y: auto;
        }}

        .sidebar.hidden {{
            display: none;
        }}

        .sidebar h2 {{
            font-size: 16px;
            margin-bottom: 15px;
            color: var(--text-primary);
        }}

        .info-card {{
            background: var(--bg-primary);
            border-radius: 8px;
            padding: 15px;
            margin-bottom: 15px;
            border: 1px solid var(--border-color);
        }}

        .info-card h3 {{
            font-size: 14px;
            color: var(--text-secondary);
            margin-bottom: 8px;
        }}

        .info-card p {{
            font-size: 14px;
            color: var(--text-primary);
        }}

        .stat {{
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid var(--border-color);
        }}

        .stat:last-child {{
            border-bottom: none;
        }}

        .stat-label {{
            color: var(--text-secondary);
            font-size: 13px;
        }}

        .stat-value {{
            font-weight: 600;
            font-size: 13px;
        }}

        .code-block {{
            background: var(--bg-primary);
            border-radius: 8px;
            padding: 15px;
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 12px;
            overflow-x: auto;
            white-space: pre-wrap;
            word-break: break-all;
            max-height: 300px;
            overflow-y: auto;
            border: 1px solid var(--border-color);
        }}

        .toggle-btn {{
            position: fixed;
            right: 360px;
            top: 140px;
            width: 24px;
            height: 48px;
            background: var(--accent-color);
            border: none;
            border-radius: 4px 0 0 4px;
            color: white;
            cursor: pointer;
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 10;
        }}

        .toggle-btn.sidebar-hidden {{
            right: 0;
            border-radius: 4px 0 0 4px;
        }}

        .legend {{
            margin-top: 20px;
        }}

        .legend-item {{
            display: flex;
            align-items: center;
            gap: 10px;
            padding: 8px 0;
            font-size: 13px;
        }}

        .legend-line {{
            width: 40px;
            height: 3px;
        }}

        .legend-line.solid {{ background: #2196f3; }}
        .legend-line.dashed {{
            background: repeating-linear-gradient(90deg, #ff9800, #ff9800 5px, transparent 5px, transparent 10px);
        }}
        .legend-line.thick {{ background: #4caf50; height: 5px; }}

        @media (max-width: 768px) {{
            .sidebar {{ display: none; }}
            .toggle-btn {{ display: none; }}
        }}
    </style>
</head>
<body class="{dark_class}">
    <header class="header">
        <h1>DashFlow Graph Visualizer</h1>
        <div class="header-controls">
            <button class="btn btn-primary" onclick="toggleDarkMode()">
                <span id="theme-icon">🌙</span> Theme
            </button>
            <button class="btn btn-primary" onclick="downloadSVG()">
                📥 Export SVG
            </button>
            <button class="btn btn-primary" onclick="copyMermaid()">
                📋 Copy Mermaid
            </button>
        </div>
    </header>

    <div class="toolbar">
        <div class="zoom-controls">
            <button onclick="zoomOut()">−</button>
            <span class="zoom-level" id="zoom-level">100%</span>
            <button onclick="zoomIn()">+</button>
            <button onclick="resetZoom()">↺</button>
        </div>
        <button class="btn" onclick="fitToScreen()" style="background: var(--bg-primary); border: 1px solid var(--border-color); color: var(--text-primary);">
            ⊡ Fit to Screen
        </button>
    </div>

    <div class="main-container">
        <div class="graph-panel" id="graph-panel">
            <div class="mermaid-container" id="mermaid-container">
                <pre class="mermaid" id="mermaid-graph">{html_escaped_diagram}</pre>
            </div>
        </div>

        <button class="toggle-btn" id="toggle-btn" onclick="toggleSidebar()">◀</button>

        <aside class="sidebar" id="sidebar">
            <h2>Graph Information</h2>

            <div class="info-card">
                <h3>Statistics</h3>
                <div class="stat">
                    <span class="stat-label">Nodes</span>
                    <span class="stat-value" id="node-count">-</span>
                </div>
                <div class="stat">
                    <span class="stat-label">Edges</span>
                    <span class="stat-value" id="edge-count">-</span>
                </div>
                <div class="stat">
                    <span class="stat-label">Conditional Edges</span>
                    <span class="stat-value" id="conditional-count">-</span>
                </div>
            </div>

            <div class="info-card legend">
                <h3>Legend</h3>
                <div class="legend-item">
                    <div class="legend-line solid"></div>
                    <span>Simple edge (→)</span>
                </div>
                <div class="legend-item">
                    <div class="legend-line dashed"></div>
                    <span>Conditional edge (→|condition|)</span>
                </div>
                <div class="legend-item">
                    <div class="legend-line thick"></div>
                    <span>Parallel edge (⟹)</span>
                </div>
            </div>

            <div class="info-card">
                <h3>Mermaid Source</h3>
                <div class="code-block" id="mermaid-source"></div>
            </div>
        </aside>
    </div>

    <script>
        const mermaidSource = `{escaped_diagram}`;
        let currentZoom = 1;

        // Initialize Mermaid
        mermaid.initialize({{
            startOnLoad: true,
            theme: document.body.classList.contains('dark') ? 'dark' : 'default',
            flowchart: {{
                useMaxWidth: false,
                htmlLabels: true,
                curve: 'basis'
            }}
        }});

        // Display source
        document.getElementById('mermaid-source').textContent = mermaidSource;

        // Calculate statistics
        function updateStats() {{
            const lines = mermaidSource.split('\n');
            let nodes = new Set();
            let edges = 0;
            let conditionalEdges = 0;

            for (const line of lines) {{
                // Count edges
                if (line.includes('-->')) {{
                    edges++;
                    if (line.includes('|')) {{
                        conditionalEdges++;
                    }}
                }} else if (line.includes('==>')) {{
                    edges++;
                }}

                // Extract node names
                const nodeMatch = line.match(/^\s*(\w+)\[/);
                if (nodeMatch) {{
                    nodes.add(nodeMatch[1]);
                }}
            }}

            document.getElementById('node-count').textContent = nodes.size;
            document.getElementById('edge-count').textContent = edges;
            document.getElementById('conditional-count').textContent = conditionalEdges;
        }}
        updateStats();

        // Zoom functions
        function updateZoom() {{
            const container = document.getElementById('mermaid-container');
            container.style.transform = `scale(${{currentZoom}})`;
            document.getElementById('zoom-level').textContent = Math.round(currentZoom * 100) + '%';
        }}

        function zoomIn() {{
            currentZoom = Math.min(currentZoom + 0.1, 3);
            updateZoom();
        }}

        function zoomOut() {{
            currentZoom = Math.max(currentZoom - 0.1, 0.2);
            updateZoom();
        }}

        function resetZoom() {{
            currentZoom = 1;
            updateZoom();
        }}

        function fitToScreen() {{
            const panel = document.getElementById('graph-panel');
            const container = document.getElementById('mermaid-container');
            const svg = container.querySelector('svg');

            if (svg) {{
                const svgWidth = svg.getBoundingClientRect().width / currentZoom;
                const svgHeight = svg.getBoundingClientRect().height / currentZoom;
                const panelWidth = panel.clientWidth - 60;
                const panelHeight = panel.clientHeight - 60;

                const scaleX = panelWidth / svgWidth;
                const scaleY = panelHeight / svgHeight;
                currentZoom = Math.min(scaleX, scaleY, 2);
                updateZoom();
            }}
        }}

        // Sidebar toggle
        function toggleSidebar() {{
            const sidebar = document.getElementById('sidebar');
            const btn = document.getElementById('toggle-btn');
            sidebar.classList.toggle('hidden');
            btn.classList.toggle('sidebar-hidden');
            btn.textContent = sidebar.classList.contains('hidden') ? '▶' : '◀';
        }}

        // Dark mode toggle
        function toggleDarkMode() {{
            document.body.classList.toggle('dark');
            const icon = document.getElementById('theme-icon');
            icon.textContent = document.body.classList.contains('dark') ? '☀️' : '🌙';

            // Re-render Mermaid with new theme
            mermaid.initialize({{
                theme: document.body.classList.contains('dark') ? 'dark' : 'default'
            }});

            const graphDiv = document.getElementById('mermaid-graph');
            graphDiv.innerHTML = mermaidSource;
            graphDiv.removeAttribute('data-processed');
            mermaid.init(undefined, graphDiv);
        }}

        // Export SVG
        function downloadSVG() {{
            const svg = document.querySelector('#mermaid-container svg');
            if (svg) {{
                const svgData = new XMLSerializer().serializeToString(svg);
                const blob = new Blob([svgData], {{ type: 'image/svg+xml' }});
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = 'dashflow-graph.svg';
                a.click();
                URL.revokeObjectURL(url);
            }}
        }}

        // Copy Mermaid source
        function copyMermaid() {{
            navigator.clipboard.writeText(mermaidSource).then(() => {{
                alert('Mermaid diagram copied to clipboard!');
            }});
        }}

        // Mouse wheel zoom
        document.getElementById('graph-panel').addEventListener('wheel', (e) => {{
            if (e.ctrlKey || e.metaKey) {{
                e.preventDefault();
                if (e.deltaY < 0) {{
                    zoomIn();
                }} else {{
                    zoomOut();
                }}
            }}
        }});
    </script>
</body>
</html>"##,
        dark_class = if dark_mode { "dark" } else { "" },
        html_escaped_diagram = html_escaped_diagram,
        escaped_diagram = escaped_diagram
    )
}

/// Generate the upload server HTML for interactive mode
fn generate_upload_server_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DashFlow Graph Visualizer</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        :root {
            --bg-primary: #1a1a2e;
            --bg-secondary: #16213e;
            --text-primary: #e4e4e7;
            --text-secondary: #a1a1aa;
            --border-color: #3f3f46;
            --accent-color: #818cf8;
            --accent-hover: #6366f1;
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
            padding: 20px 30px;
            text-align: center;
        }

        .header h1 {
            font-size: 28px;
            font-weight: 600;
        }

        .header p {
            opacity: 0.9;
            margin-top: 8px;
        }

        .container {
            max-width: 1400px;
            margin: 0 auto;
            padding: 30px;
            display: grid;
            grid-template-columns: 400px 1fr;
            gap: 30px;
        }

        .input-panel {
            background: var(--bg-secondary);
            border-radius: 12px;
            padding: 25px;
        }

        .input-panel h2 {
            font-size: 18px;
            margin-bottom: 20px;
        }

        textarea {
            width: 100%;
            height: 400px;
            padding: 15px;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            background: var(--bg-primary);
            color: var(--text-primary);
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 13px;
            resize: vertical;
        }

        textarea:focus {
            outline: none;
            border-color: var(--accent-color);
        }

        .btn {
            width: 100%;
            padding: 12px 20px;
            margin-top: 15px;
            border: none;
            border-radius: 8px;
            cursor: pointer;
            font-size: 15px;
            font-weight: 500;
            transition: all 0.2s;
        }

        .btn-primary {
            background: var(--accent-color);
            color: white;
        }

        .btn-primary:hover {
            background: var(--accent-hover);
        }

        .btn-secondary {
            background: transparent;
            color: var(--text-primary);
            border: 1px solid var(--border-color);
        }

        .btn-secondary:hover {
            background: var(--bg-primary);
        }

        .drop-zone {
            border: 2px dashed var(--border-color);
            border-radius: 8px;
            padding: 30px;
            text-align: center;
            margin-bottom: 20px;
            cursor: pointer;
            transition: all 0.2s;
        }

        .drop-zone:hover, .drop-zone.drag-over {
            border-color: var(--accent-color);
            background: rgba(129, 140, 248, 0.1);
        }

        .drop-zone p {
            color: var(--text-secondary);
        }

        .output-panel {
            background: var(--bg-secondary);
            border-radius: 12px;
            padding: 25px;
            min-height: 500px;
        }

        .output-panel h2 {
            font-size: 18px;
            margin-bottom: 20px;
        }

        .mermaid-output {
            background: var(--bg-primary);
            border-radius: 8px;
            padding: 20px;
            min-height: 400px;
            overflow: auto;
        }

        .examples {
            margin-top: 20px;
        }

        .examples h3 {
            font-size: 14px;
            color: var(--text-secondary);
            margin-bottom: 10px;
        }

        .example-btn {
            display: block;
            width: 100%;
            padding: 10px 15px;
            margin-bottom: 8px;
            background: var(--bg-primary);
            border: 1px solid var(--border-color);
            border-radius: 6px;
            color: var(--text-primary);
            text-align: left;
            cursor: pointer;
            font-size: 13px;
        }

        .example-btn:hover {
            border-color: var(--accent-color);
        }

        @media (max-width: 900px) {
            .container {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
    <header class="header">
        <h1>DashFlow Graph Visualizer</h1>
        <p>Upload a Mermaid file or paste diagram syntax to visualize your graph</p>
    </header>

    <div class="container">
        <div class="input-panel">
            <h2>Input</h2>

            <div class="drop-zone" id="drop-zone" onclick="document.getElementById('file-input').click()">
                <p>📁 Drop a .mmd file here<br>or click to upload</p>
                <input type="file" id="file-input" accept=".mmd,.txt,.json" style="display: none">
            </div>

            <textarea id="mermaid-input" placeholder="Or paste Mermaid syntax here...

flowchart TD
    Start([Start]) --> A[Node A]
    A --> B[Node B]
    B -->|yes| C[Node C]
    B -->|no| D[Node D]
    C --> End([End])
    D --> End"></textarea>

            <button class="btn btn-primary" onclick="renderDiagram()">Render Graph</button>
            <button class="btn btn-secondary" onclick="clearInput()">Clear</button>

            <div class="examples">
                <h3>Examples</h3>
                <button class="example-btn" onclick="loadExample('simple')">Simple Flow</button>
                <button class="example-btn" onclick="loadExample('conditional')">Conditional Routing</button>
                <button class="example-btn" onclick="loadExample('parallel')">Parallel Execution</button>
                <button class="example-btn" onclick="loadExample('agent')">Agent Workflow</button>
            </div>
        </div>

        <div class="output-panel">
            <h2>Visualization</h2>
            <div class="mermaid-output" id="mermaid-output">
                <p style="color: var(--text-secondary); text-align: center; padding-top: 150px;">
                    Enter a Mermaid diagram to see the visualization
                </p>
            </div>
        </div>
    </div>

    <script>
        mermaid.initialize({
            startOnLoad: false,
            theme: 'dark',
            flowchart: {
                useMaxWidth: true,
                htmlLabels: true,
                curve: 'basis'
            }
        });

        const examples = {
            simple: `flowchart TD
    Start([Start]) --> research[Research]
    research --> analyze[Analyze]
    analyze --> write[Write Report]
    write --> End([End])

    classDef startEnd fill:#e1f5e1,stroke:#4caf50
    classDef node fill:#e3f2fd,stroke:#2196f3
    class Start,End startEnd
    class research,analyze,write node`,

            conditional: `flowchart TD
    Start([Start]) --> check[Quality Check]
    check -->|score > 0.8| approve[Approve]
    check -->|score <= 0.8| revise[Revise]
    revise --> check
    approve --> End([End])

    classDef startEnd fill:#e1f5e1,stroke:#4caf50
    classDef decision fill:#fff3e0,stroke:#ff9800
    class Start,End startEnd
    class check decision`,

            parallel: `flowchart TD
    Start([Start]) --> split[Split Task]
    split ==> worker1[Worker 1]
    split ==> worker2[Worker 2]
    split ==> worker3[Worker 3]
    worker1 --> merge[Merge Results]
    worker2 --> merge
    worker3 --> merge
    merge --> End([End])

    classDef startEnd fill:#e1f5e1,stroke:#4caf50
    classDef worker fill:#f3e5f5,stroke:#9c27b0
    class Start,End startEnd
    class worker1,worker2,worker3 worker`,

            agent: `flowchart TD
    Start([Start]) --> agent[Agent]
    agent -->|use tool| tool[Tool Executor]
    tool --> agent
    agent -->|respond| respond[Generate Response]
    respond -->|needs revision| agent
    respond -->|complete| End([End])

    classDef startEnd fill:#e1f5e1,stroke:#4caf50
    classDef agent fill:#e8f5e9,stroke:#4caf50
    classDef tool fill:#fff8e1,stroke:#ffc107
    class Start,End startEnd
    class agent agent
    class tool tool`
        };

        function renderDiagram() {
            const input = document.getElementById('mermaid-input').value.trim();
            const output = document.getElementById('mermaid-output');

            if (!input) {
                output.innerHTML = '<p style="color: var(--text-secondary); text-align: center; padding-top: 150px;">Enter a Mermaid diagram to see the visualization</p>';
                return;
            }

            output.innerHTML = '<pre class="mermaid">' + input + '</pre>';
            mermaid.init(undefined, output.querySelector('.mermaid'));
        }

        function clearInput() {
            document.getElementById('mermaid-input').value = '';
            document.getElementById('mermaid-output').innerHTML = '<p style="color: var(--text-secondary); text-align: center; padding-top: 150px;">Enter a Mermaid diagram to see the visualization</p>';
        }

        function loadExample(name) {
            document.getElementById('mermaid-input').value = examples[name];
            renderDiagram();
        }

        // Drag and drop
        const dropZone = document.getElementById('drop-zone');
        const fileInput = document.getElementById('file-input');

        dropZone.addEventListener('dragover', (e) => {
            e.preventDefault();
            dropZone.classList.add('drag-over');
        });

        dropZone.addEventListener('dragleave', () => {
            dropZone.classList.remove('drag-over');
        });

        dropZone.addEventListener('drop', (e) => {
            e.preventDefault();
            dropZone.classList.remove('drag-over');
            handleFile(e.dataTransfer.files[0]);
        });

        fileInput.addEventListener('change', (e) => {
            if (e.target.files[0]) {
                handleFile(e.target.files[0]);
            }
        });

        function handleFile(file) {
            const reader = new FileReader();
            reader.onload = (e) => {
                document.getElementById('mermaid-input').value = e.target.result;
                renderDiagram();
            };
            reader.readAsText(file);
        }
    </script>
</body>
</html>"##.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_visualization_html() {
        let mermaid = "flowchart TD\n    A --> B";
        let html = generate_visualization_html(mermaid, false);
        assert!(html.contains("DashFlow Graph Visualizer"));
        assert!(html.contains("A --> B"));
        assert!(html.contains("mermaid"));
    }

    #[test]
    fn test_generate_visualization_html_dark_mode() {
        let mermaid = "flowchart TD\n    A --> B";
        let html = generate_visualization_html(mermaid, true);
        assert!(html.contains("class=\"dark\""));
    }

    #[test]
    fn test_convert_json_graph_to_mermaid() {
        let nodes = serde_json::json!([
            {"name": "A", "label": "Node A"},
            {"name": "B", "label": "Node B"}
        ]);
        let edges = serde_json::json!([
            {"from": "A", "to": "B"}
        ]);

        let result = convert_json_graph_to_mermaid(&nodes, &edges).unwrap();
        assert!(result.contains("flowchart TD"));
        assert!(result.contains("A[Node A]"));
        assert!(result.contains("B[Node B]"));
        assert!(result.contains("A --> B"));
    }

    #[test]
    fn test_convert_json_graph_with_labels() {
        let nodes = serde_json::json!([
            {"id": "start", "label": "Start"},
            {"id": "end", "label": "End"}
        ]);
        let edges = serde_json::json!([
            {"source": "start", "target": "end", "label": "proceed"}
        ]);

        let result = convert_json_graph_to_mermaid(&nodes, &edges).unwrap();
        assert!(result.contains("start[Start]"));
        assert!(result.contains("end[End]"));
        assert!(result.contains("start -->|proceed| end"));
    }

    #[test]
    fn test_extract_mermaid_from_json_direct() {
        let json = r#"{"mermaid": "flowchart TD\n    A --> B"}"#;
        let result = extract_mermaid_from_json(json).unwrap();
        assert!(result.contains("flowchart TD"));
    }

    #[test]
    fn test_generate_upload_server_html() {
        let html = generate_upload_server_html();
        assert!(html.contains("DashFlow Graph Visualizer"));
        assert!(html.contains("drop-zone"));
        assert!(html.contains("mermaid-input"));
    }

    // M-467: XSS escaping tests
    #[test]
    fn test_html_escape_basic() {
        let escaped = html_escape("<script>alert('xss')</script>");
        assert!(!escaped.contains("<script>"));
        assert!(escaped.contains("&lt;script&gt;"));
        assert!(escaped.contains("&#39;"));
    }

    #[test]
    fn test_html_escape_ampersand() {
        let escaped = html_escape("foo & bar");
        assert!(escaped.contains("&amp;"));
        assert!(!escaped.contains(" & "));
    }

    #[test]
    fn test_html_escape_quotes() {
        let escaped = html_escape(r#"key="value" and 'single'"#);
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&#39;"));
    }

    #[test]
    fn test_js_template_escape_backticks() {
        let escaped = js_template_escape("text with `backticks`");
        assert!(escaped.contains("\\`"));
        assert!(!escaped.contains("`backticks`"));
    }

    #[test]
    fn test_js_template_escape_template_interpolation() {
        let escaped = js_template_escape("${dangerous}");
        assert!(escaped.contains("\\${"));
        // The original unescaped form should not be present as a literal template expression
        assert_eq!(escaped, "\\${dangerous}");
    }

    #[test]
    fn test_js_template_escape_script_tag() {
        // M-467: Verify </script> injection is blocked
        let escaped = js_template_escape("</script><script>alert(1)</script>");
        assert!(!escaped.contains("</script>"));
        assert!(escaped.contains("<\\/script>"));
    }

    #[test]
    fn test_js_template_escape_script_tag_case_insensitive() {
        // M-467: Script tag variations
        let escaped = js_template_escape("</SCRIPT></Script>");
        assert!(!escaped.contains("</SCRIPT>"));
        assert!(!escaped.contains("</Script>"));
        assert!(escaped.contains("<\\/SCRIPT>"));
        assert!(escaped.contains("<\\/Script>"));
    }

    #[test]
    fn test_visualization_html_xss_protection() {
        // M-467: End-to-end XSS test
        let malicious_diagram = r#"flowchart TD
    A["</script><script>alert('XSS')</script>"] --> B"#;
        let html = generate_visualization_html(malicious_diagram, false);

        // HTML context: script tags should be entity-encoded
        assert!(html.contains("&lt;/script&gt;"));
        // JS context: script tags should be escaped
        assert!(html.contains("<\\/script>"));
        // Neither context should have raw </script>
        let script_tag_count = html.matches("</script>").count();
        // Only legitimate closing script tags (the actual HTML script tags, not injected ones)
        // We expect exactly 2: one for mermaid.min.js and one for the main script
        assert_eq!(script_tag_count, 2, "Should only have legitimate script tags");
    }
}
