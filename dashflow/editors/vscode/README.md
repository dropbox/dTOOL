# DashFlow VS Code Extension

Official VS Code extension for [DashFlow](https://github.com/dropbox/dTOOL/dashflow) - the high-performance Rust agent orchestration framework.

## Features

### Graph Visualization
- **Visualize graphs** directly from Rust files or Mermaid diagrams
- Interactive web UI with zoom, pan, and export
- Automatic Mermaid extraction from doc comments

### Interactive Debugger
- Step-through execution of graphs
- Set breakpoints on nodes
- Inspect state at each step
- Export debug sessions to JSON

### Code Intelligence
- **Syntax highlighting** for Mermaid diagram files (`.mermaid`, `.mmd`)
- **Code snippets** for common DashFlow patterns
- **Hover documentation** for DashFlow types
- **CodeLens** actions on graph and agent definitions

### CLI Integration
- Run `dashflow analyze` from the editor
- Open interactive dashboards
- Execute tests with one click

## Commands

| Command | Description |
|---------|-------------|
| `DashFlow: Visualize Graph` | Open graph visualization for current file |
| `DashFlow: Visualize Selected Mermaid` | Visualize selected text as Mermaid |
| `DashFlow: Start Debug Server` | Start interactive debugger web UI |
| `DashFlow: Analyze Workflow` | Run analysis on current file |
| `DashFlow: Run Tests` | Run cargo test for workspace |
| `DashFlow: Open Dashboard` | Open performance dashboard |
| `DashFlow: Generate Graph Code` | Insert graph code snippet |
| `DashFlow: Show Documentation` | Open DashFlow documentation |

## Snippets

| Prefix | Description |
|--------|-------------|
| `dfgraph` | Create a basic DashFlow graph |
| `dfnode` | Create a graph node function |
| `dfcond` | Create a conditional edge router |
| `dfagent` | Create an agent executor |
| `dftool` | Create a tool definition |
| `dfcheckpoint` | Add checkpointing to a graph |
| `dfrag` | Create a RAG pipeline |
| `dfstate` | Create a state struct |
| `dfparallel` | Create parallel branches |
| `dfllm` | Make an LLM call |
| `dfmsg` | Create a message |
| `dferror` | Error handling pattern |
| `dfstream` | Add streaming to a graph |
| `dfoptimize` | Create an optimization pipeline |
| `dftest` | Create a test function |
| `mermaid` | Create a Mermaid graph |
| `subgraph` | Create a Mermaid subgraph |
| `mermaidcond` | Create a conditional flow |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `dashflow.cliPath` | `dashflow` | Path to the DashFlow CLI binary |
| `dashflow.visualizer.port` | `8765` | Port for the visualization server |
| `dashflow.debugger.port` | `8766` | Port for the debug server |
| `dashflow.dashboard.port` | `8767` | Port for the dashboard server |
| `dashflow.autoDetectGraphs` | `true` | Auto-detect graph definitions |
| `dashflow.showInlineHints` | `true` | Show inline hints for types |

## Requirements

- [DashFlow CLI](https://github.com/dropbox/dTOOL/dashflow) installed and in PATH
- VS Code 1.85.0 or later

## Installation

### From VSIX (Local)
```bash
cd editors/vscode
npm install
npm run package
code --install-extension dashflow-1.11.3.vsix
```

### From Source
```bash
cd editors/vscode
npm install
npm run compile
# Press F5 in VS Code to launch Extension Development Host
```

## Development

```bash
# Install dependencies
npm install

# Compile TypeScript
npm run compile

# Watch for changes
npm run watch

# Run linter
npm run lint

# Package extension
npm run package
```

## Graph Tree View

The extension adds a "DashFlow Graphs" view to the Explorer sidebar that lists:
- Rust files containing `GraphBuilder` or `StateGraph`
- Mermaid diagram files (`.mermaid`, `.mmd`)

Click any item to open the file, or use the context menu to visualize.

## License

MIT
