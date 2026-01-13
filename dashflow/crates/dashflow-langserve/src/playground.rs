//! Interactive playground UI for testing runnables

/// Get the HTML for the playground UI
///
/// This returns a simple, self-contained HTML page with inline JavaScript
/// that provides an interactive interface for testing the runnable.
///
/// # Arguments
///
/// * `base_url` - The base URL path for the runnable (e.g., "/`my_runnable`")
#[must_use]
pub fn get_playground_html(base_url: &str) -> String {
    // Remove trailing slash from base_url if present
    let base_url = base_url.trim_end_matches('/');

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LangServe Playground</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            overflow: hidden;
        }}
        header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
        }}
        header h1 {{
            font-size: 28px;
            margin-bottom: 8px;
        }}
        header p {{
            opacity: 0.9;
            font-size: 14px;
        }}
        .content {{
            padding: 30px;
        }}
        .section {{
            margin-bottom: 30px;
        }}
        h2 {{
            font-size: 20px;
            margin-bottom: 15px;
            color: #333;
        }}
        .tabs {{
            display: flex;
            border-bottom: 2px solid #e0e0e0;
            margin-bottom: 20px;
        }}
        .tab {{
            padding: 12px 24px;
            cursor: pointer;
            background: none;
            border: none;
            font-size: 16px;
            color: #666;
            transition: all 0.3s;
        }}
        .tab.active {{
            color: #667eea;
            border-bottom: 2px solid #667eea;
            margin-bottom: -2px;
        }}
        .tab:hover {{
            color: #667eea;
        }}
        .tab-content {{
            display: none;
        }}
        .tab-content.active {{
            display: block;
        }}
        textarea {{
            width: 100%;
            min-height: 150px;
            padding: 12px;
            border: 1px solid #ddd;
            border-radius: 4px;
            font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
            font-size: 14px;
            resize: vertical;
        }}
        .button-group {{
            margin-top: 15px;
            display: flex;
            gap: 10px;
        }}
        button {{
            padding: 10px 20px;
            background: #667eea;
            color: white;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-size: 14px;
            transition: background 0.3s;
        }}
        button:hover {{
            background: #5568d3;
        }}
        button:disabled {{
            background: #ccc;
            cursor: not-allowed;
        }}
        .secondary {{
            background: #6c757d;
        }}
        .secondary:hover {{
            background: #5a6268;
        }}
        .output {{
            background: #f8f9fa;
            border: 1px solid #ddd;
            border-radius: 4px;
            padding: 15px;
            min-height: 200px;
            font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
            font-size: 13px;
            white-space: pre-wrap;
            word-wrap: break-word;
            max-height: 400px;
            overflow-y: auto;
        }}
        .error {{
            color: #dc3545;
        }}
        .success {{
            color: #28a745;
        }}
        .info {{
            background: #e7f3ff;
            border-left: 4px solid #2196F3;
            padding: 12px;
            margin-bottom: 15px;
            border-radius: 4px;
        }}
        .loading {{
            display: inline-block;
            width: 14px;
            height: 14px;
            border: 2px solid #f3f3f3;
            border-top: 2px solid #667eea;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin-left: 8px;
        }}
        @keyframes spin {{
            0% {{ transform: rotate(0deg); }}
            100% {{ transform: rotate(360deg); }}
        }}
        .schema-content {{
            background: #f8f9fa;
            border: 1px solid #ddd;
            border-radius: 4px;
            padding: 15px;
            font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
            font-size: 13px;
            white-space: pre-wrap;
            max-height: 400px;
            overflow-y: auto;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦜 LangServe Playground</h1>
            <p>Interactive testing interface for: {base_url}</p>
        </header>
        <div class="content">
            <div class="section">
                <div class="tabs">
                    <button class="tab active" onclick="switchTab('invoke')">Invoke</button>
                    <button class="tab" onclick="switchTab('batch')">Batch</button>
                    <button class="tab" onclick="switchTab('stream')">Stream</button>
                    <button class="tab" onclick="switchTab('schema')">Schema</button>
                </div>

                <!-- Invoke Tab -->
                <div id="invoke-tab" class="tab-content active">
                    <h2>Single Invocation</h2>
                    <div class="info">
                        Enter your input as JSON. The runnable will process it and return the output.
                    </div>
                    <textarea id="invoke-input" placeholder='{{"text": "Hello, world!"}}'>{{"text": "Hello from the playground!"}}</textarea>
                    <div class="button-group">
                        <button onclick="invokeRunnable()">
                            Invoke<span id="invoke-loading"></span>
                        </button>
                        <button class="secondary" onclick="clearInvokeOutput()">Clear Output</button>
                    </div>
                    <h2 style="margin-top: 20px;">Output</h2>
                    <div id="invoke-output" class="output">No output yet. Click "Invoke" to run.</div>
                </div>

                <!-- Batch Tab -->
                <div id="batch-tab" class="tab-content">
                    <h2>Batch Invocation</h2>
                    <div class="info">
                        Enter an array of inputs as JSON. The runnable will process all inputs and return an array of outputs.
                    </div>
                    <textarea id="batch-input" placeholder='[{{"text": "Input 1"}}, {{"text": "Input 2"}}]'>[{{"text": "First input"}}, {{"text": "Second input"}}, {{"text": "Third input"}}]</textarea>
                    <div class="button-group">
                        <button onclick="batchRunnable()">
                            Batch Invoke<span id="batch-loading"></span>
                        </button>
                        <button class="secondary" onclick="clearBatchOutput()">Clear Output</button>
                    </div>
                    <h2 style="margin-top: 20px;">Output</h2>
                    <div id="batch-output" class="output">No output yet. Click "Batch Invoke" to run.</div>
                </div>

                <!-- Stream Tab -->
                <div id="stream-tab" class="tab-content">
                    <h2>Streaming Invocation</h2>
                    <div class="info">
                        Enter your input as JSON. The runnable will stream outputs as they're generated.
                    </div>
                    <textarea id="stream-input" placeholder='{{"text": "Hello, world!"}}'>{{"text": "Stream me some results!"}}</textarea>
                    <div class="button-group">
                        <button onclick="streamRunnable()">
                            Start Stream<span id="stream-loading"></span>
                        </button>
                        <button class="secondary" onclick="clearStreamOutput()">Clear Output</button>
                    </div>
                    <h2 style="margin-top: 20px;">Output</h2>
                    <div id="stream-output" class="output">No output yet. Click "Start Stream" to run.</div>
                </div>

                <!-- Schema Tab -->
                <div id="schema-tab" class="tab-content">
                    <h2>API Schemas</h2>
                    <div class="info">
                        View the JSON schemas for input, output, and configuration.
                    </div>
                    <h3 style="margin: 20px 0 10px 0;">Input Schema</h3>
                    <div id="input-schema" class="schema-content">Loading...</div>
                    <h3 style="margin: 20px 0 10px 0;">Output Schema</h3>
                    <div id="output-schema" class="schema-content">Loading...</div>
                    <h3 style="margin: 20px 0 10px 0;">Config Schema</h3>
                    <div id="config-schema" class="schema-content">Loading...</div>
                </div>
            </div>
        </div>
    </div>

    <script>
        const BASE_URL = '{base_url}';

        function switchTab(tabName) {{
            // Hide all tabs
            document.querySelectorAll('.tab-content').forEach(el => el.classList.remove('active'));
            document.querySelectorAll('.tab').forEach(el => el.classList.remove('active'));

            // Show selected tab
            document.getElementById(tabName + '-tab').classList.add('active');
            event.target.classList.add('active');

            // Load schemas when schema tab is opened
            if (tabName === 'schema') {{
                loadSchemas();
            }}
        }}

        async function invokeRunnable() {{
            const input = document.getElementById('invoke-input').value;
            const output = document.getElementById('invoke-output');
            const loading = document.getElementById('invoke-loading');

            loading.innerHTML = '<span class="loading"></span>';
            output.textContent = 'Invoking...';

            try {{
                const json = JSON.parse(input);
                const response = await fetch(BASE_URL + '/invoke', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ input: json }})
                }});

                if (!response.ok) {{
                    throw new Error(`HTTP ${{response.status}}: ${{await response.text()}}`);
                }}

                const result = await response.json();
                output.innerHTML = '<span class="success">✓ Success</span>\n\n' + JSON.stringify(result.output, null, 2);
            }} catch (error) {{
                output.innerHTML = '<span class="error">✗ Error</span>\n\n' + error.message;
            }} finally {{
                loading.innerHTML = '';
            }}
        }}

        async function batchRunnable() {{
            const input = document.getElementById('batch-input').value;
            const output = document.getElementById('batch-output');
            const loading = document.getElementById('batch-loading');

            loading.innerHTML = '<span class="loading"></span>';
            output.textContent = 'Batch invoking...';

            try {{
                const json = JSON.parse(input);
                if (!Array.isArray(json)) {{
                    throw new Error('Input must be an array of objects');
                }}

                const response = await fetch(BASE_URL + '/batch', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ inputs: json }})
                }});

                if (!response.ok) {{
                    throw new Error(`HTTP ${{response.status}}: ${{await response.text()}}`);
                }}

                const result = await response.json();
                output.innerHTML = '<span class="success">✓ Success</span>\n\n' +
                    'Processed ' + result.output.length + ' inputs\n\n' +
                    JSON.stringify(result.output, null, 2);
            }} catch (error) {{
                output.innerHTML = '<span class="error">✗ Error</span>\n\n' + error.message;
            }} finally {{
                loading.innerHTML = '';
            }}
        }}

        async function streamRunnable() {{
            const input = document.getElementById('stream-input').value;
            const output = document.getElementById('stream-output');
            const loading = document.getElementById('stream-loading');

            loading.innerHTML = '<span class="loading"></span>';
            output.textContent = 'Streaming...\n\n';

            try {{
                const json = JSON.parse(input);
                const response = await fetch(BASE_URL + '/stream', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ input: json }})
                }});

                if (!response.ok) {{
                    throw new Error(`HTTP ${{response.status}}: ${{await response.text()}}`);
                }}

                const reader = response.body.getReader();
                const decoder = new TextDecoder();
                let buffer = '';
                let chunkCount = 0;

                while (true) {{
                    const {{ done, value }} = await reader.read();
                    if (done) break;

                    buffer += decoder.decode(value, {{ stream: true }});
                    const lines = buffer.split('\n');
                    buffer = lines.pop() || '';

                    for (const line of lines) {{
                        if (line.startsWith('data: ')) {{
                            const data = line.slice(6);
                            if (data.trim()) {{
                                try {{
                                    const chunk = JSON.parse(data);
                                    chunkCount++;
                                    output.innerHTML += `<span class="success">Chunk ${{chunkCount}}:</span>\n${{JSON.stringify(chunk, null, 2)}}\n\n`;
                                    output.scrollTop = output.scrollHeight;
                                }} catch (e) {{
                                    // Ignore parse errors for SSE format lines
                                }}
                            }}
                        }} else if (line.startsWith('event: end')) {{
                            output.innerHTML += `<span class="success">✓ Stream complete (${{chunkCount}} chunks)</span>`;
                            break;
                        }}
                    }}
                }}
            }} catch (error) {{
                output.innerHTML = '<span class="error">✗ Error</span>\n\n' + error.message;
            }} finally {{
                loading.innerHTML = '';
            }}
        }}

        async function loadSchemas() {{
            try {{
                const [inputResp, outputResp, configResp] = await Promise.all([
                    fetch(BASE_URL + '/input_schema'),
                    fetch(BASE_URL + '/output_schema'),
                    fetch(BASE_URL + '/config_schema')
                ]);

                document.getElementById('input-schema').textContent =
                    JSON.stringify(await inputResp.json(), null, 2);
                document.getElementById('output-schema').textContent =
                    JSON.stringify(await outputResp.json(), null, 2);
                document.getElementById('config-schema').textContent =
                    JSON.stringify(await configResp.json(), null, 2);
            }} catch (error) {{
                document.getElementById('input-schema').innerHTML =
                    '<span class="error">Error loading schema: ' + error.message + '</span>';
            }}
        }}

        function clearInvokeOutput() {{
            document.getElementById('invoke-output').textContent = 'Output cleared.';
        }}

        function clearBatchOutput() {{
            document.getElementById('batch-output').textContent = 'Output cleared.';
        }}

        function clearStreamOutput() {{
            document.getElementById('stream-output').textContent = 'Output cleared.';
        }}
    </script>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_playground_html_basic() {
        let html = get_playground_html("/my_runnable");

        // Check that the HTML contains expected elements
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>LangServe Playground</title>"));
        assert!(html.contains("/my_runnable"));
    }

    #[test]
    fn test_get_playground_html_removes_trailing_slash() {
        let html = get_playground_html("/my_runnable/");

        // The trailing slash should be removed
        assert!(html.contains("const BASE_URL = '/my_runnable';"));
        // Should not contain double slashes in BASE_URL
        assert!(!html.contains("const BASE_URL = '/my_runnable/';"));
    }

    #[test]
    fn test_get_playground_html_preserves_no_trailing_slash() {
        let html = get_playground_html("/api/v1/chain");

        assert!(html.contains("const BASE_URL = '/api/v1/chain';"));
    }

    #[test]
    fn test_get_playground_html_contains_tabs() {
        let html = get_playground_html("/test");

        // Check that all tabs are present
        assert!(html.contains("onclick=\"switchTab('invoke')\""));
        assert!(html.contains("onclick=\"switchTab('batch')\""));
        assert!(html.contains("onclick=\"switchTab('stream')\""));
        assert!(html.contains("onclick=\"switchTab('schema')\""));
    }

    #[test]
    fn test_get_playground_html_contains_api_endpoints() {
        let html = get_playground_html("/api");

        // Check that JavaScript references the correct endpoints
        assert!(html.contains("BASE_URL + '/invoke'"));
        assert!(html.contains("BASE_URL + '/batch'"));
        assert!(html.contains("BASE_URL + '/stream'"));
        assert!(html.contains("BASE_URL + '/input_schema'"));
        assert!(html.contains("BASE_URL + '/output_schema'"));
        assert!(html.contains("BASE_URL + '/config_schema'"));
    }

    #[test]
    fn test_get_playground_html_contains_css_styles() {
        let html = get_playground_html("/test");

        // Check that CSS is included
        assert!(html.contains("<style>"));
        assert!(html.contains(".container"));
        assert!(html.contains(".tab"));
        assert!(html.contains(".output"));
    }

    #[test]
    fn test_get_playground_html_header_shows_base_url() {
        let html = get_playground_html("/custom/path");

        // Header should show the base URL
        assert!(html.contains("Interactive testing interface for: /custom/path"));
    }

    #[test]
    fn test_get_playground_html_empty_base_url() {
        let html = get_playground_html("");

        // Should still produce valid HTML with empty base_url
        assert!(html.contains("const BASE_URL = '';"));
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_get_playground_html_contains_javascript_functions() {
        let html = get_playground_html("/test");

        // Check that JavaScript functions are defined
        assert!(html.contains("function switchTab(tabName)"));
        assert!(html.contains("async function invokeRunnable()"));
        assert!(html.contains("async function batchRunnable()"));
        assert!(html.contains("async function streamRunnable()"));
        assert!(html.contains("async function loadSchemas()"));
    }
}
