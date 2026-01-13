import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';

let visualizerProcess: cp.ChildProcess | undefined;
let debuggerProcess: cp.ChildProcess | undefined;
let dashboardProcess: cp.ChildProcess | undefined;
let outputChannel: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('DashFlow');
    outputChannel.appendLine('DashFlow extension activated');

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('dashflow.visualize', visualizeGraph),
        vscode.commands.registerCommand('dashflow.visualizeSelection', visualizeSelection),
        vscode.commands.registerCommand('dashflow.debug', startDebugServer),
        vscode.commands.registerCommand('dashflow.runAnalyze', runAnalyze),
        vscode.commands.registerCommand('dashflow.runTests', runTests),
        vscode.commands.registerCommand('dashflow.openDashboard', openDashboard),
        vscode.commands.registerCommand('dashflow.generateGraph', generateGraph),
        vscode.commands.registerCommand('dashflow.showDocs', showDocs)
    );

    // Register graph tree view
    const graphProvider = new DashFlowGraphProvider();
    vscode.window.registerTreeDataProvider('dashflowGraphs', graphProvider);

    // Register code lens provider for Rust files
    const codeLensProvider = new DashFlowCodeLensProvider();
    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider({ language: 'rust' }, codeLensProvider)
    );

    // Register hover provider for DashFlow types
    const hoverProvider = new DashFlowHoverProvider();
    context.subscriptions.push(
        vscode.languages.registerHoverProvider({ language: 'rust' }, hoverProvider)
    );

    // Watch for graph file changes
    const watcher = vscode.workspace.createFileSystemWatcher('**/*.{rs,mermaid,mmd}');
    watcher.onDidChange(() => graphProvider.refresh());
    watcher.onDidCreate(() => graphProvider.refresh());
    watcher.onDidDelete(() => graphProvider.refresh());
    context.subscriptions.push(watcher);

    outputChannel.appendLine('DashFlow commands registered');
}

export function deactivate() {
    if (visualizerProcess) {
        visualizerProcess.kill();
    }
    if (debuggerProcess) {
        debuggerProcess.kill();
    }
    if (dashboardProcess) {
        dashboardProcess.kill();
    }
}

function getCliPath(): string {
    const config = vscode.workspace.getConfiguration('dashflow');
    return config.get<string>('cliPath', 'dashflow');
}

async function visualizeGraph() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showWarningMessage('No active editor');
        return;
    }

    const filePath = editor.document.uri.fsPath;
    const config = vscode.workspace.getConfiguration('dashflow');
    const port = config.get<number>('visualizer.port', 8765);
    const cliPath = getCliPath();

    outputChannel.appendLine(`Visualizing: ${filePath}`);

    // Check file type
    const ext = path.extname(filePath).toLowerCase();
    if (ext === '.mermaid' || ext === '.mmd') {
        // Direct Mermaid file
        runVisualizerServer(filePath, port, cliPath);
    } else if (ext === '.rs') {
        // Rust file - extract Mermaid from doc comments or strings
        const content = editor.document.getText();
        const mermaidMatch = extractMermaidFromRust(content);
        if (mermaidMatch) {
            // Create temp file and visualize
            const tmpFile = path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '/tmp', '.dashflow-temp.mmd');
            const fs = require('fs');
            fs.writeFileSync(tmpFile, mermaidMatch);
            runVisualizerServer(tmpFile, port, cliPath);
        } else {
            vscode.window.showWarningMessage('No Mermaid diagram found in file');
        }
    } else if (ext === '.json') {
        // JSON file with graph definition
        runVisualizerServer(filePath, port, cliPath);
    } else {
        vscode.window.showWarningMessage('Unsupported file type for visualization');
    }
}

async function visualizeSelection() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showWarningMessage('No active editor');
        return;
    }

    const selection = editor.selection;
    const text = editor.document.getText(selection);
    if (!text.trim()) {
        vscode.window.showWarningMessage('No text selected');
        return;
    }

    const config = vscode.workspace.getConfiguration('dashflow');
    const port = config.get<number>('visualizer.port', 8765);
    const cliPath = getCliPath();

    // Create temp file
    const fs = require('fs');
    const tmpFile = path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '/tmp', '.dashflow-selection.mmd');
    fs.writeFileSync(tmpFile, text);
    runVisualizerServer(tmpFile, port, cliPath);
}

function runVisualizerServer(filePath: string, port: number, cliPath: string) {
    if (visualizerProcess) {
        visualizerProcess.kill();
    }

    outputChannel.appendLine(`Starting visualizer on port ${port}`);
    visualizerProcess = cp.spawn(cliPath, ['visualize', 'view', filePath], {
        env: { ...process.env, PORT: port.toString() }
    });

    visualizerProcess.stdout?.on('data', (data) => {
        outputChannel.appendLine(`[visualizer] ${data}`);
    });

    visualizerProcess.stderr?.on('data', (data) => {
        outputChannel.appendLine(`[visualizer error] ${data}`);
    });

    visualizerProcess.on('error', (err) => {
        vscode.window.showErrorMessage(`Failed to start visualizer: ${err.message}`);
    });

    // Open in browser after a short delay
    setTimeout(() => {
        vscode.env.openExternal(vscode.Uri.parse(`http://localhost:${port}`));
    }, 1000);
}

async function startDebugServer() {
    const config = vscode.workspace.getConfiguration('dashflow');
    const port = config.get<number>('debugger.port', 8766);
    const cliPath = getCliPath();

    if (debuggerProcess) {
        debuggerProcess.kill();
    }

    outputChannel.appendLine(`Starting debug server on port ${port}`);
    debuggerProcess = cp.spawn(cliPath, ['debug', 'serve', '--port', port.toString()]);

    debuggerProcess.stdout?.on('data', (data) => {
        outputChannel.appendLine(`[debugger] ${data}`);
    });

    debuggerProcess.stderr?.on('data', (data) => {
        outputChannel.appendLine(`[debugger error] ${data}`);
    });

    debuggerProcess.on('error', (err) => {
        vscode.window.showErrorMessage(`Failed to start debugger: ${err.message}`);
    });

    // Open in browser
    setTimeout(() => {
        vscode.env.openExternal(vscode.Uri.parse(`http://localhost:${port}`));
    }, 1000);

    vscode.window.showInformationMessage(`DashFlow debugger started on port ${port}`);
}

async function runAnalyze() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showWarningMessage('No active editor');
        return;
    }

    const filePath = editor.document.uri.fsPath;
    const cliPath = getCliPath();

    const terminal = vscode.window.createTerminal('DashFlow Analyze');
    terminal.show();
    terminal.sendText(`${cliPath} analyze profile "${filePath}"`);
}

async function runTests() {
    const cliPath = getCliPath();
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

    if (!workspaceFolder) {
        vscode.window.showWarningMessage('No workspace folder open');
        return;
    }

    const terminal = vscode.window.createTerminal('DashFlow Tests');
    terminal.show();
    terminal.sendText(`cd "${workspaceFolder}" && cargo test --workspace`);
}

async function openDashboard() {
    const config = vscode.workspace.getConfiguration('dashflow');
    const port = config.get<number>('dashboard.port', 8767);
    const cliPath = getCliPath();

    if (dashboardProcess) {
        dashboardProcess.kill();
    }

    outputChannel.appendLine(`Starting dashboard on port ${port}`);
    dashboardProcess = cp.spawn(cliPath, ['analyze', 'dashboard', '--port', port.toString()]);

    dashboardProcess.stdout?.on('data', (data) => {
        outputChannel.appendLine(`[dashboard] ${data}`);
    });

    dashboardProcess.stderr?.on('data', (data) => {
        outputChannel.appendLine(`[dashboard error] ${data}`);
    });

    dashboardProcess.on('error', (err) => {
        vscode.window.showErrorMessage(`Failed to start dashboard: ${err.message}`);
    });

    setTimeout(() => {
        vscode.env.openExternal(vscode.Uri.parse(`http://localhost:${port}`));
    }, 1000);

    vscode.window.showInformationMessage(`DashFlow dashboard started on port ${port}`);
}

async function generateGraph() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        return;
    }

    const items = [
        { label: 'Simple Graph', description: 'Basic graph with two nodes' },
        { label: 'Conditional Graph', description: 'Graph with conditional routing' },
        { label: 'Agent Graph', description: 'Agent with tools and state' },
        { label: 'RAG Pipeline', description: 'Retrieval-augmented generation' },
        { label: 'Multi-Agent', description: 'Multiple agents collaborating' }
    ];

    const selected = await vscode.window.showQuickPick(items, {
        placeHolder: 'Select graph template'
    });

    if (!selected) {
        return;
    }

    let snippet = '';
    switch (selected.label) {
        case 'Simple Graph':
            snippet = getSimpleGraphSnippet();
            break;
        case 'Conditional Graph':
            snippet = getConditionalGraphSnippet();
            break;
        case 'Agent Graph':
            snippet = getAgentGraphSnippet();
            break;
        case 'RAG Pipeline':
            snippet = getRagPipelineSnippet();
            break;
        case 'Multi-Agent':
            snippet = getMultiAgentSnippet();
            break;
    }

    editor.insertSnippet(new vscode.SnippetString(snippet));
}

async function showDocs() {
    vscode.env.openExternal(vscode.Uri.parse('https://github.com/ayates_dbx/dashflow'));
}

function extractMermaidFromRust(content: string): string | null {
    // Look for Mermaid in doc comments (/// ```mermaid ... ```)
    const docCommentRegex = /\/\/\/\s*```mermaid\s*([\s\S]*?)```/;
    let match = content.match(docCommentRegex);
    if (match) {
        return match[1].replace(/\/\/\/\s*/g, '').trim();
    }

    // Look for Mermaid in block comments (/** ```mermaid ... ``` */)
    const blockCommentRegex = /\/\*\*[\s\S]*?```mermaid\s*([\s\S]*?)```[\s\S]*?\*\//;
    match = content.match(blockCommentRegex);
    if (match) {
        return match[1].replace(/\s*\*\s*/g, '').trim();
    }

    // Look for raw Mermaid string literals
    const stringLiteralRegex = /r#"(graph\s+(?:TD|TB|BT|RL|LR)[\s\S]*?)"#/;
    match = content.match(stringLiteralRegex);
    if (match) {
        return match[1].trim();
    }

    return null;
}

// Graph snippets
function getSimpleGraphSnippet(): string {
    return `use dashflow::prelude::*;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct State {
    input: String,
    output: String,
}

async fn process_node(state: State) -> Result<State, DashFlowError> {
    Ok(State {
        output: format!("Processed: {}", state.input),
        ..state
    })
}

async fn main() -> Result<(), DashFlowError> {
    let graph = GraphBuilder::<State>::new()
        .add_node("process", process_node)
        .add_edge(START, "process")
        .add_edge("process", END)
        .build()?;

    let result = graph.invoke(State { input: "\${1:hello}".into(), ..Default::default() }).await?;
    println!("Result: {:?}", result);
    Ok(())
}
`;
}

function getConditionalGraphSnippet(): string {
    return `use dashflow::prelude::*;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct State {
    value: i32,
    result: String,
}

async fn check_value(state: State) -> Result<State, DashFlowError> {
    Ok(state)
}

async fn high_path(state: State) -> Result<State, DashFlowError> {
    Ok(State { result: "High value path".into(), ..state })
}

async fn low_path(state: State) -> Result<State, DashFlowError> {
    Ok(State { result: "Low value path".into(), ..state })
}

fn route_by_value(state: &State) -> &'static str {
    if state.value > 50 { "high" } else { "low" }
}

async fn main() -> Result<(), DashFlowError> {
    let graph = GraphBuilder::<State>::new()
        .add_node("check", check_value)
        .add_node("high", high_path)
        .add_node("low", low_path)
        .add_edge(START, "check")
        .add_conditional_edges("check", route_by_value, &["high", "low"])
        .add_edge("high", END)
        .add_edge("low", END)
        .build()?;

    let result = graph.invoke(State { value: \${1:75}, ..Default::default() }).await?;
    println!("Result: {:?}", result);
    Ok(())
}
`;
}

function getAgentGraphSnippet(): string {
    return `use dashflow::prelude::*;
use dashflow::agents::*;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<Message>,
    tool_calls: Vec<ToolCall>,
}

async fn main() -> Result<(), DashFlowError> {
    let llm = OpenAI::new()?;

    let tools = vec![
        Tool::new("search", "Search the web", search_fn),
        Tool::new("calculator", "Perform calculations", calc_fn),
    ];

    let agent = AgentExecutor::builder()
        .llm(llm)
        .tools(tools)
        .max_iterations(10)
        .build()?;

    let result = agent.invoke("\${1:What is 2 + 2?}").await?;
    println!("Agent response: {}", result);
    Ok(())
}
`;
}

function getRagPipelineSnippet(): string {
    return `use dashflow::prelude::*;
use dashflow_chroma::ChromaVectorStore;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct RagState {
    query: String,
    documents: Vec<Document>,
    answer: String,
}

async fn retrieve(state: RagState) -> Result<RagState, DashFlowError> {
    let store = ChromaVectorStore::new("collection")?;
    let docs = store.similarity_search(&state.query, 5).await?;
    Ok(RagState { documents: docs, ..state })
}

async fn generate(state: RagState) -> Result<RagState, DashFlowError> {
    let llm = OpenAI::new()?;
    let context = state.documents.iter().map(|d| d.content.as_str()).collect::<Vec<_>>().join("\\n");
    let prompt = format!("Answer based on context:\\n{}\\n\\nQuestion: {}", context, state.query);
    let answer = llm.invoke(&prompt).await?;
    Ok(RagState { answer, ..state })
}

async fn main() -> Result<(), DashFlowError> {
    let graph = GraphBuilder::<RagState>::new()
        .add_node("retrieve", retrieve)
        .add_node("generate", generate)
        .add_edge(START, "retrieve")
        .add_edge("retrieve", "generate")
        .add_edge("generate", END)
        .build()?;

    let result = graph.invoke(RagState { query: "\${1:question}".into(), ..Default::default() }).await?;
    println!("Answer: {}", result.answer);
    Ok(())
}
`;
}

function getMultiAgentSnippet(): string {
    return `use dashflow::prelude::*;
use dashflow::agents::*;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct TeamState {
    task: String,
    research: String,
    analysis: String,
    report: String,
}

async fn researcher(state: TeamState) -> Result<TeamState, DashFlowError> {
    let llm = OpenAI::new()?;
    let research = llm.invoke(&format!("Research: {}", state.task)).await?;
    Ok(TeamState { research, ..state })
}

async fn analyst(state: TeamState) -> Result<TeamState, DashFlowError> {
    let llm = OpenAI::new()?;
    let analysis = llm.invoke(&format!("Analyze: {}", state.research)).await?;
    Ok(TeamState { analysis, ..state })
}

async fn writer(state: TeamState) -> Result<TeamState, DashFlowError> {
    let llm = OpenAI::new()?;
    let report = llm.invoke(&format!("Write report:\\nResearch: {}\\nAnalysis: {}", state.research, state.analysis)).await?;
    Ok(TeamState { report, ..state })
}

async fn main() -> Result<(), DashFlowError> {
    let graph = GraphBuilder::<TeamState>::new()
        .add_node("researcher", researcher)
        .add_node("analyst", analyst)
        .add_node("writer", writer)
        .add_edge(START, "researcher")
        .add_edge("researcher", "analyst")
        .add_edge("analyst", "writer")
        .add_edge("writer", END)
        .build()?;

    let result = graph.invoke(TeamState { task: "\${1:topic}".into(), ..Default::default() }).await?;
    println!("Report: {}", result.report);
    Ok(())
}
`;
}

// Tree data provider for DashFlow graphs
class DashFlowGraphProvider implements vscode.TreeDataProvider<GraphItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<GraphItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: GraphItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: GraphItem): Promise<GraphItem[]> {
        if (!vscode.workspace.workspaceFolders) {
            return [];
        }

        if (!element) {
            // Root level - find all graphs
            const graphs: GraphItem[] = [];

            // Find Rust files with GraphBuilder
            const rustFiles = await vscode.workspace.findFiles('**/*.rs', '**/target/**');
            for (const file of rustFiles) {
                const doc = await vscode.workspace.openTextDocument(file);
                const content = doc.getText();
                if (content.includes('GraphBuilder') || content.includes('StateGraph')) {
                    graphs.push(new GraphItem(
                        path.basename(file.fsPath),
                        vscode.TreeItemCollapsibleState.None,
                        file.fsPath,
                        'rust'
                    ));
                }
            }

            // Find Mermaid files
            const mermaidFiles = await vscode.workspace.findFiles('**/*.{mermaid,mmd}', '**/target/**');
            for (const file of mermaidFiles) {
                graphs.push(new GraphItem(
                    path.basename(file.fsPath),
                    vscode.TreeItemCollapsibleState.None,
                    file.fsPath,
                    'mermaid'
                ));
            }

            return graphs;
        }

        return [];
    }
}

class GraphItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly filePath: string,
        public readonly graphType: string
    ) {
        super(label, collapsibleState);
        this.tooltip = filePath;
        this.description = graphType;
        this.iconPath = new vscode.ThemeIcon(graphType === 'rust' ? 'symbol-class' : 'graph');
        this.command = {
            command: 'vscode.open',
            title: 'Open',
            arguments: [vscode.Uri.file(filePath)]
        };
    }
}

// Code lens provider for DashFlow
class DashFlowCodeLensProvider implements vscode.CodeLensProvider {
    provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
        const lenses: vscode.CodeLens[] = [];
        const text = document.getText();

        // Find GraphBuilder::new() calls
        const graphBuilderRegex = /GraphBuilder::<\w+>::new\(\)/g;
        let match;
        while ((match = graphBuilderRegex.exec(text)) !== null) {
            const position = document.positionAt(match.index);
            const range = new vscode.Range(position, position);

            lenses.push(new vscode.CodeLens(range, {
                title: '$(graph) Visualize Graph',
                command: 'dashflow.visualize'
            }));
        }

        // Find AgentExecutor::builder() calls
        const agentRegex = /AgentExecutor::builder\(\)/g;
        while ((match = agentRegex.exec(text)) !== null) {
            const position = document.positionAt(match.index);
            const range = new vscode.Range(position, position);

            lenses.push(new vscode.CodeLens(range, {
                title: '$(debug-alt) Debug Agent',
                command: 'dashflow.debug'
            }));
        }

        return lenses;
    }
}

// Hover provider for DashFlow types
class DashFlowHoverProvider implements vscode.HoverProvider {
    private readonly docs: Map<string, string> = new Map([
        ['GraphBuilder', 'Builder for creating state graphs with nodes and edges.\n\n```rust\nGraphBuilder::<State>::new()\n    .add_node("name", node_fn)\n    .add_edge(START, "name")\n    .build()\n```'],
        ['StateGraph', 'A compiled state graph ready for execution.\n\nUse `invoke()` to run the graph with initial state.'],
        ['AgentExecutor', 'Executor for running agents with tools and LLM.\n\n```rust\nAgentExecutor::builder()\n    .llm(llm)\n    .tools(tools)\n    .build()\n```'],
        ['START', 'Special constant representing the graph entry point.'],
        ['END', 'Special constant representing the graph exit point.'],
        ['DashFlowError', 'Error type for DashFlow operations.'],
        ['Checkpointer', 'Trait for implementing state persistence.\n\nImplementations: MemoryCheckpointer, FileCheckpointer, PostgresCheckpointer, RedisCheckpointer, S3Checkpointer'],
        ['Tool', 'A tool that agents can invoke.\n\n```rust\nTool::new("name", "description", function)\n```'],
        ['Message', 'A message in a conversation (user, assistant, system, or tool).'],
        ['Document', 'A document with content and metadata for RAG pipelines.'],
    ]);

    provideHover(document: vscode.TextDocument, position: vscode.Position): vscode.Hover | null {
        const wordRange = document.getWordRangeAtPosition(position);
        if (!wordRange) {
            return null;
        }

        const word = document.getText(wordRange);
        const doc = this.docs.get(word);

        if (doc) {
            const markdown = new vscode.MarkdownString();
            markdown.appendMarkdown(`**DashFlow: ${word}**\n\n`);
            markdown.appendMarkdown(doc);
            markdown.isTrusted = true;
            return new vscode.Hover(markdown);
        }

        return null;
    }
}
